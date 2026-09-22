use crate::ids::SourceId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One piece of evidence for a field. Canonical Data Specification
/// section 25: paths are JSON Pointer strings, e.g. `/pins/1/name`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceEntry {
    pub source: SourceId,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub locator: Option<String>,
}

/// JSON-Pointer-path -> evidence list. A field may have more than one
/// supporting source (section 26).
pub type ProvenanceMap = BTreeMap<String, Vec<ProvenanceEntry>>;
