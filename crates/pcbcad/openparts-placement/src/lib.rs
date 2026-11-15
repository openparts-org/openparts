//! openparts-placement: the OpenParts Placement Model
//! (docs/spec/placement-model.md) -- everything a board tool needs to put
//! a part on a board: the frame (origin, zero orientation, units), the
//! pads, the courtyard, the body, pin 1, the board-edge and keep-out
//! statements, the through-hole tails, how the 3D model aligns, and
//! where each of these came from.
//!
//! The model is built from the part's Mechanical Geometry
//! (`openparts_mcad::generate`) -- the same geometry the 3D model comes
//! from -- and is itself what `openparts-pcbcad` builds the footprint
//! from, so the KiCad footprint, the SVG preview and the board tool all
//! read one set of numbers.
//!
//! Frame (section 2 of the spec): mm, right-handed, top view +X right,
//! +Y up, +Z up from the board's top surface; origin at the land
//! pattern's centroid (IPC-7351B); zero orientation per IPC-7351B
//! Level A (pin 1 upper left). Placed on a board, a part's points map
//! as `board = T · R(θ) · M` ([`Placement`]).

mod build;
mod hash;
mod transform;

pub use build::{build, from_geometry};
pub use transform::{normalize, BoardSide, Placement};

use openparts_core::{DiffPolarity, DiffStandard, KeepoutPurpose, KeepoutRestriction, PinType};
use serde::{Deserialize, Serialize};

/// The model's schema version (`frame.schema_version`).
pub const SCHEMA_VERSION: &str = "1";

/// A point in the part frame (mm).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

/// An extent along X and Y (mm).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Size2 {
    pub x: f64,
    pub y: f64,
}

/// IPC-7351B density level: L (least), N (nominal), M (most).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Density {
    L,
    #[default]
    N,
    M,
}

impl Density {
    /// `L`, `N` or `M` (any case).
    pub fn parse(s: &str) -> Option<Density> {
        match s.trim().to_ascii_uppercase().as_str() {
            "L" => Some(Density::L),
            "N" => Some(Density::N),
            "M" => Some(Density::M),
            _ => None,
        }
    }

    pub fn letter(self) -> &'static str {
        match self {
            Density::L => "L",
            Density::N => "N",
            Density::M => "M",
        }
    }
}

/// Section 2: the frame every coordinate of the model is in.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub schema_version: String,
    /// Always `mm`.
    pub units: String,
    /// Always `land_pattern_centroid`: the centre of the extents of the
    /// plated lands (IPC-7351B).
    pub origin: String,
    /// Always `top_view_x_right_y_up_z_up`: right-handed, viewed from
    /// the top, Z up from the board's top surface (IPC-2581, STEP).
    pub axes: String,
    /// Always `ccw_from_top`: rotations are counter-clockwise viewed from
    /// the top.
    pub rotation: String,
    /// Always `board = T·R(θ)·M`: on the bottom side, `M` mirrors
    /// x → −x in this frame before the rotation.
    pub bottom_side: String,
    pub zero_orientation: ZeroOrientation,
    /// The density level this model's courtyard was built for.
    pub density: Density,
}

/// Section 3: how the part is turned at rotation 0.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZeroOrientation {
    /// Always `ipc7351b_level_a`.
    pub rule: String,
    /// The family's row of the zero-orientation table.
    pub family: String,
    pub convention: String,
    /// The pad the rule places (pin 1, A1).
    pub pin1: Option<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadShape {
    Rect,
    Roundrect,
    Circle,
    /// Obround (a slot-shaped land).
    Oval,
    Polygon,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadKind {
    Smd,
    /// Plated through hole.
    Tht,
    /// Non-plated hole (alignment peg): no copper.
    Npth,
}

/// Copper layers a pad is on.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopperLayers {
    /// The placement side's outer layer.
    Top,
    /// Every copper layer (a plated hole).
    All,
    None,
}

/// Solder mask openings of a pad.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskLayers {
    Top,
    Both,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PadLayers {
    pub copper: CopperLayers,
    pub mask: MaskLayers,
    /// A solder paste aperture on the placement side.
    pub paste: bool,
}

/// A pad's solder paste aperture.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Paste {
    /// The aperture is the land (the board's paste rules apply).
    Full,
    /// Windows inside the land (an exposed pad's segmented paste).
    Windowed { windows: Vec<Rect2> },
}

/// An axis-aligned rectangle (centre and size).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect2 {
    pub center: Point2,
    pub size: Size2,
}

/// The device pin a pad connects to.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PadPin {
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: PinType,
    /// The device pin's differential pair, when the source names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_pair: Option<PadDiffPair>,
}

/// Section 4: a pad's pin in a differential pair (the Device pin's
/// `diff_pair`, Canonical Data Specification section 14.2), with the pad
/// that carries the partner's copper.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PadDiffPair {
    /// The partner's pin number.
    pub partner: String,
    /// The pad of this footprint carrying the partner pin (`null` when
    /// the package has no pad for it).
    pub partner_pad: Option<String>,
    pub polarity: DiffPolarity,
    pub standard: DiffStandard,
    /// The alternate function the pair belongs to, if only in one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// A differential impedance stated by the part's own source (ohm);
    /// a standard's impedance is not given here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impedance_ohm: Option<f64>,
}

/// Section 4: one pad.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pad {
    /// The pad number as the source numbers it ("" for a non-plated hole).
    pub number: String,
    pub shape: PadShape,
    pub position: Point2,
    /// Degrees CCW; `size` is before this rotation.
    pub rotation: f64,
    pub size: Size2,
    /// `roundrect`: corner radius / shorter side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_ratio: Option<f64>,
    /// `polygon`: the outline, relative to `position`, before `rotation`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Vec<Point2>>,
    pub kind: PadKind,
    pub layers: PadLayers,
    /// Hole diameter; a slot's width.
    pub drill: Option<f64>,
    /// A slot's overall length, along the pad's longer side.
    pub slot: Option<f64>,
    pub plated: bool,
    /// Solder mask expansion (mm); `None`: the board's rule.
    pub mask_expansion: Option<f64>,
    /// `None`: no paste (holes).
    pub paste: Option<Paste>,
    /// The device pin, when the model was built with the device.
    pub pin: Option<PadPin>,
    /// The package's exposed (thermal) pad.
    pub exposed: bool,
    /// Cut by the part's own edge (a module's half-hole).
    pub castellated: bool,
    /// This pad is another pad's contact overlaid under a second number
    /// (USB-C "A1 B12"): the number of that pad.
    pub shares_contact_with: Option<String>,
}

impl Pad {
    /// The pad's axis-aligned extent: (min, max) corners.
    pub fn bounds(&self) -> (Point2, Point2) {
        let quarter = (self.rotation / 90.0).round() as i64;
        let (w, h) = if quarter % 2 == 0 {
            (self.size.x, self.size.y)
        } else {
            (self.size.y, self.size.x)
        };
        (
            Point2 {
                x: self.position.x - w / 2.0,
                y: self.position.y - h / 2.0,
            },
            Point2 {
                x: self.position.x + w / 2.0,
                y: self.position.y + h / 2.0,
            },
        )
    }

    /// Copper on a board layer (not a non-plated hole).
    pub fn has_copper(&self) -> bool {
        self.layers.copper != CopperLayers::None
    }
}

/// Section 5: the courtyard.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Courtyard {
    pub density: Density,
    /// Excess around the lands and body (mm).
    pub excess: f64,
    /// Corners snapped outward to this grid (mm).
    pub grid: f64,
    /// Closed outlines (counter-clockwise), the last point not repeated.
    pub polygons: Vec<Vec<Point2>>,
}

/// The outline shape of a body region.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutlineShape {
    Rect,
    Circle,
    /// Two half circles joined by straight sides (an oval can).
    Stadium,
}

/// Section 6: one solid region of the part above the board.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BodyRegion {
    /// `body`, `pins` (header posts above the housing), ...
    pub name: String,
    pub shape: OutlineShape,
    pub center: Point2,
    pub size: Size2,
    /// The outline as a polygon (a circle as 32 segments).
    pub outline: Vec<Point2>,
    /// Above the board's top surface (mm).
    pub z_min: f64,
    pub z_max: f64,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub regions: Vec<BodyRegion>,
    /// Board surface to the body's underside (mm).
    pub standoff: f64,
    /// Board surface to the highest point (mm).
    pub height: f64,
}

/// Section 7: pin 1.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pin1 {
    pub pad: String,
    pub position: Point2,
    /// The pin-1 mark on the body (a dot), when the geometry has one.
    pub marker: Option<Point2>,
}

/// A side of the part in this frame.
pub use openparts_core::{EdgeRequirement, PackageSide as Side};

/// Section 8: the part at the board edge.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// The side facing the board edge (a mating face, an antenna end).
    pub side: Side,
    pub requirement: EdgeRequirement,
    /// Distance from the origin to the PCB edge line along `side`'s
    /// outward normal; `None` when the source draws no line.
    pub pcb_edge: Option<f64>,
    /// How far the body reaches past that line (negative: it stays
    /// short of it).
    pub overhang: Option<f64>,
}

/// Section 9: a keep-out area.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keepout {
    pub purpose: KeepoutPurpose,
    pub restricts: Vec<KeepoutRestriction>,
    /// The area, when the source locates it.
    pub polygon: Option<Vec<Point2>>,
    /// The two lengths as the source states them, when it doesn't
    /// locate the area (orientation not given).
    pub size: Option<[f64; 2]>,
    /// The part side the area lies at, when stated.
    pub side: Option<Side>,
}

/// Section 10: through-hole leads.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tht {
    /// How far the leads reach below the board's top surface (mm): the
    /// protrusion below the bottom surface is this less the board
    /// thickness.
    pub tail_length: f64,
}

/// Section 11: how the generated 3D model (STEP / 3MF / STL) sits in
/// this frame: `p_frame = R(rotation) · p_model + offset`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model3d {
    /// Degrees CCW about +Z.
    pub rotation: f64,
    /// mm, Z included (the model's seating plane is z = 0, the board's
    /// top surface).
    pub offset: [f64; 3],
}

/// Where a field group's numbers come from (section 12).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasisKind {
    /// Stated by the source document (see `sources`).
    Datasheet,
    /// Calculated per IPC-7351B from stated dimensions.
    Ipc7351,
    /// The generator's documented default.
    #[default]
    GeneratorDefault,
}

/// A provenance pointer: the Package field and the source it cites.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceRef {
    /// JSON Pointer into the Package document.
    pub pointer: String,
    pub source: String,
    pub page: Option<u32>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BasisEntry {
    pub kind: BasisKind,
    pub sources: Vec<SourceRef>,
    /// Inputs the source doesn't state, filled by generator defaults.
    pub defaults: Vec<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Basis {
    pub pads: BasisEntry,
    pub courtyard: BasisEntry,
    pub body: BasisEntry,
    pub edge: Option<BasisEntry>,
    pub keepouts: Option<BasisEntry>,
    pub tht: Option<BasisEntry>,
}

/// The Placement Model of one part (or of one package's geometry).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacementModel {
    pub frame: Frame,
    /// The Package document id, when built from one.
    pub package: Option<String>,
    /// The lead pitch, when the package has one (mm).
    pub pitch: Option<f64>,
    /// The smallest copper-to-copper gap between two pads (mm).
    pub min_pad_gap: Option<f64>,
    pub pads: Vec<Pad>,
    pub courtyard: Courtyard,
    pub body: Body,
    pub pin1: Option<Pin1>,
    pub edge: Option<Edge>,
    pub keepouts: Vec<Keepout>,
    pub tht: Option<Tht>,
    pub model3d: Model3d,
    pub basis: Basis,
    /// `sha256:<hex>` of the geometry (section 13): stale footprints
    /// carry another one.
    pub hash: String,
}

/// Rounds to 0.1 µm (the model's resolution), without a negative zero.
pub(crate) fn r4(v: f64) -> f64 {
    (v * 1e4).round() / 1e4 + 0.0
}
