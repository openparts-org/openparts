//! openparts-mcad: Package -> Geometry Generator -> Mechanical Geometry
//! (Architecture Specification section 15). `MechanicalGeometry` is the
//! common representation that openparts-solidcraft (and through it
//! openparts-step/openparts-stl) derives its output from; STEP/STL/3MF are
//! never themselves the canonical geometry.

mod axial;
mod bga;
mod chip;
mod connector;
mod defaults;
mod diode;
mod dip;
mod electrolytic;
mod header;
mod ipc;
mod land;
mod led;
mod lqfp;
mod module;
mod qfn;
mod radial;
mod sip;
mod soic;
mod sot;
mod tab;
mod tact;
mod to220;
mod to92;
mod usb_c;

pub use connector::ConnectorFeatures;
pub use electrolytic::ElectrolyticFeatures;
pub use header::{HeaderFeatures, HeaderKind, HeaderSmt, Shroud};
pub use land::pad_shape_error;
pub use module::{ModuleFeatures, ModuleHoles, ModuleShield, MountingHoles};
pub use openparts_core::{Orientation, PackageFamily};
pub use tact::SwitchFeatures;
pub use to220::To220Features;
pub use usb_c::UsbCFeatures;

use openparts_core::Package;

/// Generates `package`'s Mechanical Geometry with its family's
/// generator. The match is exhaustive: a family added to
/// [`PackageFamily`] needs a generator here before the crate compiles.
pub fn generate(package: &Package) -> Result<MechanicalGeometry, McadError> {
    use PackageFamily as F;
    if let Some(generator) = package.geometry.as_ref().and_then(|g| g.generator.as_ref()) {
        if !package.family.is_named(generator) {
            return Err(McadError::UnsupportedGenerator(generator.clone()));
        }
    }
    let mut geometry = match package.family {
        F::Lqfp => lqfp::generate_lqfp(package),
        F::Qfn | F::Dfn | F::Son | F::Lga => qfn::generate_qfn(package),
        F::Soic | F::Tssop | F::Ssop | F::Msop => soic::generate_soic(package),
        F::Sot => sot::generate_sot(package),
        F::Sot223 => tab::generate_sot223(package),
        F::Dpak => tab::generate_dpak(package),
        F::D2pak => tab::generate_d2pak(package),
        F::Sod | F::SodFlat => diode::generate_sod(package),
        F::Do214 => diode::generate_do214(package),
        F::To220 => to220::generate_to220(package),
        F::Chip => chip::generate_chip(package),
        F::Radial => radial::generate_radial(package),
        F::Axial => axial::generate_axial(package),
        F::To92 => to92::generate_to92(package),
        F::Dip => dip::generate_dip(package),
        F::Sip => sip::generate_sip(package),
        F::Bga => bga::generate_bga(package),
        F::Module => module::generate_module(package),
        F::Header | F::Socket | F::ShroudedHeader | F::RightAngleHeader => {
            header::generate_header(package)
        }
        F::TactSwitchSmd | F::TactSwitchTht => tact::generate_tact_switch(package),
        F::UsbC => usb_c::generate_usb_c(package),
        F::CardSocket
        | F::BarrelJack
        | F::TerminalBlock
        | F::FpcConnector
        | F::RfCoax
        | F::MicroUsbB => connector::generate_connector(package),
        F::SmdLed => led::generate_smd_led(package),
        F::SmdElectrolytic => electrolytic::generate_smd_electrolytic(package),
    }?;
    renumber(&mut geometry, package)?;
    // `usb_c` and the connector families draw their leads from the pads.
    if !package.pads.is_empty()
        && package.family != F::UsbC
        && !openparts_core::is_connector_family(package.family)
    {
        geometry.lands = land::from_pads(package)?;
    } else if let Some(lands) = ipc::lands(package, &geometry) {
        geometry.lands = lands;
    }
    exposed_pad_land(&mut geometry, package)?;
    Ok(geometry)
}

/// The exposed pad's land: `dimensions.exposed_pad.land_*` when the
/// source recommends a land of its own (each side independently), else
/// the pad itself.
fn exposed_pad_land(geometry: &mut MechanicalGeometry, package: &Package) -> Result<(), McadError> {
    let Some(ep) = &package.dimensions.exposed_pad else {
        return Ok(());
    };
    let w = ep.land_width.as_ref().and_then(defaults::required);
    let l = ep.land_length.as_ref().and_then(defaults::required);
    if w.is_none() && l.is_none() {
        return Ok(());
    }
    if geometry.lands.is_empty() {
        geometry.lands = geometry.leads.clone();
    }
    let Some(land) = geometry.lands.iter_mut().find(|l| l.number == "EP") else {
        return Ok(());
    };
    if let Some(w) = w {
        land.size.x = w;
    }
    if let Some(l) = l {
        land.size.y = l;
    }
    if land.size.x <= 0.0 || land.size.y <= 0.0 {
        return Err(McadError::InconsistentDimensions(
            "dimensions.exposed_pad.land_* must be positive".into(),
        ));
    }
    Ok(())
}

/// Applies `package.pad_order`: the leads numbered "1"..="lead_count" by
/// the family's position convention take the source's numbers instead.
fn renumber(geometry: &mut MechanicalGeometry, package: &Package) -> Result<(), McadError> {
    let order = &package.pad_order;
    if order.is_empty() {
        return Ok(());
    }
    if order.len() != package.lead_count as usize {
        return Err(McadError::InconsistentDimensions(format!(
            "pad_order has {} entries for lead_count {}",
            order.len(),
            package.lead_count
        )));
    }
    for lead in &mut geometry.leads {
        if let Ok(i) = lead.number.parse::<usize>() {
            if (1..=order.len()).contains(&i) {
                lead.number = order[i - 1].clone();
            }
        }
    }
    Ok(())
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
    /// (equal) are the diameter, `size.z` is the height. Unequal, the
    /// section is a stadium (an oval can such as an HC-49/U crystal):
    /// `size.x` by `size.y`, half circles on the shorter side. Deliberately
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
    /// A non-plated hole (alignment peg): no copper, `drill` is the hole.
    NonPlated,
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
    /// Slotted hole: the slot's overall length (mm), along the pad's
    /// longer side; `drill` is then the slot width. `None` for a round
    /// hole.
    pub slot: Option<f64>,
    /// The footprint land's outline within `size` (a lead's own 3D
    /// shape ignores it): a rectangle unless the source draws another.
    pub shape: LandShape,
}

/// A footprint land's outline (Canonical Data Specification 20.7).
/// `size` stays the land's extent.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum LandShape {
    /// The full `size` rectangle.
    #[default]
    Rect,
    /// A round land (`size` x = y, the diameter).
    Circle,
    /// The rectangle with 45° cuts of `size` (mm, along both edges) at
    /// `corners` (top view).
    Chamfered {
        size: f64,
        corners: Vec<openparts_core::PadCorner>,
    },
    /// An outline relative to the lead's position (mm),
    /// counter-clockwise, the last point not repeated.
    Polygon(Vec<[f64; 2]>),
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
    /// Gull-wing leads (QFP, SOIC/SOP/TSSOP/SSOP/MSOP, SOT, SOT-223,
    /// TO-252): out of the body side, bent
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
    /// Flat leads leaving the body underside straight out, on the
    /// seating plane (SOD-123F/FL, SOD-323F). Each `Lead`'s outline runs
    /// from the body edge to the lead toe.
    Flat {
        /// Lead-frame thickness (c).
        thickness: f64,
    },
    /// J-bend leads (DO-214 SMA/SMB/SMC): out of the body end, down the
    /// end face and folded under the body. Each `Lead`'s outline is the
    /// folded foot, from the overall end inward by `foot_length`.
    JBend {
        /// Lead-frame thickness (c).
        thickness: f64,
        /// Folded foot length under the body, from the overall end (L).
        foot_length: f64,
    },
}

/// What a family's detailed 3D model needs beyond the body and leads,
/// resolved here (datasheet value, else this crate's default) so the 3D
/// builder never reads the Package's dimensions itself.
#[derive(Debug, Clone, PartialEq)]
pub enum Details {
    None,
    Header(HeaderFeatures),
    Module(ModuleFeatures),
    To220(To220Features),
    Switch(SwitchFeatures),
    UsbC(UsbCFeatures),
    /// The connector families (`connector.rs`).
    Connector(ConnectorFeatures),
    /// `smd_electrolytic`: the can and the base plate.
    Electrolytic(ElectrolyticFeatures),
    /// TO-252/TO-263: lead positions cut short at the body, as (lead
    /// number, Y position).
    TrimmedLeads(Vec<(u32, f64)>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MechanicalGeometry {
    pub family: PackageFamily,
    /// The lead pitch the geometry was drawn at, when the package has one.
    pub pitch: Option<f64>,
    pub details: Details,
    /// The footprint's pads: the package's stated land pattern
    /// (`Package::pads`), else IPC-7351B lands computed from the SMD
    /// terminals (`ipc.rs`); empty means the footprint pads are the
    /// leads' outlines (through-hole, LGA, USB-C).
    pub lands: Vec<Lead>,
    pub body: Body,
    pub leads: Vec<Lead>,
    pub markers: Vec<Marker>,
    /// `None` for families whose leads aren't described by a `LeadForm`
    /// (chip terminations, through-hole wires, BGA balls).
    pub lead_form: Option<LeadForm>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum McadError {
    /// A generator was handed a family it doesn't draw (an internal
    /// dispatch error).
    #[error("this generator does not draw package family \"{0}\"")]
    UnsupportedFamily(PackageFamily),
    #[error("package.geometry.generator \"{0}\" does not match this package's family")]
    UnsupportedGenerator(String),
    #[error("missing required dimension: {0}")]
    MissingDimension(&'static str),
    /// The package's dimensions contradict each other (e.g. a lead span
    /// no larger than the body), so no geometry can satisfy them.
    #[error("inconsistent package dimensions: {0}")]
    InconsistentDimensions(String),
    /// `sot223`/`dpak`: the package has no `tab`, so the tab's pad number
    /// (which vendors assign differently) is unknown.
    #[error("family \"{family}\" requires a tab (its pad number as the source gives it)")]
    MissingTab { family: PackageFamily },
    #[error("invalid package tab: {0}")]
    InvalidTab(String),
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
    MissingLeadLayout { family: PackageFamily },
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
