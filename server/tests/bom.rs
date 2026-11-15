//! `POST /v1/bom/check` and `GET /v1/parts/{m}/{mpn}/alternatives`
//! against the real openparts-data, through the router.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tower::ServiceExt;

fn app() -> axum::Router {
    static STORE: OnceLock<Arc<openparts_server::store::Store>> = OnceLock::new();
    let store = STORE.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../openparts-data");
        Arc::new(openparts_server::store::discover_and_load(&dir).unwrap())
    });
    openparts_server::app::build_app(store.clone(), None)
}

async fn post(path: &str, body: Vec<u8>) -> (StatusCode, serde_json::Value) {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "text/csv")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn get(path: &str) -> (StatusCode, serde_json::Value) {
    let response = app()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../crates/openparts-bom/tests/fixtures")
            .join(name),
    )
    .unwrap()
}

#[tokio::test]
async fn checks_the_pico_bom() {
    for (path, name, format) in [
        ("/v1/bom/check", "pico-kicad-bom.csv", "kicad_csv"),
        ("/api/v1/bom/check", "pico.net", "kicad_netlist"),
        ("/v1/bom/check", "pico-generic.csv", "generic_csv"),
    ] {
        let (status, r) = post(path, fixture(name)).await;
        assert_eq!(status, StatusCode::OK, "{name}: {r}");
        assert_eq!(r["format"], format);
        assert_eq!(r["summary"]["components"], 33, "{name}");
        assert_eq!(r["summary"]["not_found"], 0, "{name}");
        let lines = r["lines"].as_array().unwrap();
        assert!(lines.iter().all(|l| l["kind"] == "exact"), "{name}");
        let total: u64 = ["ok", "review", "warning", "not_found"]
            .iter()
            .map(|k| r["summary"][k].as_u64().unwrap())
            .sum();
        assert_eq!(total as usize, lines.len());
    }
}

#[tokio::test]
async fn bad_input_is_400() {
    for body in [
        b"".to_vec(),
        b"just some words\nand more words\n".to_vec(),
        vec![0x89, b'P', b'N', b'G', 0, 0, 0, 1],
        b"(export (version \"E\") (components (comp (ref \"R1\")".to_vec(),
        b"\"MPN\",\"Qty\"\n\"open quote,1\n".to_vec(),
    ] {
        let (status, r) = post("/v1/bom/check", body.clone()).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{:?}",
            String::from_utf8_lossy(&body)
        );
        assert!(r["error"].is_string());
    }
}

#[tokio::test]
async fn too_many_rows_is_400() {
    let mut csv = String::from("MPN,Qty\n");
    for _ in 0..=openparts_bom::MAX_ROWS {
        csv.push_str("RC0603FR-0710KL,1\n");
    }
    let (status, r) = post("/v1/bom/check", csv.into_bytes()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(r["error"].as_str().unwrap().contains("rows"));
    // The limit itself is fine.
    let mut csv = String::from("MPN,Qty\n");
    for _ in 0..openparts_bom::MAX_ROWS {
        csv.push_str("RC0603FR-0710KL,1\n");
    }
    let (status, r) = post("/v1/bom/check", csv.into_bytes()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(r["summary"]["lines"], openparts_bom::MAX_ROWS);
}

#[tokio::test]
async fn too_large_is_413() {
    let mut body = b"MPN,Description\n".to_vec();
    while body.len() <= openparts_bom::MAX_BYTES {
        body.extend_from_slice(b"X,");
        body.extend(std::iter::repeat_n(b'a', 1000));
        body.push(b'\n');
    }
    let (status, _) = post("/v1/bom/check", body).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn alternatives_for_a_0603_cap() {
    let (status, r) = get("/v1/parts/samsung/CL10B104KB8NNNC/alternatives").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(r["part"]["mpn"], "CL10B104KB8NNNC");
    assert!(r["drop_in"].as_u64().unwrap() >= 3);
    let first = &r["alternatives"][0];
    assert_eq!(first["class"], "drop_in");
    assert_eq!(first["part"]["size_code"], "0603");
    assert_eq!(first["pins"]["compatible"], true);
    assert!(first["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["key"] == "capacitance" && c["status"] == "pass"));
    let (status, _) = get("/v1/parts/nobody/NOPE/alternatives").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn alternatives_show_the_pin_diff() {
    let (status, r) = get("/api/v1/parts/ti/LM1117MPX-3.3%2FNOPB/alternatives").await;
    assert_eq!(status, StatusCode::OK, "{r}");
    let ua = r["alternatives"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["part"]["mpn"] == "UA78M33CDCYR")
        .unwrap();
    assert_eq!(ua["class"], "footprint_compatible");
    let pads = ua["pins"]["pads"].as_array().unwrap();
    assert_eq!(pads[0]["original"]["name"], "ADJ/GND");
    assert_eq!(pads[0]["candidate"]["name"], "INPUT");
    assert_eq!(pads[0]["status"], "differs");
}
