mod app;
mod artifacts;
mod error;
mod index;
mod refresh;
mod registry;
mod version;

use std::{
    fmt::Debug,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    process::exit,
    str::FromStr,
    time::Duration,
};
use app::AppState;
use clap::Parser;
use lazy_static::lazy_static;
use reqwest::Client;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions,
};
use tokio::sync::mpsc;
use tracing::{error, log::LevelFilter};
use tracing_subscriber::EnvFilter;
use url::Url;

use crate::{refresh::refresher, registry::RegistryClient};

lazy_static! {
    static ref DEFAULT_SOCKET: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9543);
}

fn validate_redirect_url(s: &str) -> Result<Url, anyhow::Error> {
    let url = Url::parse(s)?;
    anyhow::ensure!(!url.cannot_be_a_base(), "Must be fully qualified URL");
    anyhow::ensure!(s.ends_with('/'), "URL must contain trailing slash");
    Ok(url)
}

fn parse_humantime(s: &str) -> Result<Duration, anyhow::Error> {
    match s.parse::<humantime::Duration>() {
        Ok(v) => Ok(v.into()),
        Err(e) => Err(e.into()),
    }
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Socket to listen on
    ///
    /// The host and port to bind the HTTP service
    #[arg(long, default_value_t = *DEFAULT_SOCKET, env = "TERRASHINE_HTTP_LISTEN")]
    http_listen: SocketAddr,

    /// URL for redirects, used for resolving relative URLs for redirects.
    ///
    /// This should be the URL of the load balancer or reverse proxy accessed by clients.
    ///
    /// NOTE: You must set up a TLS terminating reverse proxy in front of terrashine as
    /// terraform requires mirrors to be served over HTTPS.
    #[arg(long, value_parser = validate_redirect_url, env = "TERRASHINE_HTTP_REDIRECT_URL")]
    http_redirect_url: Url,

    /// Database connection URI
    #[arg(
        long,
        default_value = "postgres://postgres:password@localhost/",
        env = "TERRASHINE_DATABASE_URL"
    )]
    database_url: String,

    /// Number of database connections in pool
    #[arg(long, default_value_t = 5, env = "TERRASHINE_DATABASE_POOL")]
    database_pool: u32,

    /// S3 Bucket name
    ///
    /// Used to cache upstream artifacts
    #[arg(long, env = "TERRASHINE_S3_BUCKET_NAME")]
    s3_bucket_name: String,

    /// Custom S3 Endpoint
    ///
    /// Used for S3 compatible interfaces such as minio or localstack.
    /// This is discovered automatically via AWS SDK if not defined.
    #[arg(long, env = "TERRASHINE_S3_ENDPOINT")]
    s3_endpoint: Option<Url>,

    /// Refresh interval
    ///
    /// Time between terraform index refreshes.
    /// Terrashine starts a refresh clock starting when the first request arrives
    /// on this instance of the application.
    /// The clock is not persisted across application restarts.
    #[arg(long, value_parser = parse_humantime, default_value = "3600s", env = "TERRASHINE_REFRESH_INTERVAL")]
    refresh_interval: Duration,
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
    if let Err(error) = server.await {
        tracing::error!(reason=?error, "HTTP service failed");
    }
    tracing::info!("Terminating server");
}
