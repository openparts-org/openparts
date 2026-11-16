//! openparts-pcbcad: the CAD-independent PCB Symbol/Footprint
//! Intermediate Representation (Architecture Specification section 12).
//! EDA-specific concepts stay in the adapter crates (openparts-kicad,
//! openparts-librepcb); this crate only knows about
//! Device pins and Mechanical Geometry leads.

mod footprint;
mod outline;
mod svg;

pub use outline::COURTYARD_WIDTH;
pub use svg::render_svg;
mod symbol;

pub use footprint::{add_shared_pads, build_footprint, footprint_from_placement};
pub use openparts_core::PadSide;
pub use openparts_placement;
pub use symbol::{build_symbol, reference_prefix};

use openparts_core::PinType;
pub(crate) use openparts_mcad::Mounting;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinElectricalType {
    Input,
    Output,
    Bidirectional,
    PowerIn,
    PowerOut,
    Passive,
    NoConnect,
    Unspecified,
}

impl From<PinType> for PinElectricalType {
    fn from(t: PinType) -> Self {
        match t {
            PinType::Power | PinType::Ground => PinElectricalType::PowerIn,
            PinType::PowerOut => PinElectricalType::PowerOut,
            PinType::Input | PinType::Clock | PinType::Reset => PinElectricalType::Input,
            PinType::Output | PinType::ClockOut => PinElectricalType::Output,
            PinType::Io => PinElectricalType::Bidirectional,
            PinType::Analog | PinType::Reserved | PinType::Passive => PinElectricalType::Passive,
            PinType::Nc => PinElectricalType::NoConnect,
            PinType::Unknown => PinElectricalType::Unspecified,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinOrientation {
    /// Pin stub points toward +X, pin sits on the symbol's left edge.
    Left,
    /// Pin stub points toward -X, pin sits on the symbol's right edge.
    Right,
    /// Pin stub points toward +Y, pin sits on the symbol's bottom edge.
    Bottom,
    /// Pin stub points toward -Y, pin sits on the symbol's top edge.
    Top,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolPin {
    pub number: String,
    pub name: String,
    pub electrical_type: PinElectricalType,
    pub position: Point2,
    pub length: f64,
    pub orientation: PinOrientation,
    /// Which KiCad symbol unit this pin belongs to. `1` for an ordinary
    /// (non-split) symbol. `0` is KiCad's "common to every unit" sentinel
    /// -- used only when [`build_symbol`] splits a very large pin bus
    /// across multiple units (see its docs), to hold the pins (power,
    /// ground, misc signals) shared by every numbered unit.
    pub unit: u32,
    /// True for every member of a stacked name-group (see
    /// `group_by_exact_name`) after the first: KiCad draws every pin's
    /// name/number text at its own `(at ...)` coordinate with no
    /// overlap-avoidance of its own, so multiple *visible* pins stacked
    /// at the identical position render as garbled overlapping text --
    /// confirmed directly against KiCad's own official RP2040 symbol
    /// (gitlab.com/kicad/libraries/kicad-symbols
    /// `MCU_RaspberryPi.kicad_symdir/RP2040.kicad_sym`): only the first
    /// pin in each stack (e.g. IOVDD pin 1) is visible; the other five
    /// IOVDD pins (10/22/33/42/49) each carry `(hide yes)`. Hidden pins
    /// are still fully real, individually-numbered connection points --
    /// KiCad just doesn't draw their text -- so nothing about netlist
    /// correctness changes, only what's drawn.
    pub hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphicKind {
    Rectangle { start: Point2, end: Point2 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Graphic {
    pub kind: GraphicKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolUnit {
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcbSymbol {
    pub name: String,
    /// Reference designator prefix ("U", "R", ...).
    pub reference: String,
    /// The symbol's value (the MPN).
    pub value: String,
    /// The footprint's library id (`openparts:<file name>`).
    pub footprint: String,
    /// A datasheet URL, when the caller knows one.
    pub datasheet: String,
    pub units: Vec<SymbolUnit>,
    pub pins: Vec<SymbolPin>,
    pub graphics: Vec<Graphic>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PadShape {
    Rect,
    Circle,
    /// Obround: a through-hole pad longer than it is wide (slotted
    /// shell tabs).
    Oval,
    /// The `size` rectangle with 45° cuts of `size` (mm, along both
    /// edges) at `corners` (Y up: `TopLeft` is at -x, +y).
    Chamfered {
        size: f64,
        corners: Vec<PadCorner>,
    },
    /// An outline relative to the pad's position (mm, Y up),
    /// counter-clockwise; `size` is its extent.
    Polygon(Vec<Point2>),
}

pub use openparts_placement::PadCorner;

impl PadShape {
    /// The outline of a `Chamfered` or `Polygon` pad of size `(w, h)`,
    /// relative to its position (Y up, counter-clockwise); `None` for
    /// the shapes every tool draws natively.
    pub fn outline(&self, (w, h): (f64, f64)) -> Option<Vec<Point2>> {
        match self {
            PadShape::Rect | PadShape::Circle | PadShape::Oval => None,
            PadShape::Polygon(points) => Some(points.clone()),
            PadShape::Chamfered { size, corners } => {
                let (hw, hh) = (w / 2.0, h / 2.0);
                let mut out = Vec::new();
                // Counter-clockwise from the bottom-left corner.
                for (corner, (sx, sy)) in [
                    (PadCorner::BottomLeft, (-1.0, -1.0)),
                    (PadCorner::BottomRight, (1.0, -1.0)),
                    (PadCorner::TopRight, (1.0, 1.0)),
                    (PadCorner::TopLeft, (-1.0, 1.0)),
                ] {
                    let (x, y) = (sx * hw, sy * hh);
                    if corners.contains(&corner) {
                        // The cut's two ends, in walking order: the
                        // side arriving at the corner, then the leaving one.
                        let along_x = (sx * sy) < 0.0;
                        let (a, b) = if along_x {
                            ((x - sx * size, y), (x, y - sy * size))
                        } else {
                            ((x, y - sy * size), (x - sx * size, y))
                        };
                        out.push(Point2 { x: a.0, y: a.1 });
                        out.push(Point2 { x: b.0, y: b.1 });
                    } else {
                        out.push(Point2 { x, y });
                    }
                }
                Some(out)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pad {
    pub number: String,
    pub position: Point2,
    pub size: (f64, f64),
    pub shape: PadShape,
    /// SMD or through-hole -- taken straight from the generating
    /// `Lead`'s own `mounting` (already a dependency of this crate).
    pub mounting: openparts_mcad::Mounting,
    /// Drill diameter (mm). `Some` only when `mounting` is
    /// `ThroughHole`.
    pub drill: Option<f64>,
    /// Slot length (mm) for a slotted hole, along the pad's longer side
    /// (`drill` is the slot width); `None` for a round hole.
    pub slot: Option<f64>,
    /// The board side of an SMD land (`Bottom`: on the opposite outer
    /// layer, seen from the top like the rest of the footprint).
    pub side: openparts_core::PadSide,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Courtyard {
    pub outline: Vec<Point2>,
}

/// A footprint drawing layer (the courtyard is [`Courtyard`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpLayer {
    /// Silkscreen: printed on the board.
    Silk,
    /// Fabrication / documentation: the body outline for assembly drawings.
    Fab,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FpShape {
    Line(Point2, Point2),
    /// Closed.
    Polygon(Vec<Point2>),
    Circle {
        center: Point2,
        radius: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FpGraphic {
    pub layer: FpLayer,
    /// Stroke width (mm).
    pub width: f64,
    pub shape: FpShape,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcbFootprint {
    pub name: String,
    pub pads: Vec<Pad>,
    pub graphics: Vec<FpGraphic>,
    pub courtyard: Option<Courtyard>,
    /// Optional 3D model attached to the footprint. [`build_footprint`]
    /// fills this in by default (see [`default_model_path`]); `None`
    /// means "no 3D model reference is emitted".
    pub model: Option<Model3d>,
    /// The Placement Model's content hash (`sha256:...`) this footprint
    /// was built from, when built from a part's full model: a board tool
    /// compares it with the current model to find stale footprints.
    pub placement_hash: Option<String>,
}

impl PcbFootprint {
    /// Replaces (or sets) the footprint's 3D model reference.
    pub fn with_model(mut self, model: Model3d) -> Self {
        self.model = Some(model);
        self
    }

    /// Drops the footprint's 3D model reference.
    pub fn without_model(mut self) -> Self {
        self.model = None;
        self
    }
}

/// A 3D model reference attached to a footprint (CAD-independent).
///
/// `path` is passed through to the EDA adapter verbatim (it may contain
/// tool-specific variables such as KiCad's `${KIPRJMOD}`); the adapter
/// is responsible only for quoting/escaping it. The transform is the
/// one applied to the model relative to the footprint origin, in the
/// footprint's Y-up frame: `offset` in mm, `scale` as a unitless
/// factor, `rotate` in degrees counter-clockwise (right-handed) about
/// X/Y/Z, applied before the offset (adapters convert to their tool's
/// conventions).
///
/// OpenParts' 3D models are generated in their geometry generator's
/// frame; the Placement Model's `model3d` says how that frame sits in
/// the footprint's (a quarter-turn and an offset to the land-pattern
/// centroid), which [`footprint_from_placement`] copies here.
#[derive(Debug, Clone, PartialEq)]
pub struct Model3d {
    pub path: String,
    pub offset: [f64; 3],
    pub scale: [f64; 3],
    pub rotate: [f64; 3],
}

impl Model3d {
    /// A model at `path` with the identity transform (offset 0, scale 1,
    /// rotation 0).
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            offset: [0.0; 3],
            scale: [1.0; 3],
            rotate: [0.0; 3],
        }
    }
}

/// File-name sanitisation for generated artifacts: an MPN may contain
/// path separators (e.g. `LPC1114FBD48/302`), which are replaced by
/// `-`. This is the single definition shared by every place that names
/// a file after a part -- it must stay byte-identical to the
/// `safe_filename` used by `openparts-cli` when it writes
/// `<name>.kicad_mod` / `<name>.step` (which the CLI can switch to
/// calling directly).
pub fn safe_filename(name: &str) -> String {
    name.replace(['/', '\\'], "-")
}

/// Default 3D model path for a footprint named `footprint_name`:
/// the bare, relative file name `<safe_filename(name)>.step`.
///
/// Why a bare relative path (and not `${KIPRJMOD}/...`, an absolute
/// path, or a custom `${OPENPARTS_3DMODEL_DIR}` variable):
///
/// * `openparts-cli generate` writes `<name>.kicad_mod` and
///   `<name>.step` into the same directory. KiCad's 3D path resolver
///   (`FILENAME_RESOLVER::ResolvePath`) expands env vars, then tries a
///   relative name against (1) the current project directory
///   (`${KIPRJMOD}`), then (2) the "working path", which since KiCad 7
///   ("3D model paths can now be resolved relative to a footprint")
///   is the footprint's own library directory, then (3) the legacy
///   `${KICADn_3DMODEL_DIR}`, then (4) user aliases. So on KiCad 7-9+
///   a bare name finds the STEP sitting next to the `.kicad_mod` as
///   soon as that directory is added as a footprint library -- with
///   no user configuration -- and on every version (including 6) it
///   also finds it if the STEP is dropped into the project directory.
/// * `${KIPRJMOD}/...` only works when the model lives inside the
///   project, which is not where the CLI writes it (the KiCad plugin
///   does install into the project, and it rewrites the path to its
///   own `${KIPRJMOD}/openparts_3d/...` location).
/// * A custom variable such as `${OPENPARTS_3DMODEL_DIR}` requires the
///   user to define it in Preferences > Configure Paths first;
///   unresolved, KiCad shows nothing. Absolute paths are not portable
///   and would make output depend on the machine (breaking
///   reproducibility).
///
/// Known KiCad caveat: some versions' *Footprint Editor* 3D preview
/// does not resolve library-relative paths (KiCad issue #21844), while
/// the board 3D viewer does. Callers that want a different convention
/// can override it with [`PcbFootprint::with_model`].
pub fn default_model_path(footprint_name: &str) -> String {
    format!("{}.step", safe_filename(footprint_name))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_chamfered_outline_cuts_only_its_corners_counter_clockwise() {
        use crate::{PadCorner, PadShape, Point2};
        let shape = PadShape::Chamfered {
            size: 0.25,
            corners: vec![PadCorner::TopRight, PadCorner::BottomLeft],
        };
        let pts = shape.outline((2.0, 1.0)).unwrap();
        let p = |x: f64, y: f64| Point2 { x, y };
        assert_eq!(
            pts,
            vec![
                p(-1.0, -0.25),
                p(-0.75, -0.5),
                p(1.0, -0.5),
                p(1.0, 0.25),
                p(0.75, 0.5),
                p(-1.0, 0.5),
            ]
        );
        assert_eq!(PadShape::Rect.outline((1.0, 1.0)), None);
    }

    use super::*;

    #[test]
    fn safe_filename_matches_cli_convention() {
        assert_eq!(safe_filename("LPC1114FBD48/302"), "LPC1114FBD48-302");
        assert_eq!(safe_filename("A\\B"), "A-B");
        assert_eq!(safe_filename("2N7002ET1G"), "2N7002ET1G");
    }
}
