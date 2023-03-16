#[cfg(test)]
mod common;

#[cfg(test)]
mod cli {
    use predicates::prelude::*;

    use crate::common::get_file;
    use trow_server::ImageValidationConfig;
    use trow_server::RegistryProxyConfig;

    fn get_command() -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
        cmd.arg("--no-tls");
        cmd.arg("--dry-run");
        cmd
    }

    #[test]
    fn invalid_argument() {
        get_command()
            .arg("-Z")
            .assert()
            .stderr(predicate::str::contains(
                "Found argument '-Z' which wasn't expected, or isn't valid in this context",
            ))
            .failure();

        get_command()
            .arg("-Z")
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "error: Found argument '-Z' which wasn't expected",
            ));
    }

    #[test]
    fn help_works() {
        get_command()
            .arg("-h")
            .assert()
            .success()
            .stdout(predicate::str::contains("Trow"));

        get_command()
            .arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("Trow"));
    }

    #[test]
    fn host_name_parsing() {
        get_command()
            .args(&["-n", "myhost.com"])
            .assert()
            .success()
            .stdout(predicate::str::contains(": \"myhost.com\""));

        get_command()
            .args(&["--name", "trow.test"])
            .assert()
            .success()
            .stdout(predicate::str::contains(": \"trow.test\""));

        get_command()
            .args(&["-n=port.io:3833"])
            .assert()
            .success()
            .stdout(predicate::str::contains(": \"port.io:3833\""));
    }

    #[test]
    fn image_validation() {
        get_command()
            .assert()
            .success()
            .stdout(predicate::str::contains("Proxy registries not configured"));

        let file = get_file(ImageValidationConfig {
            allow: vec!["trow.test/".to_string()],
            deny: vec!["toto".to_string()],
            default: "Allow".to_string(),
        });

        get_command()
            .args(&[
                "--image-validation-config-file",
                file.path().to_str().unwrap(),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains(
                [
                    "Image validation webhook configured:",
                    "  Default action: Allow",
                    "  Allowed prefixes: [\"trow.test/\"]",
                    "  Denied prefixes: [\"toto\"]",
                ]
                .join("\n"),
            ));
    }

    #[test]
    fn registry_proxy() {
        get_command()
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "Image validation webhook not configured",
            ));

        let file = get_file::<Vec<RegistryProxyConfig>>(vec![
            RegistryProxyConfig {
                alias: "lovni".to_string(),
                host: "jul.example.com".to_string(),
                username: Some("robert".to_string()),
                password: Some("1234".to_string()),
            },
            RegistryProxyConfig {
                alias: "trow".to_string(),
                host: "127.0.0.1".to_string(),
                username: None,
                password: None,
            },
        ]);

        get_command()
            .args(&[
                "--proxy-registry-config-file",
                file.path().to_str().unwrap(),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains(
                [
                    "Proxy registries configured:",
                    "  - lovni: jul.example.com",
                    "  - trow: 127.0.0.1",
                ]
                .join("\n"),
            ));
    }

    #[test]
    fn cors() {
        get_command()
            .args(&["--enable-cors"])
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "Cross-Origin Resource Sharing(CORS) requests are allowed",
            ));
    }

    #[test]
    fn file_size_parsing() {
        get_command()
            .args(&["--max-manifest-size", "3"])
            .assert()
            .success()
            .stdout(predicate::str::contains("manifest size: 3"));

        get_command()
            .args(&["--max-manifest-size", "-4"])
            .assert()
            .failure();

        get_command()
            .args(&["--max-manifest-size", "1.1"])
            .assert()
            .failure();
    }

    #[test]
    fn log_level_setting() {
        get_command()
            .args(&["--log-level", "TRACE"])
            .assert()
            .success();
    }
}
