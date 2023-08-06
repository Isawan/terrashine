use std::marker::Send;

use async_trait::async_trait;
use sqlx::PgPool;

use super::CredentialHelper;

// Credential helper implementation by storing in the database
#[derive(Clone)]
pub struct DatabaseCredentials {
    pool: PgPool,
}

impl DatabaseCredentials {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CredentialHelper for DatabaseCredentials {
    async fn get(&self, hostname: impl AsRef<str> + Send) -> Result<Option<String>, anyhow::Error> {
        let query = sqlx::query!(
            r#"
            select auth_token from terraform_registry_host
            where hostname = $1;
        "#,
            hostname.as_ref()
        );
        let result = query.fetch_optional(&self.pool).await?;
        Ok(result.and_then(|v| v.auth_token))
    }

    async fn store(&mut self, hostname: String, cred: String) -> Result<(), anyhow::Error> {
        let query = sqlx::query!(
            r#"
            insert into "terraform_registry_host" ("hostname", "auth_token")
            values ($1, $2)
            on conflict ("hostname")
                do update set "auth_token" = "excluded"."auth_token";
        "#,
            hostname,
            cred,
        );
        let _ = query.execute(&self.pool).await?;
        tracing::info!(?hostname, "store new auth_token");
        Ok(())
    }

    async fn forget(&mut self, hostname: impl AsRef<str> + Send) -> Result<(), anyhow::Error> {
        let query = sqlx::query!(
            r#"
            delete from "terraform_registry_host" where "hostname" = $1;
        "#,
            hostname.as_ref()
        );
        let _ = query.execute(&self.pool).await?;
        Ok(())
    }
}
