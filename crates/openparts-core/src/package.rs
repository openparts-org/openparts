use crate::dimension::Dimension;
use crate::ids::PackageId;
use crate::kind::Kind;
use crate::provenance::ProvenanceMap;
use crate::serde_util::deserialize_no_dup_map;
use serde::{Deserialize, Serialize};

/// The center thermal/ground pad on the underside of leadframe packages
/// (QFN, DFN, ...) -- a physical package fact from the datasheet's
/// package outline drawing, independent of which Device pin (if any) is
/// bonded to it. `None` means the package has no exposed pad.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExposedPadDimensions {
    pub width: Dimension,
    pub length: Dimension,
}

/// Canonical Data Specification section 20.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageDimensions {
    pub body_width: Dimension,
    pub body_length: Dimension,
    #[serde(default)]
    pub body_height: Option<Dimension>,
    #[serde(default)]
    pub exposed_pad: Option<ExposedPadDimensions>,
}

/// Canonical Data Specification section 21.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryType {
    Parametric,
    Procedural,
    Handcrafted,
    External,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageGeometrySpec {
    #[serde(rename = "type")]
    pub geometry_type: GeometryType,
    /// Name of the CAD-independent geometry generator (e.g. "lqfp"). Not
    /// a KiCad/STEP generator name — see openparts-mcad.
    #[serde(default)]
    pub generator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Package {
    pub schema_version: String,
    pub kind: Kind,
    pub id: PackageId,
    pub family: String,
    pub lead_count: u32,
    pub pitch: Dimension,
    pub dimensions: PackageDimensions,
    /// Explicit per-side lead counts, in pin-number order starting from
    /// the pin-1 side, for families whose physical pin-to-side
    /// assignment isn't uniquely derivable from `lead_count` alone
    /// (Canonical Data Specification section 20) -- e.g. SOT's
    /// asymmetric 2-sided layout, where vendors disagree on which side
    /// gets the lower numbers. Not needed by families with a fixed or
    /// symmetric convention (lqfp, qfn, soic all split `lead_count`
    /// evenly and don't use this field).
    #[serde(default)]
    pub lead_layout: Option<Vec<u32>>,
    #[serde(default)]
    pub geometry: Option<PackageGeometrySpec>,
    #[serde(default, deserialize_with = "deserialize_no_dup_map")]
    pub provenance: ProvenanceMap,
}
