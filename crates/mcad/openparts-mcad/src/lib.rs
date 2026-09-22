//! openparts-mcad: Package -> Geometry Generator -> Mechanical Geometry
//! (Architecture Specification section 15). `MechanicalGeometry` is the
//! common representation that openparts-step/openparts-stl/openparts-gltf
//! all derive their output from; STEP/STL/glTF are never themselves the
//! canonical geometry.

mod lqfp;

pub use lqfp::generate_lqfp;

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
    #[error("package family \"{0}\" has no supported generator (only \"lqfp\" is implemented)")]
    UnsupportedFamily(String),
    #[error("package.geometry.generator is \"{0}\", expected \"lqfp\"")]
    UnsupportedGenerator(String),
    #[error("missing required dimension: {0}")]
    MissingDimension(&'static str),
    #[error("lead_count ({0}) is not evenly divisible by 4")]
    InvalidLeadCount(u32),
}
