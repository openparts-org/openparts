//! The review status and file paths of a part's documents, the catalog's
//! review facet, and the opt-in source document endpoint -- against a
//! small fixture dataset written to a temporary directory. The document
//! endpoint's upstream is a local mock server: no test touches the
//! network.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::routing::get;
use http_body_util::BodyExt;
use openparts_server::sources::SourceCache;
use openparts_server::store::{discover_and_load, Store};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

fn temp_dir(name: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "openparts-server-{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn source_yaml(id: &str, url: &str, sha256: Option<&str>) -> String {
    let doc = id.rsplit('/').next().unwrap();
    let mut s = format!(
        "schema_version: '0.1'\nkind: source\nid: {id}\npublisher: ex\ndocument_id: {doc}\n\
         document_type: datasheet\nrevision: '2'\ndate: '2026-01'\nurl: '{url}'\n"
    );
    if let Some(h) = sha256 {
        s.push_str(&format!("sha256: '{h}'\n"));
    }
    s
}

const REVIEWED: &str = "review: {status: reviewed, by: [alice], pr: 12, date: 2026-11-20}\n";

/// Two parts sharing a reviewed package: EX-DONE (part and device
/// reviewed too) and EX-NEW (neither).
fn fixture() -> PathBuf {
    let dir = temp_dir("data");
    write(
        &dir,
        "sources/ex/DS1.yaml",
        &source_yaml("ex/DS1", "https://example.com/ds1.pdf", None),
    );
    write(
        &dir,
        "packages/standards/T.yaml",
        &format!(
            "schema_version: '0.1'\nkind: package\nid: standards/T\nfamily: soic\nlead_count: 2\n\
             pitch: {{nominal: 1.27, unit: mm}}\ndimensions:\n\
             \x20 body_width: {{nominal: 3.9, unit: mm}}\n\
             \x20 body_length: {{nominal: 4.9, unit: mm}}\n{REVIEWED}"
        ),
    );
    for (dev, review) in [("DONE", REVIEWED), ("NEW", "")] {
        write(
            &dir,
            &format!("devices/ex/D{dev}.yaml"),
            &format!(
                "schema_version: '0.1'\nkind: device\nid: ex/D{dev}\nmanufacturer: ex\ncategory: resistor\n\
                 pins:\n  '1': {{name: A, type: passive}}\n  '2': {{name: B, type: passive}}\n{review}"
            ),
        );
        write(
            &dir,
            &format!("parts/ex/ex-{}/EX-{dev}.yaml", dev.to_lowercase()),
            &format!(
                "schema_version: '0.1'\nkind: part\nid: ex/EX-{dev}\nmanufacturer: ex\nmpn: EX-{dev}\n\
                 device: ex/D{dev}\npackage: standards/T\nexistence: {{status: verified}}\n\
                 lifecycle: {{status: active}}\nsources: [ex/DS1]\n{review}"
            ),
        );
    }
    dir
}

fn app(store: Store) -> axum::Router {
    openparts_server::routes::build_router(Arc::new(store))
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let res = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn documents_carry_paths_and_review_blocks() {
    let app = app(discover_and_load(&fixture()).unwrap());
    let (status, done) = get_json(&app, "/v1/parts/ex/EX-DONE/documents").await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["review"], "reviewed");
    assert_eq!(done["part"]["path"], "parts/ex/ex-done/EX-DONE.yaml");
    assert_eq!(done["device"]["path"], "devices/ex/DDONE.yaml");
    assert_eq!(done["package"]["path"], "packages/standards/T.yaml");
    assert_eq!(done["part"]["review"]["by"][0], "alice");
    assert_eq!(done["part"]["review"]["pr"], 12);
    assert_eq!(done["part"]["review"]["date"], "2026-11-20");

    let (_, new) = get_json(&app, "/v1/parts/ex/EX-NEW/documents").await;
    assert_eq!(
        new["review"], "partly",
        "only the shared package is reviewed"
    );
    assert!(new["part"]["review"].is_null());
    assert_eq!(new["package"]["review"]["status"], "reviewed");

    let (status, _) = get_json(&app, "/v1/parts/ex/NOPE/documents").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The Part document itself carries its own block.
    let (_, part) = get_json(&app, "/v1/parts/ex/EX-DONE").await;
    assert_eq!(part["review"]["pr"], 12);
}

#[tokio::test]
async fn the_catalog_filters_on_review() {
    let app = app(discover_and_load(&fixture()).unwrap());
    let (status, all) = get_json(&app, "/v1/catalog").await;
    assert_eq!(status, StatusCode::OK, "{all}");
    let facet: Vec<(String, u64)> = all["facets"]["review"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["value"].as_str().unwrap().to_string(),
                v["count"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        facet,
        [("reviewed".to_string(), 1), ("partly".to_string(), 1)]
    );
    let item = |body: &Value, mpn: &str| {
        body["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["mpn"] == mpn)
            .cloned()
            .unwrap()
    };
    let new = item(&all, "EX-NEW");
    assert_eq!(new["review"]["state"], "partly");
    assert_eq!(new["review"]["package"], true);
    assert_eq!(new["review"]["part"], false);

    let (_, reviewed) = get_json(&app, "/v1/catalog?review=reviewed").await;
    assert_eq!(reviewed["total"], 1);
    assert_eq!(reviewed["items"][0]["mpn"], "EX-DONE");
    let (_, none) = get_json(&app, "/v1/catalog?review=unreviewed").await;
    assert_eq!(none["total"], 0);
}

// ---- GET /v1/sources/{id}/document ----

const PDF: &[u8] = b"%PDF-1.4\n% fixture\n";

/// A local upstream: `/ok.pdf`, `/octet` (a PDF as octet-stream),
/// `/forbidden` (403), `/html` (a challenge page), `/big.pdf`, and
/// `/n/<k>.pdf` (a 600-byte PDF each).
async fn mock_upstream() -> String {
    let router = axum::Router::new()
        .route(
            "/ok.pdf",
            get(|| async { ([(header::CONTENT_TYPE, "application/pdf")], PDF) }),
        )
        .route(
            "/octet",
            get(|| async { ([(header::CONTENT_TYPE, "application/octet-stream")], PDF) }),
        )
        .route(
            "/forbidden",
            get(|| async { (StatusCode::FORBIDDEN, "no robots") }),
        )
        .route(
            "/html",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/html")],
                    "<html>checking your browser</html>",
                )
            }),
        )
        .route(
            "/big.pdf",
            get(|| async {
                let mut body = PDF.to_vec();
                body.resize(4096, b' ');
                ([(header::CONTENT_TYPE, "application/pdf")], body)
            }),
        )
        .route(
            "/n/*k",
            get(|| async {
                let mut body = PDF.to_vec();
                body.resize(600, b' ');
                ([(header::CONTENT_TYPE, "application/pdf")], body)
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{addr}")
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn document_fixture(upstream: &str) -> PathBuf {
    let dir = temp_dir("docs");
    let good = sha256_hex(PDF);
    let wrong = "ab".repeat(32);
    let mut sources = vec![
        ("ex/OK".to_string(), "ok.pdf".to_string(), Some(good)),
        ("ex/OCTET".into(), "octet".into(), None),
        ("ex/FORBIDDEN".into(), "forbidden".into(), None),
        ("ex/HTML".into(), "html".into(), None),
        ("ex/BIG".into(), "big.pdf".into(), None),
        ("ex/WRONGHASH".into(), "ok.pdf".into(), Some(wrong)),
    ];
    for k in 1..=3 {
        sources.push((format!("ex/N{k}"), format!("n/{k}.pdf"), None));
    }
    for (id, path, sha) in sources {
        write(
            &dir,
            &format!("sources/{id}.yaml"),
            &source_yaml(&id, &format!("{upstream}/{path}"), sha.as_deref()),
        );
    }
    dir
}

fn pdfs_in(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "pdf")
        })
        .count()
}

#[tokio::test(flavor = "multi_thread")]
async fn source_documents_are_fetched_only_for_known_sources() {
    let upstream = mock_upstream().await;
    let data = document_fixture(&upstream);
    let cache_dir = temp_dir("cache");

    // Off without a cache directory (as on any public deployment): the
    // site links to the original instead.
    let off = app(discover_and_load(&data).unwrap());
    let (status, body) = get_json(&off, "/v1/sources/ex/OK/document").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["url"], format!("{upstream}/ok.pdf"));
    let (_, config) = get_json(&off, "/v1/config").await;
    assert_eq!(config["source_documents"], false);

    let mut store = discover_and_load(&data).unwrap();
    store.source_cache =
        Some(SourceCache::new(cache_dir.clone(), 1024, Duration::from_secs(5)).unwrap());
    let on = app(store);
    let (_, config) = get_json(&on, "/v1/config").await;
    assert_eq!(config["source_documents"], true);

    let res = on
        .clone()
        .oneshot(
            Request::get("/v1/sources/ex/OK/document")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/pdf");
    assert_eq!(res.headers()[header::CONTENT_SECURITY_POLICY], "sandbox");
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], PDF);
    assert_eq!(pdfs_in(&cache_dir), 1);

    let (status, _) = get_json(&on, "/v1/sources/ex/OCTET/document").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a PDF sent as octet-stream is still a PDF"
    );

    for (id, needle) in [
        ("ex/FORBIDDEN", "HTTP 403"),
        ("ex/HTML", "text/html"),
        ("ex/BIG", "limit"),
        ("ex/WRONGHASH", "sha256"),
    ] {
        let (status, body) = get_json(&on, &format!("/v1/sources/{id}/document")).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY, "{id}: {body}");
        let message = body["error"].as_str().unwrap();
        assert!(message.contains(needle), "{id}: {message}");
        assert!(body["url"].as_str().unwrap().starts_with(&upstream));
    }

    // Only ids of loaded sources: there is no way to name a URL.
    for uri in [
        "/v1/sources/ex/NOPE/document",
        "/v1/sources/document?url=http://127.0.0.1/",
        "/v1/sources/..%2F..%2Fetc%2Fpasswd/document",
    ] {
        let (status, _) = get_json(&on, uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    // Only the two PDFs were cached; refusals and unknown ids leave nothing.
    assert_eq!(pdfs_in(&cache_dir), 2);

    // The Source itself.
    let (status, body) = get_json(&on, "/v1/sources/ex/OK").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["document_id"], "OK");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_cache_keeps_under_its_cap_dropping_the_oldest() {
    let upstream = mock_upstream().await;
    let data = document_fixture(&upstream);
    let cache_dir = temp_dir("cache-cap");
    let mut cache = SourceCache::new(cache_dir.clone(), 1024, Duration::from_secs(5)).unwrap();
    // Room for two 600-byte documents.
    cache.max_total_bytes = 1300;
    let mut store = discover_and_load(&data).unwrap();
    store.source_cache = Some(cache.clone());
    let on = app(store);
    let fetch = |k: u32| {
        let on = on.clone();
        async move {
            let (status, _) = get_json(&on, &format!("/v1/sources/ex/N{k}/document")).await;
            assert_eq!(status, StatusCode::OK);
        }
    };
    let cached = |url: &str| {
        let key: String = {
            use sha2::Digest;
            sha2::Sha256::digest(url.as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        };
        cache_dir.join(format!("{key}.pdf")).exists()
    };
    let url = |k: u32| format!("{upstream}/n/{k}.pdf");
    fetch(1).await;
    std::thread::sleep(Duration::from_millis(20));
    fetch(2).await;
    std::thread::sleep(Duration::from_millis(20));
    // Using N1 again makes N2 the least recently used.
    fetch(1).await;
    std::thread::sleep(Duration::from_millis(20));
    fetch(3).await;
    assert_eq!(pdfs_in(&cache_dir), 2);
    assert!(cached(&url(1)) && cached(&url(3)) && !cached(&url(2)));
}
