use axum::{
    extract::{Path, State},
    Json,
};
use http::StatusCode;
use serde_json::Value;

use crate::{credhelper::CredentialHelper, http::api::APIState};

#[derive(Debug, serde::Deserialize)]
pub(crate) struct UpdateRequest {
    pub(crate) data: UpdateData,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct UpdateData {
    pub(crate) token: String,
}

pub(crate) async fn update<C: CredentialHelper>(
    State(APIState {
        mut credentials, ..
    }): State<APIState<C>>,
    Path(hostname): Path<String>,
    Json(UpdateRequest {
        data: UpdateData { token: value },
    }): Json<UpdateRequest>,
) -> (StatusCode, Json<Value>) {
    match credentials.store(hostname, value).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "data": {} }))),
        Err(e) => {
            tracing::error!(reason=?e, "Error occurred updating credential");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    serde_json::json!({ "error": { "msg": "Error occurred updating credential" } }),
                ),
            )
        }
    }
}

pub(crate) async fn delete<C: CredentialHelper>(
    State(APIState {
        mut credentials, ..
    }): State<APIState<C>>,
    Path(hostname): Path<String>,
) -> (StatusCode, Json<Value>) {
    match credentials.forget(&hostname).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "data": {} }))),
        Err(e) => {
            tracing::error!(reason=?e, "Error occurred deleting credential");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    serde_json::json!({ "error": { "msg": "Error occurred deleting credential" } }),
                ),
            )
        }
    }
}

pub(crate) async fn exists<C: CredentialHelper>(
    State(APIState { credentials, .. }): State<APIState<C>>,
    Path(hostname): Path<String>,
) -> (StatusCode, Json<Value>) {
    match credentials.get(&hostname).await {
        Ok(cred) => match cred {
            crate::credhelper::Credential::Entry(_) => (
                StatusCode::OK,
                Json(serde_json::json!({ "data": { "exists": true } })),
            ),
            crate::credhelper::Credential::NotFound => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "data": { "exists": false } })),
            ),
        },
        Err(e) => {
            tracing::error!(reason=?e, "Error occurred checking credential");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    serde_json::json!({ "error": { "msg": "Error occurred checking credential" } }),
                ),
            )
        }
    }
}

// Testing the update and delete of the credentials
#[cfg(test)]
mod tests {
    use tower::ServiceExt;
    use tracing_test::traced_test;

    use super::*;
    use crate::{
        credhelper::{faulty::FaultyCredentials, memory::MemoryCredentials, Credential},
        http::api::routes,
    };

    #[traced_test]
    #[tokio::test]
    async fn test_update_credential_for_hostname() {
        let credentials = MemoryCredentials::default();
        let state = APIState {
            credentials: credentials.clone(),
        };
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/credentials/example.com")
            .header("content-type", "application/json")
            .body(
                r#"{
                    "data": {
                        "token": "password1"
                    }
                }"#
                .into(),
            )
            .unwrap();
        let response = routes(state).oneshot(request).await.unwrap();
        assert!(response.status().is_success());

        assert_eq!(
            credentials.get("example.com").await.unwrap(),
            Credential::Entry(Some("password1".into()))
        );
    }

    #[tokio::test]
    async fn test_delete_credential_for_hostname() {
        let mut credentials = MemoryCredentials::default();
        credentials
            .store("example.com".into(), "password1".into())
            .await
            .unwrap();

        let state = APIState {
            credentials: credentials.clone(),
        };
        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri("/api/v1/credentials/example.com")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = routes(state).oneshot(request).await.unwrap();
        assert!(response.status().is_success());

        assert_eq!(
            credentials.get("example.com").await.unwrap(),
            Credential::NotFound
        );
    }

    #[tokio::test]
    async fn test_existing_credential_response() {
        let mut credentials = MemoryCredentials::default();
        credentials
            .store("example.com".into(), "password1".into())
            .await
            .unwrap();

        let state = APIState {
            credentials: credentials.clone(),
        };
        let request = axum::http::Request::builder()
            .method("GET")
            .uri("/api/v1/credentials/example.com")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = routes(state).oneshot(request).await.unwrap();
        assert!(response.status().is_success());
    }

    #[tokio::test]
    async fn test_not_existing_credential_response() {
        let credentials = MemoryCredentials::default();
        let state = APIState {
            credentials: credentials.clone(),
        };
        let request = axum::http::Request::builder()
            .method("GET")
            .uri("/api/v1/credentials/example.com")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = routes(state).oneshot(request).await.unwrap();
        assert!(response.status().is_client_error());
    }

    /// Test error handling of updates when error occurs
    #[tokio::test]
    async fn test_error_occur_on_insert() {
        let credentials = FaultyCredentials::new();
        let state = APIState {
            credentials: credentials,
        };
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/credentials/example.com")
            .header("content-type", "application/json")
            .body(
                r#"{
                    "data": {
                        "token": "password1"
                    }
                }"#
                .into(),
            )
            .unwrap();
        let response = routes(state).oneshot(request).await.unwrap();
        assert!(response.status() == StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_error_occur_on_delete() {
        let credentials = FaultyCredentials::new();
        let state = APIState {
            credentials: credentials,
        };
        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri("/api/v1/credentials/example.com")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = routes(state).oneshot(request).await.unwrap();
        assert!(response.status() == StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_error_occur_on_get() {
        let credentials = FaultyCredentials::new();
        let state = APIState {
            credentials: credentials,
        };
        let request = axum::http::Request::builder()
            .method("GET")
            .uri("/api/v1/credentials/example.com")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = routes(state).oneshot(request).await.unwrap();
        assert!(response.status() == StatusCode::INTERNAL_SERVER_ERROR);
    }
}
