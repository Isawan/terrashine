use axum::{routing::post, Router};

use crate::credhelper::CredentialHelper;

use self::v1::credential::{delete, exists, update};

pub(crate) mod v1;

#[derive(Clone)]
pub(crate) struct APIState<C> {
    pub(crate) credentials: C,
}

pub(crate) fn routes<S, C: Clone + Send + Sync + 'static + CredentialHelper>(
    state: APIState<C>,
) -> Router<S> {
    Router::new()
        .route(
            "/api/v1/credentials/{hostname}",
            post(update).delete(delete).get(exists),
        )
        .with_state(state)
}
