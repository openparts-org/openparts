//! openparts-mcad: Package -> Geometry Generator -> Mechanical Geometry
//! (Architecture Specification section 15). `MechanicalGeometry` is the
//! common representation that openparts-step/openparts-stl/openparts-gltf
//! all derive their output from; STEP/STL/glTF are never themselves the
//! canonical geometry.

mod chip;
mod lqfp;
mod qfn;
mod radial;
mod soic;
mod sot;

pub use chip::generate_chip;
pub use lqfp::generate_lqfp;
pub use qfn::generate_qfn;
pub use radial::generate_radial;
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
        "radial" => generate_radial(package),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyShape {
    Box,
    /// An exact cylinder -- `size.x`/`size.y` (equal) are the diameter,
    /// `size.z` is the height. Deliberately exact, not pre-tessellated:
    /// how many sides to approximate it with when drawing is an
    /// output-format concern (STL has no choice but to tessellate;
    /// STEP could later emit a true CYLINDRICAL_SURFACE instead), not
    /// a fact about the part -- so that choice belongs to each writer,
    /// not to this shared geometry model. See `cylinder_ring`.
    Cylinder,
}

/// A box (or cylinder -- see `BodyShape`) centered at `position` (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    pub position: Point3,
    pub size: Size3,
    pub shape: BodyShape,
}

/// N points evenly spaced around a circle of `diameter` in the XY
/// plane, centered on the origin -- the one piece of cylinder-
/// tessellation math both `openparts-step` and `openparts-stl` need
/// identically for `BodyShape::Cylinder`, shared here so their
/// polygon approximations of the same body can never quietly drift
/// apart from each other. Each writer picks its own `segments` count
/// and does its own face/triangle construction from the ring -- this
/// function only computes the ring itself.
pub fn cylinder_ring(diameter: f64, segments: u32) -> Vec<(f64, f64)> {
    let radius = diameter / 2.0;
    (0..segments)
        .map(|i| {
            let angle = 2.0 * std::f64::consts::PI * i as f64 / segments as f64;
            (radius * angle.cos(), radius * angle.sin())
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mounting {
    Smd,
    ThroughHole,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lead {
    /// Matches the physical pin number ("1".."N") -- independent of any
    /// Device pin name; Package geometry has no concept of pin names.
    pub number: String,
    pub position: Point3,
    pub size: Size3,
    pub mounting: Mounting,
    /// Drill diameter (mm). `Some` only when `mounting` is `ThroughHole`.
    pub drill: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerKind {
    Pin1Dot,
    /// Polarity marking on a polarized 2-terminal part's body, next to
    /// its negative terminal (e.g. a radial electrolytic capacitor's
    /// "-" stripe) -- distinct from the pad-shape polarity cue
    /// (`openparts-pcbcad::build_footprint`'s pin1-vs-rest shape rule),
    /// which is what a real KiCad footprint actually relies on; this
    /// is the 3D/mechanical-body equivalent.
    NegativeStripe,
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
        "package family \"{0}\" has no supported generator (lqfp, qfn, chip, soic, sot, radial are implemented)"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cylinder_ring_returns_the_requested_point_count() {
        assert_eq!(cylinder_ring(5.0, 24).len(), 24);
    }

    #[test]
    fn cylinder_ring_points_all_sit_on_the_circle() {
        let radius = 2.5;
        for (x, y) in cylinder_ring(radius * 2.0, 16) {
            assert!(((x * x + y * y).sqrt() - radius).abs() < 1e-9);
        }
    }

    #[test]
    fn cylinder_ring_first_point_is_on_the_positive_x_axis() {
        let ring = cylinder_ring(5.0, 8);
        assert!((ring[0].0 - 2.5).abs() < 1e-9);
        assert!(ring[0].1.abs() < 1e-9);
    }
}
