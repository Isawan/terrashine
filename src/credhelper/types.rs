use async_trait::async_trait;
use reqwest::RequestBuilder;
use std::marker::Send;

#[derive(PartialEq, Eq)]
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
    use super::super::types::Credential;
    use super::*;
    use async_trait::async_trait;
    use std::{collections::HashMap, marker::Send};

    // Credential helper implementation by storing in the database
    #[derive(Clone)]
    pub struct MemoryCredentials {
        map: HashMap<String, Option<String>>,
    }

    impl MemoryCredentials {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self {
                map: HashMap::new(),
            }
        }
    }

    #[async_trait]
    impl CredentialHelper for MemoryCredentials {
        async fn get(&self, hostname: impl AsRef<str> + Send) -> Result<Credential, anyhow::Error> {
            Ok(self
                .map
                .get(hostname.as_ref())
                .map_or(Credential::NotFound, |v| Credential::Entry(v.clone())))
        }

        async fn store(&mut self, hostname: String, cred: String) -> Result<(), anyhow::Error> {
            self.map.insert(hostname, Some(cred));
            Ok(())
        }

        async fn forget(&mut self, hostname: impl AsRef<str> + Send) -> Result<(), anyhow::Error> {
            self.map.remove(hostname.as_ref());
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_request_transform_known_credential() {
        let mut creds = MemoryCredentials::new();
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
        let mut creds = MemoryCredentials::new();
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
