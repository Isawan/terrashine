use axum::response::IntoResponse;
use http::{header::CONTENT_TYPE, HeaderValue};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::fmt::Debug;
use tokio_stream::StreamExt;

use axum::{
    extract::{Path, State},
    response::Response,
};
use hyper::{HeaderMap, StatusCode};

use crate::{app::AppState, error::TerrashineError, registry_client::RegistryClient};

#[derive(Serialize, Debug)]
pub struct MirrorIndex {
    // TODO: the nested hash value is always empty, we should implement
    // custom serialize to avoid unneeded work.
    versions: HashMap<String, HashMap<String, String>>,
}

impl IntoResponse for MirrorIndex {
    fn into_response(self) -> Response {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let response = match serde_json::to_string(&self) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(reason = ?e, "Could not serialize MirrorIndex");
                panic!();
            }
        };
        (headers, response).into_response()
    }
}

#[derive(Deserialize, Debug)]
pub struct ProviderVersions {
    versions: Vec<ProviderVersionItem>,
}

#[allow(dead_code)]
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

pub async fn index_handler(
    State(AppState {
        db_client: mut db,
        registry_client: registry,
        ..
    }): State<AppState>,
    Path((hostname, namespace, provider_type)): Path<(String, String, String)>,
) -> Result<MirrorIndex, StatusCode> {
    match list_provider_versions(&db, &hostname, &namespace, &provider_type).await {
        Ok(Some(mirror_index)) => {
            return Ok(mirror_index);
        }
        Ok(None) => {
            tracing::debug!("Unknown provider requested, fetching upstream");
        }
        Err(error) => {
            tracing::warn!(
                reason = ?error,
                "Error occured fetching provider from database, fetching upstream"
            );
        }
    }

    let provider_versions =
        refresh_versions(&db, registry, &hostname, &namespace, &provider_type).await?;

    let mirror_index = MirrorIndex::from(provider_versions);
    Result::Ok(mirror_index)
}

async fn refresh_versions(
    db: &PgPool,
    registry: RegistryClient,
    hostname: &str,
    namespace: &str,
    provider_type: &str,
) -> Result<ProviderVersions, TerrashineError> {
    let provider_versions: ProviderVersions = registry
        .provider_get(&hostname, format!("{namespace}/{provider_type}/versions"))
        .await?;

    let result = store_provider_versions(
        &db,
        &hostname,
        &namespace,
        &provider_type,
        &provider_versions,
    )
    .await?;

    Ok(provider_versions)
}

async fn list_provider_versions(
    db: &PgPool,
    hostname: &str,
    namespace: &str,
    provider_type: &str,
) -> Result<Option<MirrorIndex>, TerrashineError> {
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
    db: &PgPool,
    hostname: &str,
    namespace: &str,
    provider_type: &str,
    response: &ProviderVersions,
) -> Result<u64, TerrashineError> {
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
        insert into "terraform_provider" ("hostname", "namespace", "type", "last_refreshed")
        values ($1,$2,$3, now())
        on conflict ("hostname", "namespace", "type")
            do update set "last_refreshed" = "excluded"."last_refreshed"
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

    let query = sqlx::query!(
        r#"
        insert into "terraform_provider_version"
            ("version", "os", "arch", "provider_id", "artifact_id")
            select "t1"."hostname", "t1"."namespace", "t1"."type", "t2"."id", null from
                (select * from unnest($1::text[], $2::text[], $3::text[]))
                    as "t1"("hostname", "namespace", "type")
                    cross join
                (select "id" from "terraform_provider"
                    where "hostname" = $4
                        and "namespace" = $5
                        and "type" = $6 limit 1) as t2
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
    Ok(count
        .try_into()
        .expect("Could not cast rows count to 64 bit number"))
}

impl From<ProviderVersions> for MirrorIndex {
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
