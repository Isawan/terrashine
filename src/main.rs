use clap::Parser;
use terrashine::{config::Args, run};
use tokio::{select, signal::unix::SignalKind, task};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .init();

    let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate()).unwrap();
    let mut sigint = tokio::signal::unix::signal(SignalKind::interrupt()).unwrap();
    let cancel = CancellationToken::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = task::spawn(run(args, cancel.child_token(), tx));
    rx.await.unwrap();
    tracing::info!("Server ready");

    select! {
        _ = handle => tracing::info!("Server shutdown"),
        _ = sigterm.recv() => tracing::info!("Received SIGTERM"),
        _ = sigint.recv() => tracing::info!("Received SIGINT"),
    }
    cancel.cancel();
    tracing::info!("Terminating server");
}
