use clap::Parser;
use jemallocator::Jemalloc;
use terrashine::{config::ServerArgs, run_lambda};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[derive(Parser, Debug)]
struct Args {
    #[command(flatten)]
    delegate: ServerArgs,
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let args = Args::parse().delegate;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .compact()
        .init();

    let cancel = CancellationToken::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = run_lambda(args, cancel.child_token(), tx);

    // Either wait until the server is ready or complete
    if (rx.await).is_ok() {
        tracing::info!("Server ready");
    }

    handle.await.unwrap();
    tracing::info!("Server shutdown");
    Ok(())
}
