use crate::app::AppState;
use ::chrono::SecondsFormat;
use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
};

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
    pub last_refreshed: String,
}

pub(crate) async fn handle_provider_page<C>(
    State(AppState { db_client, .. }): State<AppState<C>>,
) -> impl IntoResponse {
    sqlx::query!("SELECT id, namespace, hostname, type, last_refreshed FROM terraform_provider")
        .fetch_all(&db_client)
        .await
        .map_err(|e| {
            tracing::error!("Database query failed: {}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })
        .map(|rows| ProviderPage {
            providers: rows
                .iter()
                .map(|row| Provider {
                    id: row.id,
                    namespace: row.namespace.clone(),
                    hostname: row.hostname.clone(),
                    r#type: row.r#type.clone(),
                    last_refreshed: row
                        .last_refreshed
                        .to_rfc3339_opts(SecondsFormat::Secs, true),
                })
                .collect(),
        })
        .map(|page| Html(page.to_string()))
}
