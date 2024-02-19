use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use tracing::error;

use crate::config::MigrateArgs;

pub async fn run_migrate(config: MigrateArgs) -> Result<(), ()> {
    let db_result = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(config.database_url.clone())
        .await;

    let db = match db_result {
        Ok(pool) => pool,
        Err(error) => {
            error!(reason = %error, "Could not initialize pool, exiting.");
            return Err(());
        }
    };

    sqlx::migrate!("./migrations").run(&db).await.map_err(|e| {
        tracing::error!(?e, "Failed to run migrations");
    })?;

    Ok(())
}
