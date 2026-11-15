//! Integration tests against the real `openparts-data` fixtures, driven
//! through the router directly (`tower::ServiceExt::oneshot`) rather
//! than a bound socket.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../openparts-data")
}

fn app() -> axum::Router {
    let store = openparts_server::store::discover_and_load(&data_dir()).unwrap();
    openparts_server::routes::build_router(Arc::new(store))
}

async fn body_string(response: axum::response::Response) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn health_check_ok() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn search_finds_rp2040() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/search?q=RP2040")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("\"mpn\":\"RP2040\""));
}

#[tokio::test]
async fn get_part_returns_404_for_unknown_part() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/nobody/NOPE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_part_returns_rp2040() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/raspberrypi/RP2040")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("\"mpn\":\"RP2040\""));
}

#[tokio::test]
async fn kicad_symbol_artifact_matches_device_pin_count() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/raspberrypi/RP2040/artifacts/kicad-symbol")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-content-hash"));
    let body = body_string(response).await;
    // 56 perimeter pins + EP.
    assert_eq!(body.matches("(number \"").count(), 57);
}

#[tokio::test]
async fn stl_artifact_has_one_solid_per_part() {
    if !openparts_solidcraft::available() {
        eprintln!("SolidCraft is not installed; skipping");
        return;
    }
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/raspberrypi/RP2040/artifacts/stl")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-content-hash"));
    let body = response.into_body().collect().await.unwrap().to_bytes();
    // Binary STL: the body and 56 terminals, at least 12 triangles each.
    let triangles = openparts_stl::triangle_count(&body).expect("well-formed binary STL");
    assert!(triangles >= 57 * 12);
}

#[tokio::test]
async fn revision_query_param_changes_effective_model() {
    // RP2040's real B0/B1/B2 revisions have no documented pin
    // differences (unlike the old fictional exemplar fixture) -- this
    // just proves the revision selection path is wired end to end, not
    // that it changes output for this specific device.
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/raspberrypi/RP2040/artifacts/kicad-symbol?silicon_revision=rev-b2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn unknown_revision_is_rejected_with_422() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/raspberrypi/RP2040/artifacts/kicad-symbol?silicon_revision=does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn esp32_c6_footprint_now_generates_after_jedec_dimensions_were_added() {
    // ESP32-C6's package pitch/height weren't confirmable from its own
    // datasheet text (only an embedded mechanical drawing image); this
    // used to make footprint generation correctly fail with 422
    // (missing dimension) rather than guess. openparts-data later added
    // JEDEC MO-220 standard pitch/height for this exact lead-count/
    // body-size combination (see that repo's history), so generation
    // now succeeds -- this is a regression test for that fix, not a
    // live example of "missing dimension blocks generation" anymore.
    // That principle itself stays covered by openparts-mcad's own
    // McadError::MissingDimension unit tests.
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/espressif/ESP32-C6/artifacts/kicad-footprint")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    // 40 terminals and the exposed pad.
    assert_eq!(body.matches("(pad \"").count(), 41);
}

#[tokio::test]
async fn chip_resistor_footprint_has_two_pads() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/yageo/RC0603FR-0710KL/artifacts/kicad-footprint")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert_eq!(body.matches("(pad \"").count(), 2);
}

#[tokio::test]
async fn get_device_resolves_slash_containing_id() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/devices/raspberrypi/RP2040")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

// --- Prebuilt artifacts (OPENPARTS_ARTIFACT_DIR) ---------------------

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A tiny bundle, laid out as `openparts bundle` writes one, holding
/// RP2040's STEP, 3MF and KiCad symbol/footprint (stand-in bytes, so the
/// tests can tell bundled from live output) and an entry whose file is
/// missing.
fn tiny_bundle(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "openparts-server-bundle-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let store = root.join("artifacts/sha256");
    std::fs::create_dir_all(&store).unwrap();
    use openparts_artifacts::bundle::{self as b, Manifest, PartEntry, Status};
    let mut artifacts = std::collections::BTreeMap::new();
    for (kind, bytes, content_type, write) in [
        ("step", &b"ISO-10303-21; bundled"[..], "model/step", true),
        ("3mf", &b"PK\x03\x04 bundled"[..], "model/3mf", true),
        (
            "kicad-symbol",
            &b"(kicad_symbol_lib bundled)"[..],
            "text/plain; charset=utf-8",
            true,
        ),
        (
            "kicad-footprint",
            &b"(footprint bundled)"[..],
            "text/plain; charset=utf-8",
            true,
        ),
        ("stl", &b"never written"[..], "model/stl", false),
    ] {
        let hex = sha256_hex(bytes);
        if write {
            std::fs::write(store.join(&hex), bytes).unwrap();
        }
        artifacts.insert(
            kind.to_string(),
            b::Artifact {
                sha256: hex,
                size: bytes.len() as u64,
                content_type: content_type.to_string(),
                filename: format!("RP2040.{kind}"),
            },
        );
    }
    let manifest = Manifest {
        format: b::FORMAT.into(),
        format_version: b::FORMAT_VERSION,
        artifact_path: b::ARTIFACT_PATH.into(),
        reproducibility: b::Reproducibility {
            toolchain: [("solidcraft".to_string(), "solidcraft-cli test".to_string())].into(),
            ..Default::default()
        },
        summary: b::Summary::default(),
        parts: vec![PartEntry {
            manufacturer: "raspberrypi".into(),
            mpn: "RP2040".into(),
            part_id: "raspberrypi/RP2040".into(),
            device: "raspberrypi/RP2040".into(),
            package: "standards/QFN56-RP2040".into(),
            status: Status::Ok,
            artifacts,
            errors: vec![],
        }],
        load_errors: vec![],
    };
    std::fs::write(
        root.join(b::MANIFEST_FILE),
        serde_json::to_string(&manifest).unwrap(),
    )
    .unwrap();
    root
}

fn app_with_bundle(root: &std::path::Path) -> axum::Router {
    let mut store = openparts_server::store::discover_and_load(&data_dir()).unwrap();
    store.bundle = Some(openparts_server::bundle::Bundle::load(root).unwrap());
    openparts_server::routes::build_router(Arc::new(store))
}

async fn get(app: axum::Router, uri: &str) -> axum::response::Response {
    app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn body_bytes(response: axum::response::Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

#[tokio::test]
async fn bundled_3d_models_are_served_with_their_hash_for_every_revision() {
    let root = tiny_bundle("3d");
    for (uri, expected, content_type) in [
        (
            "/v1/parts/raspberrypi/RP2040/artifacts/step",
            &b"ISO-10303-21; bundled"[..],
            "model/step",
        ),
        (
            "/v1/parts/raspberrypi/RP2040/artifacts/3mf",
            &b"PK\x03\x04 bundled"[..],
            "model/3mf",
        ),
        // 3D models depend on the package only.
        (
            "/v1/parts/raspberrypi/RP2040/artifacts/3mf?silicon_revision=rev-b2",
            &b"PK\x03\x04 bundled"[..],
            "model/3mf",
        ),
    ] {
        let response = get(app_with_bundle(&root), uri).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        assert_eq!(response.headers()["content-type"], content_type);
        assert_eq!(
            response.headers()["x-content-hash"],
            format!("sha256:{}", sha256_hex(expected)).as_str()
        );
        assert_eq!(body_bytes(response).await, expected, "{uri}");
    }
    // An unknown revision is still rejected, bundle or not.
    let response = get(
        app_with_bundle(&root),
        "/v1/parts/raspberrypi/RP2040/artifacts/step?silicon_revision=does-not-exist",
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn a_revision_specific_symbol_is_generated_live_not_bundled() {
    let root = tiny_bundle("symbol");
    let base = get(
        app_with_bundle(&root),
        "/v1/parts/raspberrypi/RP2040/artifacts/kicad-symbol",
    )
    .await;
    assert_eq!(body_bytes(base).await, b"(kicad_symbol_lib bundled)");
    let revised = get(
        app_with_bundle(&root),
        "/v1/parts/raspberrypi/RP2040/artifacts/kicad-symbol?silicon_revision=rev-b2",
    )
    .await;
    assert_eq!(revised.status(), StatusCode::OK);
    let body = String::from_utf8(body_bytes(revised).await).unwrap();
    assert_eq!(body.matches("(number \"").count(), 57);
    // The footprint depends on the package only.
    let footprint = get(
        app_with_bundle(&root),
        "/v1/parts/raspberrypi/RP2040/artifacts/kicad-footprint?silicon_revision=rev-b2",
    )
    .await;
    assert_eq!(body_bytes(footprint).await, b"(footprint bundled)");
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn a_corrupted_bundled_artifact_is_not_served() {
    let root = tiny_bundle("corrupt");
    let hex = sha256_hex(b"ISO-10303-21; bundled");
    std::fs::write(root.join("artifacts/sha256").join(hex), b"tampered").unwrap();
    let response = get(
        app_with_bundle(&root),
        "/v1/parts/raspberrypi/RP2040/artifacts/step",
    )
    .await;
    // Falls back to live generation: built with SolidCraft, else 503.
    if openparts_solidcraft::available() {
        assert_eq!(response.status(), StatusCode::OK);
        assert!(body_bytes(response).await.starts_with(b"ISO-10303-21;"));
    } else {
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn a_model_missing_from_the_bundle_needs_solidcraft() {
    // The tiny bundle lists RP2040's STL but its file is missing, and has
    // no ESP32-C6 entry at all.
    let root = tiny_bundle("missing");
    for uri in [
        "/v1/parts/raspberrypi/RP2040/artifacts/stl",
        "/v1/parts/espressif/ESP32-C6/artifacts/3mf",
    ] {
        let response = get(app_with_bundle(&root), uri).await;
        if openparts_solidcraft::available() {
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
        } else {
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{uri}");
            let body = body_string(response).await;
            assert!(body.contains("SolidCraft is not installed"), "{body}");
        }
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn live_3mf_is_a_coloured_zip() {
    if !openparts_solidcraft::available() {
        eprintln!("SolidCraft is not installed; skipping");
        return;
    }
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/raspberrypi/RP2040/artifacts/3mf")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "model/3mf");
    let body = body_bytes(response).await;
    assert!(body.starts_with(b"PK\x03\x04"));
}

#[tokio::test]
async fn artifact_index_lists_3mf() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/raspberrypi/RP2040/artifacts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = body_string(response).await;
    assert!(body.contains("\"3mf\":\"/v1/parts/raspberrypi/RP2040/artifacts/3mf\""));
}

#[test]
fn a_manifest_of_another_format_is_refused() {
    let root = tiny_bundle("format");
    std::fs::write(
        root.join("manifest.json"),
        r#"{"format":"something-else","format_version":1,"artifact_path":"{sha256}","parts":[]}"#,
    )
    .unwrap();
    assert!(openparts_server::bundle::Bundle::load(&root).is_err());
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn sources_list_every_cited_document_with_its_link() {
    let response = get(app(), "/v1/parts/raspberrypi/RP2040/sources").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_str(&body_string(response).await).unwrap();
    let list = body.as_array().unwrap();
    // The datasheet, possibly in more than one revision.
    let ds: Vec<&serde_json::Value> = list
        .iter()
        .filter(|s| {
            s["source"]["document_id"]
                .as_str()
                .unwrap()
                .contains("RP-008371")
        })
        .collect();
    assert!(!ds.is_empty(), "the RP2040 datasheet is cited");
    assert!(ds
        .iter()
        .all(|s| s["source"]["url"].as_str().unwrap().starts_with("https://")));
    let cited_by: Vec<&str> = ds
        .iter()
        .flat_map(|s| s["cited_by"].as_array().unwrap())
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        cited_by.contains(&"part") && cited_by.contains(&"device"),
        "{cited_by:?}"
    );
}

#[tokio::test]
async fn footprint_svg_preview_shows_every_pad() {
    let response = get(
        app(),
        "/v1/parts/raspberrypi/RP2040/artifacts/footprint-svg",
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/svg+xml");
    let svg = body_string(response).await;
    assert!(svg.starts_with("<svg"));
    // 56 numbered pads and the exposed pad, plus the scale bar's label.
    assert_eq!(svg.matches("<text").count(), 57 + 1);
    assert!(svg.contains(">1</text>") && svg.contains(">56</text>") && svg.contains(">EP</text>"));
}

async fn search_mpns(q: &str) -> Vec<(String, String)> {
    let uri = format!(
        "/v1/search?q={}",
        q.bytes().map(|b| format!("%{b:02X}")).collect::<String>()
    );
    let response = app()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_str(&body_string(response).await).unwrap();
    body.as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["mpn"].as_str().unwrap().to_string(),
                p["category"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

#[tokio::test]
async fn search_normalises_and_ranks_the_exact_mpn_first() {
    for q in ["RP2040", "ＲＰ２０４０", "rp-2040", "rp 2040"] {
        let hits = search_mpns(q).await;
        assert_eq!(hits[0].0, "RP2040", "query {q:?}");
    }
}

#[tokio::test]
async fn search_matches_package_and_japanese_category() {
    let hits = search_mpns("0603 コンデンサ").await;
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|(_, c)| c == "capacitor"));
    let hits = search_mpns("qfn56").await;
    assert!(hits.iter().any(|(m, _)| m == "RP2040"));
    // `ldo` must not match inside a package id such as `AXIAL-DO41`.
    let hits = search_mpns("ldo").await;
    assert!(hits.iter().all(|(_, c)| c != "diode"));
}

async fn catalog(query: &str) -> serde_json::Value {
    let response = get(app(), &format!("/v1/catalog?{query}")).await;
    assert_eq!(response.status(), StatusCode::OK, "{query}");
    serde_json::from_str(&body_string(response).await).unwrap()
}

fn facet<'a>(body: &'a serde_json::Value, name: &str) -> &'a Vec<serde_json::Value> {
    body["facets"][name].as_array().unwrap()
}

fn count_of(body: &serde_json::Value, name: &str, value: &str) -> u64 {
    facet(body, name)
        .iter()
        .find(|v| v["value"] == value)
        .map_or(0, |v| v["count"].as_u64().unwrap())
}

fn facet_sum(body: &serde_json::Value, name: &str) -> u64 {
    facet(body, name)
        .iter()
        .map(|v| v["count"].as_u64().unwrap())
        .sum()
}

#[tokio::test]
async fn catalog_narrows_to_0603_capacitors() {
    let body = catalog("category=capacitor&size=0603&limit=500").await;
    let items = body["items"].as_array().unwrap();
    assert!(!items.is_empty());
    assert_eq!(body["total"].as_u64().unwrap(), items.len() as u64);
    for it in items {
        assert_eq!(it["category"], "capacitor", "{it}");
        assert_eq!(it["size_code"], "0603", "{it}");
        assert_eq!(it["size_code_metric"], "1608", "{it}");
        assert_eq!(it["package_family"], "chip");
        assert_eq!(it["mounting"], "smd");
    }
    // The metric code selects the same parts.
    let metric = catalog("category=capacitor&size=1608&limit=500").await;
    assert_eq!(metric["total"], body["total"]);
    // Picked options stay listed, and so do their siblings.
    let sizes = facet(&body, "size");
    let picked = sizes.iter().find(|v| v["value"] == "0603").unwrap();
    assert_eq!(picked["selected"], true);
    assert_eq!(picked["count"], body["total"]);
    assert!(sizes
        .iter()
        .any(|v| v["value"] == "0402" && v["count"].as_u64() > Some(0)));
    assert!(count_of(&body, "category", "resistor") > 0);
}

#[tokio::test]
async fn catalog_facet_counts_apply_the_other_filters() {
    let all = catalog("limit=0").await;
    let total = all["total"].as_u64().unwrap();
    assert!(total > 500 && all["items"].as_array().unwrap().is_empty());
    // Every part has one category and one lifecycle: the counts add up.
    assert_eq!(facet_sum(&all, "category"), total);
    assert_eq!(facet_sum(&all, "lifecycle"), total);
    // A category filter doesn't change the category facet's counts.
    let caps = catalog("category=capacitor&limit=0").await;
    let counts = |b: &serde_json::Value| -> Vec<(String, u64)> {
        facet(b, "category")
            .iter()
            .map(|v| {
                (
                    v["value"].as_str().unwrap().to_string(),
                    v["count"].as_u64().unwrap(),
                )
            })
            .collect()
    };
    assert_eq!(counts(&caps), counts(&all));
    assert_eq!(
        caps["total"].as_u64().unwrap(),
        count_of(&all, "category", "capacitor")
    );
    assert_eq!(
        facet_sum(&caps, "package_family"),
        caps["total"].as_u64().unwrap()
    );
    // Each option's count is what selecting it would give.
    for v in facet(&caps, "size") {
        let value = v["value"].as_str().unwrap();
        let picked = catalog(&format!("category=capacitor&size={value}&limit=0")).await;
        assert_eq!(picked["total"], v["count"], "size {value}");
    }
    // Two values of one facet are OR'd; repeated and comma forms agree.
    let either = catalog("category=capacitor,resistor&limit=0").await;
    let repeated = catalog("category=capacitor&category=resistor&limit=0").await;
    assert_eq!(either["total"], repeated["total"]);
    assert_eq!(
        either["total"].as_u64().unwrap(),
        count_of(&all, "category", "capacitor") + count_of(&all, "category", "resistor")
    );
}

#[tokio::test]
async fn catalog_height_range_and_sort() {
    let all = catalog("limit=0").await;
    let body = catalog("height_max=1.0&sort=height&order=desc&limit=500").await;
    let items = body["items"].as_array().unwrap();
    assert!(!items.is_empty());
    assert!(body["total"].as_u64() < all["total"].as_u64());
    let heights: Vec<f64> = items
        .iter()
        .map(|i| i["body_height"].as_f64().unwrap())
        .collect();
    assert!(heights.iter().all(|&h| h <= 1.0));
    assert!(heights.windows(2).all(|w| w[0] >= w[1]), "tallest first");
    // The height facet still spans every height (its own range is not applied).
    assert_eq!(body["facets"]["body_height"], all["facets"]["body_height"]);
    // Ranges compose with the other filters.
    let tiny = catalog("category=resistor&width_max=0.6&length_max=1.1&limit=500").await;
    assert!(tiny["total"].as_u64() > Some(0));
    for it in tiny["items"].as_array().unwrap() {
        assert!(it["body_width"].as_f64().unwrap() <= 0.6);
        assert!(it["body_length"].as_f64().unwrap() <= 1.1);
    }
}

#[tokio::test]
async fn catalog_connectors_by_pitch_paged() {
    let query = "category=connector&pitch=2.54&mounting=tht&sort=pins&limit=5";
    let body = catalog(query).await;
    let total = body["total"].as_u64().unwrap();
    assert!(total > 5);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 5);
    let pins: Vec<u64> = items
        .iter()
        .map(|i| i["lead_count"].as_u64().unwrap())
        .collect();
    assert!(pins.windows(2).all(|w| w[0] <= w[1]));
    let mut seen = 0;
    let mut offset = 0;
    while offset < total {
        let page = catalog(&format!("{query}&offset={offset}")).await;
        for it in page["items"].as_array().unwrap() {
            assert_eq!(it["pitch"].as_f64(), Some(2.54));
            seen += 1;
        }
        offset += 5;
    }
    assert_eq!(seen, total);
    // Text narrows too, through the search index.
    let q = catalog("q=RP2040&limit=5").await;
    assert_eq!(q["items"][0]["mpn"], "RP2040");
}

fn param_facet<'a>(body: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    body["facets"]["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"] == key)
}

fn mpns(body: &serde_json::Value) -> Vec<&str> {
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| it["mpn"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn catalog_filters_and_facets_electrical_parameters() {
    // A range typed with prefixes, in SI underneath.
    let body =
        catalog("category=capacitor&capacitance_min=10n&capacitance_max=0.2u&limit=500").await;
    assert!(mpns(&body).contains(&"GRM155R71C104KA88D"), "{body}");
    for it in body["items"].as_array().unwrap() {
        let c = it["parameters"]["capacitance"]["value"].as_f64().unwrap();
        assert!((1e-8..=2e-7).contains(&c), "{it}");
    }
    let item = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|it| it["mpn"] == "GRM155R71C104KA88D")
        .unwrap();
    assert_eq!(item["parameters"]["capacitance"]["label"], "100 nF");
    assert_eq!(item["parameters"]["dielectric"]["label"], "X7R");
    assert_eq!(item["parameters"]["tolerance"]["label"], "±10%");

    // Discrete values, picked by any spelling of the value.
    for v in ["100nF", "0.1u", "100n"] {
        let body = catalog(&format!("capacitance={v}&limit=500")).await;
        assert!(mpns(&body).contains(&"GRM155R71C104KA88D"), "{v}");
        let f = param_facet(&body, "capacitance").unwrap();
        let picked: Vec<_> = f["values"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["selected"] == true)
            .collect();
        assert_eq!(picked.len(), 1, "{v}: {f}");
        assert_eq!(picked[0]["label"], "100 nF");
    }

    // Facets show only keys some result has; a facet's counts ignore
    // its own filter, like every other facet.
    let caps = catalog("category=capacitor&limit=0").await;
    let dielectric = param_facet(&caps, "dielectric").unwrap();
    assert_eq!(dielectric["kind"], "enumerated");
    let x7r = dielectric["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["value"] == "X7R")
        .unwrap()["count"]
        .as_u64()
        .unwrap();
    let picked = catalog("category=capacitor&dielectric=x7r&limit=0").await;
    assert_eq!(picked["total"].as_u64().unwrap(), x7r);
    assert_eq!(
        param_facet(&picked, "dielectric").unwrap()["values"]
            .as_array()
            .unwrap()
            .clone(),
        dielectric["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| {
                let mut o = o.clone();
                o["selected"] = (o["value"] == "X7R").into();
                o
            })
            .collect::<Vec<_>>()
    );
    let resistors = catalog("category=resistor&limit=0").await;
    assert!(param_facet(&resistors, "capacitance").is_none());
    assert!(param_facet(&resistors, "resistance").is_some());

    // Sorting by a parameter puts the parts that state it first.
    let body = catalog("category=resistor&sort=resistance&limit=3").await;
    let values: Vec<f64> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| it["parameters"]["resistance"]["value"].as_f64().unwrap())
        .collect();
    assert!(values.windows(2).all(|w| w[0] <= w[1]), "{values:?}");
}

#[tokio::test]
async fn catalog_category_tree_counts_subtrees() {
    let all = catalog("limit=0").await;
    let tree = all["facets"]["category_tree"].as_array().unwrap();
    let total = all["total"].as_u64().unwrap();
    let count = |n: &serde_json::Value| n["count"].as_u64().unwrap();
    // Every part sits in exactly one top-level node.
    assert_eq!(tree.iter().map(count).sum::<u64>(), total);
    let node = |v: &str| tree.iter().find(|n| n["value"] == v).unwrap().clone();
    let power = node("power");
    let children = power["children"].as_array().unwrap();
    assert!(children.iter().any(|c| c["value"] == "linear_regulator"));
    assert!(children.iter().any(|c| c["value"] == "switching_regulator"));
    // A group counts its subcategories (and parts placed on it).
    assert!(count(&power) >= children.iter().map(count).sum::<u64>());

    // Picking a group selects its subcategories.
    let picked = catalog("category=power&limit=500").await;
    assert_eq!(picked["total"].as_u64().unwrap(), count(&power));
    for it in picked["items"].as_array().unwrap() {
        let c = it["category"].as_str().unwrap();
        assert!(
            c == "power" || children.iter().any(|n| n["value"] == c),
            "{c}"
        );
    }
    let tree = picked["facets"]["category_tree"].as_array().unwrap();
    assert!(tree
        .iter()
        .any(|n| n["value"] == "power" && n["selected"] == true));
    // The tree's counts ignore the category pick, like every facet.
    assert_eq!(tree.iter().map(count).sum::<u64>(), total);

    let switches = catalog("category=tact_switch&limit=500").await;
    assert!(switches["total"].as_u64().unwrap() > 0);
    assert!(switches["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|it| it["category"] == "tact_switch"));
}

#[tokio::test]
async fn search_finds_subcategories_by_japanese_and_shorthand_names() {
    let hits = search_mpns("タクトスイッチ").await;
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|(_, c)| c == "tact_switch"), "{hits:?}");
    let hits = search_mpns("LDO").await;
    assert!(hits.iter().any(|(_, c)| c == "linear_regulator"));
    // Category matches rank above pin-name matches (an MCU's `VLDO` pin).
    let regulators = hits.iter().filter(|(_, c)| c == "linear_regulator").count();
    assert!(
        hits[..regulators]
            .iter()
            .all(|(_, c)| c == "linear_regulator"),
        "{hits:?}"
    );
    let hits = search_mpns("オペアンプ").await;
    assert!(!hits.is_empty() && hits.iter().all(|(_, c)| c == "op_amp"));
    let hits = search_mpns("レギュレータ").await;
    assert!(hits.iter().any(|(_, c)| c == "switching_regulator"));
    let response = get(app(), "/v1/search?category=memory").await;
    let body: serde_json::Value = serde_json::from_str(&body_string(response).await).unwrap();
    let cats: std::collections::BTreeSet<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["category"].as_str().unwrap())
        .collect();
    assert!(
        cats.contains("flash") && cats.contains("eeprom"),
        "{cats:?}"
    );
}

#[tokio::test]
async fn free_text_matches_formatted_parameter_values() {
    for q in ["100nF", "0.1uF", "100 nF"] {
        let body = catalog(&format!("q={}&limit=500", q.replace(' ', "%20"))).await;
        assert!(mpns(&body).contains(&"GRM155R71C104KA88D"), "{q}");
    }
    let body = catalog("q=10k&category=resistor&limit=500").await;
    assert!(mpns(&body).contains(&"RC0201FR-0710KL"));
}

#[tokio::test]
async fn catalog_rejects_bad_numbers() {
    let response = get(app(), "/v1/catalog?width_min=wide").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = get(app(), "/v1/catalog?capacitance_min=10V").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = get(app(), "/v1/catalog?sort=price").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

async fn placement(uri: &str) -> serde_json::Value {
    let response = get(app(), uri).await;
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    serde_json::from_str(&body_string(response).await).unwrap()
}

/// The footprint's `OpenParts placement` property.
fn footprint_placement_hash(fp: &str) -> Option<String> {
    let marker = "(property \"OpenParts placement\" \"";
    let start = fp.find(marker)? + marker.len();
    Some(fp[start..start + fp[start..].find('"')?].to_string())
}

#[tokio::test]
async fn placement_model_of_the_rp2040() {
    let m = placement("/v1/parts/raspberrypi/RP2040/placement").await;
    assert_eq!(m["frame"]["units"], "mm");
    assert_eq!(m["frame"]["density"], "N");
    assert_eq!(m["frame"]["zero_orientation"]["rule"], "ipc7351b_level_a");
    let pads = m["pads"].as_array().unwrap();
    assert_eq!(pads.len(), 57);
    let pad = |n: &str| pads.iter().find(|p| p["number"] == n).unwrap();
    // Pin 1 upper left, with its device pin.
    assert!(pad("1")["position"]["x"].as_f64().unwrap() < 0.0);
    assert!(pad("1")["position"]["y"].as_f64().unwrap() > 0.0);
    assert_eq!(pad("1")["pin"]["name"], "IOVDD");
    assert_eq!(pad("EP")["exposed"], true);
    assert_eq!(pad("EP")["position"]["x"], 0.0);
    assert_eq!(m["pitch"], 0.4);
    assert_eq!(m["courtyard"]["excess"], 0.25);
    assert_eq!(m["basis"]["courtyard"]["kind"], "ipc7351");
    assert_eq!(m["basis"]["pads"]["kind"], "ipc7351");
    // The KiCad footprint carries the same hash.
    let hash = m["hash"].as_str().unwrap();
    assert!(hash.starts_with("sha256:"));
    let fp = body_string(
        get(
            app(),
            "/v1/parts/raspberrypi/RP2040/artifacts/kicad-footprint",
        )
        .await,
    )
    .await;
    assert_eq!(footprint_placement_hash(&fp).as_deref(), Some(hash));
}

#[tokio::test]
async fn placement_density_levels() {
    let width = |m: &serde_json::Value| {
        let xs: Vec<f64> = m["courtyard"]["polygons"][0]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["x"].as_f64().unwrap())
            .collect();
        xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min)
    };
    let base = "/v1/parts/raspberrypi/RP2040/placement";
    let l = placement(&format!("{base}?density=L")).await;
    let n = placement(&format!("{base}?density=n")).await;
    let m = placement(&format!("{base}?density=M")).await;
    assert_eq!(l["frame"]["density"], "L");
    assert_eq!(m["courtyard"]["excess"], 0.5);
    assert!(width(&l) < width(&n) && width(&n) < width(&m));
    assert_ne!(l["hash"], n["hash"]);
    let bad = get(app(), &format!("{base}?density=X")).await;
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
    let missing = get(app(), "/v1/parts/nobody/NOPE/placement").await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn placement_of_a_usb_c_receptacle() {
    let m = placement("/v1/parts/gct/USB4105-GF-A/placement").await;
    // A1 upper left: the opening faces -Y, the 3D model turned 180°.
    assert_eq!(m["model3d"]["rotation"], 180.0);
    let pads = m["pads"].as_array().unwrap();
    let a1 = pads.iter().find(|p| p["number"] == "A1").unwrap();
    assert!(a1["position"]["x"].as_f64().unwrap() < 0.0);
    // Contacts overlaid under a second number (A1 B12).
    assert!(pads
        .iter()
        .any(|p| p["number"] == "B12" && p["shares_contact_with"] == "A1"));
    assert!(m["tht"].is_object());
    // The PCB edge the drawing states, where the dataset records it.
    let package = body_string(get(app(), "/v1/packages/manufacturers/gct/USB4105").await).await;
    if package.contains("\"edge\"") {
        assert_eq!(m["edge"]["side"], "-y");
        assert!(m["edge"]["overhang"].as_f64().unwrap().abs() < 0.01);
        assert_eq!(m["basis"]["edge"]["kind"], "datasheet");
    }
}

/// The Pico board mistake: a 2x10 header where a 1x20 was meant. The
/// structure facets and the free-text forms tell them apart.
#[tokio::test]
async fn catalog_structure_facets_tell_1x20_from_2x10() {
    let body = catalog("category=connector&rows=1x20&pitch=2.54&limit=100").await;
    let items = body["items"].as_array().unwrap();
    assert!(!items.is_empty());
    assert!(items.iter().all(|i| i["rows"] == "1x20"));
    assert!(items.iter().any(|i| i["mpn"] == "61302011121"));
    assert!(!items.iter().any(|i| i["mpn"] == "61302021121"));
    // Facet counts: 2x10 still offered (the rows facet ignores itself).
    assert!(count_of(&body, "rows", "2x10") > 0);
    assert!(count_of(&body, "rows", "1x20") > 0);
    let gender = facet(&body, "gender");
    assert!(gender.iter().any(|v| v["value"] == "male"));
    // `1×20` and the `layout` alias normalise to the same filter.
    let alias = catalog("category=connector&layout=1%C3%9720&pitch=2.54&limit=100").await;
    assert_eq!(alias["total"], body["total"]);
    let sockets = catalog("category=connector&gender=female&orientation=vertical&limit=200").await;
    assert!(sockets["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["gender"] == "female" && i["orientation"] == "vertical"));
    let angled = catalog("category=connector&orientation=right_angle&limit=200").await;
    assert!(angled["total"].as_u64().unwrap() > 0);
    let shrouded = catalog("shrouded=yes&limit=200").await;
    assert!(shrouded["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["package_family"] == "shrouded_header"));
    let ep = catalog("q=RP2040&exposed_pad=yes").await;
    assert_eq!(ep["items"][0]["mpn"], "RP2040");
    assert_eq!(catalog("q=RP2040&exposed_pad=no").await["total"], 0);
}

#[tokio::test]
async fn search_finds_structure_by_free_text() {
    let mpns = |hits: Vec<(String, String)>| hits.into_iter().map(|h| h.0).collect::<Vec<_>>();
    let hits = mpns(search_mpns("2.54 header 1x20").await);
    assert!(hits.contains(&"61302011121".to_string()), "{hits:?}");
    assert!(!hits.contains(&"61302021121".to_string()));
    let hits = mpns(search_mpns("2x10").await);
    assert!(hits.contains(&"61302021121".to_string()));
    assert!(!hits.contains(&"61302011121".to_string()));
    // Rows are matched whole: 1x2 is not 1x20.
    assert!(!mpns(search_mpns("1x2").await).contains(&"61302011121".to_string()));
    assert_eq!(
        mpns(search_mpns("1×20 header").await),
        mpns(search_mpns("1x20 header").await)
    );
    for (q, family) in [
        ("right angle", "right_angle_header"),
        ("ライトアングル ヘッダ", "right_angle_header"),
        ("ピンソケット", "socket"),
        ("メス 2.54", "socket"),
        ("オス 2.54", "header"),
    ] {
        let body = catalog(&format!(
            "q={}&limit=200",
            q.bytes().map(|b| format!("%{b:02X}")).collect::<String>()
        ))
        .await;
        let items = body["items"].as_array().unwrap();
        assert!(!items.is_empty(), "{q}");
        assert!(
            items.iter().any(|i| i["package_family"] == family),
            "{q}: no {family}"
        );
    }
    // Female finds no pin headers, male no sockets.
    let female = catalog("q=%E3%83%A1%E3%82%B9&limit=500").await;
    assert!(female["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["gender"] == "female"));
    let male = catalog("q=%E3%82%AA%E3%82%B9&limit=500").await;
    assert!(male["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["gender"] == "male"));
}

/// An MPN with `/` in it (Kingbright "APT1608QBC/D", in the real data)
/// is one `:mpn` segment sent as `%2F`: every part route finds it, at
/// `/` and under `/api`, and the artifact index links stay encoded.
#[tokio::test]
async fn an_mpn_containing_a_slash_is_one_percent_encoded_segment() {
    let base = "/v1/parts/kingbright/APT1608QBC%2FD";
    let app = app();
    let response = get(app.clone(), base).await;
    assert_eq!(response.status(), StatusCode::OK);
    let part: serde_json::Value = serde_json::from_str(&body_string(response).await).unwrap();
    assert_eq!(part["mpn"], "APT1608QBC/D");

    for sub in [
        "/sources",
        "/provenance",
        "/revisions",
        "/placement",
        "/documents",
        "/alternatives",
        "/artifacts/kicad-symbol",
        "/artifacts/kicad-footprint",
        "/artifacts/footprint-svg",
    ] {
        let response = get(app.clone(), &format!("{base}{sub}")).await;
        assert_eq!(response.status(), StatusCode::OK, "{base}{sub}");
    }
    let response = get(app.clone(), &format!("{base}/artifacts")).await;
    let index: serde_json::Value = serde_json::from_str(&body_string(response).await).unwrap();
    assert_eq!(
        index["kicad-footprint"],
        "/v1/parts/kingbright/APT1608QBC%2FD/artifacts/kicad-footprint"
    );
    // The link it gives works as is.
    let link = index["kicad-symbol"].as_str().unwrap().to_string();
    assert_eq!(get(app, &link).await.status(), StatusCode::OK);

    // The deployed app (the API under /api too).
    let store = openparts_server::store::discover_and_load(&data_dir()).unwrap();
    let deployed = openparts_server::app::build_app(Arc::new(store), None);
    let response = get(deployed.clone(), &format!("/api{base}")).await;
    assert_eq!(response.status(), StatusCode::OK);
    // An unencoded `/` is a different (unknown) route.
    let response = get(deployed, "/api/v1/parts/kingbright/APT1608QBC/D").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reference_circuits_are_served_per_part_and_by_id() {
    let store = openparts_server::store::discover_and_load(&data_dir()).unwrap();
    if store.dataset.circuits.is_empty() {
        return; // a dataset without circuits/ (before they landed)
    }
    let app = openparts_server::routes::build_router(Arc::new(store));
    let response = get(app.clone(), "/v1/parts/raspberrypi/RP2040/circuits").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    let list: serde_json::Value = serde_json::from_str(&body).unwrap();
    let list = list.as_array().unwrap();
    let ids: Vec<&str> = list.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"raspberrypi/rp2040-power"), "{ids:?}");
    // Building blocks before the composite that includes them.
    assert_eq!(ids.last(), Some(&"raspberrypi/rp2040-minimal"));
    let power = list
        .iter()
        .find(|c| c["id"] == "raspberrypi/rp2040-power")
        .unwrap();
    assert_eq!(power["reviewed"], false);
    let iovdd = power["roles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["role"] == "C_IOVDD")
        .unwrap();
    assert_eq!(iovdd["count"], 6);
    assert_eq!(iovdd["value"], "100 nF");
    assert_eq!(iovdd["level"], "recommended");
    assert_eq!(iovdd["cite"]["page"], 152);
    assert!(power["required"].as_u64().unwrap() >= 2);
    // The crystal's load capacitance: computed and stated, no verdict.
    assert!(body.contains("\"computed\":\"10.5 pF\""), "{body}");

    // A generic circuit attaches to any matching receptacle.
    let response = get(app.clone(), "/v1/parts/gct/USB4105-GF-A/circuits").await;
    let body = body_string(response).await;
    assert!(body.contains("usb-if/typec-usb2-sink"), "{body}");

    // The document itself, for the board tool.
    let response = get(app.clone(), "/v1/circuits/raspberrypi/rp2040-xosc-12mhz").await;
    assert_eq!(response.status(), StatusCode::OK);
    let doc: serde_json::Value = serde_json::from_str(&body_string(response).await).unwrap();
    assert_eq!(
        doc["roles"]["X1"]["requires"]["part"],
        "abracon/ABM8-272-T3"
    );
    assert_eq!(
        doc["roles"]["C_XIN"]["requires"]["capacitance"]["unit"],
        "pF"
    );
    assert!(doc["provenance"]["/roles/X1/level"].is_array());
    let response = get(app.clone(), "/v1/circuits/raspberrypi/nope").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = get(app, "/v1/circuits").await;
    let all: serde_json::Value = serde_json::from_str(&body_string(response).await).unwrap();
    assert!(all.as_array().unwrap().len() >= 8);
}
