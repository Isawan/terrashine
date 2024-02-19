use axum::{extract::FromRef, routing::get, Router};
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

use crate::http::api::APIState;
use crate::{
    config::ServerArgs,
    credhelper::{database::DatabaseCredentials, CredentialHelper},
    http::artifacts::artifacts_handler,
    http::healthcheck::healthcheck_handler,
    http::index::index_handler,
    http::version::version_handler,
    refresh::RefreshRequest,
    registry::RegistryClient,
};

#[derive(Clone)]
pub(crate) struct AppState<C> {
    pub(crate) s3_client: aws_sdk_s3::Client,
    pub(crate) http_client: reqwest::Client,
    pub(crate) db_client: Pool<Postgres>,
    pub(crate) registry_client: RegistryClient<DatabaseCredentials>,
    pub(crate) config: ServerArgs,
    pub(crate) refresher_tx: mpsc::Sender<RefreshRequest>,
    pub(crate) credentials: C,
}

impl<C> AppState<C> {
    pub(crate) fn new(
        config: ServerArgs,
        s3: aws_sdk_s3::Client,
        db: Pool<Postgres>,
        http: reqwest::Client,
        refresher_tx: mpsc::Sender<RefreshRequest>,
        credentials: C,
    ) -> Self {
        Self {
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
            credentials,
        }
    }
}

impl<C: Clone> FromRef<AppState<C>> for APIState<C> {
    fn from_ref(state: &AppState<C>) -> Self {
        Self {
            credentials: state.credentials.clone(),
        }
    }
}

pub(crate) fn provider_mirror_app<C: Clone + Send + Sync + CredentialHelper + 'static>(
    state: AppState<C>,
    metric_handle: Option<PrometheusHandle>,
) -> Router {
    let metric_layer = PrometheusMetricLayerBuilder::new()
        .with_group_patterns_as(
            "/api/v1/credentials/:hostname",
            &["/api/v1/credentials/:hostname"],
        )
        .with_group_patterns_as(
            "/mirror/v1/:hostname/:namespace/:provider_type/index.json",
            &["/mirror/v1/:hostname/:namespace/:provider_type/index.json"],
        )
        .with_group_patterns_as(
            "/mirror/v1/:hostname/:namespace/:provider_type/:version",
            &["/mirror/v1/:hostname/:namespace/:provider_type/:version"],
        )
        .with_group_patterns_as(
            "/mirror/v1/artifacts/:version_id",
            &["/mirror/v1/artifacts/:version_id"],
        )
        .build();

    let api = crate::http::api::routes(APIState::from_ref(&state));
    let mirror = Router::new()
        .route(
            "/mirror/v1/:hostname/:namespace/:provider_type/index.json",
            get(index_handler),
        )
        .route(
            "/mirror/v1/:hostname/:namespace/:provider_type/:version",
            get(version_handler),
        )
        .route("/mirror/v1/artifacts/:version_id", get(artifacts_handler))
        .route("/healthcheck", get(healthcheck_handler))
        .route(
            "/metrics",
            get(|| async move { metric_handle.map_or("".to_string(), |x| x.render()) }),
        );
    Router::new()
        .merge(api)
        .merge(mirror)
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
