use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
};
use sqlx::types::time::OffsetDateTime;

use crate::app::AppState;

#[derive(Template)]
#[template(path = "provider.html")]
pub(crate) struct ProviderPage {
    pub(crate) providers: Vec<Provider>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Provider {
    pub id: i64,
    pub namespace: String,
    pub hostname: String,
    pub r#type: String,
    pub last_refreshed: OffsetDateTime,
}

pub(crate) async fn handle_provider_page<C>(
    State(AppState { db_client, .. }): State<AppState<C>>,
) -> impl IntoResponse {
    sqlx::query_as!(
        Provider,
        "SELECT id, namespace, hostname, type, last_refreshed FROM terraform_provider"
    )
    .fetch_all(&db_client)
    .await
    .map_err(|e| {
        tracing::error!("Database query failed: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })
    .map(|rows| {
        tracing::error!("Fetched {:?} providers", rows);
        ProviderPage { providers: rows }
    })
    .map(|page| Html(page.to_string()))
}
