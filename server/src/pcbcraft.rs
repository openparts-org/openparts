//! OpenParts as an external parts catalog for PCBCraft
//! (docs/spec/pcbcraft-export.md, "Catalog API"): search, details and
//! on-demand resolution of a part into its exact, immutable PCBCraft
//! native records.
//!
//! - `GET /v1/pcbcraft`: whether the catalog is ready, and whether its
//!   revisions are persistent;
//! - `GET /v1/pcbcraft/search?q=&category=&manufacturer=&exportable=&offset=&limit=`;
//! - `GET /v1/pcbcraft/parts/{manufacturer}/{mpn}`: one part's summary
//!   and every record address it depends on;
//! - `GET /v1/parts/{manufacturer}/{mpn}/artifacts/pcbcraft`: the part
//!   as a `pcbcraft-library-bundle/1` (its dependency closure), `422`
//!   with the reason when PCBCraft cannot represent it;
//! - `GET /v1/pcbcraft/libraries` and `/v1/pcbcraft/libraries/{id}`: the
//!   whole dataset as one library per category group, and each as a
//!   bundle.
//!
//! The libraries are exported once, in the background after startup,
//! exactly as `openparts pcbcraft --group-by category-group` does
//! (`openparts_artifacts::pcbcraft`): a part fetched alone carries the
//! very documents of its library, under the same addresses. PCBCraft
//! quarantines two different bodies under one (library, kind, id,
//! revision), so revisions come from a ledger: with
//! `OPENPARTS_PCBCRAFT_LEDGER=<file>` it is read at startup, new content
//! gets the next revision, and the file is replaced atomically before
//! anything is served. Without it the ledger lives in memory, every
//! record starts at revision 1 on each start, and responses say the
//! revisions are not persistent.

use crate::error::ApiError;
use crate::store::Store;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use openparts_artifacts::pcbcraft::adapter::ledger::Ledger;
use openparts_artifacts::pcbcraft::adapter::{bundle_of, MAX_BUNDLE_BYTES};
use openparts_artifacts::pcbcraft::{self as shared, Exported, GroupBy, Settings};
use openparts_core::ComponentCategory;
use openparts_data::Dataset;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

/// The public site the records' locators point at by default.
pub const DEFAULT_PUBLIC_URL: &str = "https://openparts.jijinbei.jp";

pub fn router() -> Router<Arc<Store>> {
    Router::new()
        .route("/v1/pcbcraft", get(get_status))
        .route("/v1/pcbcraft/search", get(search))
        .route("/v1/pcbcraft/parts/:manufacturer/:mpn", get(get_details))
        .route("/v1/pcbcraft/libraries", get(list_libraries))
        .route("/v1/pcbcraft/libraries/:id", get(get_library))
        .route(
            "/v1/parts/:manufacturer/:mpn/artifacts/pcbcraft",
            get(get_part_bundle),
        )
        .route(
            "/v1/parts/:manufacturer/:mpn/artifacts/pcbcraft-symbol-svg",
            get(get_part_symbol_svg),
        )
}

/// How the catalog is made.
#[derive(Debug, Clone)]
pub struct PcbcraftSettings {
    /// The persistent revision ledger (`OPENPARTS_PCBCRAFT_LEDGER`);
    /// `None`: in memory only.
    pub ledger: Option<PathBuf>,
    /// The base of the records' locators (`OPENPARTS_PUBLIC_URL`).
    pub public_url: String,
    /// The dataset revision named in each new record's source reference
    /// (`OPENPARTS_DATA_REVISION`, else the data checkout's git HEAD).
    pub data_revision: String,
    pub license: String,
}

impl Default for PcbcraftSettings {
    fn default() -> Self {
        PcbcraftSettings {
            ledger: None,
            public_url: DEFAULT_PUBLIC_URL.into(),
            data_revision: "unknown".into(),
            license: "CC0-1.0".into(),
        }
    }
}

impl PcbcraftSettings {
    pub fn from_env(data_dir: &FsPath) -> PcbcraftSettings {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        PcbcraftSettings {
            ledger: var("OPENPARTS_PCBCRAFT_LEDGER").map(PathBuf::from),
            public_url: var("OPENPARTS_PUBLIC_URL")
                .unwrap_or_else(|| DEFAULT_PUBLIC_URL.into())
                .trim_end_matches('/')
                .to_string(),
            data_revision: var("OPENPARTS_DATA_REVISION")
                .or_else(|| git_head(data_dir))
                .unwrap_or_else(|| "unknown".into()),
            ..PcbcraftSettings::default()
        }
    }
}

fn git_head(dir: &FsPath) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

/// The catalog: built once ([`PcbcraftService::prepare`]), then read-only.
#[derive(Default)]
pub struct PcbcraftService {
    pub settings: PcbcraftSettings,
    state: OnceLock<Result<Catalog, String>>,
}

impl std::fmt::Debug for PcbcraftService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcbcraftService")
            .field("settings", &self.settings)
            .field("status", &self.status())
            .finish()
    }
}

struct LibraryEntry {
    exported: Exported,
    bundle_bytes: usize,
    bundle_sha256: String,
}

struct PartEntry {
    library: usize,
    refusal: Option<String>,
    manufacturer_part: Option<PcbcraftAddress>,
    part: Option<PcbcraftAddress>,
}

struct Catalog {
    libraries: Vec<LibraryEntry>,
    /// OpenParts part id -> where it is.
    parts: HashMap<String, PartEntry>,
    build_ms: u64,
    /// Revisions the ledger allocated in this build.
    new_revisions: usize,
}

/// What [`PcbcraftService::prepare`] did.
#[derive(Debug, Clone)]
pub struct PrepareReport {
    pub libraries: usize,
    pub parts: usize,
    pub refused: usize,
    pub new_revisions: usize,
    pub build_ms: u64,
}

impl PcbcraftService {
    pub fn new(settings: PcbcraftSettings) -> PcbcraftService {
        PcbcraftService {
            settings,
            state: OnceLock::new(),
        }
    }

    pub fn persistent(&self) -> bool {
        self.settings.ledger.is_some()
    }

    /// Exports every part of `ds`, allocating revisions in the ledger
    /// (and writing it back atomically when it changed), then serves the
    /// result. Runs once; a failure (an unreadable ledger, a write that
    /// fails) is kept and every PCBCraft endpoint answers `503` with it:
    /// nothing is served whose revision might not survive a restart.
    pub fn prepare(&self, ds: &Dataset) -> Result<PrepareReport, String> {
        let result = self.build(ds);
        let report = result.as_ref().map(|c| PrepareReport {
            libraries: c.libraries.len(),
            parts: c.parts.values().filter(|p| p.refusal.is_none()).count(),
            refused: c.parts.values().filter(|p| p.refusal.is_some()).count(),
            new_revisions: c.new_revisions,
            build_ms: c.build_ms,
        });
        let report = report.map_err(Clone::clone);
        if self.state.set(result).is_err() {
            return Err("the PCBCraft catalog was already prepared".into());
        }
        report
    }

    fn build(&self, ds: &Dataset) -> Result<Catalog, String> {
        let started = Instant::now();
        let mut ledger = match &self.settings.ledger {
            Some(path) => read_ledger(path)?,
            None => Ledger::default(),
        };
        let before = ledger.clone();
        let settings = Settings {
            library_id: "openparts".into(),
            library_name: "OpenParts".into(),
            license: self.settings.license.clone(),
            source_reference: shared::source_reference(&self.settings.data_revision),
            locator_base: self.settings.public_url.trim_end_matches('/').to_string(),
            group_by: GroupBy::CategoryGroup,
        };
        let parts: Vec<_> = ds.parts.iter().collect();
        let exported = shared::export(ds, &parts, &settings, &mut ledger)
            .map_err(|e| format!("PCBCraft export failed: {e}"))?;
        let new_revisions = count_revisions(&ledger) - count_revisions(&before);
        if ledger != before {
            if let Some(path) = &self.settings.ledger {
                write_atomically(path, &ledger.to_json()).map_err(|e| {
                    format!(
                        "cannot write the PCBCraft ledger {}: {e}; not serving revisions that would not survive a restart",
                        path.display()
                    )
                })?;
            }
        }

        let mut libraries = Vec::new();
        let mut entries = HashMap::new();
        for (index, exported) in exported.into_iter().enumerate() {
            let lib = &exported.library;
            for (part, path) in &lib.manufacturer_parts {
                let mpart = address(&lib.id, path, &lib.documents[path]);
                let generic = mpart.as_ref().and_then(|m| {
                    let doc: serde_json::Value = serde_json::from_str(&lib.documents[path]).ok()?;
                    let id = doc.get("part")?.get("id")?.as_str()?;
                    let gpath = format!("parts/{id}.json");
                    address(&m.library, &gpath, lib.documents.get(&gpath)?)
                });
                entries.insert(
                    part.clone(),
                    PartEntry {
                        library: index,
                        refusal: None,
                        manufacturer_part: mpart,
                        part: generic,
                    },
                );
            }
            for r in &lib.refusals {
                entries.insert(
                    r.part.clone(),
                    PartEntry {
                        library: index,
                        refusal: Some(r.reason.clone()),
                        manufacturer_part: None,
                        part: None,
                    },
                );
            }
            let bundle = lib.bundle();
            libraries.push(LibraryEntry {
                bundle_bytes: bundle.len(),
                bundle_sha256: crate::routes::sha256_hex(bundle.as_bytes()),
                exported,
            });
        }
        Ok(Catalog {
            libraries,
            parts: entries,
            build_ms: started.elapsed().as_millis() as u64,
            new_revisions,
        })
    }

    fn catalog(&self) -> Result<&Catalog, ApiError> {
        match self.state.get() {
            Some(Ok(c)) => Ok(c),
            Some(Err(e)) => Err(ApiError::Unavailable(format!(
                "the PCBCraft catalog is unavailable: {e}"
            ))),
            None => Err(ApiError::Unavailable(
                "the PCBCraft catalog is still being built (it is exported once after the server starts); retry shortly".into(),
            )),
        }
    }

    pub fn status(&self) -> PcbcraftStatus {
        let (state, error, catalog) = match self.state.get() {
            None => (PcbcraftState::Building, None, None),
            Some(Err(e)) => (PcbcraftState::Failed, Some(e.clone()), None),
            Some(Ok(c)) => (PcbcraftState::Ready, None, Some(c)),
        };
        PcbcraftStatus {
            state,
            error,
            persistent_revisions: self.persistent(),
            license: self.settings.license.clone(),
            libraries: catalog.map_or(0, |c| c.libraries.len()),
            parts: catalog.map_or(0, |c| {
                c.parts.values().filter(|p| p.refusal.is_none()).count()
            }),
            refused: catalog.map_or(0, |c| {
                c.parts.values().filter(|p| p.refusal.is_some()).count()
            }),
            build_ms: catalog.map(|c| c.build_ms),
        }
    }
}

fn count_revisions(ledger: &Ledger) -> usize {
    ledger
        .libraries
        .values()
        .flat_map(|l| l.records.values())
        .map(|r| r.revisions.len())
        .sum()
}

/// The ledger at `path`; an absent file is an empty ledger. A file that
/// does not parse is an error, never silently replaced (its revisions
/// would be allocated again for other content).
fn read_ledger(path: &FsPath) -> Result<Ledger, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ledger::from_json(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Ledger::default()),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Writes `text` to a temporary file next to `path`, flushes it, and
/// renames it over `path`: a crash leaves the old ledger or the new one.
fn write_atomically(path: &FsPath, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    std::fs::create_dir_all(&dir)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "ledger.json".into());
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)?;
        if let Ok(d) = std::fs::File::open(&dir) {
            let _ = d.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// The record kind of a document path (`parts/x.json` -> `part`).
fn kind_of(path: &str) -> Option<&'static str> {
    Some(match path.split('/').next()? {
        "symbols" => "symbol",
        "footprints" => "footprint",
        "categories" => "category",
        "parts" => "part",
        "manufacturer-parts" => "manufacturer-part",
        _ => return None,
    })
}

fn address(library: &str, path: &str, text: &str) -> Option<PcbcraftAddress> {
    let kind = kind_of(path)?;
    let id = path.rsplit('/').next()?.strip_suffix(".json")?;
    let doc: serde_json::Value = serde_json::from_str(text).ok()?;
    Some(PcbcraftAddress {
        library: library.to_string(),
        kind: kind.to_string(),
        id: id.to_string(),
        revision: u32::try_from(doc.get("revision")?.as_u64()?).ok()?,
        sha256: crate::routes::sha256_hex(text.as_bytes()),
    })
}

// ---- wire types -------------------------------------------------------

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcbcraftState {
    /// Being exported after startup: the endpoints answer `503`.
    Building,
    Ready,
    /// The export or the ledger failed (`error`): the endpoints answer
    /// `503`.
    Failed,
}

/// `GET /v1/pcbcraft` (and `pcbcraft` in `GET /v1/config`).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct PcbcraftStatus {
    pub state: PcbcraftState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Revisions are kept in a ledger file across restarts and deploys.
    /// `false`: this server numbers them afresh on each start, so a
    /// client must not treat an address as immutable across restarts.
    pub persistent_revisions: bool,
    /// The licence every record declares.
    pub license: String,
    pub libraries: usize,
    /// Parts exported.
    pub parts: usize,
    /// Parts PCBCraft cannot represent.
    pub refused: usize,
    /// How long the export took.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_ms: Option<u64>,
}

/// A record's PCBCraft address and the SHA-256 of its document text as
/// served (one address never has two texts here).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PcbcraftAddress {
    pub library: String,
    /// `symbol`, `footprint`, `category`, `part` or `manufacturer-part`.
    pub kind: String,
    pub id: String,
    pub revision: u32,
    pub sha256: String,
}

/// A part as PCBCraft sees it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct PcbcraftPartSummary {
    /// The OpenParts manufacturer id (`.../parts/{manufacturer}/{mpn}`).
    pub manufacturer: String,
    pub manufacturer_name: String,
    pub mpn: String,
    pub category: Option<ComponentCategory>,
    /// A one-line description (category, device and package).
    pub description: String,
    /// The library the part is (or would be) in (`openparts-<group>`).
    pub library: Option<String>,
    /// Its manufacturer part record; `None` when refused.
    pub manufacturer_part: Option<PcbcraftAddress>,
    /// The generic part the manufacturer part refers to.
    pub part: Option<PcbcraftAddress>,
    pub refused: bool,
    /// Why PCBCraft cannot represent the part.
    pub refusal: Option<String>,
    /// `.../artifacts/pcbcraft`, the part's bundle; `None` when refused.
    pub bundle: Option<String>,
}

/// `GET /v1/pcbcraft/search`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct PcbcraftSearchResponse {
    /// Matches before paging: a page is complete only when
    /// `offset + results.len() == total`.
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub persistent_revisions: bool,
    pub results: Vec<PcbcraftPartSummary>,
}

/// `GET /v1/pcbcraft/parts/{manufacturer}/{mpn}`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct PcbcraftPartDetails {
    pub summary: PcbcraftPartSummary,
    /// Every record in the part's bundle (its exact dependency closure),
    /// in path order; empty when refused.
    pub dependencies: Vec<PcbcraftAddress>,
    pub license: String,
    pub persistent_revisions: bool,
}

/// One library of `GET /v1/pcbcraft/libraries`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct PcbcraftLibrarySummary {
    pub id: String,
    pub name: String,
    /// The top-level category the library holds.
    pub category: Option<ComponentCategory>,
    pub parts: usize,
    pub refused: usize,
    /// Native documents (`library.json` and records).
    pub documents: usize,
    /// The bundle's size and SHA-256.
    pub bundle_bytes: usize,
    pub bundle_sha256: String,
    /// `/v1/pcbcraft/libraries/{id}`; `None` when the bundle is over
    /// PCBCraft's limits (`limit_error`).
    pub bundle: Option<String>,
    pub limit_error: Option<String>,
}

/// `GET /v1/pcbcraft/libraries`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct PcbcraftLibraries {
    pub persistent_revisions: bool,
    pub license: String,
    pub libraries: Vec<PcbcraftLibrarySummary>,
}

#[cfg(feature = "schema")]
pub fn subschemas(gen: &mut schemars::SchemaGenerator) {
    gen.subschema_for::<PcbcraftStatus>();
    gen.subschema_for::<PcbcraftSearchResponse>();
    gen.subschema_for::<PcbcraftPartDetails>();
    gen.subschema_for::<PcbcraftLibraries>();
}

// ---- handlers ---------------------------------------------------------

fn humanize(snake: &str) -> String {
    let s = snake.replace('_', " ");
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => s,
    }
}

fn summary(store: &Store, catalog: &Catalog, part: &openparts_core::Part) -> PcbcraftPartSummary {
    let ds = &store.dataset;
    let device = ds.devices.get(&part.device.0);
    let category = device.map(|d| d.category);
    let entry = catalog.parts.get(&part.id.0);
    let library = entry.map(|e| catalog.libraries[e.library].exported.library.id.clone());
    let refusal = match entry {
        Some(e) => e.refusal.clone(),
        None => Some("not in the PCBCraft export".into()),
    };
    let description = format!(
        "{} {} in {}",
        category.map_or_else(|| "Part".into(), |c| humanize(c.name())),
        part.device.0,
        part.package.0
    );
    PcbcraftPartSummary {
        manufacturer: part.manufacturer.0.clone(),
        manufacturer_name: ds
            .manufacturers
            .get(&part.manufacturer.0)
            .map_or_else(|| part.manufacturer.0.clone(), |m| m.name.clone()),
        mpn: part.mpn.clone(),
        category,
        description,
        library,
        manufacturer_part: entry.and_then(|e| e.manufacturer_part.clone()),
        part: entry.and_then(|e| e.part.clone()),
        refused: refusal.is_some(),
        bundle: refusal.is_none().then(|| {
            format!(
                "/v1/parts/{}/{}/artifacts/pcbcraft",
                crate::routes::encode_segment(&part.manufacturer.0),
                crate::routes::encode_segment(&part.mpn)
            )
        }),
        refusal,
    }
}

async fn get_status(State(store): State<Arc<Store>>) -> Json<PcbcraftStatus> {
    Json(store.pcbcraft.status())
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
    category: Option<ComponentCategory>,
    manufacturer: Option<String>,
    /// `true`: only parts PCBCraft can represent; `false`: only refused.
    exportable: Option<bool>,
    offset: Option<usize>,
    limit: Option<usize>,
}

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 200;

/// `GET /v1/pcbcraft/search?q=&category=&manufacturer=&exportable=&offset=&limit=`
async fn search(
    State(store): State<Arc<Store>>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<PcbcraftSearchResponse>, ApiError> {
    let catalog = store.pcbcraft.catalog()?;
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_LIMIT {
        return Err(ApiError::BadRequest(format!(
            "limit must be 1-{MAX_LIMIT}, not {limit}"
        )));
    }
    let offset = params.offset.unwrap_or(0);
    let ds = &store.dataset;
    let matches: Vec<&openparts_core::Part> =
        crate::routes::ranked_parts(&store, params.q.as_deref().unwrap_or(""))
            .into_iter()
            .map(|i| &ds.parts[i])
            .filter(|p| match params.category {
                None => true,
                Some(wanted) => ds
                    .devices
                    .get(&p.device.0)
                    .is_some_and(|d| d.category.is_within(wanted)),
            })
            .filter(|p| {
                params
                    .manufacturer
                    .as_deref()
                    .is_none_or(|m| p.manufacturer.0 == m)
            })
            .filter(|p| match params.exportable {
                None => true,
                Some(want) => {
                    catalog
                        .parts
                        .get(&p.id.0)
                        .is_some_and(|e| e.refusal.is_none())
                        == want
                }
            })
            .collect();
    let results = matches
        .iter()
        .skip(offset)
        .take(limit)
        .map(|p| summary(&store, catalog, p))
        .collect();
    Ok(Json(PcbcraftSearchResponse {
        total: matches.len(),
        offset,
        limit,
        persistent_revisions: store.pcbcraft.persistent(),
        results,
    }))
}

/// The part's documents, or why there are none.
fn part_documents<'c>(
    catalog: &'c Catalog,
    part: &openparts_core::Part,
) -> Result<(&'c LibraryEntry, BTreeMap<String, String>), ApiError> {
    let refused = |reason: String| ApiError::Refused {
        message: format!(
            "{}/{} cannot be exported to PCBCraft: {reason}",
            part.manufacturer.0, part.mpn
        ),
        reason,
    };
    let entry = catalog
        .parts
        .get(&part.id.0)
        .ok_or_else(|| refused("not in the PCBCraft export".into()))?;
    if let Some(reason) = &entry.refusal {
        return Err(refused(reason.clone()));
    }
    let lib = &catalog.libraries[entry.library];
    let docs = lib
        .exported
        .library
        .part_documents(&part.id.0)
        .ok_or_else(|| ApiError::GenerationFailed(format!("{}: incomplete records", part.id.0)))?;
    Ok((lib, docs))
}

/// `GET /v1/pcbcraft/parts/{manufacturer}/{mpn}`
async fn get_details(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<PcbcraftPartDetails>, ApiError> {
    let part = crate::routes::resolve_part(&store, &manufacturer, &mpn)?;
    let catalog = store.pcbcraft.catalog()?;
    let summary = summary(&store, catalog, part);
    let dependencies = match part_documents(catalog, part) {
        Ok((lib, docs)) => docs
            .iter()
            .filter_map(|(path, text)| address(&lib.exported.library.id, path, text))
            .collect(),
        Err(_) => vec![],
    };
    Ok(Json(PcbcraftPartDetails {
        summary,
        dependencies,
        license: store.pcbcraft.settings.license.clone(),
        persistent_revisions: store.pcbcraft.persistent(),
    }))
}

/// A file name of letters, digits, `.`, `-` and `_`.
fn safe(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn bundle_response(store: &Store, text: String, file_name: &str) -> Response {
    let hash = crate::routes::sha256_hex(text.as_bytes());
    let mut res = (StatusCode::OK, text).into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{file_name}\"")) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    if let Ok(v) = HeaderValue::from_str(&format!("sha256:{hash}")) {
        h.insert("x-content-hash", v);
    }
    h.insert(
        "x-pcbcraft-revisions",
        HeaderValue::from_static(if store.pcbcraft.persistent() {
            "persistent"
        } else {
            "ephemeral"
        }),
    );
    res
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts/pcbcraft`: the part's
/// `pcbcraft-library-bundle/1`. Built from the catalog, never from the
/// content-addressed artifact bundle: its revisions come from the
/// ledger.
async fn get_part_bundle(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    part_bundle(&store, &manufacturer, &mpn)
}

/// The part's PCBCraft bundle (`.../artifacts/pcbcraft`).
pub(crate) fn part_bundle(
    store: &Store,
    manufacturer: &str,
    mpn: &str,
) -> Result<Response, ApiError> {
    let part = crate::routes::resolve_part(store, manufacturer, mpn)?;
    let catalog = store.pcbcraft.catalog()?;
    let (_, docs) = part_documents(catalog, part)?;
    let text = bundle_of(&docs);
    if text.len() > MAX_BUNDLE_BYTES {
        return Err(ApiError::Refused {
            message: format!("{}/{}: the bundle is over 16 MiB", manufacturer, mpn),
            reason: format!("the bundle is {} bytes, over PCBCraft's 16 MiB", text.len()),
        });
    }
    let name = format!(
        "{}-{}.pcbcraft.json",
        safe(&part.manufacturer.0),
        safe(&part.mpn)
    );
    Ok(bundle_response(store, text, &name))
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts/pcbcraft-symbol-svg`:
/// the part's PCBCraft symbol drawn as PCBCraft draws it (`422` with
/// the reason when PCBCraft can't represent the part).
async fn get_part_symbol_svg(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    part_symbol_svg(&store, &manufacturer, &mpn)
}

pub(crate) fn part_symbol_svg(
    store: &Store,
    manufacturer: &str,
    mpn: &str,
) -> Result<Response, ApiError> {
    let part = crate::routes::resolve_part(store, manufacturer, mpn)?;
    let catalog = store.pcbcraft.catalog()?;
    let (_, docs) = part_documents(catalog, part)?;
    let symbol = docs
        .iter()
        .find(|(path, _)| path.starts_with("symbols/"))
        .map(|(_, text)| text)
        .ok_or_else(|| ApiError::GenerationFailed(format!("{}: no symbol record", part.id.0)))?;
    let svg = openparts_artifacts::pcbcraft::adapter::render_symbol_svg(symbol)
        .map_err(ApiError::GenerationFailed)?;
    Ok(crate::routes::artifact_response(
        svg.into_bytes(),
        "image/svg+xml",
    ))
}

fn library_summary(entry: &LibraryEntry) -> PcbcraftLibrarySummary {
    let lib = &entry.exported.library;
    let limit_error = lib.check_limits().err().or_else(|| {
        (entry.bundle_bytes > MAX_BUNDLE_BYTES).then(|| {
            format!(
                "the bundle is {} bytes, over PCBCraft's 16 MiB: export it as a folder with `openparts pcbcraft --out`",
                entry.bundle_bytes
            )
        })
    });
    PcbcraftLibrarySummary {
        id: lib.id.clone(),
        name: serde_json::from_str::<serde_json::Value>(&lib.documents["library.json"])
            .ok()
            .and_then(|v| Some(v.get("name")?.as_str()?.to_string()))
            .unwrap_or_else(|| lib.id.clone()),
        category: entry.exported.group,
        parts: lib.parts,
        refused: lib.refusals.len(),
        documents: lib.documents.len(),
        bundle_bytes: entry.bundle_bytes,
        bundle_sha256: entry.bundle_sha256.clone(),
        bundle: limit_error
            .is_none()
            .then(|| format!("/v1/pcbcraft/libraries/{}", lib.id)),
        limit_error,
    }
}

/// `GET /v1/pcbcraft/libraries`
async fn list_libraries(
    State(store): State<Arc<Store>>,
) -> Result<Json<PcbcraftLibraries>, ApiError> {
    let catalog = store.pcbcraft.catalog()?;
    Ok(Json(PcbcraftLibraries {
        persistent_revisions: store.pcbcraft.persistent(),
        license: store.pcbcraft.settings.license.clone(),
        libraries: catalog.libraries.iter().map(library_summary).collect(),
    }))
}

/// `GET /v1/pcbcraft/libraries/{id}`: the whole library as one bundle.
async fn get_library(
    State(store): State<Arc<Store>>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let catalog = store.pcbcraft.catalog()?;
    let entry = catalog
        .libraries
        .iter()
        .find(|l| l.exported.library.id == id)
        .ok_or_else(|| ApiError::NotFound(format!("no PCBCraft library {id}")))?;
    if let Some(e) = library_summary(entry).limit_error {
        return Err(ApiError::Refused {
            message: format!("library {id} is over PCBCraft's limits for one bundle"),
            reason: e,
        });
    }
    let text = entry.exported.library.bundle();
    Ok(bundle_response(
        &store,
        text,
        &format!("{}.pcbcraft.json", safe(&id)),
    ))
}
