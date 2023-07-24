mod app;
mod config;
mod error;
mod http;
mod refresh;
mod registry;

use app::AppState;
use clap::Parser;
use config::Args;
use futures::{channel::oneshot::Cancellation, Future};
use hyper::{server::conn::AddrIncoming, Server};
use reqwest::Client;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions,
};
use std::{str::FromStr, time::Duration};
use tokio::{select, signal::unix::SignalKind, sync::mpsc, task};
use tokio_test::task::spawn;
use tokio_util::sync::CancellationToken;
use tracing::{error, log::LevelFilter};
use tracing_subscriber::EnvFilter;

use crate::{refresh::refresher, registry::RegistryClient};

async fn serve(config: Args, cancel: CancellationToken) -> Result<(), ()> {
    let cancel_token = CancellationToken::new();

    let (tx, rx) = mpsc::channel(10000);

    // path style required for minio to work
    // Set up AWS SDK
    let aws_config = aws_config::from_env().load().await;
    let mut s3_config = aws_sdk_s3::config::Builder::from(&aws_config).force_path_style(true);
    if let Some(endpoint) = &config.s3_endpoint {
        s3_config = s3_config.endpoint_url(endpoint.as_str());
    }
    let s3 = aws_sdk_s3::Client::from_conf(s3_config.build());

    // Set up database connection
    let mut db_options = PgConnectOptions::from_str(&config.database_url)
        .expect("Could not parse database URL")
        .log_statements(LevelFilter::Debug);

    let db_result = PgPoolOptions::new()
        .max_connections(config.database_pool)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(db_options)
        .await;

    let db = match db_result {
        Ok(pool) => pool,
        Err(error) => {
            error!(reason = %error, "Could not initialize pool, exiting.");
            return Err(());
        }
    };

    // Set up HTTP pool
    let http_builder = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60));
    let http = match http_builder.build() {
        Ok(client) => client,
        Err(error) => {
            error!(reason = %error, "Could not initialize http client, exiting.");
            return Err(());
        }
    };

    let refresher_db = db.clone();
    let refresher_registry = RegistryClient::new(config.upstream_registry_port, http.clone());
    let refresher = refresher(
        &refresher_db,
        &refresher_registry,
        rx,
        config.refresh_interval,
        cancel.child_token(),
    );

    let bind_addr = config.http_listen.clone();
    let app = app::provider_mirror_app(AppState::new(config, s3, db, http, tx));

    let server = axum::Server::bind(&bind_addr).serve(app.into_make_service());
    tracing::info!("Started server");
    select! {
        _ = server => (),
        _ = refresher => (),
        _ = cancel_token.cancelled() => tracing::trace!("Cancellation requested"),
    }
    tracing::debug!("Shutting down server");
    Ok(())
}

#[tokio::main]
async fn main() {
    let args = config::Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .init();

    tracing::info!("Started server");

    let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate()).unwrap();
    let mut sigint = tokio::signal::unix::signal(SignalKind::interrupt()).unwrap();
    let cancel = CancellationToken::new();
    task::spawn(serve(args, cancel.child_token()));
    select! {
        _ = sigterm.recv() => tracing::info!("Received SIGTERM"),
        _ = sigint.recv() => tracing::info!("Received SIGINT"),
    }
    cancel.cancel();
    tracing::info!("Terminating server");
}
