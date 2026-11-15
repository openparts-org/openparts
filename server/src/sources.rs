//! Source documents for the site's datasheet viewer.
//!
//! - `GET /v1/sources/{id}`: the Source document (YAML fields as JSON).
//! - `GET /v1/sources/{id}/document`: the PDF the Source's `url` points
//!   at, fetched by this server and cached on disk
//!   (`OPENPARTS_SOURCE_CACHE`), so the browser can render it (PDF.js)
//!   despite the publisher's site not allowing cross-origin reads.
//!
//! It is not a proxy: the only URLs it ever fetches are the `url`s of
//! the loaded `sources/*.yaml`, looked up by Source id -- a request
//! can't name a URL. Downloads are plain GETs with an honest user
//! agent, a timeout and a size limit, and must be a PDF (by content
//! type and by the `%PDF-` signature); a recorded `sha256` must match.
//! A publisher that refuses (403, a bot check page) gets a clear error
//! and the site opens the original link instead.
//!
//! Off unless `OPENPARTS_SOURCE_CACHE` is set, and it must stay off on
//! a public deployment: serving the PDFs redistributes manufacturer
//! documents (Architecture Specification section 20, Canonical Data
//! Specification section 23). It is for a private or local server.
//! Without it the site links each value to the original PDF at the
//! cited page. The cache is capped (`OPENPARTS_SOURCE_CACHE_MAX_MB`,
//! default 500): the least recently used documents go first.

use crate::store::Store;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// The user agent of document downloads.
pub const USER_AGENT: &str = concat!(
    "openparts-server/",
    env!("CARGO_PKG_VERSION"),
    " (datasheet cache; +https://github.com/openparts-org/openparts)"
);

pub fn routes() -> Router<Arc<Store>> {
    Router::new().route("/v1/sources/*rest", get(source_route))
}

/// Where downloaded documents are kept, and the limits.
#[derive(Debug, Clone)]
pub struct SourceCache {
    pub dir: PathBuf,
    /// Largest single document.
    pub max_bytes: u64,
    /// Largest total size of the cached documents.
    pub max_total_bytes: u64,
    pub timeout: Duration,
}

impl SourceCache {
    /// From `OPENPARTS_SOURCE_CACHE` (the directory; unset: off),
    /// `OPENPARTS_SOURCE_MAX_MB` (one document, default 50),
    /// `OPENPARTS_SOURCE_CACHE_MAX_MB` (the whole cache, default 500)
    /// and `OPENPARTS_SOURCE_TIMEOUT_SECS` (default 30).
    pub fn from_env() -> std::io::Result<Option<SourceCache>> {
        let Some(dir) = std::env::var_os("OPENPARTS_SOURCE_CACHE") else {
            return Ok(None);
        };
        let num = |k: &str, d: u64| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(d)
        };
        let mut cache = SourceCache::new(
            PathBuf::from(dir),
            num("OPENPARTS_SOURCE_MAX_MB", 50) * 1024 * 1024,
            Duration::from_secs(num("OPENPARTS_SOURCE_TIMEOUT_SECS", 30)),
        )?;
        cache.max_total_bytes = num("OPENPARTS_SOURCE_CACHE_MAX_MB", 500) * 1024 * 1024;
        Ok(Some(cache))
    }

    /// A cache in `dir` with the default total cap (500 MB).
    pub fn new(dir: PathBuf, max_bytes: u64, timeout: Duration) -> std::io::Result<SourceCache> {
        std::fs::create_dir_all(&dir)?;
        Ok(SourceCache {
            dir,
            max_bytes,
            max_total_bytes: 500 * 1024 * 1024,
            timeout,
        })
    }

    /// Removes the least recently used documents until the cache holds
    /// at most `max_total_bytes`. A document is "used" when it is
    /// downloaded or served (its modification time).
    pub fn evict(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "pdf"))
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                Some((meta.modified().ok()?, meta.len(), e.path()))
            })
            .collect();
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        files.sort();
        for (_, len, path) in files {
            if total <= self.max_total_bytes {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total -= len;
            }
        }
    }

    fn path_of(&self, url: &str) -> PathBuf {
        let key: String = Sha256::digest(url.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        self.dir.join(format!("{key}.pdf"))
    }
}

fn error(status: StatusCode, message: impl Into<String>, url: &str) -> Response {
    (
        status,
        Json(serde_json::json!({ "error": message.into(), "url": url })),
    )
        .into_response()
}

async fn source_route(State(store): State<Arc<Store>>, Path(rest): Path<String>) -> Response {
    let (id, document) = match rest.strip_suffix("/document") {
        Some(id) => (id, true),
        None => (rest.as_str(), false),
    };
    let Some(source) = store.dataset.sources.get(id) else {
        return error(StatusCode::NOT_FOUND, format!("no source {id}"), "");
    };
    if !document {
        return Json(source.clone()).into_response();
    }
    let url = source.url.clone();
    let Some(cache) = store.source_cache.clone() else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "this server doesn't fetch source documents (OPENPARTS_SOURCE_CACHE is not set): open the original link",
            &url,
        );
    };
    let expected = source.sha256.clone();
    let fetched = tokio::task::spawn_blocking(move || fetch_cached(&cache, &url, expected))
        .await
        .unwrap_or_else(|e| Err(Failure::internal(e.to_string())));
    match fetched {
        Ok(bytes) => {
            let mut res = (StatusCode::OK, bytes).into_response();
            let h = res.headers_mut();
            h.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/pdf"),
            );
            h.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=86400"),
            );
            h.insert(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            );
            // Rendered by PDF.js, never run as a page of this origin.
            h.insert(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static("sandbox"),
            );
            res
        }
        Err(f) => error(f.status, f.message, &source.url),
    }
}

#[derive(Debug)]
struct Failure {
    status: StatusCode,
    message: String,
}

impl Failure {
    fn upstream(message: impl Into<String>) -> Failure {
        Failure {
            status: StatusCode::BAD_GATEWAY,
            message: message.into(),
        }
    }
    fn internal(message: impl Into<String>) -> Failure {
        Failure {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The document at `url`: from the cache, else downloaded (and cached).
fn fetch_cached(
    cache: &SourceCache,
    url: &str,
    expected_sha256: Option<String>,
) -> Result<Vec<u8>, Failure> {
    let path = cache.path_of(url);
    if let Ok(bytes) = std::fs::read(&path) {
        if expected_sha256
            .as_deref()
            .is_none_or(|h| h == sha256_hex(&bytes))
        {
            // Recently used: evicted last.
            if let Ok(f) = std::fs::File::options().append(true).open(&path) {
                f.set_modified(std::time::SystemTime::now()).ok();
            }
            return Ok(bytes);
        }
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(Failure::upstream(format!("{url} is not an http(s) link")));
    }
    let agent = ureq::AgentBuilder::new()
        .timeout(cache.timeout)
        .redirects(5)
        .user_agent(USER_AGENT)
        .build();
    let response = match agent.get(url).set("Accept", "application/pdf").call() {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => {
            return Err(Failure::upstream(format!(
                "the publisher refused the download (HTTP {code}): open the original link"
            )))
        }
        Err(e) => {
            return Err(Failure::upstream(format!(
                "could not download the document: {e}"
            )))
        }
    };
    let content_type = response.content_type().to_ascii_lowercase();
    let pdf_type = matches!(
        content_type.as_str(),
        "application/pdf" | "application/x-pdf"
    );
    let generic = matches!(
        content_type.as_str(),
        "application/octet-stream" | "binary/octet-stream" | "application/download" | ""
    );
    if !pdf_type && !generic {
        return Err(Failure::upstream(format!(
            "the publisher answered with {content_type}, not a PDF (a sign-in or bot-check page?): open the original link"
        )));
    }
    if let Some(len) = response
        .header("content-length")
        .and_then(|l| l.parse::<u64>().ok())
    {
        if len > cache.max_bytes {
            return Err(Failure::upstream(format!(
                "the document is {len} bytes, more than this server's limit of {}",
                cache.max_bytes
            )));
        }
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(cache.max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Failure::upstream(format!("download interrupted: {e}")))?;
    if bytes.len() as u64 > cache.max_bytes {
        return Err(Failure::upstream(format!(
            "the document is larger than this server's limit of {} bytes",
            cache.max_bytes
        )));
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err(Failure::upstream(
            "the download is not a PDF: open the original link",
        ));
    }
    if let Some(expected) = expected_sha256 {
        let got = sha256_hex(&bytes);
        if got != expected {
            return Err(Failure::upstream(format!(
                "the document's sha256 is {got}, not the {expected} its Source records (a newer revision?)"
            )));
        }
    }
    if bytes.len() as u64 > cache.max_total_bytes {
        // Served, but larger than the whole cache: not kept.
        return Ok(bytes);
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    if std::fs::write(&tmp, &bytes)
        .and_then(|_| std::fs::rename(&tmp, &path))
        .is_err()
    {
        std::fs::remove_file(&tmp).ok();
        tracing::warn!(path = %path.display(), "could not cache a source document");
    }
    cache.evict();
    Ok(bytes)
}
