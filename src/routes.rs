//! Versioned REST API (Architecture Specification section 46-49).
//! Canonical Data editing is intentionally out of scope for this crate
//! (section 4.8 / section 20: "Canonical Data編集機能はServerの責務に
//!含めない") -- every route here is read-only.

use crate::error::ApiError;
use crate::store::Store;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use openparts_core::{Device, Package, Part, ProvenanceMap, SourceId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::sync::Arc;

pub fn build_router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/search", get(search))
        .route("/v1/parts/:manufacturer/:mpn", get(get_part))
        .route(
            "/v1/parts/:manufacturer/:mpn/provenance",
            get(get_provenance),
        )
        .route("/v1/parts/:manufacturer/:mpn/revisions", get(get_revisions))
        .route(
            "/v1/parts/:manufacturer/:mpn/artifacts",
            get(list_artifacts),
        )
        .route(
            "/v1/parts/:manufacturer/:mpn/artifacts/kicad-symbol",
            get(artifact_kicad_symbol),
        )
        .route(
            "/v1/parts/:manufacturer/:mpn/artifacts/kicad-footprint",
            get(artifact_kicad_footprint),
        )
        .route(
            "/v1/parts/:manufacturer/:mpn/artifacts/step",
            get(artifact_step),
        )
        .route(
            "/v1/parts/:manufacturer/:mpn/artifacts/stl",
            get(artifact_stl),
        )
        .route(
            "/v1/parts/:manufacturer/:mpn/artifacts/3mf",
            get(artifact_3mf),
        )
        .route("/v1/devices/*id", get(get_device))
        .route("/v1/packages/*id", get(get_package))
        .with_state(store)
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
    category: Option<openparts_core::ComponentCategory>,
}

#[derive(Serialize)]
struct PartSummary {
    manufacturer: String,
    mpn: String,
    existence: openparts_core::ExistenceStatus,
    lifecycle: openparts_core::LifecycleStatus,
    // The Part itself doesn't carry category (Canonical Data
    // Specification section 35: don't duplicate an engineering fact
    // that already lives on the referenced Device) -- resolved here so
    // clients can filter/browse without a second fetch per part. None
    // only if the Part's device reference doesn't resolve, which
    // openparts-validator should already prevent for valid data.
    category: Option<openparts_core::ComponentCategory>,
}

/// `GET /v1/search?q=...&category=...`
async fn search(
    State(store): State<Arc<Store>>,
    Query(params): Query<SearchQuery>,
) -> Json<Vec<PartSummary>> {
    let needle = params.q.unwrap_or_default().to_lowercase();
    let mut results: Vec<PartSummary> = store
        .parts
        .values()
        .filter(|p| {
            needle.is_empty()
                || p.mpn.to_lowercase().contains(&needle)
                || p.manufacturer.0.to_lowercase().contains(&needle)
        })
        .map(|p| PartSummary {
            manufacturer: p.manufacturer.0.clone(),
            mpn: p.mpn.clone(),
            existence: p.existence.status,
            lifecycle: p.lifecycle.status,
            category: store.devices.get(&p.device.0).map(|d| d.category),
        })
        .filter(|p| match params.category {
            None => true,
            Some(wanted) => p.category == Some(wanted),
        })
        .collect();
    results.sort_by(|a, b| (&a.manufacturer, &a.mpn).cmp(&(&b.manufacturer, &b.mpn)));
    Json(results)
}

fn resolve_part<'s>(store: &'s Store, manufacturer: &str, mpn: &str) -> Result<&'s Part, ApiError> {
    store
        .parts
        .get(&(manufacturer.to_string(), mpn.to_string()))
        .ok_or_else(|| ApiError::NotFound(format!("no part {manufacturer}/{mpn}")))
}

fn resolve_device<'s>(store: &'s Store, id: &str) -> Result<&'s Device, ApiError> {
    store
        .devices
        .get(id)
        .ok_or_else(|| ApiError::NotFound(format!("no device {id}")))
}

fn resolve_package<'s>(store: &'s Store, id: &str) -> Result<&'s Package, ApiError> {
    store
        .packages
        .get(id)
        .ok_or_else(|| ApiError::NotFound(format!("no package {id}")))
}

/// `GET /v1/parts/{manufacturer}/{mpn}`
async fn get_part(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<Part>, ApiError> {
    Ok(Json(resolve_part(&store, &manufacturer, &mpn)?.clone()))
}

/// `GET /v1/devices/{id}` (`id` may itself contain `/`, e.g.
/// `raspberrypi/RP2040` -- captured as a wildcard tail segment).
async fn get_device(
    State(store): State<Arc<Store>>,
    Path(id): Path<String>,
) -> Result<Json<Device>, ApiError> {
    Ok(Json(resolve_device(&store, &id)?.clone()))
}

/// `GET /v1/packages/{id}`
async fn get_package(
    State(store): State<Arc<Store>>,
    Path(id): Path<String>,
) -> Result<Json<Package>, ApiError> {
    Ok(Json(resolve_package(&store, &id)?.clone()))
}

#[derive(Serialize)]
struct ProvenanceResponse {
    part: ProvenanceMap,
    device: ProvenanceMap,
    package: ProvenanceMap,
}

/// `GET /v1/parts/{manufacturer}/{mpn}/provenance`
async fn get_provenance(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<ProvenanceResponse>, ApiError> {
    let part = resolve_part(&store, &manufacturer, &mpn)?;
    let device = resolve_device(&store, &part.device.0)?;
    let package = resolve_package(&store, &part.package.0)?;
    Ok(Json(ProvenanceResponse {
        part: part.provenance.clone(),
        device: device.provenance.clone(),
        package: package.provenance.clone(),
    }))
}

#[derive(Serialize)]
struct RevisionSummary {
    id: String,
    manufacturer_revision: String,
}

/// `GET /v1/parts/{manufacturer}/{mpn}/revisions`
async fn get_revisions(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<Vec<RevisionSummary>>, ApiError> {
    let part = resolve_part(&store, &manufacturer, &mpn)?;
    let device = resolve_device(&store, &part.device.0)?;
    let mut revisions: Vec<RevisionSummary> = device
        .revisions
        .iter()
        .map(|(id, rev)| RevisionSummary {
            id: id.clone(),
            manufacturer_revision: rev.manufacturer_revision.clone(),
        })
        .collect();
    revisions.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Json(revisions))
}

#[derive(Serialize)]
struct ArtifactsResponse {
    #[serde(rename = "kicad-symbol")]
    kicad_symbol: String,
    #[serde(rename = "kicad-footprint")]
    kicad_footprint: String,
    step: String,
    stl: String,
    #[serde(rename = "3mf")]
    three_mf: String,
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts`
async fn list_artifacts(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<ArtifactsResponse>, ApiError> {
    resolve_part(&store, &manufacturer, &mpn)?;
    let base = format!("/v1/parts/{manufacturer}/{mpn}/artifacts");
    Ok(Json(ArtifactsResponse {
        kicad_symbol: format!("{base}/kicad-symbol"),
        kicad_footprint: format!("{base}/kicad-footprint"),
        step: format!("{base}/step"),
        stl: format!("{base}/stl"),
        three_mf: format!("{base}/3mf"),
    }))
}

#[derive(Deserialize)]
struct RevisionQuery {
    silicon_revision: Option<String>,
}

/// Loads and validates the Part/Device/Package triple, then builds the
/// Effective Model for the requested (or base) revision. Shared by all
/// three artifact endpoints (Architecture Specification section 51:
/// Request -> Part Resolution -> Effective Model -> Domain IR -> Format
/// Adapter -> Artifact).
fn build_model(
    store: &Store,
    manufacturer: &str,
    mpn: &str,
    revision: Option<&str>,
) -> Result<openparts_core::EffectiveModel, ApiError> {
    let part = resolve_part(store, manufacturer, mpn)?.clone();
    let device = resolve_device(store, &part.device.0)?.clone();
    let package = resolve_package(store, &part.package.0)?.clone();

    let known_sources: BTreeSet<SourceId> = store
        .sources
        .keys()
        .map(|id| SourceId(id.clone()))
        .collect();
    let diags = openparts_validator::validate_part(&part, &device, &package, &known_sources);
    if !diags.is_empty() {
        return Err(ApiError::ValidationFailed(diags));
    }

    openparts_core::build_effective_model(part, &device, package, revision)
        .map_err(|e| ApiError::GenerationFailed(e.to_string()))
}

fn artifact_response(content: Vec<u8>, content_type: &str) -> Response {
    let hash = sha256_hex(&content);
    let mut response = (StatusCode::OK, content).into_response();
    if let Ok(value) = header::HeaderValue::from_str(content_type) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    if let Ok(value) = header::HeaderValue::from_str(&format!("sha256:{hash}")) {
        response.headers_mut().insert("x-content-hash", value);
    }
    response
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The prebuilt `kind` artifact from the bundle (`OPENPARTS_ARTIFACT_DIR`),
/// if there is one and it reads back intact.
async fn bundled(store: &Store, manufacturer: &str, mpn: &str, kind: &str) -> Option<Response> {
    let bundle = store.bundle.as_ref()?;
    let artifact = bundle.get(manufacturer, mpn, kind)?;
    let bytes = bundle.read(artifact).await?;
    Some(artifact_response(bytes, &artifact.content_type))
}

const KICAD_CONTENT_TYPE: &str = "text/plain; charset=utf-8";

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts/kicad-symbol?silicon_revision=...`
///
/// The bundle holds the base revision's symbol; a `silicon_revision`
/// changes pins, so that is always generated live (no SolidCraft needed).
async fn artifact_kicad_symbol(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
    Query(q): Query<RevisionQuery>,
) -> Result<Response, ApiError> {
    let model = build_model(&store, &manufacturer, &mpn, q.silicon_revision.as_deref())?;
    if q.silicon_revision.is_none() {
        if let Some(response) = bundled(&store, &manufacturer, &mpn, "kicad-symbol").await {
            return Ok(response);
        }
    }
    let symbol = openparts_pcbcad::build_symbol(&model.part.mpn, &model.device);
    let text = openparts_kicad::render_symbol(&symbol);
    Ok(artifact_response(text.into_bytes(), KICAD_CONTENT_TYPE))
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts/kicad-footprint?silicon_revision=...`
///
/// A footprint depends on the package only, so the bundled one serves
/// every revision.
async fn artifact_kicad_footprint(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
    Query(q): Query<RevisionQuery>,
) -> Result<Response, ApiError> {
    let model = build_model(&store, &manufacturer, &mpn, q.silicon_revision.as_deref())?;
    if let Some(response) = bundled(&store, &manufacturer, &mpn, "kicad-footprint").await {
        return Ok(response);
    }
    let geometry = openparts_mcad::generate(&model.package)
        .map_err(|e| ApiError::GenerationFailed(e.to_string()))?;
    let footprint = openparts_pcbcad::build_footprint(&model.part.mpn, &geometry);
    let text = openparts_kicad::render_footprint(&footprint);
    Ok(artifact_response(text.into_bytes(), KICAD_CONTENT_TYPE))
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts/step?silicon_revision=...`
/// (coloured AP242)
async fn artifact_step(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
    Query(q): Query<RevisionQuery>,
) -> Result<Response, ApiError> {
    serve_3d(&store, &manufacturer, &mpn, &q, Model3d::STEP).await
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts/stl?silicon_revision=...`
/// (binary STL)
async fn artifact_stl(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
    Query(q): Query<RevisionQuery>,
) -> Result<Response, ApiError> {
    serve_3d(&store, &manufacturer, &mpn, &q, Model3d::STL).await
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts/3mf?silicon_revision=...`
/// (3MF with a colour per body)
async fn artifact_3mf(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
    Query(q): Query<RevisionQuery>,
) -> Result<Response, ApiError> {
    serve_3d(&store, &manufacturer, &mpn, &q, Model3d::THREE_MF).await
}

/// A 3D artifact kind: its bundle key, SolidCraft export and media type.
#[derive(Clone, Copy)]
struct Model3d {
    kind: &'static str,
    format: openparts_solidcraft::Format,
    content_type: &'static str,
}

impl Model3d {
    const STEP: Model3d = Model3d {
        kind: "step",
        format: openparts_solidcraft::Format::Step,
        content_type: "model/step",
    };
    const STL: Model3d = Model3d {
        kind: "stl",
        format: openparts_solidcraft::Format::Stl,
        content_type: "model/stl",
    };
    const THREE_MF: Model3d = Model3d {
        kind: "3mf",
        format: openparts_solidcraft::Format::ThreeMf,
        content_type: "model/3mf",
    };
}

/// Serves a 3D model: from the bundle when it has one, else built live
/// by SolidCraft when this server has it, else `503`.
///
/// 3D models depend on the package only, so the bundled (base revision)
/// model serves every `silicon_revision`; the revision is still checked,
/// so an unknown one is `422` either way.
async fn serve_3d(
    store: &Store,
    manufacturer: &str,
    mpn: &str,
    q: &RevisionQuery,
    model3d: Model3d,
) -> Result<Response, ApiError> {
    let model = build_model(store, manufacturer, mpn, q.silicon_revision.as_deref())?;
    if let Some(response) = bundled(store, manufacturer, mpn, model3d.kind).await {
        return Ok(response);
    }
    let bytes = render_3d(model, model3d, store.bundle.is_some()).await?;
    Ok(artifact_response(bytes, model3d.content_type))
}

/// Builds the detailed 3D model of `model` live. The model is built by
/// an external SolidCraft process, so it runs on the blocking pool
/// instead of holding up an async worker.
async fn render_3d(
    model: openparts_core::EffectiveModel,
    model3d: Model3d,
    have_bundle: bool,
) -> Result<Vec<u8>, ApiError> {
    tokio::task::spawn_blocking(move || {
        if !openparts_solidcraft::available() {
            let why = if have_bundle {
                "the artifact bundle has no"
            } else {
                "no artifact bundle is configured (OPENPARTS_ARTIFACT_DIR) with a"
            };
            return Err(ApiError::Unavailable(format!(
                "{why} {} model for {}/{}, and SolidCraft is not installed on this server to build one",
                model3d.kind, model.part.manufacturer, model.part.mpn
            )));
        }
        let geometry = openparts_mcad::generate(&model.package)
            .map_err(|e| ApiError::GenerationFailed(e.to_string()))?;
        let input = openparts_solidcraft::ModelInput {
            package: &model.package,
            geometry: &geometry,
            category: Some(model.device.category),
        };
        let mut files = openparts_solidcraft::generate(&input, &model.part.mpn, &[model3d.format])
            .map_err(|e| ApiError::GenerationFailed(e.to_string()))?;
        Ok(files.remove(0))
    })
    .await
    .map_err(|e| ApiError::GenerationFailed(e.to_string()))?
}
