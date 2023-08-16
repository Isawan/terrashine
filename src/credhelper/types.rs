use async_trait::async_trait;
use reqwest::RequestBuilder;
use std::marker::Send;

#[derive(PartialEq, Eq, Debug)]
pub enum Credential {
    NotFound,
    Entry(Option<String>),
}

#[async_trait]
pub trait CredentialHelper: Sync {
    async fn get(&self, hostname: impl AsRef<str> + Send) -> Result<Credential, anyhow::Error>;
    async fn store(&mut self, hostname: String, cred: String) -> Result<(), anyhow::Error>;

    /// Forget a credential.
    /// NOTE: Deleting a non-existing credential is not an error
    async fn forget(&mut self, hostname: impl AsRef<str> + Send) -> Result<(), anyhow::Error>;

    async fn transform(
        &self,
        request: RequestBuilder,
        hostname: impl AsRef<str> + Send,
    ) -> Result<RequestBuilder, anyhow::Error> {
        match self.get(hostname).await? {
            Credential::NotFound => Ok(request),
            Credential::Entry(None) => Ok(request),
            Credential::Entry(Some(token)) => Ok(request.bearer_auth(token)),
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
