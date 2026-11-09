//! openparts-mcad: Package -> Geometry Generator -> Mechanical Geometry
//! (Architecture Specification section 15). `MechanicalGeometry` is the
//! common representation that openparts-solidcraft (and through it
//! openparts-step/openparts-stl) derives its output from; STEP/STL/3MF are
//! never themselves the canonical geometry.

mod axial;
mod bga;
mod chip;
mod defaults;
mod dip;
mod lqfp;
mod qfn;
mod radial;
mod sip;
mod soic;
mod sot;
mod to92;

pub use axial::generate_axial;
pub use bga::generate_bga;
pub use chip::generate_chip;
pub use dip::generate_dip;
pub use lqfp::generate_lqfp;
pub use qfn::generate_qfn;
pub use radial::generate_radial;
pub use sip::generate_sip;
pub use soic::generate_soic;
pub use sot::generate_sot;
pub use to92::generate_to92;

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
        "axial" => generate_axial(package),
        "to92" => generate_to92(package),
        "dip" => generate_dip(package),
        "sip" => generate_sip(package),
        "bga" => generate_bga(package),
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
    /// An exact cylinder, axis along Z (vertical -- "standing up" on the
    /// board, e.g. a radial electrolytic can) -- `size.x`/`size.y`
    /// (equal) are the diameter, `size.z` is the height. Deliberately
    /// exact, not pre-tessellated: turning this into a renderable shape
    /// (a true curved STEP surface, or a tessellated STL mesh) is
    /// `openparts-brep`'s job, not a fact about the part, so that choice
    /// belongs downstream, not to this shared geometry model.
    Cylinder,
    /// Same as `Cylinder`, but lying on its side with its axis along X
    /// (horizontal -- e.g. an axial-leaded diode) -- `size.x` is the
    /// height (length along the axis), `size.y`/`size.z` (equal) are
    /// the diameter.
    CylinderX,
}

/// A box (or cylinder -- see `BodyShape`) centered at `position` (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    pub position: Point3,
    pub size: Size3,
    pub shape: BodyShape,
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

/// How a package's leads are formed, with the dimensions a detailed 3D
/// model needs beyond each `Lead`'s board-plane outline. Each value is
/// the package's datasheet dimension when it states one, else this
/// crate's family default (see `defaults.rs`) -- this crate owns those
/// defaults so the footprint pads and the 3D leads come from the same
/// numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LeadForm {
    /// Gull-wing leads (QFP, SOIC/SOP, SOT): out of the body side, bent
    /// down to a flat foot on the seating plane. Each perimeter `Lead`'s
    /// outline runs from the body edge to the lead toe.
    GullWing {
        /// Lead-frame thickness (JEDEC c).
        thickness: f64,
        /// Flat foot length on the seating plane, ending at the toe (L).
        foot_length: f64,
        /// Seating plane to the body underside (A1).
        standoff: f64,
    },
    /// Leadless terminals flush with the body underside and edge (QFN,
    /// DFN). Each perimeter `Lead`'s outline is the terminal.
    Flush {
        /// Terminal thickness visible on the sawn edge (A3).
        thickness: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MechanicalGeometry {
    pub body: Body,
    pub leads: Vec<Lead>,
    pub markers: Vec<Marker>,
    /// `None` for families whose leads aren't described by a `LeadForm`
    /// (chip terminations, through-hole wires, BGA balls).
    pub lead_form: Option<LeadForm>,
}

#[derive(Debug, thiserror::Error)]
pub enum McadError {
    #[error(
        "package family \"{0}\" has no supported generator (lqfp, qfn, chip, soic, sot, radial, axial, to92, dip, sip, bga are implemented)"
    )]
    UnsupportedFamily(String),
    #[error("package.geometry.generator \"{0}\" does not match this package's family")]
    UnsupportedGenerator(String),
    #[error("missing required dimension: {0}")]
    MissingDimension(&'static str),
    /// The package's dimensions contradict each other (e.g. a lead span
    /// no larger than the body), so no geometry can satisfy them.
    #[error("inconsistent package dimensions: {0}")]
    InconsistentDimensions(String),
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
    /// Reused by both `sot` (where `layout` must sum to `lead_count`)
    /// and `bga` (where `layout` is `[rows, cols]` and must multiply to
    /// `lead_count`) -- the message stays generic across both meanings.
    #[error("lead_layout {layout:?} is not valid for lead_count ({lead_count})")]
    InvalidLeadLayout { layout: Vec<u32>, lead_count: u32 },
    /// `bga`-specific: the row count exceeds this generator's supported
    /// single-letter row-designator range (A-Z minus I/O/Q/S, ~22 rows)
    /// -- see `bga.rs::row_letter`'s docs.
    #[error(
        "bga row count ({0}) exceeds this generator's supported row-letter range (~22 rows, \
         double-letter designators like \"AA\" are not implemented)"
    )]
    TooManyBgaRows(u32),
}
