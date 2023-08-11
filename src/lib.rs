mod app;
pub mod config;
pub mod credhelper;
mod error;
mod http;
mod refresh;
mod registry;

use app::AppState;
use axum_prometheus::metrics_exporter_prometheus::PrometheusHandle;
use config::Args;
use reqwest::Client;
use sqlx::postgres::PgPoolOptions;
use std::{net::SocketAddr, time::Duration};
use tokio::{
    select,
    sync::{mpsc, oneshot::Sender},
};
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    credhelper::database::DatabaseCredentials, refresh::refresher, registry::RegistryClient,
};

#[derive(Debug)]
pub struct StartUpNotify {
    pub bind_socket: SocketAddr,
}

pub async fn run(
    config: Args,
    metric_handle: Option<PrometheusHandle>,
    cancel: CancellationToken,
    startup: Sender<StartUpNotify>,
) -> Result<(), ()> {
    let (tx, rx) = mpsc::channel(10000);

    // path style required for minio to work
    // Set up AWS SDK
    let aws_config = aws_config::from_env().load().await;
    let mut s3_config = aws_sdk_s3::config::Builder::from(&aws_config).force_path_style(true);
    if let Some(endpoint) = &config.s3_endpoint {
        s3_config = s3_config.endpoint_url(endpoint.as_str());
    }
    let s3 = aws_sdk_s3::Client::from_conf(s3_config.build());

    let db_result = PgPoolOptions::new()
        .max_connections(config.database_pool)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(config.database_url.clone())
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
    let refresher_registry = RegistryClient::new(
        config.upstream_registry_port,
        http.clone(),
        DatabaseCredentials::new(db.clone()),
    );
    let refresher = refresher(
        &refresher_db,
        &refresher_registry,
        rx,
        config.refresh_interval,
        cancel.child_token(),
    );

    let bind_addr = config.http_listen;
    let app = app::provider_mirror_app(AppState::new(config, s3, db, http, tx), metric_handle);

    let server = axum::Server::bind(&bind_addr).serve(app.into_make_service());

    startup
        .send(StartUpNotify {
            bind_socket: server.local_addr(),
        })
        .expect("Sender channel has already been used");

    select! {
        _ = server => (),
        _ = refresher => (),
        _ = cancel.cancelled() => tracing::trace!("Cancellation requested"),
    }
    tracing::debug!("Shutting down server");
    Ok(())
}
