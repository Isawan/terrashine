mod app;
mod config;
mod error;
mod http;
mod refresh;
mod registry;

use app::AppState;
use clap::Parser;
use reqwest::Client;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions,
};
use std::{process::exit, str::FromStr, time::Duration};
use tokio::{select, signal::unix::SignalKind, sync::mpsc};
use tracing::{error, log::LevelFilter};
use tracing_subscriber::EnvFilter;

use crate::{refresh::refresher, registry::RegistryClient};

#[tokio::main]
async fn main() {
    let args = config::Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .init();

    // Set up AWS SDK
    let config = aws_config::from_env().load().await;
    let mut s3_config = aws_sdk_s3::config::Builder::from(&config).force_path_style(true); // path style required for minio to work
    if let Some(endpoint) = &args.s3_endpoint {
        s3_config = s3_config.endpoint_url(endpoint.as_str());
    }
    let s3 = aws_sdk_s3::Client::from_conf(s3_config.build());

    // Set up database connection
    let mut db_options =
        PgConnectOptions::from_str(&args.database_url).expect("Could not parse database URL");
    db_options.log_statements(LevelFilter::Debug);

    let db_result = PgPoolOptions::new()
        .max_connections(args.database_pool)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(db_options)
        .await;

    let db = match db_result {
        Ok(pool) => pool,
        Err(error) => {
            error!(reason = %error, "Could not initialize pool, exiting.");
            exit(1);
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
            exit(1);
        }
    };

    let (tx, rx) = mpsc::channel(10000);

    let refresher_db = db.clone();
    let refresher_registry = RegistryClient::new(http.clone());
    tokio::spawn(async move {
        refresher(
            &refresher_db,
            &refresher_registry,
            rx,
            args.refresh_interval,
        )
        .await
    });

    // build application
    let app = app::provider_mirror_app(AppState::new(args.clone(), s3, db, http, tx));

    let server = axum::Server::bind(&args.http_listen).serve(app.into_make_service());
    tracing::info!("Started server");
    tokio::spawn(server);

    // TODO: Handle cancellation properly rather than dropping everything
    let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate()).unwrap();
    let mut sigint = tokio::signal::unix::signal(SignalKind::interrupt()).unwrap();
    select! {
        _ = sigterm.recv() => tracing::info!("Received SIGTERM"),
        _ = sigint.recv() => tracing::info!("Received SIGINT"),
    }

    tracing::info!("Terminating server");
}
