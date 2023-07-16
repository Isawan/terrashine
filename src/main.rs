mod app;
mod config;
mod error;
mod http;
mod refresh;
mod registry;

use app::AppState;
use axum::{routing::IntoMakeService, Router};
use clap::Parser;
use config::Args;
use futures::{channel::oneshot::Cancellation, Future};
use hyper::{server::conn::AddrIncoming, Server};
use reqwest::Client;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions,
};
use std::{process::exit, str::FromStr, time::Duration};
use tokio::{select, signal::unix::SignalKind, sync::mpsc, task};
use tokio_test::task::spawn;
use tokio_util::sync::CancellationToken;
use tracing::{error, log::LevelFilter};
use tracing_subscriber::EnvFilter;

use crate::{refresh::refresher, registry::RegistryClient};

async fn serve(config: Args, cancel: CancellationToken) {
    let cancel_token = CancellationToken::new();
    let s3_config = aws_config::load_from_env().await;
    let s3 = aws_sdk_s3::Client::new(&s3_config);
    let db_options =
        PgConnectOptions::from_str(&config.database_url).expect("Could not parse database URL");
    let db = PgPoolOptions::new()
        .max_connections(config.database_pool)
        .connect_with(db_options)
        .await
        .expect("Could not initialize pool");
    let http = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .expect("Could not initialize http client");

    let (tx, rx) = mpsc::channel(10000);

    let refresher_db = db.clone();
    let refresher_registry = RegistryClient::new(http.clone());
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
