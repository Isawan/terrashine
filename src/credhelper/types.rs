use futures::Future;
use reqwest::RequestBuilder;
use std::marker::Send;

#[derive(PartialEq, Eq, Debug)]
pub enum Credential {
    NotFound,
    Entry(Option<String>),
}

pub trait CredentialHelper: Sync {
    fn get(
        &self,
        hostname: impl AsRef<str> + Send,
    ) -> impl Future<Output = Result<Credential, anyhow::Error>> + Send;
    fn store(
        &mut self,
        hostname: String,
        cred: String,
    ) -> impl Future<Output = Result<(), anyhow::Error>> + Send;

    /// Forget a credential.
    /// NOTE: Deleting a non-existing credential is not an error
    fn forget(
        &mut self,
        hostname: impl AsRef<str> + Send,
    ) -> impl Future<Output = Result<(), anyhow::Error>> + Send;

    fn transform(
        &self,
        request: RequestBuilder,
        hostname: impl AsRef<str> + Send,
    ) -> impl std::future::Future<Output = Result<RequestBuilder, anyhow::Error>> + Send {
        async {
            match self.get(hostname).await? {
                Credential::NotFound => Ok(request),
                Credential::Entry(None) => Ok(request),
                Credential::Entry(Some(token)) => Ok(request.bearer_auth(token)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::memory::MemoryCredentials;
    use super::*;

    #[tokio::test]
    async fn test_request_transform_known_credential() {
        let mut creds = MemoryCredentials::default();
        creds
            .store("localhost".into(), "password1".into())
            .await
            .expect("Error occurred");
        let client = reqwest::Client::new();
        let request = client.get("http://localhost");
        let request = creds
            .transform(request, "localhost")
            .await
            .expect("Unexpected error")
            .build()
            .expect("Remove");
        let auth_header = request
            .headers()
            .get("authorization")
            .expect("Header not found");

        assert_eq!(
            auth_header, "Bearer password1",
            "Authorization header not set"
        );
    }

    #[tokio::test]
    async fn test_request_transform_unknown_credential() {
        let mut creds = MemoryCredentials::default();
        creds
            .store("localhost".into(), "password1".into())
            .await
            .expect("Error occurred");
        let client = reqwest::Client::new();
        let request = client.get("http://test.test");
        let request = creds
            .transform(request, "test.test")
            .await
            .expect("Unexpected error")
            .build()
            .expect("Remove");
        let auth_header = request.headers().get("authorization");

        assert_eq!(auth_header, None, "Authorization header set");
    }
}
