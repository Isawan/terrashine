use std::process::exit;

use axum_prometheus::{
    metrics_exporter_prometheus::{Matcher, PrometheusBuilder},
    AXUM_HTTP_REQUESTS_DURATION_SECONDS,
};
use clap::Parser;
use jemallocator::Jemalloc;
use terrashine::{config::Args, run};
use tokio::{select, signal::unix::SignalKind, task};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .compact()
        .init();

    let metric_handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full(AXUM_HTTP_REQUESTS_DURATION_SECONDS.to_string()),
            &[
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ],
        )
        .unwrap()
        .install_recorder()
        .unwrap();

    let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate()).unwrap();
    let mut sigint = tokio::signal::unix::signal(SignalKind::interrupt()).unwrap();
    let cancel = CancellationToken::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = task::spawn(run(args, Some(metric_handle), cancel.child_token(), tx));

    let _handle_signal = task::spawn(async move {
        select! {
            _ = sigterm.recv() => tracing::info!("Received SIGTERM"),
            _ = sigint.recv() => tracing::info!("Received SIGINT"),
        }
        tracing::info!("Terminating server");
        cancel.cancel();
    });

    // Either wait until the server is ready or complete
    if (rx.await).is_ok() {
        tracing::info!("Server ready");
    }

    let result = handle.await.unwrap();
    tracing::info!("Server shutdown");

    match result {
        Ok(_) => exit(0),
        Err(_) => exit(1),
    }
}
