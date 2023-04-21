use crate::{
    app::AppState,
    error::TerrashineError,
    refresh::{RefreshRequest, RefreshResponse, TerraformProvider},
    registry::{ProviderPlatform, ProviderVersionItem, ProviderVersions, RegistryClient},
};
use axum::response::IntoResponse;
use axum::{
    extract::{Path, State},
    response::Response,
};
use http::{
    header::{CACHE_CONTROL, CONTENT_TYPE},
    HeaderValue,
};
use hyper::HeaderMap;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use std::{fmt::Debug, time::Duration};
use tokio::sync::{mpsc::error::SendTimeoutError, oneshot};
use tracing::Span;

#[derive(Serialize, Debug)]
pub(crate) struct MirrorIndex {
    // TODO: the nested hash value is always empty, we should implement
    // custom serialize to avoid unneeded work.
    versions: HashMap<String, HashMap<String, String>>,
}

impl IntoResponse for MirrorIndex {
    fn into_response(self) -> Response {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        // This is safe to cache as it is the first endpoint hit by the client
        // as part of the terraform mirror provider protocol.
        // This plus the fact that the providers are never deleted,
        // ensures that the client behaves correctly if the cached entry is stale
        // as subsequent requests to endpoint will exist.
        headers.insert(
            CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=60"),
        );
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

pub(crate) async fn index_handler(
    State(AppState {
        db_client: db,
        refresher_tx: tx,
        ..
    }): State<AppState>,
    Path((hostname, namespace, provider_type)): Path<(String, String, String)>,
) -> Result<MirrorIndex, TerrashineError> {
    match list_provider_versions(&db, &hostname, &namespace, &provider_type).await {
        Ok(Some(mirror_index)) => {
            let provider = TerraformProvider {
                hostname,
                namespace,
                provider_type,
            };
            let result = tx.try_send(RefreshRequest {
                provider,
                response_channel: None,
                span: Span::current(),
            });
            // We don't care if it errors in this path, log and continue on.
            if let Err(e) = result {
                tracing::trace!(reason=?e, "Failed to send provider refresh request");
            }
            return Ok(mirror_index);
        }
        Ok(None) => {
            tracing::debug!("Unknown provider requested, fetching upstream");
        }
        Err(error) => {
            tracing::warn!(
                reason = %error,
                "Error occurred fetching provider from database, fetching upstream"
            );
        }
    }

    // If we didn't see anything in the database, now we'll request it from upstream
    // We do this by sending a message to the refresher channel and then wait for a
    // message back via the oneshot channel to confirm provider has been refreshed.
    // This path only occurs when a brand new provider index is first encountered.
    // The refresher handles updating known terraform provider versions.
    let (resp_tx, resp_rx) = oneshot::channel();

    let provider = TerraformProvider {
        hostname,
        namespace,
        provider_type,
    };

    tracing::debug!("Sending request to refresher task");
    tx.send_timeout(
        RefreshRequest {
            provider: provider.clone(),
            response_channel: Some(resp_tx),
            span: Span::current(),
        },
        Duration::from_secs(1),
    )
    .await
    .map_err(|e| {
        match e {
            SendTimeoutError::Timeout(_) => {
                tracing::error!("Sending to the refresher channel has timed out. Server may be overloaded");
                TerrashineError::TooManyRequestsInChannel { channel_name: "refresher" }
            }
            SendTimeoutError::Closed(_) => {
                tracing::error!("The provider refresher has dropped the channel for unknown reasons. This is really bad and the server may need restarting. Terrashine may now be read-only");
                TerrashineError::BrokenRefresherChannel
            }
        }
    }
    )?;

    match resp_rx.await {
        Ok(RefreshResponse::RefreshPerformed(Ok(versions))) => Ok(MirrorIndex::from(versions)),
        Ok(RefreshResponse::RefreshPerformed(Err(err))) => {
            tracing::error!(reason=%err, "Occurred occurred while adding new provider from upstream");
            Err(err)
        }
        Ok(RefreshResponse::ProviderVersionNotStale) => {
            let err = TerrashineError::ConcurrentProviderFetch { provider };
            tracing::error!(reason=%err, "Concurrent upstream fetch occurred while fetching provider from upstream");
            Err(err)
        }
        // This condition probably warrants a server restart.
        // Let the operator know, but it can continue operating as read-only so we don't
        // kill everything. Something has gone _very_ wrong in any case.
        Err(_) => {
            tracing::error!("The provider refresher has dropped the channel for unknown reasons. This is really bad and the server may need restarting. Terrashine may now be read-only");
            Err(TerrashineError::BrokenRefresherChannel)
        }
    }
}

pub(crate) async fn refresh_versions(
    db: &PgPool,
    registry: &RegistryClient,
    hostname: &str,
    namespace: &str,
    provider_type: &str,
) -> Result<ProviderVersions, TerrashineError> {
    let provider_versions = registry
        .provider_get(hostname, &format!("{namespace}/{provider_type}/versions"))
        .await?;

    store_provider_versions(db, hostname, namespace, provider_type, &provider_versions).await?;

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

    let rows = query.fetch_all(db).await?;

    for row in rows.into_iter() {
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
        insert into "terraform_provider"
            ("hostname", "namespace", "type", "last_refreshed")
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

    // Insert all the provider versions that are not already known
    // The query is a little hairy, but its efficiently passing the
    // version tuples as an array, turning them into rows and joining it
    // on the hostname, namespace and type with the known providers to get
    // the provider id.
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
        .expect("Could not cast row count to 64 bit number"))
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
