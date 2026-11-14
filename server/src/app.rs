//! The deployed application: the API router (`routes`) mounted at both
//! `/` (as before) and `/api` (what openparts-web calls when it is served
//! from the same origin), optionally the built web site as static files,
//! and cache headers suited to a CDN in front (Cloudflare Tunnel).

use crate::routes;
use crate::store::Store;
use axum::body::Body;
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::Router;
use std::path::Path;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

/// The API at `/` and `/api`, plus `web_dir` (the web site's `vite
/// build` output) as the fallback for every other path when given. The
/// site is a single-page app: a path with no file behind it (`/parts`,
/// `/en/search`) gets its `index.html` and routes on the client.
pub fn build_app(store: Arc<Store>, web_dir: Option<&Path>) -> Router {
    let api = routes::build_router(store);
    let mut app = Router::new().nest("/api", api.clone()).merge(api);
    if let Some(dir) = web_dir {
        let index = dir.join("index.html");
        app = app.fallback_service(
            ServeDir::new(dir)
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new(index)),
        );
    }
    app.layer(middleware::from_fn(cache_headers))
}

/// How long a response may be cached, by what it is:
/// - artifacts (`.../artifacts/<kind>`): a day; their `x-content-hash`
///   becomes the ETag, so a changed dataset revalidates cleanly;
/// - Vite's content-hashed assets (`/assets/`): a year, immutable;
/// - other API JSON: five minutes;
/// - web pages: one minute.
async fn cache_headers(req: Request<Body>, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let mut res = next.run(req).await;
    if res.status() != StatusCode::OK || res.headers().contains_key(header::CACHE_CONTROL) {
        return res;
    }
    let api = path.starts_with("/api/") || path.starts_with("/v1/");
    let policy = if path.starts_with("/assets/") {
        "public, max-age=31536000, immutable"
    } else if api && path.contains("/artifacts/") {
        if let Some(hash) = res.headers().get("x-content-hash").cloned() {
            if let Ok(s) = hash.to_str() {
                if let Ok(etag) = HeaderValue::from_str(&format!("\"{s}\"")) {
                    res.headers_mut().insert(header::ETAG, etag);
                }
            }
        }
        "public, max-age=86400"
    } else if api {
        "public, max-age=300"
    } else {
        "public, max-age=60"
    };
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
    res
}
