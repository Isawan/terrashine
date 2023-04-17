use axum::{routing::get, Router};
use sqlx::{Pool, Postgres};
use tokio::sync::mpsc;
use tower_http::{
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;

use crate::{
    artifacts::artifacts_handler, index::index_handler, refresh::RefreshRequest,
    registry::RegistryClient, version::version_handler, Args,
};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) s3_client: aws_sdk_s3::Client,
    pub(crate) http_client: reqwest::Client,
    pub(crate) db_client: Pool<Postgres>,
    pub(crate) registry_client: RegistryClient,
    pub(crate) args: Args,
    pub(crate) refresher_tx: mpsc::Sender<RefreshRequest>,
}

impl AppState {
    pub(crate) fn new(
        args: Args,
        s3: aws_sdk_s3::Client,
        db: Pool<Postgres>,
        http: reqwest::Client,
        refresher_tx: mpsc::Sender<RefreshRequest>,
    ) -> AppState {
        AppState {
            s3_client: s3,
            http_client: http.clone(),
            db_client: db,
            registry_client: RegistryClient::new(http),
            args,
            refresher_tx,
        }
    }
}

pub(crate) fn provider_mirror_app(state: AppState) -> Router {
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
        .with_state(state)
}
