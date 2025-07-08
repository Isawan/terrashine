use clap::{command, Parser};
use lazy_static::lazy_static;
use reqwest::NoProxy;
use sqlx::postgres::PgConnectOptions;
use std::{
    fmt::Debug,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    time::Duration,
};
use url::Url;

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

fn parse_no_proxy(s: &str) -> Result<Option<NoProxy>, anyhow::Error> {
    NoProxy::from_string(s)
        .map(Some)
        .ok_or(anyhow::anyhow!("Could not parse no_proxy"))
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
#[allow(clippy::large_enum_variant)]
pub enum Args {
    Server(ServerArgs),
    Migrate(MigrateArgs),
    IsHealthy(IsHealthyArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct ServerArgs {
    /// Socket to listen on
    ///
    /// The host and port to bind the HTTP service
    #[arg(long, default_value_t = *DEFAULT_SOCKET, env = "TERRASHINE_HTTP_LISTEN")]
    pub http_listen: SocketAddr,

    /// URL for redirects, used for resolving relative URLs for redirects.
    ///
    /// This should be the URL of the load balancer or reverse proxy accessed by clients.
    ///
    /// NOTE: You must set up a TLS terminating reverse proxy in front of terrashine as
    /// terraform requires mirrors to be served over HTTPS.
    #[arg(long, value_parser = validate_redirect_url, env = "TERRASHINE_HTTP_REDIRECT_URL")]
    pub http_redirect_url: Url,

    /// Database connection URI
    #[arg(
        long,
        default_value = "postgres://postgres:password@localhost/",
        env = "TERRASHINE_DATABASE_URL"
    )]
    pub database_url: PgConnectOptions,

    /// Number of database connections in pool
    #[arg(long, default_value_t = 5, env = "TERRASHINE_DATABASE_POOL")]
    pub database_pool: u32,

    /// S3 Bucket name
    ///
    /// Used to cache upstream artifacts
    #[arg(long, env = "TERRASHINE_S3_BUCKET_NAME")]
    pub s3_bucket_name: String,

    /// S3 Bucket prefix
    ///
    /// Prefix for object keys
    #[arg(long, default_value = "", env = "TERRASHINE_S3_BUCKET_PREFIX")]
    pub s3_bucket_prefix: String,

    /// Custom S3 Endpoint
    ///
    /// Used for S3 compatible interfaces such as minio or localstack.
    /// This is discovered automatically via AWS SDK if not defined.
    #[arg(long, env = "TERRASHINE_S3_ENDPOINT")]
    pub s3_endpoint: Option<Url>,

    /// Refresh interval
    ///
    /// Time between terraform index refreshes.
    /// Terrashine starts a refresh clock starting when the first request arrives
    /// on this instance of the application.
    /// The clock is not persisted across application restarts.
    #[arg(long, value_parser = parse_humantime, default_value = "3600s", env = "TERRASHINE_REFRESH_INTERVAL")]
    pub refresh_interval: Duration,

    /// Upstream terraform registry port
    ///
    /// This is used to construct the default registry URL for upstream requests.
    /// This should only ever be used for testing purposes where the upstream registry
    /// is not listening on the default port.
    /// This should basically never be used in outside of a development or testing environment.
    #[arg(
        long,
        env = "TERRASHINE_UPSTREAM_REGISTRY_PORT",
        default_value = "443",
        hide = true
    )]
    pub upstream_registry_port: u16,

    /// Proxy for HTTP downloading registry
    ///
    /// The address to the proxy server.
    /// For example "http://127.0.0.1:3128"
    #[arg(long, default_value = None, env = "HTTP_PROXY")]
    pub http_proxy: Option<String>,

    /// Exclusions to proxy
    ///
    /// A comma separated list of domains to exclude from the proxy.
    /// For example "localhost,github.com"
    #[arg(long, value_parser = parse_no_proxy, default_value = None, env = "NO_PROXY")]
    pub no_proxy: Option<NoProxy>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct MigrateArgs {
    /// Database connection URI
    #[arg(
        long,
        default_value = "postgres://postgres:password@localhost/",
        env = "TERRASHINE_DATABASE_URL"
    )]
    pub database_url: PgConnectOptions,
}

#[derive(clap::Args, Debug, Clone)]
pub struct IsHealthyArgs {
    /// The host and port to bind the HTTP service
    #[arg(long, default_value_t = *DEFAULT_SOCKET, env = "TERRASHINE_HTTP_LISTEN")]
    pub http_listen: SocketAddr,
}

// implement test
#[cfg(test)]
mod tests {
    use super::*;

    // Test URL validation
    #[tokio::test]
    async fn test_url_validation() {
        let url = "https://example.com/mirror/v1";
        assert!(validate_redirect_url(url).is_err());

        let url = "/provider/";
        assert!(validate_redirect_url(url).is_err());

        let url = "https://example.com/mirror/v1/";
        assert!(validate_redirect_url(url).is_ok());
    }

    // Validate clap CLI parsing
    #[tokio::test]
    async fn test_clap_cli_parsing() {
        let _ = Args::try_parse_from([
            "./terrashine",
            "server",
            "--http-redirect-url",
            "https://example.com/",
            "--s3-bucket-name",
            "terrashine",
        ])
        .expect("Could not parse");
    }
}
