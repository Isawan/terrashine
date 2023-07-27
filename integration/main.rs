use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    process::Stdio,
    str::from_utf8,
    time::Duration,
};

use reqwest::StatusCode;
use sqlx::{pool::PoolOptions, postgres::PgConnectOptions, Postgres};
use terrashine::{self, config::Args};
use tokio::select;
use tracing_test::traced_test;
use url::Url;

#[sqlx::test]
fn test_server_startup(_: PoolOptions<Postgres>, db_options: PgConnectOptions) {
    let config = Args {
        database_url: db_options,
        database_pool: 3,
        s3_bucket_name: "terrashine".to_string(),
        s3_endpoint: Some(Url::parse("http://localhost:9000").unwrap()),
        http_redirect_url: Url::parse("https://localhost:9443/").unwrap(),
        http_listen: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
        refresh_interval: Duration::from_secs(10),
        upstream_registry_port: 443,
    };
    let cancellation_token = tokio_util::sync::CancellationToken::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(terrashine::run(
        config,
        cancellation_token.child_token(),
        tx,
    ));
    let socket = rx.await.unwrap().bind_socket;
    select! {
        _ = handle => {
            assert!(false, "Server shutdown before client");
        },
        status = reqwest::get(format!("http://localhost:{}/healthcheck", socket.port())) => {
            assert_eq!(status.unwrap().status(), StatusCode::OK);
            cancellation_token.cancel();
        },
        _ = tokio::time::sleep(Duration::from_secs(1)) => {
            cancellation_token.cancel();
            assert!(false, "Test did not complete in time")
        }
    }
}

#[sqlx::test]
fn test_end_to_end_terraform_flow(_: PoolOptions<Postgres>, db_options: PgConnectOptions) {
    let config = Args {
        database_url: db_options,
        database_pool: 3,
        s3_bucket_name: "terrashine".to_string(),
        s3_endpoint: Some(Url::parse("http://localhost:9000").unwrap()),
        http_redirect_url: Url::parse("https://localhost:9443/").unwrap(),
        http_listen: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9543),
        refresh_interval: Duration::from_secs(10),
        upstream_registry_port: 443,
    };
    let cancellation_token = tokio_util::sync::CancellationToken::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(terrashine::run(
        config,
        cancellation_token.child_token(),
        tx,
    ));
    let _ = rx.await.unwrap().bind_socket;
    let mut terraform = tokio::process::Command::new("terraform");
    let process = terraform
        .arg("-chdir=resources/test/terraform/random-import-stack/")
        .arg("init")
        .env(
            "TF_CLI_CONFIG_FILE",
            "resources/test/terraform/random-import-stack/terraform.tfrc",
        )
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .output();
    select! {
        _ = handle => {
            assert!(false, "Server shutdown before client");
        },
        result = process => {
            let result = result.unwrap();
            let stdout = from_utf8(&result.stdout).expect("Could not parse stdout as utf-8");
            let stderr = from_utf8(&result.stderr).expect("Could not parse stderr as utf-8");
            let help_message = format!("Stdout from terraform: {}\nStderr from terraform: {}", stdout, stderr);
            assert_eq!(result.status.success(), true, "{}", help_message);
            cancellation_token.cancel();
        },
        _ = tokio::time::sleep(Duration::from_secs(60)) => {
            cancellation_token.cancel();
            assert!(false, "Test did not complete in time")
        }
    }
}
