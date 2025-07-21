use tracing::error;

use crate::config::IsHealthyArgs;
use std::convert::Into;

pub async fn run_healthy(args: IsHealthyArgs) -> Result<(), ()> {
    let result = reqwest::get(format!("http://{}/healthcheck", args.http_listen))
        .await
        .map_err(Into::<anyhow::Error>::into)
        .and_then(|x| x.error_for_status().map_err(Into::into));
    match result {
        Ok(_) => Ok(()),
        Err(err) => {
            error!("{}", err);
            Err(())
        }
    }
}
