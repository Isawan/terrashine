mod app;
mod artifacts;
mod error;
mod index;
mod version;
mod registry_client;

use std::{
    fmt::Debug,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    process::exit,
    str::FromStr,
    time::Duration,
};

use app::AppState;
use clap::Parser;
use http::Uri;
use lazy_static::lazy_static;
use reqwest::Client;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions,
};
use tracing::{error, log::LevelFilter};
use tracing_subscriber::EnvFilter;

lazy_static! {
    static ref DEFAULT_SOCKET: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9543);
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Socket to listen on
    ///
    /// The host and port to bind the HTTP service
    #[arg(long, default_value_t = *DEFAULT_SOCKET)]
    http_listen: SocketAddr,

    /// URL for redirects, used for resolving relative URLs for redirects.
    ///
    /// This should be the URL of the load balancer or reverse proxy accessed by clients.
    ///
    /// NOTE: You must set up a TLS terminating reverse proxy in front of terrashine as
    /// terraform requires mirrors to be served over HTTPS.
    #[arg(long)]
    http_redirect_url: String,

    /// Database connection URI
    #[arg(long, default_value = "postgres://postgres:password@localhost/")]
    database_url: String,

    /// Number of database connections in pool
    #[arg(long, default_value_t = 5)]
    database_pool: u32,

    /// Minimum TTL on database cache before getting newer versions (seconds).
    ///
    /// NOT IMPLEMENTED
    #[arg(long, default_value_t = 300)]
    database_ttl_minimum: usize,

    /// Maximum number of entries for in-memory cache
    ///
    /// NOT IMPLEMENTED
    #[arg(long, default_value_t = 64_000)]
    cache_entry_max_count: usize,

    /// S3 Bucket name
    ///
    /// Used to cache upstream artifacts
    #[arg(long)]
    s3_bucket_name: String,

    /// Custom S3 Endpoint
    ///
    /// Used for S3 compatible interfaces such as minio or localstack.
    /// This is discovered automatically via AWS SDK if not defined.
    #[arg(long)]
    s3_endpoint: Option<Uri>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        //.json()
        .init();

    // Set up AWS SDK
    let config = aws_config::from_env().load().await;
    let mut s3_config = aws_sdk_s3::config::Builder::from(&config).force_path_style(true); // path style required for minio to work
    if let Some(endpoint) = args.s3_endpoint {
        s3_config = s3_config.endpoint_url(endpoint.to_string());
    }
    let s3 = aws_sdk_s3::Client::from_conf(s3_config.build());

    // Set up database connection
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

    // build application
    let app = app::provider_mirror_app(AppState::new(s3, db, http));

    let server = axum::Server::bind(&args.http_listen).serve(app.into_make_service());
    tracing::info!("Started server");
    if let Err(error) = server.await {
        tracing::error!(reason=?error, "HTTP service failed");
    }
    tracing::info!("Terminating server");
}
