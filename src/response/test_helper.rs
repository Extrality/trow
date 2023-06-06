#![cfg(test)]

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use axum::Router;

use crate::client_interface::ClientInterface;
use crate::routes::create_app;
use crate::{GrpcConfig, TrowConfig, TrowServerState};

pub fn test_client() -> Router {
    let state = TrowServerState {
        client: ClientInterface::new("lol".to_string()).unwrap(),
        config: TrowConfig {
            data_dir: "".to_string(),
            addr: SocketAddr::from((IpAddr::from_str("127.0.0.1").unwrap(), 51000)),
            tls: None,
            grpc: GrpcConfig {
                listen: "trow:51000".to_owned(),
            },
            proxy_registry_config: vec![],
            image_validation_config: None,
            service_name: String::new(),
            dry_run: false,
            max_manifest_size: 1,
            max_blob_size: 100,
            token_secret: "secret".to_string(),
            user: None,
            cors: false,
            log_level: "error".to_string(),
        },
    };

    create_app(state)
}
