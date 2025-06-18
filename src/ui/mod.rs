mod index;
mod provider;

use axum::{response::Html, routing::get, Router};

use crate::{app::AppState, credhelper::CredentialHelper, ui::index::IndexPage};

pub(crate) fn routes<S, C: Clone + Send + Sync + 'static + CredentialHelper>(
    state: AppState<C>,
) -> Router<S> {
    Router::new()
        .route("/ui/", get(|| async { Html(IndexPage {}.to_string()) }))
        .route("/ui/providers", get(provider::handle_provider_page::<C>))
        .with_state(state)
}
