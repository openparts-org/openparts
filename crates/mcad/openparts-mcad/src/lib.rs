//! openparts-mcad: Package -> Geometry Generator -> Mechanical Geometry
//! (Architecture Specification section 15). `MechanicalGeometry` is the
//! common representation that openparts-step/openparts-stl/openparts-gltf
//! all derive their output from; STEP/STL/glTF are never themselves the
//! canonical geometry.

mod chip;
mod lqfp;
mod qfn;
mod soic;
mod sot;

pub use chip::generate_chip;
pub use lqfp::generate_lqfp;
pub use qfn::generate_qfn;
pub use soic::generate_soic;
pub use sot::generate_sot;

use openparts_core::Package;

/// Dispatches to the generator matching `package.family`, so callers
/// (CLI, tests) don't need to know which generator a given Package uses.
pub fn generate(package: &Package) -> Result<MechanicalGeometry, McadError> {
    match package.family.to_ascii_lowercase().as_str() {
        "lqfp" => generate_lqfp(package),
        "qfn" => generate_qfn(package),
        "chip" => generate_chip(package),
        "soic" => generate_soic(package),
        "sot" => generate_sot(package),
        _ => Err(McadError::UnsupportedFamily(package.family.clone())),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A box centered at `position` (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    pub position: Point3,
    pub size: Size3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lead {
    /// Matches the physical pin number ("1".."N") -- independent of any
    /// Device pin name; Package geometry has no concept of pin names.
    pub number: String,
    pub position: Point3,
    pub size: Size3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerKind {
    Pin1Dot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub kind: MarkerKind,
    pub position: Point3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MechanicalGeometry {
    pub body: Body,
    pub leads: Vec<Lead>,
    pub markers: Vec<Marker>,
}

#[derive(Debug, thiserror::Error)]
pub enum McadError {
    #[error(
        "package family \"{0}\" has no supported generator (lqfp, qfn, chip, soic, sot are implemented)"
    )]
    UnsupportedFamily(String),
    #[error("package.geometry.generator \"{0}\" does not match this package's family")]
    UnsupportedGenerator(String),
    #[error("missing required dimension: {0}")]
    MissingDimension(&'static str),
    #[error("lead_count ({0}) is not valid for this package family")]
    InvalidLeadCount(u32),
    /// SOT-family packages: the physical pin-to-side assignment isn't
    /// derivable from `lead_count` alone (vendors disagree on which
    /// side gets the lower numbers for asymmetric layouts like SOT-23's
    /// 2-vs-1 split) -- see `Package::lead_layout`'s docs.
    #[error(
        "family \"{family}\" requires an explicit lead_layout (its physical pin-to-side \
         assignment isn't derivable from lead_count alone)"
    )]
    MissingLeadLayout { family: String },
    #[error("lead_layout {layout:?} does not sum to lead_count ({lead_count})")]
    InvalidLeadLayout { layout: Vec<u32>, lead_count: u32 },
}
