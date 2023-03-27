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
    time::Duration,
};

use app::AppState;
use clap::Parser;
use lazy_static::lazy_static;
use moka::future::{Cache, CacheBuilder};
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

    /// Database connection URI
    #[arg(long, default_value = "postgres://postgres:password@localhost/")]
    database_url: String,

    /// Number of database connections in pool
    #[arg(long, default_value_t = 5)]
    database_pool: u32,

    /// Minimum TTL on database cache before getting newer versions (seconds)
    /// TODO: minimum TTL cache
    #[arg(long, default_value_t = 300)]
    database_ttl_minimum: usize,

    /// Maximum number of entries for in-memory cache
    #[arg(long, default_value_t = 64_000)]
    cache_entry_max_count: usize,
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

    let cache = Cache::builder()
        .initial_capacity(args.cache_entry_max_count)
        .max_capacity(args.cache_entry_max_count as u64)
        .build();

    // build application
    let app = app::provider_mirror_app(AppState::new(db, http, cache));

    // run it with hyper on localhost:3000
    let server = axum::Server::bind(&args.listen).serve(app.into_make_service());
    tracing::info!("Started server");
    server.await.unwrap();
    tracing::info!("Terminating server");
}
