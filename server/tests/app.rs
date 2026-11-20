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
    assert_eq!(
        res.headers()[header::CACHE_CONTROL],
        "public, max-age=60, stale-while-revalidate=300"
    );
}

#[tokio::test]
async fn plain_artifact_links_are_cached_briefly_with_their_hash_as_etag() {
    let app = openparts_server::app::build_app(store(), None);
    let res = get(
        app,
        "/api/v1/parts/raspberrypi/RP2040/artifacts/kicad-footprint",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()[header::CACHE_CONTROL],
        "public, max-age=300, stale-while-revalidate=86400"
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

#[tokio::test]
async fn an_unchanged_artifact_answers_not_modified() {
    let uri = "/api/v1/parts/raspberrypi/RP2040/artifacts/footprint-svg";
    let res = get(openparts_server::app::build_app(store(), None), uri).await;
    let etag = res.headers()[header::ETAG].clone();
    let res = openparts_server::app::build_app(store(), None)
        .oneshot(
            Request::builder()
                .uri(uri)
                .header(header::IF_NONE_MATCH, etag.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(res.headers()[header::ETAG], etag);
    assert!(res.headers().contains_key(header::CACHE_CONTROL));
    // In a list, weak, or another tag.
    for (tag, status) in [
        (
            format!("\"x\", W/{}", etag.to_str().unwrap()),
            StatusCode::NOT_MODIFIED,
        ),
        ("\"sha256:0\"".to_string(), StatusCode::OK),
    ] {
        let res = openparts_server::app::build_app(store(), None)
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header(header::IF_NONE_MATCH, tag.as_str())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), status, "{tag}");
    }
}

#[tokio::test]
async fn a_hashed_artifact_link_is_immutable_and_revalidates() {
    let app = openparts_server::app::build_app(store(), None);
    let res = get(app.clone(), "/api/v1/parts/raspberrypi/RP2040/artifacts").await;
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let link = format!("/api{}", list["footprint-svg"].as_str().unwrap());
    let res = get(app.clone(), &link).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    let etag = res.headers()[header::ETAG].clone();
    let res = app
        .oneshot(
            Request::builder()
                .uri(&link)
                .header(header::IF_NONE_MATCH, etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(
        res.headers()[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
}
