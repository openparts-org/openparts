//! The deployed application: the API router (`routes`) mounted at both
//! `/` (as before) and `/api` (what openparts-web calls when it is served
//! from the same origin), optionally the built web site as static files,
//! and cache headers suited to a CDN in front (Cloudflare Tunnel).

use crate::routes;
use crate::store::Store;
use axum::body::Body;
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
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

/// The `If-None-Match` header names `etag` (or is `*`).
fn matches_etag(if_none_match: Option<&HeaderValue>, etag: &str) -> bool {
    let Some(v) = if_none_match.and_then(|v| v.to_str().ok()) else {
        return false;
    };
    v.split(',').map(str::trim).any(|t| {
        let t = t.strip_prefix("W/").unwrap_or(t);
        t == "*" || t == etag
    })
}

/// How long a response may be cached, by what it is:
/// - content-addressed artifacts (`/v1/a/<sha256>/...`): a year,
///   immutable (set by the route; a stale digest is redirected);
/// - artifacts by part (`.../artifacts/<kind>`): five minutes, then
///   served stale while revalidated -- the URL names the part, not the
///   content; the `x-content-hash` is the ETag and an unchanged
///   artifact answers `304 Not Modified`;
/// - Vite's content-hashed assets (`/assets/`): a year, immutable;
/// - other API JSON: a minute;
/// - web pages: a minute;
/// - API errors: not stored.
async fn cache_headers(req: Request<Body>, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let if_none_match = req.headers().get(header::IF_NONE_MATCH).cloned();
    let mut res = next.run(req).await;
    let api = path.starts_with("/api/") || path.starts_with("/v1/");
    let status = res.status();
    if status.is_client_error() || status.is_server_error() {
        if api && !res.headers().contains_key(header::CACHE_CONTROL) {
            res.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        }
        return res;
    }
    if status != StatusCode::OK {
        return res;
    }
    if !res.headers().contains_key(header::CACHE_CONTROL) {
        let policy = if path.starts_with("/assets/") {
            routes::IMMUTABLE
        } else if api && path.contains("/artifacts/") {
            "public, max-age=300, stale-while-revalidate=86400"
        } else if api {
            "public, max-age=60, stale-while-revalidate=300"
        } else {
            "public, max-age=60"
        };
        res.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
    }
    let etag = res
        .headers()
        .get("x-content-hash")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| HeaderValue::from_str(&format!("\"{s}\"")).ok());
    if let Some(etag) = etag {
        if matches_etag(if_none_match.as_ref(), etag.to_str().unwrap_or_default()) {
            let mut not_modified = StatusCode::NOT_MODIFIED.into_response();
            let h = not_modified.headers_mut();
            h.insert(header::ETAG, etag);
            if let Some(cc) = res.headers().get(header::CACHE_CONTROL) {
                h.insert(header::CACHE_CONTROL, cc.clone());
            }
            return not_modified;
        }
        res.headers_mut().insert(header::ETAG, etag);
    }
    res
}
