use crate::ids::{DeviceId, ManufacturerId};
use crate::kind::Kind;
use crate::pin::{Pin, PinOverride};
use crate::provenance::ProvenanceMap;
use crate::serde_util::deserialize_no_dup_map;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Canonical Data Specification section 18.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevisionDetection {
    pub method: String,
    #[serde(default)]
    pub register: Option<String>,
    #[serde(default)]
    pub mask: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

/// Canonical Data Specification section 19: a diff against the base
/// Device, not a full copy.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeviceOverrides {
    #[serde(default, deserialize_with = "deserialize_no_dup_map")]
    pub pins: BTreeMap<String, PinOverride>,
}

/// Canonical Data Specification section 17.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceRevision {
    pub manufacturer_revision: String,
    #[serde(default)]
    pub detection: Vec<RevisionDetection>,
    #[serde(default)]
    pub overrides: DeviceOverrides,
}

/// Canonical Data Specification section 14.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub schema_version: String,
    pub kind: Kind,
    pub id: DeviceId,
    pub manufacturer: ManufacturerId,
    #[serde(default)]
    pub family: Option<String>,
    /// Keyed by pin-number string (e.g. "1", "42", "A1", "EP"). Never
    /// coerced to an integer — Pad grid position != Pin number.
    #[serde(deserialize_with = "deserialize_no_dup_map")]
    pub pins: BTreeMap<String, Pin>,
    #[serde(default, deserialize_with = "deserialize_no_dup_map")]
    pub revisions: BTreeMap<String, DeviceRevision>,
    #[serde(default, deserialize_with = "deserialize_no_dup_map")]
    pub provenance: ProvenanceMap,
}
