use http::{header::CONTENT_TYPE, HeaderValue};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{collections::HashMap, hash::Hash, mem};
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
#[derive(sqlx::FromRow)]
struct VersionTuple {
    version: String,
    os: String,
    arch: String,
}

pub async fn index_handler(
    State(AppState {
        db_client: mut db,
        http_client: http,
        ..
    }): State<AppState>,
    Path((hostname, namespace, provider_type)): Path<(String, String, String)>,
) -> Result<(HeaderMap, String), StatusCode> {
    let upstream_url =
        build_url(&hostname, &namespace, &provider_type).map_err(|_| StatusCode::BAD_GATEWAY)?;

    let upstream_response = http
        .get(upstream_url)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let body = upstream_response
        .bytes()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let provider_versions: ProviderVersions =
        serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_GATEWAY)?;

    store_provider_versions(&mut db, &hostname, &provider_versions)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let mirror_index = MirrorIndex::from(provider_versions);
    let response_body = serde_json::to_string(&mirror_index).expect("Failed to serialize");
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

async fn store_provider_versions(
    db: &mut PgPool,
    hostname: &str,
    response: &ProviderVersions,
) -> Result<u32, anyhow::Error> {
    let mut transaction = db.begin().await?;
    let mut saved_versions = 0;
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
    let query = sqlx::query_as!(
        VersionTuple,
        r#"
        insert into "terraform_provider"
            ("version", "os", "arch", "registry_id", "upstream_package_url", "sha256sum" )
            select "t1".*, "t2"."id", null, null from
                (select * from unnest($1::text[], $2::text[], $3::text[])) as t1,
                (select "id" from "upstream_registry" where "hostname" = $4) as t2
            on conflict do nothing
            returning 
            "version", "os", "arch";
        "#,
        &versions[..],
        &oses[..],
        &arches[..],
        &hostname,
    );
    let records = query.fetch_all(&mut transaction).await?;
    for VersionTuple { version, os, arch } in records.iter() {
        saved_versions += 1;
        tracing::trace!( %hostname, %version, %os, %arch, "Saving new entry to database");
    }
    transaction.commit().await?;
    tracing::info!(entries = %saved_versions, "Saved entries to the database");
    Ok(saved_versions)
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
