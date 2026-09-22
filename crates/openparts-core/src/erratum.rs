use crate::ids::{DeviceId, PackageId, SourceId};
use crate::kind::Kind;
use serde::{Deserialize, Serialize};

/// Canonical Data Specification section 29.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErratumCategory {
    SiliconErratum,
    DocumentationErratum,
    DatasheetClarification,
    PackageDocumentationErratum,
    Unknown,
}

/// Canonical Data Specification section 31.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Applicability {
    #[serde(default)]
    pub silicon_revisions: Vec<String>,
    #[serde(default)]
    pub packages: Vec<PackageId>,
}

/// Canonical Data Specification section 28. `hardware_changed` is `None`
/// when the document omits it (unknown), matching section 30's rule that
/// "unknown" must never be guessed as `true`/`false`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Erratum {
    pub schema_version: String,
    pub kind: Kind,
    pub id: String,
    pub device: DeviceId,
    pub category: ErratumCategory,
    #[serde(default)]
    pub hardware_changed: Option<bool>,
    #[serde(default)]
    pub applies_to: Applicability,
    pub description: String,
    #[serde(default)]
    pub sources: Vec<SourceId>,
}
