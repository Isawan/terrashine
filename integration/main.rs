mod credhelper;
mod util;

use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    path::Path,
    process::Stdio,
    str::from_utf8,
    time::Duration,
};

use crate::util::copy_dir;
use reqwest::StatusCode;
use sqlx::{pool::PoolOptions, postgres::PgConnectOptions, Postgres};
use terrashine::{
    self,
    config::{IsHealthyArgs, ServerArgs},
};
use tokio::select;
use tower_http::trace;
use tracing_test::traced_test;
use url::Url;
use uuid::Uuid;

#[traced_test]
#[sqlx::test]
fn test_server_startup(_: PoolOptions<Postgres>, db_options: PgConnectOptions) {
    let prefix = format!("{}/", Uuid::new_v4());
    let config = ServerArgs {
        database_url: db_options,
        database_pool: 3,
        s3_bucket_name: "terrashine".to_string(),
        s3_bucket_prefix: prefix,
        s3_endpoint: Some(Url::parse("http://localhost:9000").unwrap()),
        http_redirect_url: Url::parse("https://localhost:9443/").unwrap(),
        http_listen: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
        refresh_interval: Duration::from_secs(10),
        upstream_registry_port: 443,
        http_proxy: None,
        no_proxy: None,
    };
    let cancellation_token = tokio_util::sync::CancellationToken::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(terrashine::run_server(
        config,
        None,
        cancellation_token.child_token(),
        tx,
    ));
    let socket = rx.await.unwrap().msg;
    select! {
        _ = handle => {
            assert!(false, "Server shutdown before client");
        },
        status = reqwest::get(format!("http://localhost:{}/healthcheck", socket.port())) => {
            assert_eq!(status.unwrap().status(), StatusCode::OK);
            cancellation_token.cancel();
        },
        _ = tokio::time::sleep(Duration::from_secs(10)) => {
            cancellation_token.cancel();
            assert!(false, "Test did not complete in time")
        }
    }
}

#[traced_test]
#[sqlx::test]
fn test_end_to_end_terraform_flow(_: PoolOptions<Postgres>, db_options: PgConnectOptions) {
    let prefix = format!("{}/", Uuid::new_v4());
    let config = ServerArgs {
        database_url: db_options,
        database_pool: 3,
        s3_bucket_name: "terrashine".to_string(),
        s3_bucket_prefix: prefix,
        s3_endpoint: Some(Url::parse("http://localhost:9000").unwrap()),
        http_redirect_url: Url::parse("https://localhost:9443/mirror/v1/").unwrap(),
        http_listen: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9543),
        refresh_interval: Duration::from_secs(10),
        upstream_registry_port: 443,
        http_proxy: None,
        no_proxy: None,
    };
    let cancellation_token = tokio_util::sync::CancellationToken::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(terrashine::run_server(
        config,
        None,
        cancellation_token.child_token(),
        tx,
    ));
    let _ = rx.await.unwrap().msg;

    // Set up temp folder
    let folder1 = tempfile::tempdir().expect("Could not create folder");
    let folder2 = tempfile::tempdir().expect("Could not create folder");

    // Perform two downloads
    // The first pulls the data into cache
    // The second confirms that the data in cache is usable
    for folder in [folder1, folder2] {
        copy_dir("resources/test/terraform/random-import-stack", &folder)
            .expect("Could not copy folder");
        let temp_folder = folder.path().to_str().expect("Could not get tempdir path");
        let config_path = format!("{temp_folder}/terraform.tfrc");
        assert!(Path::new(&config_path).exists());

        let mut terraform = tokio::process::Command::new("terraform");
        let process = terraform
            .arg(format!("-chdir={temp_folder}"))
            .arg("init")
            .env("TF_CLI_CONFIG_FILE", config_path)
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .output();
        select! {
            result = process => {
                let result = result.unwrap();
                let stdout = from_utf8(&result.stdout).expect("Could not parse stdout as utf-8");
                let stderr = from_utf8(&result.stderr).expect("Could not parse stderr as utf-8");
                let help_message = format!("Stdout from terraform: {}\nStderr from terraform: {}", stdout, stderr);
                assert_eq!(result.status.success(), true, "{}", help_message);
            },
            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                assert!(false, "Test did not complete in time")
            }
        }
    }

    cancellation_token.cancel();
    handle.abort();
}

/// Set up a terrashine and use the client to test the health check endpoint
#[traced_test]
#[sqlx::test]
fn test_health_check_functionality(_: PoolOptions<Postgres>, db_options: PgConnectOptions) {
    let prefix = format!("{}/", Uuid::new_v4());
    let config = ServerArgs {
        database_url: db_options,
        database_pool: 3,
        s3_bucket_name: "terrashine".to_string(),
        s3_bucket_prefix: prefix,
        s3_endpoint: Some(Url::parse("http://localhost:9000").unwrap()),
        http_redirect_url: Url::parse("https://localhost:9445/mirror/v1/").unwrap(),
        http_listen: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9545),
        refresh_interval: Duration::from_secs(10),
        upstream_registry_port: 443,
        http_proxy: None,
        no_proxy: None,
    };
    let cancellation_token = tokio_util::sync::CancellationToken::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(terrashine::run_server(
        config,
        None,
        cancellation_token.child_token(),
        tx,
    ));
    let _ = rx.await.unwrap().msg;

    terrashine::healthy::run_healthy(IsHealthyArgs {
        http_listen: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9545),
    })
    .await
    .expect("Health check failed");

    // run is_healthy
    handle.abort();
}
