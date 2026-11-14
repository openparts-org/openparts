//! The deployed application (`app::build_app`): the API under `/api` as
//! well as `/`, the web site as static files, and cache headers.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

fn store() -> Arc<openparts_server::store::Store> {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../openparts-data");
    Arc::new(openparts_server::store::discover_and_load(&data).unwrap())
}

async fn get(app: axum::Router, uri: &str) -> axum::response::Response {
    app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn the_api_answers_under_api_and_at_the_root() {
    let app = openparts_server::app::build_app(store(), None);
    for uri in ["/health", "/api/health", "/api/v1/parts/raspberrypi/RP2040"] {
        assert_eq!(
            get(app.clone(), uri).await.status(),
            StatusCode::OK,
            "{uri}"
        );
    }
}

#[tokio::test]
async fn api_json_gets_a_short_cache_policy() {
    let app = openparts_server::app::build_app(store(), None);
    let res = get(app, "/api/v1/parts/raspberrypi/RP2040").await;
    assert_eq!(res.headers()[header::CACHE_CONTROL], "public, max-age=300");
}

#[tokio::test]
async fn kicad_artifacts_are_cached_for_a_day_with_their_hash_as_etag() {
    let app = openparts_server::app::build_app(store(), None);
    let res = get(
        app,
        "/api/v1/parts/raspberrypi/RP2040/artifacts/kicad-footprint",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()[header::CACHE_CONTROL],
        "public, max-age=86400"
    );
    let hash = res.headers()["x-content-hash"]
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(res.headers()[header::ETAG], format!("\"{hash}\"").as_str());
}

#[tokio::test]
async fn the_web_site_is_served_with_hashed_assets_immutable() {
    let dir = std::env::temp_dir().join(format!("openparts-web-test-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("index.html"), "<h1>OpenParts</h1>").unwrap();
    std::fs::write(dir.join("assets/app.abc123.js"), "1").unwrap();
    let app = openparts_server::app::build_app(store(), Some(&dir));

    let res = get(app.clone(), "/").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<h1>OpenParts</h1>");

    let res = get(app.clone(), "/assets/app.abc123.js").await;
    assert_eq!(
        res.headers()[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );

    // A client-side route gets the single-page app.
    let res = get(app.clone(), "/en/parts").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<h1>OpenParts</h1>");
    // The API still wins over the site.
    assert_eq!(get(app, "/api/health").await.status(), StatusCode::OK);
    let _ = std::fs::remove_dir_all(&dir);
}
