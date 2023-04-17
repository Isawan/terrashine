use crate::{
    app::AppState,
    registry::{ProviderResponse, RegistryClient},
};
use anyhow::Context;
use aws_sdk_s3::{
    presigning::PresigningConfig,
    primitives::ByteStream,
    types::{CompletedMultipartUpload, CompletedPart},
};
use axum::{
    body::Bytes,
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use futures::{future::TryFutureExt, StreamExt};
use http::{HeaderValue, StatusCode, Uri};
use reqwest::Client;
use sqlx::{query_as, PgPool};
use std::{pin::Pin, time::Duration};
use tokio::try_join;
use tokio_stream::Stream;

const PREALLOCATED_BUFFER_BYTES: usize = 12_582_912;
const S3_MINIMUM_UPLOAD_CHUNK_BYTES: usize = 10_485_760;

struct ArtifactResponse {
    uri: HeaderValue,
}

impl ArtifactResponse {
    fn new(uri: Uri) -> Self {
        ArtifactResponse {
            uri: HeaderValue::try_from(uri.to_string()).expect("URL not a valid header"),
        }
    }
}

impl IntoResponse for ArtifactResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::TEMPORARY_REDIRECT,
            [
                (http::header::LOCATION, self.uri),
                (
                    http::header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=60"),
                ),
            ],
        )
            .into_response()
    }
}

pub(crate) async fn artifacts_handler(
    State(AppState {
        http_client: http,
        registry_client: registry,
        db_client: db,
        s3_client: s3,
        args,
        ..
    }): State<AppState>,
    Path(version_id): Path<i64>,
) -> Result<impl IntoResponse, StatusCode> {
    tracing::debug!("Get artifact details from database");
    let artifact_detail = match get_artifact_from_database(&db, version_id).await {
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
    let artifact = match artifact_detail.artifact_id {
        Some(id) => {
            tracing::debug!("Artifact already downloaded");
            Artifact {
                version_id: artifact_detail.version_id,
                hostname: artifact_detail.hostname,
                namespace: artifact_detail.namespace,
                provider_type: artifact_detail.provider_type,
                version: artifact_detail.version,
                os: artifact_detail.os,
                arch: artifact_detail.arch,
                artifact_id: id,
            }
        }
        None => {
            // Make upstream request and stash if artifact not stored.
            tracing::debug!("Fetching artifact from upstream");
            let upstream_response = get_upstream(http, registry, &artifact_detail).map_err(|e| {
                tracing::error!(reason = ?e, "Error occured fetching artifact upstream");
                StatusCode::BAD_GATEWAY
            });
            let response_id = allocate_artifact_id(&db).map_err(|e| {
                tracing::error!(reason = ?e, "Error occured allocating artifact id from database");
                StatusCode::INTERNAL_SERVER_ERROR
            });
            let (id, body) = try_join!(response_id, upstream_response)?;
            let artifact = Artifact {
                version_id: artifact_detail.version_id,
                hostname: artifact_detail.hostname,
                namespace: artifact_detail.namespace,
                provider_type: artifact_detail.provider_type,
                version: artifact_detail.version,
                os: artifact_detail.os,
                arch: artifact_detail.arch,
                artifact_id: id,
            };
            stash_artifact(&db, &s3, &args.s3_bucket_name, &artifact, body)
                .await
                .map_err(|e| {
                    tracing::error!(reason = ?e, "Error occured stashing artifact");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            artifact
        }
    };
    let req = presign_request(&s3, &args.s3_bucket_name, &artifact)
        .await
        .map_err(|e| {
            tracing::error!(reason = ?e, "Error presigning url");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let response = ArtifactResponse::new(req);
    Ok(response)
}

#[derive(sqlx::FromRow)]
struct ArtifactDetails {
    version_id: i64,
    hostname: String,
    namespace: String,
    #[sqlx(rename = "type")]
    provider_type: String,
    version: String,
    os: String,
    arch: String,
    artifact_id: Option<i64>,
}

#[allow(dead_code)]
struct Artifact {
    version_id: i64,
    hostname: String,
    namespace: String,
    provider_type: String,
    version: String,
    os: String,
    arch: String,
    artifact_id: i64,
}

impl Artifact {
    fn to_s3_key(&self) -> String {
        let mut key = String::from("artifacts/");
        key.push_str(&self.artifact_id.to_string());
        key
    }
}

async fn get_artifact_from_database(
    db: &PgPool,
    version_id: i64,
) -> Result<Option<ArtifactDetails>, anyhow::Error> {
    let result = query_as!(
        ArtifactDetails,
        r#"
        select
            "terraform_provider_version"."id" as "version_id",
            "hostname",
            "namespace",
            "type" as "provider_type",
            "version",
            "os",
            "arch",
            "artifact_id"
        from "terraform_provider_version"
        inner join "terraform_provider"
            on "terraform_provider_version"."provider_id" = "terraform_provider"."id"
            where "terraform_provider_version"."id" = $1;
        "#,
        version_id
    )
    .fetch_optional(db)
    .await?;
    Ok(result)
}

async fn get_upstream(
    http: Client,
    registry: RegistryClient,
    artifact: &ArtifactDetails,
) -> Result<Pin<Box<impl Stream<Item = reqwest::Result<Bytes>>>>, anyhow::Error> {
    let provider_path = format!(
        "{}/{}/{}/download/{}/{}",
        artifact.namespace, artifact.provider_type, artifact.version, artifact.os, artifact.arch
    );
    let provider: ProviderResponse = registry
        .provider_get(&artifact.hostname, &provider_path)
        .await?;
    let stream = http
        .get(provider.download_url)
        .send()
        .await?
        .error_for_status()?
        .bytes_stream();
    Ok(Box::pin(stream))
}

async fn allocate_artifact_id(db: &PgPool) -> Result<i64, anyhow::Error> {
    sqlx::query!(
        r#"
            select nextval('artifact_ids') as "id!";
        "#
    )
    .fetch_one(db)
    .await
    .map(|x| x.id)
    .context("Failure allocating next id")
}

async fn stash_artifact(
    db: &PgPool,
    s3: &aws_sdk_s3::Client,
    bucket_name: &str,
    artifact: &Artifact,
    mut stream: Pin<Box<impl Stream<Item = reqwest::Result<Bytes>>>>,
) -> Result<(), anyhow::Error> {
    let key = artifact.to_s3_key();
    let req = s3.create_multipart_upload().bucket(bucket_name).key(&key);
    let multipart_upload = req.send().await?;
    let upload_id = multipart_upload
        .upload_id()
        .context("No upload id returned from endpoint")?;
    let mut upload_buffer = Vec::with_capacity(PREALLOCATED_BUFFER_BYTES);
    let mut upload_parts = Vec::new();
    let mut part_number = 0;

    loop {
        match stream.next().await {
            Some(Ok(chunk)) => {
                upload_buffer.extend_from_slice(&chunk.slice(..));
                if upload_buffer.len() < S3_MINIMUM_UPLOAD_CHUNK_BYTES {
                    continue;
                }
                // Once enough of the buffer has filled up, make the upload
                let upload_part = s3
                    .upload_part()
                    .key(&key)
                    .bucket(bucket_name)
                    .upload_id(upload_id)
                    .body(ByteStream::from(Bytes::from(upload_buffer)))
                    .part_number(part_number)
                    .send()
                    .await?;
                upload_parts.push(
                    CompletedPart::builder()
                        .e_tag(upload_part.e_tag().context("No etag found on response")?)
                        .part_number(part_number)
                        .build(),
                );

                // prepare for next round
                part_number += 1;
                // We have to allocate a new vec here because upload_part() builder takes
                // ownership of the old vector to create a ByteStream.
                upload_buffer = Vec::with_capacity(PREALLOCATED_BUFFER_BYTES);
            }
            Some(Err(e)) => {
                // Cleanup aborted upload before returning error
                let abort_response = s3
                    .abort_multipart_upload()
                    .key(&key)
                    .bucket(bucket_name)
                    .upload_id(upload_id)
                    .send()
                    .await;

                return match abort_response {
                    Ok(_) => Err(e).context("Upstream aborted while streaming"),
                    Err(response_err) => Err(response_err).context(e),
                };
            }
            None => {
                break;
            }
        }
    }
    // Upload anything remaining in the buffer before stream completion
    if !upload_buffer.is_empty() {
        let upload_part = s3
            .upload_part()
            .key(&key)
            .bucket(bucket_name)
            .upload_id(upload_id)
            .body(upload_buffer.into())
            .part_number(part_number)
            .send()
            .await?;
        upload_parts.push(
            CompletedPart::builder()
                .e_tag(upload_part.e_tag().context("No etag found on response")?)
                .part_number(part_number)
                .build(),
        );
    }
    // Finalize upload
    let completed_upload_request = CompletedMultipartUpload::builder()
        .set_parts(Some(upload_parts))
        .build();
    s3.complete_multipart_upload()
        .bucket(bucket_name)
        .key(&key)
        .multipart_upload(completed_upload_request)
        .upload_id(upload_id)
        .send()
        .await?;

    store_artifact_in_database(db, artifact).await?;

    Ok(())
}

async fn store_artifact_in_database(db: &PgPool, artifact: &Artifact) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
            update "terraform_provider_version"
            set "artifact_id" = $1,
                "artifact_timestamp" = now()
                where "id" = $2;
        "#,
        artifact.artifact_id,
        artifact.version_id,
    )
    .execute(db)
    .await
    .with_context(|| format!("Writing artifact id({}) to database", artifact.artifact_id))?;
    Ok(())
}

async fn presign_request(
    s3: &aws_sdk_s3::Client,
    bucket_name: &str,
    artifact: &Artifact,
) -> Result<Uri, anyhow::Error> {
    let expires_in = Duration::from_secs(120);
    let presigned_request = s3
        .get_object()
        .bucket(bucket_name)
        .key(artifact.to_s3_key())
        .presigned(PresigningConfig::expires_in(expires_in)?)
        .await?;
    Ok(presigned_request.uri().clone())
}
