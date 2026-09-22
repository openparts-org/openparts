use crate::existence::Existence;
use crate::ids::{DeviceId, ManufacturerId, PackageId, PartId, SourceId};
use crate::kind::Kind;
use crate::lifecycle::Lifecycle;
use crate::provenance::ProvenanceMap;
use crate::serde_util::deserialize_no_dup_map;
use serde::{Deserialize, Serialize};

/// Canonical Data Specification section 11. Never duplicates Device
/// pinout or Package dimensions — only references them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub schema_version: String,
    pub kind: Kind,
    pub id: PartId,
    pub manufacturer: ManufacturerId,
    pub mpn: String,
    pub device: DeviceId,
    pub package: PackageId,
    pub existence: Existence,
    pub lifecycle: Lifecycle,
    #[serde(default)]
    pub sources: Vec<SourceId>,
    #[serde(default, deserialize_with = "deserialize_no_dup_map")]
    pub provenance: ProvenanceMap,
}
