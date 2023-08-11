use axum::{routing::get, Router};
use axum_prometheus::{
    metrics_exporter_prometheus::PrometheusHandle, PrometheusMetricLayerBuilder,
};
use sqlx::{Pool, Postgres};
use tokio::sync::mpsc;
use tower_http::{
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;

use crate::{
    config::Args, credhelper::database::DatabaseCredentials, http::artifacts::artifacts_handler,
    http::healthcheck::healthcheck_handler, http::index::index_handler,
    http::version::version_handler, refresh::RefreshRequest, registry::RegistryClient,
};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) s3_client: aws_sdk_s3::Client,
    pub(crate) http_client: reqwest::Client,
    pub(crate) db_client: Pool<Postgres>,
    pub(crate) registry_client: RegistryClient<DatabaseCredentials>,
    pub(crate) config: Args,
    pub(crate) refresher_tx: mpsc::Sender<RefreshRequest>,
}

impl AppState {
    pub(crate) fn new(
        config: Args,
        s3: aws_sdk_s3::Client,
        db: Pool<Postgres>,
        http: reqwest::Client,
        refresher_tx: mpsc::Sender<RefreshRequest>,
    ) -> AppState {
        AppState {
            s3_client: s3,
            http_client: http.clone(),
            db_client: db.clone(),
            registry_client: RegistryClient::new(
                config.upstream_registry_port,
                http,
                DatabaseCredentials::new(db),
            ),
            config,
            refresher_tx,
        }
    }
}

pub(crate) fn provider_mirror_app(
    state: AppState,
    metric_handle: Option<PrometheusHandle>,
) -> Router {
    let metric_layer = PrometheusMetricLayerBuilder::new()
        .with_group_patterns_as(
            "/:hostname/:namespace/:provider_type/index.json",
            &["/:hostname/:namespace/:provider_type/index.json"],
        )
        .with_group_patterns_as(
            "/:hostname/:namespace/:provider_type/:version",
            &["/:hostname/:namespace/:provider_type/:version"],
        )
        .with_group_patterns_as("/artifacts/:version_id", &["/artifacts/:version_id"])
        .build();

    Router::new()
        .route(
            "/:hostname/:namespace/:provider_type/index.json",
            get(index_handler),
        )
        .route(
            "/:hostname/:namespace/:provider_type/:version",
            get(version_handler),
        )
        .route("/artifacts/:version_id", get(artifacts_handler))
        .route("/healthcheck", get(healthcheck_handler))
        .route(
            "/metrics",
            get(|| async move { metric_handle.map_or("".to_string(), |x| x.render()) }),
        )
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Micros),
                ),
        )
        .layer(metric_layer)
        .with_state(state)
}
