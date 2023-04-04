use anyhow::Context;
use axum::{
    body::Bytes,
    extract::{Path, State},
};
use http::StatusCode;
use reqwest::{Body, Client, Response, Url};
use sqlx::{query, query_as, PgPool};
use tokio_stream::Stream;
use url::ParseError;

use crate::{app::AppState, index};

pub async fn artifacts_handler<'a>(
    State(AppState {
        http_client: http,
        db_client: db,
        ..
    }): State<AppState>,
    Path((version_id)): Path<(i64)>,
) -> Result<Bytes, StatusCode> {
    let artifact = match get_artifact(db, version_id).await {
        Ok(Some(x)) => x,
        Ok(None) => {
            tracing::debug!(?version_id, "Version id requested not found in database");
            return Err(StatusCode::NOT_FOUND);
        }
        Err(e) => {
            tracing::error!(reason=?e, ?version_id, "Error querying database for artifact details");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    todo!();
}

#[derive(sqlx::FromRow)]
struct ArtifactDetails {
    hostname: String,
    namespace: String,
    #[sqlx(rename = "type")]
    provider_type: String,
    version: String,
    os: String,
    arch: String,
    sha256sum: Option<String>,
}

async fn get_artifact(
    db: PgPool,
    version_id: i64,
) -> Result<Option<ArtifactDetails>, anyhow::Error> {
    let result = query_as!(
        ArtifactDetails,
        r#"
        select
            "hostname",
            "namespace",
            "type" as "provider_type",
            "version",
            "os",
            "arch",
            "sha256sum"
        from "terraform_provider_version"
        inner join "terraform_provider"
            on "terraform_provider_version"."provider_id" = "terraform_provider"."id"
            where "terraform_provider_version"."id" = $1;
        "#,
        version_id
    )
    .fetch_optional(&db)
    .await?;
    return Ok(result);
}

fn build_url(artifact: ArtifactDetails) -> Result<Url, ParseError> {
    let mut url_builder = String::new();
    url_builder.push_str("https://");
    url_builder.push_str(&artifact.hostname);
    url_builder.push_str("/v1/providers/");
    url_builder.push_str(&artifact.namespace);
    url_builder.push_str("/");
    url_builder.push_str(&artifact.provider_type);
    url_builder.push_str("/");
    url_builder.push_str(&artifact.version);
    url_builder.push_str("/download");
    url_builder.push_str(&artifact.os);
    url_builder.push_str("/");
    url_builder.push_str(&artifact.arch);

    Url::parse(&url_builder)
}

async fn get_upstream(
    http: Client,
    artifact: ArtifactDetails,
) -> Result<impl Stream<Item = reqwest::Result<Bytes>>, anyhow::Error> {
    let url = build_url(artifact)?;
    let response = http.get(url).send().await?;
    let success = response.error_for_status()?;
    let stream = success.bytes_stream();
    return Ok(stream);
}
