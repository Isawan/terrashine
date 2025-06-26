use axum::{extract::Path, response::IntoResponse};
use mime_guess::from_path;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "resources/ui/static/"]
struct StaticFiles;

pub struct StaticFile<T>(pub T);

pub(crate) async fn handle_static_files(Path(path): Path<String>) -> impl IntoResponse {
    StaticFile(path)
}

impl<T> IntoResponse for StaticFile<T>
where
    T: Into<String>,
{
    fn into_response(self) -> axum::response::Response {
        let Self(path) = self;
        let path = path.into();
        match StaticFiles::get(&path) {
            Some(content) => {
                let body = content.data;
                let mime = from_path(&path).first_or_octet_stream();
                axum::response::Response::builder()
                    .header("Content-Type", mime.as_ref())
                    .body(body.into())
                    .unwrap()
            }
            None => axum::http::StatusCode::NOT_FOUND.into_response(),
        }
    }
}
