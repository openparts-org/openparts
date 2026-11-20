//! Versioned REST API (Architecture Specification section 46-49).
//! Canonical Data editing is intentionally out of scope for this crate
//! (section 4.8 / section 20: "Canonical Data編集機能はServerの責務に
//!含めない") -- every route here is read-only.

use crate::error::ApiError;
use crate::store::Store;
use axum::extract::{OriginalUri, Path, Query, RawQuery, State};
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
        .route("/v1/parts/:manufacturer/:mpn/placement", get(get_placement))
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
            "/v1/parts/:manufacturer/:mpn/artifacts/symbol-svg",
            get(artifact_symbol_svg),
        )
        .route(
            "/v1/a/:sha256/:manufacturer/:mpn/:kind",
            get(hashed_artifact),
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
        .merge(crate::documents::router())
        .merge(crate::circuits::router())
        .merge(crate::sources::routes())
        .merge(crate::pcbcraft::router())
        .with_state(store)
}

async fn health() -> &'static str {
    "ok"
}

/// The dataset parts matching `query`, best first: by score (ties by
/// manufacturer and MPN, so results are stable), or every part by name
/// for an empty query.
pub(crate) fn ranked_parts(store: &Store, query: &str) -> Vec<usize> {
    let ds = &store.dataset;
    let mut hits = store.search.search(query);
    if query.trim().is_empty() {
        hits.sort_by(|a, b| {
            let (pa, pb) = (&ds.parts[a.0], &ds.parts[b.0]);
            (&pa.manufacturer.0, &pa.mpn).cmp(&(&pb.manufacturer.0, &pb.mpn))
        });
    } else {
        hits.sort_by(|a, b| {
            let (pa, pb) = (&ds.parts[a.0], &ds.parts[b.0]);
            b.1.cmp(&a.1)
                .then_with(|| (&pa.manufacturer.0, &pa.mpn).cmp(&(&pb.manufacturer.0, &pb.mpn)))
        });
    }
    hits.into_iter().map(|(i, _)| i).collect()
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
    let hits = ranked_parts(&store, &query);
    let results: Vec<PartSummary> = hits
        .into_iter()
        .map(|i| &ds.parts[i])
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

/// `GET /v1/catalog?q=&category=&manufacturer=&family=&size=&pins=&pitch=&mounting=&lifecycle=&existence=&rows=&gender=&orientation=&shrouded=&exposed_pad=&width_min=&width_max=&length_min=&length_max=&height_min=&height_max=&<parameter>=&<parameter>_min=&<parameter>_max=&sort=&order=&offset=&limit=`
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

pub(crate) fn resolve_part<'s>(
    store: &'s Store,
    manufacturer: &str,
    mpn: &str,
) -> Result<&'s Part, ApiError> {
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

/// Every artifact of a part, by kind id: its content-addressed link
/// (`/v1/a/<sha256>/...`, cacheable for good) where the content is known
/// without building a 3D model, else its plain link
/// (`/v1/parts/.../artifacts/<kind>`). Either answers the artifact.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize)]
struct ArtifactsResponse {
    #[serde(rename = "kicad-symbol")]
    kicad_symbol: String,
    #[serde(rename = "kicad-footprint")]
    kicad_footprint: String,
    #[serde(rename = "footprint-svg")]
    footprint_svg: String,
    /// The KiCad symbol drawn as SVG, every unit.
    #[serde(rename = "symbol-svg")]
    symbol_svg: String,
    step: String,
    stl: String,
    #[serde(rename = "3mf")]
    three_mf: String,
    /// The part as a `pcbcraft-library-bundle/1` with its dependency
    /// closure (crate::pcbcraft; `422` when PCBCraft can't represent it).
    pcbcraft: String,
    /// The PCBCraft symbol drawn as SVG (`422` like `pcbcraft`).
    #[serde(rename = "pcbcraft-symbol-svg")]
    pcbcraft_symbol_svg: String,
}

/// The kinds `GET /v1/a/...` serves.
const SERVED_KINDS: [&str; 9] = [
    "kicad-symbol",
    "kicad-footprint",
    "footprint-svg",
    "symbol-svg",
    "step",
    "stl",
    "3mf",
    "pcbcraft",
    "pcbcraft-symbol-svg",
];

/// `?silicon_revision=...`, or nothing.
fn revision_query(revision: Option<&str>) -> String {
    revision
        .map(|r| format!("?silicon_revision={}", encode_segment(r)))
        .unwrap_or_default()
}

/// An artifact's plain link (names the part, not the content).
pub(crate) fn plain_artifact_path(
    manufacturer: &str,
    mpn: &str,
    kind: &str,
    revision: Option<&str>,
) -> String {
    format!(
        "/v1/parts/{}/{}/artifacts/{kind}{}",
        encode_segment(manufacturer),
        encode_segment(mpn),
        revision_query(revision)
    )
}

/// An artifact's content-addressed link: what it answers never changes.
pub(crate) fn hashed_artifact_path(
    sha256: &str,
    manufacturer: &str,
    mpn: &str,
    kind: &str,
    revision: Option<&str>,
) -> String {
    format!(
        "/v1/a/{sha256}/{}/{}/{kind}{}",
        encode_segment(manufacturer),
        encode_segment(mpn),
        revision_query(revision)
    )
}

/// The response's `x-content-hash` digest (hex, without `sha256:`).
fn content_hash(response: &Response) -> Option<String> {
    let v = response.headers().get("x-content-hash")?.to_str().ok()?;
    Some(v.strip_prefix("sha256:").unwrap_or(v).to_string())
}

/// The current `kind` artifact of a part, as its plain link answers it.
pub(crate) async fn current_artifact(
    store: &Arc<Store>,
    manufacturer: &str,
    mpn: &str,
    revision: Option<&str>,
    kind: &str,
) -> Result<Response, ApiError> {
    match kind {
        "pcbcraft" => crate::pcbcraft::part_bundle(store, manufacturer, mpn),
        "pcbcraft-symbol-svg" => crate::pcbcraft::part_symbol_svg(store, manufacturer, mpn),
        id => match ArtifactKind::from_id(id).filter(|_| SERVED_KINDS.contains(&id)) {
            Some(kind) => {
                artifact(
                    store.clone(),
                    manufacturer.to_string(),
                    mpn.to_string(),
                    RevisionQuery {
                        silicon_revision: revision.map(str::to_string),
                    },
                    kind,
                )
                .await
            }
            None => Err(ApiError::NotFound(format!("no artifact kind {id}"))),
        },
    }
}

/// The digest of the current `kind` artifact, when it is known without
/// building a 3D model (a model's comes from the bundle manifest only).
/// Remembered: nothing it depends on changes while the server runs.
async fn artifact_hash(
    store: &Arc<Store>,
    manufacturer: &str,
    mpn: &str,
    revision: Option<&str>,
    kind: &str,
) -> Option<String> {
    let key = format!(
        "{manufacturer}\0{mpn}\0{kind}\0{}",
        revision.unwrap_or_default()
    );
    if let Some(h) = store.artifact_hashes.lock().ok()?.get(&key) {
        return Some(h.clone());
    }
    let hash = match ArtifactKind::from_id(kind) {
        Some(k) if k.model_format().is_some() => {
            let bundled = store.bundle.as_ref()?.get(manufacturer, mpn, k.id())?;
            bundled.sha256.to_ascii_lowercase()
        }
        _ => {
            let res = current_artifact(store, manufacturer, mpn, revision, kind)
                .await
                .ok()?;
            content_hash(&res)?
        }
    };
    store.artifact_hashes.lock().ok()?.insert(key, hash.clone());
    Some(hash)
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts?silicon_revision=...`
async fn list_artifacts(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
    Query(q): Query<RevisionQuery>,
) -> Result<Json<ArtifactsResponse>, ApiError> {
    // Checks the part and the revision.
    build_part(&store, &manufacturer, &mpn, q.silicon_revision.as_deref())?;
    let rev = q.silicon_revision.as_deref();
    let mut links = std::collections::HashMap::new();
    for kind in SERVED_KINDS {
        // An MPN may contain `/` ("APT1608QBC/D"): each segment is
        // percent-encoded so the link stays one `:mpn` segment.
        let link = match artifact_hash(&store, &manufacturer, &mpn, rev, kind).await {
            Some(h) => hashed_artifact_path(&h, &manufacturer, &mpn, kind, rev),
            None => plain_artifact_path(&manufacturer, &mpn, kind, rev),
        };
        links.insert(kind, link);
    }
    let mut take = |k: &str| links.remove(k).unwrap_or_default();
    Ok(Json(ArtifactsResponse {
        kicad_symbol: take("kicad-symbol"),
        kicad_footprint: take("kicad-footprint"),
        footprint_svg: take("footprint-svg"),
        symbol_svg: take("symbol-svg"),
        step: take("step"),
        stl: take("stl"),
        three_mf: take("3mf"),
        pcbcraft: take("pcbcraft"),
        pcbcraft_symbol_svg: take("pcbcraft-symbol-svg"),
    }))
}

/// How long a stale content-addressed link's redirect may be cached.
const REDIRECT_CACHE: &str = "public, max-age=60";
pub const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// `GET /v1/a/{sha256}/{manufacturer}/{mpn}/{kind}?silicon_revision=...`
///
/// The artifact by content: answered with a year-long immutable cache
/// policy when `sha256` is the current content's digest, else a `302`
/// to the current content's link (the dataset changed since the link
/// was given out).
async fn hashed_artifact(
    State(store): State<Arc<Store>>,
    OriginalUri(uri): OriginalUri,
    Path((sha256, manufacturer, mpn, kind)): Path<(String, String, String, String)>,
    Query(q): Query<RevisionQuery>,
) -> Result<Response, ApiError> {
    let wanted = sha256.to_ascii_lowercase();
    if wanted.len() != 64 || !wanted.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ApiError::NotFound(format!(
            "{sha256} is not a SHA-256 digest"
        )));
    }
    let rev = q.silicon_revision.as_deref();
    let mut res = current_artifact(&store, &manufacturer, &mpn, rev, &kind).await?;
    let Some(have) = content_hash(&res) else {
        return Ok(res);
    };
    if have == wanted {
        res.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static(IMMUTABLE),
        );
        return Ok(res);
    }
    // Under `/api` too: the prefix the request came in under.
    let path = uri.path();
    let prefix = path.find("/v1/a/").map_or("", |i| &path[..i]);
    let location = format!(
        "{prefix}{}",
        hashed_artifact_path(&have, &manufacturer, &mpn, &kind, rev)
    );
    let mut redirect = StatusCode::FOUND.into_response();
    let h = redirect.headers_mut();
    if let Ok(v) = header::HeaderValue::from_str(&location) {
        h.insert(header::LOCATION, v);
    }
    h.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static(REDIRECT_CACHE),
    );
    Ok(redirect)
}

/// RFC 3986 percent-encoding of a path segment (everything but the
/// unreserved characters, byte by byte).
pub(crate) fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
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

#[derive(Deserialize)]
struct PlacementQuery {
    density: Option<String>,
    silicon_revision: Option<String>,
}

/// `GET /v1/parts/{manufacturer}/{mpn}/placement?density=N|L|M&silicon_revision=...`
///
/// The part's Placement Model (docs/spec/placement-model.md): pads,
/// courtyard at the IPC-7351B density level (`N` by default), body,
/// pin 1, board-edge and keep-out statements, 3D alignment, each group's
/// basis, and the content hash its KiCad footprint carries.
async fn get_placement(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
    Query(q): Query<PlacementQuery>,
) -> Result<Json<openparts_placement::PlacementModel>, ApiError> {
    let density = match q.density.as_deref() {
        None | Some("") => openparts_placement::Density::N,
        Some(d) => openparts_placement::Density::parse(d).ok_or_else(|| {
            ApiError::BadRequest(format!("density must be L, N or M, not \"{d}\""))
        })?,
    };
    let build = build_part(&store, &manufacturer, &mpn, q.silicon_revision.as_deref())?;
    Ok(Json(build.placement(density)?))
}

pub(crate) fn artifact_response(content: Vec<u8>, content_type: &str) -> Response {
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
artifact_route!(artifact_symbol_svg, ArtifactKind::SymbolSvg);
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
        crate::documents::subschemas(&mut gen);
        crate::circuits::subschemas(&mut gen);
        crate::pcbcraft::subschemas(&mut gen);
        let defs = serde_json::Value::Object(gen.definitions().clone());
        std::fs::write(out, serde_json::to_string_pretty(&defs).unwrap()).unwrap();
    }
}
