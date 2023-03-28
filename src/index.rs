use http::{header::CONTENT_TYPE, HeaderValue};
use moka::future::Cache;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::fmt::Debug;
use std::time::{Duration, Instant};
use std::{collections::HashMap, fmt::Display, hash::Hash, mem};
use tokio::time::sleep;
use tokio_stream::{self as stream, StreamExt};
use tracing::{info, span, Level};
use url::ParseError;

use axum::{
    extract::{Path, State},
    response::Response,
    Json,
};
use hyper::{header::HeaderName, HeaderMap, StatusCode};

use crate::app::AppState;

const MAX_INDEX_RESPONSE: u64 = 4_000_000;

#[derive(Serialize, Debug)]
struct MirrorIndex {
    // TODO: the nested hash value is always empty, we should implement
    // custom serialize to avoid unneeded work.
    versions: HashMap<String, HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
pub struct ProviderVersions {
    versions: Vec<ProviderVersionItem>,
}

#[derive(Deserialize, Debug)]
struct ProviderVersionItem {
    version: String,
    protocols: Vec<String>,
    platforms: Vec<ProviderPlatform>,
}

#[derive(Deserialize, Debug)]
struct ProviderPlatform {
    os: String,
    arch: String,
}

#[derive(sqlx::FromRow, Debug)]
struct VersionTuple {
    version: String,
    os: String,
    arch: String,
}

pub async fn index_handler(
    State(AppState {
        db_client: mut db,
        http_client: http,
        meta_cache: cache,
    }): State<AppState>,
    Path((hostname, namespace, provider_type)): Path<(String, String, String)>,
) -> Result<(HeaderMap, String), StatusCode> {
    if let Some(value) = cache.get(&(hostname.clone(), namespace.clone(), provider_type.clone())) {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        return Ok((headers, value));
    }

    match list_provider_versions(&db, &hostname, &namespace, &provider_type).await {
        Ok(Some(mirror_index)) => {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let value = serde_json::to_string(&mirror_index).unwrap();
            cache
                .insert((hostname, namespace, provider_type), value.clone())
                .await;
            return Ok((headers, value));
        }
        Ok(None) => {
            tracing::info!("Unknown provider found, fetching upstream");
        }
        Err(error) => {
            tracing::warn!(
                reason = ?error,
                "Error occured fetching provider from database, fetching upstream"
            );
        }
    }

    let upstream_url =
        build_url(&hostname, &namespace, &provider_type).map_err(|_| StatusCode::BAD_GATEWAY)?;

    let upstream_response = match http.get(upstream_url).send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::error!(%error, "Error making request to upstream");
            return Err(StatusCode::BAD_GATEWAY);
        }
    };

    let body = match upstream_response.bytes().await {
        Ok(b) => b,
        Err(error) => {
            tracing::error!(%error, "Error receiving body from upstream");
            return Err(StatusCode::BAD_GATEWAY);
        }
    };

    let provider_versions: ProviderVersions = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(error) => {
            tracing::error!(%error, "Could not deserialize upstream response");
            return Err(StatusCode::BAD_GATEWAY);
        }
    };

    let result = store_provider_versions(
        &mut db,
        &hostname,
        &namespace,
        &provider_type,
        &provider_versions,
    )
    .await;
    if let Result::Err(error) = result {
        tracing::warn!(%error, "Could not store terraform provider metadata to database");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let mirror_index = MirrorIndex::from(provider_versions);
    let response_body = match serde_json::to_string(&mirror_index) {
        Ok(body) => body,
        Err(error) => {
            tracing::warn!(reason = ?error, "Failed to serialize");
            return Err(StatusCode::BAD_GATEWAY);
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    Result::Ok((headers, response_body))
}

fn build_url(hostname: &str, namespace: &str, provider_type: &str) -> Result<Url, ParseError> {
    let mut url_builder = String::new();
    url_builder.push_str("https://");
    url_builder.push_str(hostname);
    url_builder.push_str("/v1/providers/");
    url_builder.push_str(namespace);
    url_builder.push_str("/");
    url_builder.push_str(provider_type);
    url_builder.push_str("/versions");

    Url::parse(&url_builder)
}

async fn list_provider_versions(
    db: &PgPool,
    hostname: &str,
    namespace: &str,
    provider_type: &str,
) -> Result<Option<MirrorIndex>, anyhow::Error> {
    let mut result = vec![];
    let mut row_count = 0;

    let query = sqlx::query!(
        r#"
        select "version" as "version?" from "terraform_provider_version"
        left join "terraform_provider" on 
            "terraform_provider_version"."provider_id" = "terraform_provider"."id"
            where "terraform_provider"."hostname" = $1
                and "terraform_provider"."namespace" = $2
                and "terraform_provider"."type" = $3;
        "#,
        hostname,
        namespace,
        provider_type,
    );

    let mut rows = query.fetch(db);

    while let Some(row) = rows.try_next().await? {
        row_count += 1;
        if let Some(version) = row.version {
            result.push(version);
        } else {
            return Ok(Some(result.into()));
        }
    }
    if row_count == 0 {
        return Ok(None);
    }
    Ok(Some(result.into()))
}

async fn store_provider_versions(
    db: &mut PgPool,
    hostname: &str,
    namespace: &str,
    provider_type: &str,
    response: &ProviderVersions,
) -> Result<u32, anyhow::Error> {
    let mut transaction = db.begin().await?;
    let mut versions = vec![];
    let mut oses = vec![];
    let mut arches = vec![];
    for version_item @ ProviderVersionItem { version, .. } in response.versions.iter() {
        for ProviderPlatform { os, arch } in version_item.platforms.iter() {
            versions.push(version.to_string());
            oses.push(os.to_string());
            arches.push(arch.to_string());
        }
    }

    // insert unknown providers if not existing in database
    let query = sqlx::query!(
        r#"
        insert into "terraform_provider" ("hostname", "namespace", "type", "expires_at")
        values ($1,$2,$3, now())
        on conflict do nothing
        "#,
        &hostname[..],
        &namespace[..],
        &provider_type[..]
    );
    let rows = query.execute(&mut transaction).await?;
    if rows.rows_affected() > 0 {
        tracing::debug!(
            %hostname,
            %namespace,
            %provider_type,
            "Saving new provider version to the database"
        );
    };

    let query = sqlx::query_as!(
        VersionTuple,
        r#"
        insert into "terraform_provider_version"
            ("version", "os", "arch", "provider_id", "upstream_package_url", "sha256sum" )
            select "t1".*, "t2"."id", null, null from
                (select * from unnest($1::text[], $2::text[], $3::text[])) as t1,
                (select "id" from "terraform_provider"
                    where "hostname" = $4
                        and "namespace" = $5
                        and "type" = $6) as t2
            on conflict do nothing
            returning 
            "version", "os", "arch";
        "#,
        &versions[..],
        &oses[..],
        &arches[..],
        &hostname,
        &namespace[..],
        &provider_type[..],
    );
    let records = query.fetch_all(&mut transaction).await?;
    tracing::debug!(?records, "Saving new provider versions to database");
    transaction.commit().await?;
    let count = records.len();
    tracing::info!(%count, "Saved provider versions to the database");
    Ok(count.try_into()?)
}

impl<'a> From<ProviderVersions> for MirrorIndex {
    fn from(provider_versions: ProviderVersions) -> MirrorIndex {
        let mut versions = HashMap::new();
        for version in provider_versions.versions.iter() {
            versions.insert(version.version.to_owned(), HashMap::new());
        }
        MirrorIndex { versions }
    }
}

impl From<Vec<String>> for MirrorIndex {
    fn from(versions: Vec<String>) -> MirrorIndex {
        let mut version_maps = HashMap::new();
        for version in versions.iter() {
            version_maps.insert(version.to_owned(), HashMap::new());
        }
        MirrorIndex {
            versions: version_maps,
        }
    }
}
