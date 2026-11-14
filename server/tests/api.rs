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
