//! The prebuilt artifact bundle's layout and `manifest.json` schema,
//! shared by its writer (`openparts bundle`) and its reader
//! (openparts-server):
//!
//! ```text
//! <bundle>/
//! ├── manifest.json
//! └── artifacts/sha256/<hex>
//! ```
//!
//! The manifest maps (manufacturer, mpn) to each artifact kind's digest,
//! size and content type, lists every part that failed (and why), and
//! records the conditions the bundle was built under (Architecture
//! Specification section 23). It has no time stamp, so the same dataset
//! and toolchain give the same bundle, byte for byte.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Version of the layout and of [`Manifest`].
pub const FORMAT_VERSION: u32 = 1;
pub const FORMAT: &str = "openparts-bundle";
pub const MANIFEST_FILE: &str = "manifest.json";
/// Where an artifact lives, relative to the bundle root.
pub const ARTIFACT_PATH: &str = "artifacts/sha256/{sha256}";

/// The relative path of the artifact with digest `sha256`.
pub fn artifact_path(sha256: &str) -> String {
    ARTIFACT_PATH.replace("{sha256}", sha256)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub format_version: u32,
    pub artifact_path: String,
    pub reproducibility: Reproducibility,
    pub summary: Summary,
    /// Sorted by (manufacturer, mpn).
    pub parts: Vec<PartEntry>,
    /// Dataset files that could not be loaded at all.
    #[serde(default)]
    pub load_errors: Vec<LoadError>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Reproducibility {
    pub dataset: DatasetRevision,
    /// `schema_version`s of the Parts/Devices/Packages bundled.
    pub schema_versions: BTreeSet<String>,
    /// Generator crate versions.
    pub generator: BTreeMap<String, String>,
    /// The openparts checkout the generators were built from.
    pub generator_revision: Option<GitRevision>,
    pub toolchain: BTreeMap<String, String>,
    /// The Effective Model's revision: always the base device.
    pub silicon_revision: Option<String>,
    /// Artifact kinds not byte-identical across rebuilds of the same
    /// input (SolidCraft's design file keeps build timings).
    pub not_byte_reproducible: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatasetRevision {
    pub git: Option<GitRevision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRevision {
    pub commit: String,
    /// Uncommitted changes were present.
    pub dirty: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Summary {
    pub parts: usize,
    pub ok: usize,
    pub partial: usize,
    pub failed: usize,
    pub artifacts: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartEntry {
    pub manufacturer: String,
    pub mpn: String,
    pub part_id: String,
    pub device: String,
    pub package: String,
    pub status: Status,
    /// Artifact kind id ([`crate::ArtifactKind::id`]) -> artifact.
    pub artifacts: BTreeMap<String, Artifact>,
    #[serde(default)]
    pub errors: Vec<PartError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Every artifact was built.
    Ok,
    /// Some were built; `errors` says why the rest were not.
    Partial,
    /// None were built.
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub sha256: String,
    pub size: u64,
    pub content_type: String,
    /// A file name to save it under.
    pub filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartError {
    /// [`crate::Stage::id`].
    pub stage: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadError {
    pub path: String,
    pub message: String,
}
