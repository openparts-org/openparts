//! Versioned REST API (Architecture Specification section 46-49).
//! Canonical Data editing is intentionally out of scope for this crate
//! (section 4.8 / section 20: "Canonical Data編集機能はServerの責務に
//!含めない") -- every route here is read-only.

use crate::error::ApiError;
use crate::store::Store;
use axum::extract::{Path, Query, RawQuery, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use openparts_artifacts::{ArtifactKind, BuildError, PartBuild};
use openparts_core::{Device, Package, Part, ProvenanceMap, Source};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub fn build_router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/search", get(search))
        .route("/v1/catalog", get(catalog))
        .route("/v1/parts/:manufacturer/:mpn", get(get_part))
        .route(
            "/v1/parts/:manufacturer/:mpn/provenance",
            get(get_provenance),
        )
        .route("/v1/parts/:manufacturer/:mpn/revisions", get(get_revisions))
        .route("/v1/parts/:manufacturer/:mpn/sources", get(get_sources))
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
            "/v1/parts/:manufacturer/:mpn/artifacts/footprint-svg",
            get(artifact_footprint_svg),
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
        .merge(crate::bom::router())
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

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    let ds = &store.dataset;
    let query = params.q.unwrap_or_default();
    let mut hits = store.search.search(&query);
    if query.trim().is_empty() {
        hits.sort_by(|a, b| {
            let (pa, pb) = (&ds.parts[a.0], &ds.parts[b.0]);
            (&pa.manufacturer.0, &pa.mpn).cmp(&(&pb.manufacturer.0, &pb.mpn))
        });
    } else {
        // Best score first; ties by name, so results are stable.
        hits.sort_by(|a, b| {
            let (pa, pb) = (&ds.parts[a.0], &ds.parts[b.0]);
            b.1.cmp(&a.1)
                .then_with(|| (&pa.manufacturer.0, &pa.mpn).cmp(&(&pb.manufacturer.0, &pb.mpn)))
        });
    }
    let results: Vec<PartSummary> = hits
        .into_iter()
        .map(|(i, _)| &ds.parts[i])
        .map(|p| PartSummary {
            manufacturer: p.manufacturer.0.clone(),
            mpn: p.mpn.clone(),
            existence: p.existence.status,
            lifecycle: p.lifecycle.status,
            category: ds.devices.get(&p.device.0).map(|d| d.category),
        })
        .filter(|p| match params.category {
            None => true,
            // A group (`power`) takes in its subcategories.
            Some(wanted) => p.category.is_some_and(|c| c.is_within(wanted)),
        })
        .collect();
    Json(results)
}

/// `GET /v1/catalog?q=&category=&manufacturer=&family=&size=&pins=&pitch=&mounting=&lifecycle=&existence=&width_min=&width_max=&length_min=&length_max=&height_min=&height_max=&<parameter>=&<parameter>_min=&<parameter>_max=&sort=&order=&offset=&limit=`
///
/// Faceted search (`crate::catalog`): multi-select filters take
/// comma-separated values or repeat; body ranges are mm; electrical
/// parameters (`capacitance=100nF`, `resistance_max=10k`) take typed
/// values with unit prefixes. Answers
/// `{ total, offset, limit, items, facets }`.
async fn catalog(
    State(store): State<Arc<Store>>,
    RawQuery(raw): RawQuery,
) -> Result<Json<crate::catalog::CatalogResponse>, ApiError> {
    let pairs: Vec<(String, String)> = serde_urlencoded::from_str(raw.as_deref().unwrap_or(""))
        .map_err(|e| ApiError::BadRequest(format!("malformed query: {e}")))?;
    let query = crate::catalog::CatalogQuery::from_pairs(
        pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())),
    )
    .map_err(ApiError::BadRequest)?;
    let hits = store.search.search(&query.q);
    Ok(Json(store.catalog.query(&hits, &query)))
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

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize)]
struct PartSource {
    source: Source,
    /// Which documents cite it: "part", "device", "package".
    cited_by: Vec<String>,
}

/// `GET /v1/parts/{manufacturer}/{mpn}/sources`: every Source the part,
/// its device and its package cite (in `sources` or field provenance),
/// with what cites it. Unknown ids are skipped (validation reports
/// them).
async fn get_sources(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<Vec<PartSource>>, ApiError> {
    let part = resolve_part(&store, &manufacturer, &mpn)?;
    let device = resolve_device(&store, &part.device.0)?;
    let package = resolve_package(&store, &part.package.0)?;
    let mut cited: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut cite = |id: &str, by: &str| {
        let list = cited.entry(id.to_string()).or_default();
        if !list.iter().any(|b| b == by) {
            list.push(by.to_string());
        }
    };
    for id in &part.sources {
        cite(&id.0, "part");
    }
    for (by, provenance) in [
        ("part", &part.provenance),
        ("device", &device.provenance),
        ("package", &package.provenance),
    ] {
        for entry in provenance.values().flatten() {
            cite(&entry.source.0, by);
        }
    }
    let mut out: Vec<PartSource> = cited
        .into_iter()
        .filter_map(|(id, cited_by)| {
            let source = store.dataset.sources.get(&id)?.clone();
            Some(PartSource { source, cited_by })
        })
        .collect();
    out.sort_by(|a, b| {
        (&a.source.publisher, &a.source.document_id)
            .cmp(&(&b.source.publisher, &b.source.document_id))
    });
    Ok(Json(out))
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize)]
struct ArtifactsResponse {
    #[serde(rename = "kicad-symbol")]
    kicad_symbol: String,
    #[serde(rename = "kicad-footprint")]
    kicad_footprint: String,
    #[serde(rename = "footprint-svg")]
    footprint_svg: String,
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
        footprint_svg: format!("{base}/footprint-svg"),
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
artifact_route!(artifact_footprint_svg, ArtifactKind::FootprintSvg);
artifact_route!(artifact_step, ArtifactKind::Step);
artifact_route!(artifact_stl, ArtifactKind::Stl);
artifact_route!(artifact_3mf, ArtifactKind::ThreeMf);

/// Writes the JSON Schema of every document the API returns (the
/// openparts-core documents and the responses above) to
/// `$OPENPARTS_SCHEMA_OUT`, one `$defs` map -- web/scripts/gen-api-types.sh
/// turns it into the site's MoonBit types.
#[cfg(all(test, feature = "schema"))]
mod schema_export {
    use super::*;

    #[test]
    fn export_schemas() {
        let Ok(out) = std::env::var("OPENPARTS_SCHEMA_OUT") else {
            return;
        };
        let mut gen = schemars::generate::SchemaSettings::draft2020_12().into_generator();
        gen.subschema_for::<Part>();
        gen.subschema_for::<Device>();
        gen.subschema_for::<Package>();
        gen.subschema_for::<PartSummary>();
        gen.subschema_for::<crate::catalog::CatalogResponse>();
        gen.subschema_for::<PartSource>();
        gen.subschema_for::<RevisionSummary>();
        gen.subschema_for::<ArtifactsResponse>();
        gen.subschema_for::<ProvenanceResponse>();
        crate::error::subschemas(&mut gen);
        crate::bom::subschemas(&mut gen);
        let defs = serde_json::Value::Object(gen.definitions().clone());
        std::fs::write(out, serde_json::to_string_pretty(&defs).unwrap()).unwrap();
    }
}
