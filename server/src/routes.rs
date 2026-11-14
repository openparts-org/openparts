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
use openparts_artifacts::{ArtifactKind, BuildError, PartBuild};
use openparts_core::{Device, Package, Part, ProvenanceMap};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields = nullable))]
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
    let ds = &store.dataset;
    let mut results: Vec<PartSummary> = ds
        .parts
        .iter()
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
            category: ds.devices.get(&p.device.0).map(|d| d.category),
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
        .dataset
        .part(manufacturer, mpn)
        .ok_or_else(|| ApiError::NotFound(format!("no part {manufacturer}/{mpn}")))
}

fn resolve_device<'s>(store: &'s Store, id: &str) -> Result<&'s Device, ApiError> {
    store
        .dataset
        .devices
        .get(id)
        .ok_or_else(|| ApiError::NotFound(format!("no device {id}")))
}

fn resolve_package<'s>(store: &'s Store, id: &str) -> Result<&'s Package, ApiError> {
    store
        .dataset
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

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields = nullable))]
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

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields = nullable))]
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

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields = nullable))]
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

impl From<BuildError> for ApiError {
    fn from(e: BuildError) -> Self {
        match e {
            BuildError::Resolve(e) => ApiError::NotFound(e.to_string()),
            BuildError::Validation(diags) => ApiError::ValidationFailed(diags),
            other => ApiError::GenerationFailed(other.to_string()),
        }
    }
}

/// Validates the part and builds its Effective Model for the requested
/// (or base) revision (Architecture Specification section 51: Request ->
/// Part Resolution -> Effective Model -> Domain IR -> Format Adapter ->
/// Artifact).
fn build_part(
    store: &Store,
    manufacturer: &str,
    mpn: &str,
    revision: Option<&str>,
) -> Result<PartBuild, ApiError> {
    let part = resolve_part(store, manufacturer, mpn)?;
    Ok(PartBuild::from_dataset(&store.dataset, part, revision)?)
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
async fn bundled(
    store: &Store,
    manufacturer: &str,
    mpn: &str,
    kind: ArtifactKind,
) -> Option<Response> {
    let bundle = store.bundle.as_ref()?;
    let artifact = bundle.get(manufacturer, mpn, kind.id())?;
    let bytes = bundle.read(artifact).await?;
    Some(artifact_response(bytes, &artifact.content_type))
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts/{kind}?silicon_revision=...`
///
/// Served from the bundle when it has the artifact, else built live (3D
/// models only where SolidCraft is installed, else `503`). The bundle
/// holds base-revision artifacts: those that depend on the revision
/// (the symbol's pins) are always built live for a `silicon_revision`;
/// the rest depend on the package only, so the bundled one serves every
/// revision. The revision is still checked, so an unknown one is `422`.
async fn artifact(
    store: Arc<Store>,
    manufacturer: String,
    mpn: String,
    q: RevisionQuery,
    kind: ArtifactKind,
) -> Result<Response, ApiError> {
    let build = build_part(&store, &manufacturer, &mpn, q.silicon_revision.as_deref())?;
    if q.silicon_revision.is_none() || !kind.depends_on_revision() {
        if let Some(response) = bundled(&store, &manufacturer, &mpn, kind).await {
            return Ok(response);
        }
    }
    if kind.model_format().is_none() {
        return Ok(artifact_response(build.render(kind)?, kind.content_type()));
    }
    let have_bundle = store.bundle.is_some();
    // SolidCraft runs as an external process: off the async workers.
    let bytes = tokio::task::spawn_blocking(move || {
        if !openparts_solidcraft::available() {
            let why = if have_bundle {
                "the artifact bundle has no"
            } else {
                "no artifact bundle is configured (OPENPARTS_ARTIFACT_DIR) with a"
            };
            return Err(ApiError::Unavailable(format!(
                "{why} {} model for {manufacturer}/{mpn}, and SolidCraft is not installed on this server to build one",
                kind.id()
            )));
        }
        Ok(build.render(kind)?)
    })
    .await
    .map_err(|e| ApiError::GenerationFailed(e.to_string()))??;
    Ok(artifact_response(bytes, kind.content_type()))
}

/// One route per artifact kind (axum paths can't share a parameter
/// segment with the fixed `/artifacts` listing).
macro_rules! artifact_route {
    ($name:ident, $kind:expr) => {
        async fn $name(
            State(store): State<Arc<Store>>,
            Path((manufacturer, mpn)): Path<(String, String)>,
            Query(q): Query<RevisionQuery>,
        ) -> Result<Response, ApiError> {
            artifact(store, manufacturer, mpn, q, $kind).await
        }
    };
}

artifact_route!(artifact_kicad_symbol, ArtifactKind::KicadSymbol);
artifact_route!(artifact_kicad_footprint, ArtifactKind::KicadFootprint);
artifact_route!(artifact_step, ArtifactKind::Step);
artifact_route!(artifact_stl, ArtifactKind::Stl);
artifact_route!(artifact_3mf, ArtifactKind::ThreeMf);
