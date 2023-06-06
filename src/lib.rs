mod client_interface;

pub mod response;
#[allow(clippy::too_many_arguments)]
mod routes;
pub mod types;

mod registry_interface;
#[cfg(feature = "sqlite")]
mod users;

use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::{env, fs};

use anyhow::{anyhow, Context, Result};
use axum::extract::FromRef;
use axum::http::header;
use axum::http::method::Method;
use axum::response::Response;
use axum_server::tls_rustls::RustlsConfig;
use chrono::Utc;
use client_interface::ClientInterface;
use futures::Future;
use hyper::http::HeaderValue;
use log::{debug, LevelFilter, SetLoggerError};
use thiserror::Error;
use tower::ServiceBuilder;
use tower_http::cors;
use trow_server::{ImageValidationConfig, RegistryProxyConfig};
use uuid::Uuid;

//TODO: Make this take a cause or description
#[derive(Error, Debug)]
#[error("invalid data directory")]
pub struct ConfigError {}

#[derive(Clone, Debug)]
pub struct NetAddr {
    pub host: String,
    pub port: u16,
}

#[derive(Debug)]
pub struct TrowServerState {
    pub client: ClientInterface,
    pub config: TrowConfig,
}

impl FromRef<Arc<TrowServerState>> for TrowConfig {
    fn from_ref(state: &Arc<TrowServerState>) -> Self {
        state.config.clone()
    }
}

/*
 * Configuration for Trow. This isn't direct fields on the builder so that we can pass it
 * to Rocket to manage.
 */
#[derive(Clone, Debug)]
pub struct TrowConfig {
    data_dir: String,
    addr: SocketAddr,
    tls: Option<TlsConfig>,
    grpc: GrpcConfig,
    service_name: String,
    proxy_registry_config: Vec<RegistryProxyConfig>,
    image_validation_config: Option<ImageValidationConfig>,
    dry_run: bool,
    max_manifest_size: u32,
    max_blob_size: u32,
    token_secret: String,
    user: Option<UserConfig>,
    cors: bool,
    log_level: String,
}

#[derive(Clone, Debug)]
struct GrpcConfig {
    listen: String,
}

#[derive(Clone, Debug)]
struct TlsConfig {
    cert_file: String,
    key_file: String,
}

#[derive(Clone, Debug)]
struct UserConfig {
    user: String,
    hash_encoded: String, //Surprised not bytes
}

fn init_trow_server(
    config: TrowConfig,
) -> Result<impl Future<Output = Result<(), tonic::transport::Error>>> {
    debug!("Starting Trow server");

    //Could pass full config here.
    //Pros: less work, new args added automatically
    //-s: ties frontend to backend, some uneeded/unwanted vars

    let ts = trow_server::build_server(
        &config.data_dir,
        config.grpc.listen.parse::<std::net::SocketAddr>()?,
        config.proxy_registry_config,
        config.image_validation_config,
    );
    //TODO: probably shouldn't be reusing this cert
    let ts = if let Some(tls) = config.tls {
        ts.add_tls(fs::read(tls.cert_file)?, fs::read(tls.key_file)?)
    } else {
        ts
    };

    Ok(ts.get_server_future())
}

/// Build the logging agent with formatting.
fn init_logger(log_level: String) -> Result<(), SetLoggerError> {
    // If there env variable RUST_LOG is set, then take the configuration from it.
    // Otherwise create a default logger
    let mut builder = env_logger::Builder::new();
    builder
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] {} {}",
                Utc::now().format("%Y-%m-%dT%H:%M:%S"),
                record.target(),
                record.level(),
                record.args()
            )
        })
        .filter(None, LevelFilter::from_str(&log_level).unwrap());
    builder.init();
    Ok(())
}

pub struct TrowBuilder {
    config: TrowConfig,
}

impl TrowBuilder {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        data_dir: String,
        addr: SocketAddr,
        listen: String,
        service_name: String,
        dry_run: bool,
        cors: bool,
        max_manifest_size: u32,
        max_blob_size: u32,
        log_level: String,
    ) -> TrowBuilder {
        let config = TrowConfig {
            data_dir,
            addr,
            tls: None,
            grpc: GrpcConfig { listen },
            service_name,
            proxy_registry_config: Vec::new(),
            image_validation_config: None,
            dry_run,
            max_manifest_size,
            max_blob_size,
            token_secret: Uuid::new_v4().to_string(),
            user: None,
            cors,
            log_level,
        };
        TrowBuilder { config }
    }

    pub fn with_proxy_registries(&mut self, config_file: impl AsRef<str>) -> Result<&mut Self> {
        let config_file = config_file.as_ref();
        let config_str = fs::read_to_string(config_file)
            .with_context(|| format!("Could not read file `{}`", config_file))?;
        let config = serde_yaml::from_str::<Vec<RegistryProxyConfig>>(&config_str)
            .with_context(|| format!("Could not parse file `{}`", config_file))?;
        self.config.proxy_registry_config = config;
        Ok(self)
    }

    pub fn with_image_validation(&mut self, config_file: impl AsRef<str>) -> Result<&mut Self> {
        let config_file = config_file.as_ref();
        let config_str = fs::read_to_string(config_file)
            .with_context(|| format!("Could not read file `{}`", config_file))?;
        let config = serde_yaml::from_str::<ImageValidationConfig>(&config_str)
            .with_context(|| format!("Could not parse file `{}`", config_file))?;
        self.config.image_validation_config = Some(config);
        Ok(self)
    }

    pub fn with_tls(&mut self, cert_file: String, key_file: String) -> &mut TrowBuilder {
        let cfg = TlsConfig {
            cert_file,
            key_file,
        };
        self.config.tls = Some(cfg);
        self
    }

    pub fn with_user(&mut self, user: String, pass: String) -> &mut TrowBuilder {
        let hash_config = argon2::Config::default();
        let hash_encoded =
            argon2::hash_encoded(pass.as_bytes(), Uuid::new_v4().as_bytes(), &hash_config)
                .expect("Error hashing password");
        let usercfg = UserConfig { user, hash_encoded };
        self.config.user = Some(usercfg);
        self
    }

    pub async fn start(&self) -> Result<()> {
        init_logger(self.config.log_level.clone())?;

        println!(
            "Starting Trow {} on {}",
            env!("CARGO_PKG_VERSION"),
            self.config.addr
        );
        println!(
            "\nMaximum blob size: {} Mebibytes",
            self.config.max_blob_size
        );
        println!(
            "Maximum manifest size: {} Mebibytes",
            self.config.max_manifest_size
        );

        println!(
            "Hostname of this registry (for the MutatingWebhook): {:?}",
            self.config.service_name
        );
        match self.config.image_validation_config {
            Some(ref config) => {
                println!("Image validation webhook configured:");
                println!("  Default action: {}", config.default);
                println!("  Allowed prefixes: {:?}", config.allow);
                println!("  Denied prefixes: {:?}", config.deny);
            }
            None => println!("Image validation webhook not configured"),
        }
        if !self.config.proxy_registry_config.is_empty() {
            println!("Proxy registries configured:");
            for config in &self.config.proxy_registry_config {
                println!("  - {}: {}", config.alias, config.host);
            }
        } else {
            println!("Proxy registries not configured");
        }

        if self.config.cors {
            println!("Cross-Origin Resource Sharing(CORS) requests are allowed\n");
        }

        if self.config.dry_run {
            println!("Dry run, exiting.");
            std::process::exit(0);
        }

        let s = format!("https://{}", self.config.grpc.listen);
        let server_state = TrowServerState {
            config: self.config.clone(),
            client: build_handlers(s)?,
        };

        let mut app = routes::create_app(server_state);

        if self.config.cors {
            app = app.layer(
                cors::CorsLayer::new()
                    .allow_credentials(true)
                    .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
                    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                    .allow_origin(cors::AllowOrigin::any()),
            );
        }

        let app = app.layer(
            // Set API Version Header
            ServiceBuilder::new().map_response(|mut r: Response| {
                r.headers_mut().insert(
                    "Docker-Distribution-API-Version",
                    HeaderValue::from_static("registry/2.0"),
                );
                r
            }),
        );

        let rt = tokio::runtime::Builder::new_multi_thread()
            // NOTE: graceful shutdown depends on the "rocket-worker" prefix.
            .thread_name("rocket-worker-thread")
            .enable_all()
            .build()?;

        // Start GRPC Backend thread.
        rt.spawn(init_trow_server(self.config.clone())?);

        if let Some(ref tls) = self.config.tls {
            if !(Path::new(&tls.cert_file).is_file() && Path::new(&tls.key_file).is_file()) {
                return Err(anyhow!(
                    "Could not find TLS certificate and key at {} and {}",
                    tls.cert_file,
                    tls.key_file
                ));
            }

            let config = RustlsConfig::from_pem_file(&tls.cert_file, &tls.key_file)
                .await
                .unwrap();

            axum_server::bind_rustls(self.config.addr, config)
                .serve(app.into_make_service())
                .await?;
        } else {
            axum_server::bind(self.config.addr)
                .serve(app.into_make_service())
                .await?;
        };

        Ok(())
    }
}

pub fn build_handlers(listen_addr: String) -> Result<ClientInterface> {
    debug!("Address for backend: {}", listen_addr);

    //TODO this function is useless currently
    ClientInterface::new(listen_addr)
}
