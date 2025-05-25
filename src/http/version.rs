use crate::app::AppState;
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use http::{header::CONTENT_TYPE, HeaderMap, HeaderValue};
use hyper::StatusCode;
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::HashMap;
use tokio_stream::StreamExt;

use super::response_types::{MirrorVersion, TargetPlatformIdentifier};

pub(crate) async fn version_handler<C>(
    State(AppState {
        db_client: db,
        config: args,
        ..
    }): State<AppState<C>>,
    Path((hostname, namespace, provider_type, version)): Path<(String, String, String, Version)>,
) -> Result<MirrorVersion, StatusCode> {
    let downloads_result =
        list_downloads(&db, &hostname, &namespace, &provider_type, version.prefix()).await;
    let downloads = match downloads_result {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(reason=?e,"Error occured querying database");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    Ok(MirrorVersion::build(
        downloads,
        args.http_redirect_url.as_str(),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct Version {
    full: String,
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("Could not parse {0}")]
pub struct VersionParseError(String);

impl Version {
    fn prefix(&self) -> &str {
        // This is safe as we verified this when creating the type.
        self.full.strip_suffix(".json").unwrap()
    }
}

impl TryFrom<String> for Version {
    type Error = VersionParseError;
    fn try_from(value: String) -> Result<Version, VersionParseError> {
        match value.strip_suffix(".json") {
            Some(_) => Ok(Version { full: value }),
            None => Err(VersionParseError(value)),
        }
    }
}

fn archive_name(os: &str, arch: &str) -> String {
    let mut s = String::with_capacity(os.len() + 1 + arch.len());
    s.push_str(os);
    s.push('_');
    s.push_str(arch);
    s
}

fn build_url(base_url: String, id: i64) -> String {
    let mut s = base_url;
    s.push_str("artifacts/");
    s.push_str(&id.to_string());
    s
}

struct DatabaseDownloadResult {
    os: String,
    arch: String,
    id: i64,
}

impl MirrorVersion {
    fn build(result: Vec<DatabaseDownloadResult>, base_url: &str) -> Self {
        let mut archives = HashMap::new();
        for DatabaseDownloadResult { os, arch, id } in result.iter() {
            let target = archive_name(os, arch);
            let url = build_url(base_url.to_string(), *id);
            archives.insert(target, TargetPlatformIdentifier { url });
        }
        Self { archives }
    }
}

impl IntoResponse for MirrorVersion {
    fn into_response(self) -> Response {
        let mut headers = HeaderMap::new();
        // NOTE: We don't cache here as in a highly available setup there are
        // race conditions.
        // This occurs when a newer version entry in the index list is serving a
        // version which does not yet exist in the stale cache for this endpoint.
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let response = match serde_json::to_string(&self) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(reason = ?e, "Could not serialize VersionIndex");
                return (headers, StatusCode::INTERNAL_SERVER_ERROR).into_response();
            }
        };
        (headers, response).into_response()
    }
}

async fn list_downloads(
    db: &PgPool,
    hostname: &str,
    namespace: &str,
    provider_type: &str,
    version: &str,
) -> Result<Vec<DatabaseDownloadResult>, anyhow::Error> {
    tracing::trace!(?hostname, ?namespace, ?provider_type, ?version);
    let query = sqlx::query!(
        r#"
        select "terraform_provider_version"."id", "os", "arch"
        from "terraform_provider_version"
        inner join "terraform_provider" on
            "terraform_provider_version"."provider_id" = "terraform_provider"."id"
        where
            "terraform_provider_version"."version" = $1
            and "terraform_provider"."hostname" = $2
            and "terraform_provider"."namespace" = $3
            and "terraform_provider"."type" = $4;
        "#,
        version,
        hostname,
        namespace,
        provider_type,
    );
    let mut rows = query.fetch(db);
    let mut result = vec![];
    while let Some(row) = rows.try_next().await? {
        result.push(DatabaseDownloadResult {
            id: row.id,
            os: row.os,
            arch: row.arch,
        });
    }
    Ok(result)
}
