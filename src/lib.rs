mod app;
pub mod config;
pub mod credhelper;
mod error;
pub mod healthy;
mod http;
mod migrate;
mod refresh;
mod registry;

use app::AppState;
use aws_config::BehaviorVersion;
use axum::extract::Request;
use axum_prometheus::metrics_exporter_prometheus::PrometheusHandle;
use config::{Args, ServerArgs};
use futures::join;
use hyper::body::Incoming;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server,
};
use migrate::run_migrate;
use reqwest::{Certificate, Client, Proxy};
use rustls_native_certs::CertificateResult;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    net::TcpListener,
    select,
    sync::{mpsc, oneshot::Sender},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;
use tower::Service;
use tracing::{error, warn};

use crate::{
    credhelper::database::DatabaseCredentials, healthy::run_healthy, refresh::refresher,
    registry::RegistryClient,
};

#[derive(Debug)]
pub struct StartUpNotify<T> {
    pub msg: T,
}

async fn serve(listener: TcpListener, cancel: CancellationToken, app: axum::Router) {
    let mut join_set = JoinSet::new();
    loop {
        select! {
                result = listener.accept() => {
                let (socket, _remote_addr) = match result {
                    Ok(x) => x,
                    Err(err) => {
                        warn!(reason = %err, "failed to accept connection");
                        continue;
                    }
                };
                let tower_service = app.clone();

                // Spawn a task to handle the connection. That way we can multiple connections
                // concurrently.
                join_set.spawn(async move {
                    let socket = TokioIo::new(socket);

                    // Hyper also has its own `Service` trait and doesn't use tower. We can use
                    // `hyper::service::service_fn` to create a hyper `Service` that calls our app through
                    // `tower::Service::call`.
                    let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                        // We have to clone `tower_service` because hyper's `Service` uses `&self` whereas
                        // tower's `Service` requires `&mut self`.
                        //
                        // We don't need to call `poll_ready` since `Router` is always ready.
                        tower_service.clone().call(request)
                    });

                    if let Err(err) = server::conn::auto::Builder::new(TokioExecutor::new())
                        .serve_connection(socket, hyper_service)
                        .await
                    {
                        tracing::warn!("failed to serve connection: {err:#}");
                    }
                });
                while join_set.try_join_next().is_some() {} // clean up any completed requests
            }

            _ = cancel.cancelled() => {
                break;
            }
        }
    }
    while (join_set.join_next().await).is_some() {} // wait until all requests are done
}

pub async fn run(
    config: Args,
    metric_handle: Option<PrometheusHandle>,
    cancel: CancellationToken,
    startup: Sender<StartUpNotify<SocketAddr>>,
) -> Result<(), ()> {
    match config {
        Args::Server(args) => run_server(args, metric_handle, cancel, startup).await,
        Args::Migrate(args) => run_migrate(args).await,
        Args::IsHealthy(args) => run_healthy(args).await,
    }
}

pub async fn setup_server(
    config: &ServerArgs,
) -> Result<
    (
        reqwest::Client,
        PgPool,
        aws_sdk_s3::Client,
        DatabaseCredentials,
    ),
    (),
> {
    let CertificateResult {
        certs: certificates,
        errors: cert_errors,
        ..
    } = rustls_native_certs::load_native_certs();
    for error in cert_errors {
        warn!(reason = %error, "Could not load certificate");
    }

    // path style required for minio to work
    // Set up AWS SDK
    let aws_config = aws_config::defaults(BehaviorVersion::v2025_01_17())
        .load()
        .await;
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
    let mut http_builder = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60));
    for cert in certificates.iter() {
        http_builder = http_builder
            .add_root_certificate(Certificate::from_der(cert.as_ref()).expect("Not a certificate"));
    }
    if let Some(proxy) = &config.http_proxy {
        let proxy = match Proxy::all(proxy) {
            Ok(proxy) => proxy,
            Err(error) => {
                error!(reason = %error, "Could not initialize proxy, exiting.");
                return Err(());
            }
        };
        let proxy = proxy.no_proxy(config.no_proxy.clone());
        http_builder = http_builder.proxy(proxy);
    };
    let http = match http_builder.build() {
        Ok(client) => client,
        Err(error) => {
            error!(reason = %error, "Could not initialize http client, exiting.");
            return Err(());
        }
    };

    // Set up credentials
    let credentials = DatabaseCredentials::new(db.clone());

    Ok((http, db, s3, credentials))
}

pub async fn run_server(
    config: ServerArgs,
    metric_handle: Option<PrometheusHandle>,
    cancel: CancellationToken,
    startup: Sender<StartUpNotify<SocketAddr>>,
) -> Result<(), ()> {
    let (http, db, s3, credentials) = setup_server(&config).await.unwrap();

    let (tx, rx) = mpsc::channel(10000);

    let refresher_db = db.clone();
    let refresher_registry = RegistryClient::new(
        config.upstream_registry_port,
        http.clone(),
        credentials.clone(),
    );
    let refresher = refresher(
        &refresher_db,
        &refresher_registry,
        rx,
        config.refresh_interval,
        cancel.child_token(),
    );

    let bind_addr = config.http_listen;
    let app = app::provider_mirror_app(
        AppState::new(config.clone(), s3, db, http, tx, credentials.clone()),
        metric_handle,
    );

    let listener = TcpListener::bind(&bind_addr).await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    let server = serve(listener, cancel.child_token(), app);

    startup
        .send(StartUpNotify { msg: local_addr })
        .expect("Sender channel has already been used");

    join!(server, refresher);
    tracing::debug!("Shutting down server");
    Ok(())
}
