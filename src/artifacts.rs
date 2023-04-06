use std::pin::Pin;

use anyhow::Context;
use aws_sdk_s3::{
    primitives::ByteStream,
    types::{CompletedMultipartUpload, CompletedPart},
};
use axum::{
    body::Bytes,
    extract::{Path, State},
};
use futures::future::TryFutureExt;

use http::StatusCode;
use reqwest::{Client, Url};
use sqlx::{query_as, PgPool};
use tokio::try_join;
use tokio_stream::{Stream, StreamExt};
use tower_http::classify::StatusInRangeAsFailures;
use url::ParseError;

use crate::app::AppState;

const PREALLOCATED_BUFFER_BYTES: usize = 12_582_912;
const S3_MINIMUM_UPLOAD_CHUNK_BYTES: usize = 12_582_912;

pub async fn artifacts_handler<'a>(
    State(AppState {
        http_client: http,
        db_client: db,
        s3_client: s3,
        ..
    }): State<AppState>,
    Path((version_id)): Path<(i64)>,
) -> Result<Bytes, StatusCode> {
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
        Some(id) => Artifact {
            hostname: artifact_detail.hostname,
            namespace: artifact_detail.namespace,
            provider_type: artifact_detail.provider_type,
            version: artifact_detail.version,
            os: artifact_detail.os,
            arch: artifact_detail.arch,
            artifact_id: id,
        },
        None => {
            // Make upstream request and stash if artifact not stored.
            let upstream_response = get_upstream(http, &artifact_detail).map_err(|e| {
                tracing::error!(reason = ?e, "Error occured fetching artifact upstream");
                StatusCode::BAD_GATEWAY
            });
            let response_id = allocate_artifact_id(&db).map_err(|e| {
                tracing::error!(reason = ?e, "Error occured allocating artifact id from database");
                StatusCode::INTERNAL_SERVER_ERROR
            });
            let (id, body) = try_join!(response_id, upstream_response)?;
            let artifact = Artifact {
                hostname: artifact_detail.hostname,
                namespace: artifact_detail.namespace,
                provider_type: artifact_detail.provider_type,
                version: artifact_detail.version,
                os: artifact_detail.os,
                arch: artifact_detail.arch,
                artifact_id: id,
            };
            stash_artifact(s3, &artifact, body).await.map_err(|e| {
                tracing::error!(reason = ?e, "Error occured stashing artifact");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            artifact
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
    artifact_id: Option<i64>,
}
struct Artifact {
    hostname: String,
    namespace: String,
    provider_type: String,
    version: String,
    os: String,
    arch: String,
    artifact_id: i64,
}

async fn get_artifact_from_database(
    db: &PgPool,
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
    return Ok(result);
}

async fn get_upstream(
    http: Client,
    artifact: &ArtifactDetails,
) -> Result<Pin<Box<impl Stream<Item = reqwest::Result<Bytes>>>>, anyhow::Error> {
    let url = build_url(artifact)?;
    let response = http.get(url).send().await?;
    let success = response.error_for_status()?;
    let stream = success.bytes_stream();
    return Ok(Box::pin(stream));
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
    s3: aws_sdk_s3::Client,
    artifact: &Artifact,
    mut stream: Pin<Box<impl Stream<Item = reqwest::Result<Bytes>>>>,
) -> Result<(), anyhow::Error> {
    let mut key = String::from("artifacts/");
    key.push_str(&artifact.artifact_id.to_string());

    let multipart_upload = s3
        .create_multipart_upload()
        .bucket("terrashine")
        .key(&key)
        .send()
        .await?;

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
                let upload_part = s3
                    .upload_part()
                    .key(&key)
                    .bucket("terrashine")
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
                    .bucket("terrashine")
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
    // Upload anything remaining in the buffer on stream completion
    if upload_buffer.len() > 0 {
        let upload_part = s3
            .upload_part()
            .key(&key)
            .bucket("terrashine")
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
    let completed_upload_response = s3
        .complete_multipart_upload()
        .bucket("terrashine")
        .key(&key)
        .multipart_upload(completed_upload_request)
        .upload_id(upload_id)
        .send()
        .await?;

    todo!();
}

fn build_url(artifact: &ArtifactDetails) -> Result<Url, ParseError> {
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
