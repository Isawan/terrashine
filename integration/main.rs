use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use sqlx::{pool::PoolOptions, postgres::PgConnectOptions, Postgres};
use terrashine::{self, config::Args};
use tokio::select;
use url::Url;

#[sqlx::test]
fn test_server_startup(_: PoolOptions<Postgres>, db_options: PgConnectOptions) {
    let config = Args {
        database_url: db_options,
        database_pool: 3,
        s3_bucket_name: "terrashine".to_string(),
        s3_endpoint: Some(Url::parse("http://localhost:9000").unwrap()),
        http_redirect_url: Url::parse("https://localhost:9443/").unwrap(),
        http_listen: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 6000),
        refresh_interval: Duration::from_secs(10),
        upstream_registry_port: 5000,
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
            assert_eq!(status.unwrap().status(), reqwest::StatusCode::OK);
        },
        _ = tokio::time::sleep(Duration::from_secs(1)) => {
            cancellation_token.cancel();
            assert!(false, "Test did not complete in time")
        }
    }
}
