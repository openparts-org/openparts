//! Review endpoints, the export of their submissions, and the source
//! document endpoint -- against small fixture datasets written to a
//! temporary directory (and, read-only, the real openparts-data). The
//! document endpoint's upstream is a local mock server: no test touches
//! the network.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::routing::get;
use http_body_util::BodyExt;
use openparts_server::review::ReviewService;
use openparts_server::sources::SourceCache;
use openparts_server::store::{discover_and_load, Store};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

const TOKEN: &str = "test-token-0123456789";

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

/// Two parts sharing a package: EX-CAP with an electrical parameter,
/// EX-PLAIN without.
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
        "schema_version: '0.1'\nkind: package\nid: standards/T\nfamily: soic\nlead_count: 2\n\
         pitch: {nominal: 1.27, unit: mm}\ndimensions:\n\
         \x20 body_width: {nominal: 3.9, unit: mm}\n\
         \x20 body_length: {nominal: 4.9, unit: mm}\n\
         provenance:\n\
         \x20 /pitch: [{source: ex/DS1, page: 4, locator: 'Package outline, e'}]\n\
         \x20 /dimensions/body_width: [{source: ex/DS1, page: 4, locator: 'Package outline, E1'}]\n",
    );
    for (dev, category) in [("CAP", "capacitor"), ("PLAIN", "resistor")] {
        write(
            &dir,
            &format!("devices/ex/D{dev}.yaml"),
            &format!(
                "schema_version: '0.1'\nkind: device\nid: ex/D{dev}\nmanufacturer: ex\ncategory: {category}\n\
                 pins:\n  '1': {{name: A, type: passive}}\n  '2': {{name: B, type: passive}}\n"
            ),
        );
    }
    let part = |mpn: &str, dev: &str, extra: &str| {
        format!(
            "schema_version: '0.1'\nkind: part\nid: ex/{mpn}\nmanufacturer: ex\nmpn: {mpn}\n\
             device: ex/D{dev}\npackage: standards/T\nexistence: {{status: verified}}\n\
             lifecycle: {{status: active}}\nsources: [ex/DS1]\n{extra}"
        )
    };
    write(
        &dir,
        "parts/ex/EX-CAP.yaml",
        &part(
            "EX-CAP",
            "CAP",
            "parameters:\n  capacitance: {nominal: 100, unit: nF}\nprovenance:\n\
             \x20 /parameters/capacitance: [{source: ex/DS1, page: 2, locator: 'Table 1, \"100 nF\"'}]\n\
             \x20 /existence: [{source: ex/DS1, page: 1, locator: 'Ordering information'}]\n",
        ),
    );
    write(
        &dir,
        "parts/ex/EX-PLAIN.yaml",
        &part(
            "EX-PLAIN",
            "PLAIN",
            "provenance:\n  /existence: [{source: ex/DS1, page: 1, locator: 'Ordering information'}]\n",
        ),
    );
    dir
}

fn store_of(data: &Path, review_dir: Option<&Path>, token: Option<&str>) -> Store {
    let mut store = discover_and_load(data).unwrap();
    store.review = ReviewService::new(review_dir, token.map(str::to_string)).unwrap();
    store
}

fn app(store: Store) -> axum::Router {
    openparts_server::routes::build_router(Arc::new(store))
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    send(app, Request::get(uri).body(Body::empty()).unwrap()).await
}

fn post(body: &Value, token: Option<&str>) -> Request<Body> {
    let mut req = Request::post("/v1/reviews").header(header::CONTENT_TYPE, "application/json");
    if let Some(t) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    req.body(Body::from(body.to_string())).unwrap()
}

/// The fact `pointer` of `part`'s review status.
async fn fact(app: &axum::Router, part: &str, pointer: &str) -> Value {
    let (status, body) = get_json(app, &format!("/v1/parts/ex/{part}/reviews")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["pointer"] == pointer)
        .cloned()
        .unwrap_or_else(|| panic!("no {pointer} in {body}"))
}

fn submission(fact: &Value, status: &str, comment: Option<&str>) -> Value {
    json!({
        "target_kind": fact["target_kind"],
        "target": fact["target"],
        "pointer": fact["pointer"],
        "status": status,
        "reviewer": "Alice",
        "comment": comment,
        "value_hash": fact["value_hash"],
    })
}

#[tokio::test]
async fn part_reviews_list_every_fact_with_counts() {
    let app = app(store_of(&fixture(), None, None));
    let (status, body) = get_json(&app, "/v1/parts/ex/EX-CAP/reviews").await;
    assert_eq!(status, StatusCode::OK);
    // 2 part facts + 0 device facts + 2 package facts.
    assert_eq!(body["total"], 4);
    assert_eq!(body["unreviewed"], 4);
    let first = &body["facts"][0];
    assert_eq!(first["pointer"], "/parameters/capacitance");
    assert_eq!(first["value"], "100 nF");
    assert_eq!(first["status"], "unreviewed");
    assert_eq!(first["evidence"][0]["page"], 2);
    assert_eq!(first["evidence"][0]["title"], "EX DS1");
    assert_eq!(first["evidence"][0]["url"], "https://example.com/ds1.pdf");
    let (status, _) = get_json(&app, "/v1/parts/ex/NOPE/reviews").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn submissions_are_refused_without_a_configured_token() {
    let reviews = temp_dir("reviews");
    // A review directory alone doesn't enable writes, nor does a short token.
    for token in [None, Some("short")] {
        let app = app(store_of(&fixture(), Some(&reviews), token));
        let (_, config) = get_json(&app, "/v1/reviews/config").await;
        assert_eq!(config["writes_enabled"], false);
        let f = fact(&app, "EX-CAP", "/pitch").await;
        let (status, body) = send(
            &app,
            post(&submission(&f, "confirmed", None), Some("short")),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert!(body["error"].as_str().unwrap().contains("disabled"));
    }
    assert!(!reviews.join("submissions.jsonl").exists());
}

#[tokio::test]
async fn submissions_need_the_token_and_valid_input() {
    let reviews = temp_dir("reviews");
    let app = app(store_of(&fixture(), Some(&reviews), Some(TOKEN)));
    let (_, config) = get_json(&app, "/v1/reviews/config").await;
    assert_eq!(config["writes_enabled"], true);
    let f = fact(&app, "EX-CAP", "/parameters/capacitance").await;
    let ok = submission(&f, "confirmed", None);

    let (status, _) = send(&app, post(&ok, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(&app, post(&ok, Some("wrong-token-0123456789"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let with = |key: &str, value: Value| {
        let mut v = ok.clone();
        v[key] = value;
        v
    };
    let cases = [
        (json!({"nonsense": true}), StatusCode::BAD_REQUEST),
        (with("extra", json!(1)), StatusCode::BAD_REQUEST),
        (with("status", json!("maybe")), StatusCode::BAD_REQUEST),
        (with("pointer", json!("/nope")), StatusCode::NOT_FOUND),
        (with("target", json!("ex/NOPE")), StatusCode::NOT_FOUND),
        (
            with("value_hash", json!(format!("sha256:{}", "0".repeat(64)))),
            StatusCode::CONFLICT,
        ),
        (with("reviewer", json!("")), StatusCode::BAD_REQUEST),
        (
            with("reviewer", json!("x".repeat(65))),
            StatusCode::BAD_REQUEST,
        ),
        (with("reviewer", json!("a\u{7}b")), StatusCode::BAD_REQUEST),
        (
            with("comment", json!("x".repeat(2001))),
            StatusCode::BAD_REQUEST,
        ),
        (with("status", json!("disputed")), StatusCode::BAD_REQUEST),
        (
            with("comment", json!("x".repeat(20_000))),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ];
    for (body, expected) in cases {
        let (status, reply) = send(&app, post(&body, Some(TOKEN))).await;
        assert_eq!(status, expected, "{body}: {reply}");
    }
    assert!(!reviews.join("submissions.jsonl").exists());

    let (status, reply) = send(&app, post(&ok, Some(TOKEN))).await;
    assert_eq!(status, StatusCode::CREATED, "{reply}");
    assert_eq!(reply["fact"]["status"], "confirmed");
    assert_eq!(reply["fact"]["pending"], true);
    assert_eq!(reply["fact"]["reviews"][0]["reviewer"], "Alice");
    let (_, body) = get_json(&app, "/v1/parts/ex/EX-CAP/reviews").await;
    assert_eq!(
        (body["confirmed"].clone(), body["pending"].clone()),
        (json!(1), json!(1))
    );

    // A dispute with a comment and a corrected value.
    let mut dispute = submission(&f, "disputed", Some("Table 1 says 220 nF"));
    dispute["reviewer"] = json!("Bob");
    dispute["corrected_value"] = json!("220 nF");
    let (status, reply) = send(&app, post(&dispute, Some(TOKEN))).await;
    assert_eq!(status, StatusCode::CREATED, "{reply}");
    assert_eq!(reply["fact"]["status"], "disputed");
    assert_eq!(reply["fact"]["reviews"].as_array().unwrap().len(), 2);

    let log = std::fs::read_to_string(reviews.join("submissions.jsonl")).unwrap();
    assert_eq!(log.lines().count(), 2);
    // A restarted server reads its pending submissions back.
    let data = fixture();
    let again = app_from(&data, &reviews);
    assert_eq!(
        fact(&again, "EX-CAP", "/parameters/capacitance").await["status"],
        "disputed"
    );
}

fn app_from(data: &Path, reviews: &Path) -> axum::Router {
    app(store_of(data, Some(reviews), Some(TOKEN)))
}

#[tokio::test]
async fn the_queue_puts_parts_with_parameters_first_and_drops_reviewed_facts() {
    let reviews = temp_dir("reviews");
    let data = fixture();
    let app = app_from(&data, &reviews);
    let (status, q) = get_json(&app, "/v1/reviews/queue?limit=100").await;
    assert_eq!(status, StatusCode::OK, "{q}");
    // EX-CAP: 2 facts + the shared package's 2; EX-PLAIN: 1 of its own.
    assert_eq!(q["total"], 5);
    assert_eq!(q["items"][0]["mpn"], "EX-CAP");
    assert_eq!(q["items"][0]["fact"]["pointer"], "/parameters/capacitance");
    assert_eq!(q["items"][4]["mpn"], "EX-PLAIN");

    let f = fact(&app, "EX-CAP", "/pitch").await;
    let (status, _) = send(&app, post(&submission(&f, "confirmed", None), Some(TOKEN))).await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, q) = get_json(&app, "/v1/reviews/queue?limit=2&offset=1").await;
    assert_eq!(q["total"], 4);
    assert_eq!(q["items"].as_array().unwrap().len(), 2);
    let (_, q) = get_json(&app, "/v1/reviews/queue?status=confirmed").await;
    assert_eq!(q["total"], 1);
    let (_, q) = get_json(&app, "/v1/reviews/queue?category=resistor").await;
    assert_eq!(
        q["total"], 2,
        "EX-PLAIN's own fact and the shared package's remaining one"
    );
    let (_, q) = get_json(&app, "/v1/reviews/queue?manufacturer=ex&mpn=EX-PLAIN").await;
    assert_eq!(q["items"][0]["mpn"], "EX-PLAIN");
    let (status, _) = get_json(&app, "/v1/reviews/queue?status=bogus").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn exported_submissions_become_committed_reviews() {
    let reviews = temp_dir("reviews");
    let data = fixture();
    let app = app_from(&data, &reviews);
    let f = fact(&app, "EX-CAP", "/parameters/capacitance").await;
    let (status, _) = send(&app, post(&submission(&f, "confirmed", None), Some(TOKEN))).await;
    assert_eq!(status, StatusCode::CREATED);

    // `openparts reviews export --data <data> --from <reviews>`.
    let log = openparts_review::SubmissionLog::open(&reviews).unwrap();
    let (subs, bad) = log.read_all().unwrap();
    assert_eq!((subs.len(), bad), (1, 0));
    let report = openparts_review::export(&data, &subs).unwrap();
    assert_eq!(
        report.written,
        [PathBuf::from("reviews/part/ex/EX-CAP.yaml")]
    );

    // After the commit, the restarted server counts it as committed
    // (the same submission, still in the log, is not counted twice).
    let app = app_from(&data, &reviews);
    let f = fact(&app, "EX-CAP", "/parameters/capacitance").await;
    assert_eq!(f["status"], "confirmed");
    assert_eq!(f["pending"], false);
    assert_eq!(f["reviews"].as_array().unwrap().len(), 1);

    // The value changes: the review goes stale and the fact is
    // unreviewed again; the validator warns.
    let part = data.join("parts/ex/EX-CAP.yaml");
    let text = std::fs::read_to_string(&part).unwrap();
    std::fs::write(&part, text.replace("nominal: 100", "nominal: 220")).unwrap();
    let app = app_from(&data, &reviews);
    let f = fact(&app, "EX-CAP", "/parameters/capacitance").await;
    assert_eq!(
        (f["status"].clone(), f["stale"].clone()),
        (json!("unreviewed"), json!(1))
    );
    let ds = openparts_data::Dataset::load(&data).unwrap();
    let facts = openparts_review::FactIndex::build(&ds);
    let doc = ds.reviews.values().next().unwrap();
    let diags = openparts_validator::validate_review(
        doc,
        facts.facts(doc.target_kind, &doc.target),
        &ds.source_ids(),
    );
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, "review.stale");
}

#[tokio::test]
async fn the_real_dataset_has_reviewable_facts() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../openparts-data");
    let app = app(discover_and_load(&data).unwrap());
    let (status, body) = get_json(&app, "/v1/parts/raspberrypi/RP2040/reviews").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["total"].as_u64().unwrap() > 10, "{body}");
    assert!(body["facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["pointer"] == "/dimensions/body_width" && f["target_kind"] == "package"));
    let (status, q) = get_json(&app, "/v1/reviews/queue?limit=5").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(q["items"].as_array().unwrap().len(), 5);
}

// ---- GET /v1/sources/{id}/document ----

const PDF: &[u8] = b"%PDF-1.4\n% fixture\n";

/// A local upstream: `/ok.pdf`, `/octet` (a PDF as octet-stream),
/// `/forbidden` (403), `/html` (a challenge page), `/big.pdf`.
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
    let sources = [
        ("ex/OK", "ok.pdf", Some(good.as_str())),
        ("ex/OCTET", "octet", None),
        ("ex/FORBIDDEN", "forbidden", None),
        ("ex/HTML", "html", None),
        ("ex/BIG", "big.pdf", None),
        ("ex/WRONGHASH", "ok.pdf", Some(wrong.as_str())),
    ];
    for (id, path, sha) in sources {
        write(
            &dir,
            &format!("sources/{id}.yaml"),
            &source_yaml(id, &format!("{upstream}/{path}"), sha),
        );
    }
    dir
}

#[tokio::test(flavor = "multi_thread")]
async fn source_documents_are_fetched_only_for_known_sources() {
    let upstream = mock_upstream().await;
    let data = document_fixture(&upstream);
    let cache_dir = temp_dir("cache");

    // Off without a cache directory: the site opens the original link.
    let off = app(discover_and_load(&data).unwrap());
    let (status, body) = get_json(&off, "/v1/sources/ex/OK/document").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["url"], format!("{upstream}/ok.pdf"));
    let (_, config) = get_json(&off, "/v1/reviews/config").await;
    assert_eq!(config["documents_enabled"], false);

    let mut store = discover_and_load(&data).unwrap();
    store.source_cache =
        Some(SourceCache::new(cache_dir.clone(), 1024, Duration::from_secs(5)).unwrap());
    let on = app(store);

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
    assert_eq!(std::fs::read_dir(&cache_dir).unwrap().count(), 1);

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
    assert_eq!(std::fs::read_dir(&cache_dir).unwrap().count(), 2);

    // The Source itself.
    let (status, body) = get_json(&on, "/v1/sources/ex/OK").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["document_id"], "OK");
}
