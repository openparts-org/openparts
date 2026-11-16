use crate::existence::Existence;
use crate::ids::{DeviceId, ManufacturerId, PackageId, PartId, SourceId};
use crate::kind::Kind;
use crate::lifecycle::Lifecycle;
use crate::parameter::Parameters;
use crate::provenance::ProvenanceMap;
use crate::serde_util::deserialize_no_dup_map;
use serde::{Deserialize, Serialize};

/// Canonical Data Specification section 11. Never duplicates Device
/// pinout or Package dimensions — only references them.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// Electrical parameters of this orderable MPN (section 11.1), each
    /// cited at `/parameters/<key>` in `provenance`. Overrides the
    /// Device's family-wide value of the same key.
    #[serde(
        default,
        skip_serializing_if = "Parameters::is_empty",
        deserialize_with = "deserialize_no_dup_map"
    )]
    pub parameters: Parameters,
    /// The components mounted on a module board (section 20.12), each
    /// cited at `/assembly/<i>`; empty for an ordinary part.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assembly: Vec<crate::assembly::AssemblyComponent>,
    #[serde(default, deserialize_with = "deserialize_no_dup_map")]
    pub provenance: ProvenanceMap,
    /// Human review status (section 26.1); `None`: unreviewed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<crate::review::DocumentReview>,
}
