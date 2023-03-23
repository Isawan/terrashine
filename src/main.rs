mod app;
mod cache;
mod index;
mod version;
mod writer;
use std::{
    fmt::{Debug, Display},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    process::exit,
    str::FromStr,
};

use app::AppState;
use clap::Parser;
use lazy_static::lazy_static;
use reqwest::{Client, ClientBuilder};
use sqlx::{
    pool,
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, Pool,
};
use tracing::{error, event, log::LevelFilter, Level};
use tracing_subscriber::EnvFilter;

lazy_static! {
    static ref DEFAULT_SOCKET: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9543);
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Socket to listen on
    #[arg(short, long, default_value_t = *DEFAULT_SOCKET)]
    listen: SocketAddr,

    /// Database connection  URI
    #[arg(long, default_value = "postgres://postgres:password@localhost/")]
    database_url: String,

    /// Number of database connections
    #[arg(long, default_value_t = 4)]
    database_pool: u32,
}

#[tokio::main]
async fn main() -> () {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        //.json()
        .init();

    let mut db_options =
        PgConnectOptions::from_str(&args.database_url).expect("Could not parse database URL");
    db_options.log_statements(LevelFilter::Debug);

    let db_result = PgPoolOptions::new()
        .max_connections(args.database_pool)
        .connect_with(db_options)
        .await;

    let db = match db_result {
        Ok(pool) => pool,
        Err(error) => {
            error!(reason = %error, "Could not initialize pool, exiting.");
            exit(1);
        }
    };
    let http = Client::new();

    // build application
    let app = app::provider_mirror_app(AppState::new(db, http));

    // run it with hyper on localhost:3000
    let server = axum::Server::bind(&args.listen).serve(app.into_make_service());
    tracing::info!("Started server");
    server.await.unwrap();
    tracing::info!("Terminating server");
}
