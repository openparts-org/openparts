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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields = nullable))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExposedPadDimensions {
    pub width: Dimension,
    pub length: Dimension,
}

/// Canonical Data Specification section 20.
///
/// `body_width` runs along X and `body_length` along Y of the package's
/// own frame (the frame the Geometry Generator lays leads out in), and
/// every `overall_*` span below is measured along the same axis as the
/// body dimension of the same name. The lead dimensions are the
/// datasheet's package-outline symbols (JEDEC naming in brackets); a
/// field the datasheet doesn't state stays `None` -- the generator then
/// falls back to its own documented family default, which is never
/// written back here (section 7).
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields = nullable))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageDimensions {
    pub body_width: Dimension,
    pub body_length: Dimension,
    /// Molded body thickness, seating-plane independent (JEDEC A2 for
    /// leaded packages; A for leadless packages that state no A2).
    #[serde(default)]
    pub body_height: Option<Dimension>,
    #[serde(default)]
    pub exposed_pad: Option<ExposedPadDimensions>,
    /// Lead-tip-to-lead-tip span along X, including the leads (JEDEC E
    /// for QFP/SOIC when `body_width` is E1; HE on SOT drawings).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall_width: Option<Dimension>,
    /// Lead-tip-to-lead-tip span along Y, including the leads (JEDEC D
    /// for QFP when `body_length` is D1; HD/HE on SOT drawings).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall_length: Option<Dimension>,
    /// Seating plane to the top of the package (JEDEC A).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall_height: Option<Dimension>,
    /// Seating plane to the underside of the body (JEDEC A1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standoff: Option<Dimension>,
    /// Lead / terminal width, across the lead (JEDEC b).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_width: Option<Dimension>,
    /// Length of the lead's flat foot on the seating plane for gull-wing
    /// leads, or of the terminal for leadless packages (JEDEC L); for
    /// chip parts, the end-termination band length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_length: Option<Dimension>,
    /// Lead-frame thickness (JEDEC c; A3 terminal thickness on QFN/DFN).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_thickness: Option<Dimension>,
    /// Width of a package's wide tab, measured along the body edge it
    /// leaves (SOT-223 b2/b1, TO-252 E1/b3) -- only for families with a
    /// tab (`sot223`, `dpak`); see [`Package::tab`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_width: Option<Dimension>,
    /// How far a TO-252/DPAK tab reaches past the body edge (JEDEC L3).
    /// Not used by `sot223`, whose tab is a gull-wing lead reaching the
    /// same lead span as the leads opposite it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_length: Option<Dimension>,
    /// `module`: centre-to-centre distance between the plated holes of
    /// the two pin rows (Raspberry Pi Pico: 17.78). The footprint pads
    /// come from `overall_width`/`lead_*`; this places the 3D holes.
    /// `header`/`socket` (and the shrouded/right-angle headers): centre
    /// distance between adjacent pin rows
    /// (across the rows, along X).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_spacing: Option<Dimension>,
    /// `module`: diameter of the plated pin holes, and of the half-holes
    /// (castellations) cut by the board edge. Headers and sockets: the
    /// source's recommended PCB hole (the footprint drill).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hole_diameter: Option<Dimension>,
    /// `module` only: size along X of each pad in [`Package::aux_pads`]'s
    /// row (e.g. the Pico's SWD debug pads).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aux_pad_width: Option<Dimension>,
    /// `module` only: size along Y of each auxiliary pad.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aux_pad_length: Option<Dimension>,
    /// `module` only: from the board's left edge (the pin-1 side) to the
    /// centre of the auxiliary pad row, along X.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aux_row_x: Option<Dimension>,
    /// `module` only: from the board's bottom edge (the end away from
    /// pin 1) to the auxiliary pads' centres, along Y.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aux_row_y: Option<Dimension>,
    /// `module` only: diameter of the board's mounting holes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mounting_hole_diameter: Option<Dimension>,
    /// `module` only: centre-to-centre distance between the mounting
    /// holes along X (the pair at each end of the board).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mounting_hole_spacing: Option<Dimension>,
    /// `module` only: from each short board edge to the mounting holes'
    /// centres, along Y.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mounting_hole_inset: Option<Dimension>,
    /// `header`: how far each pin reaches above the housing's top face
    /// (the mating post); `right_angle_header`: how far the bent pin
    /// reaches beyond the housing's front (+X) face.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mating_length: Option<Dimension>,
    /// Headers and sockets: how far each pin (the solder tail) reaches
    /// below the housing's underside, into the board.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_length: Option<Dimension>,
    /// Headers and sockets: diameter of the source's recommended round
    /// land around each hole (`hole_diameter`). Absent: the generator's
    /// annular-ring default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pad_diameter: Option<Dimension>,
    /// `shrouded_header`/`right_angle_header`: signed distance along X
    /// from the pin grid's centre line to the housing's centre (positive
    /// towards +X, in the top view with pin 1 at the top-left). A JST XH
    /// housing sits off-centre around its pin row; a right-angle
    /// header's housing lies beside its holes. Absent: the housing is
    /// centred on the grid (not allowed for `right_angle_header`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_offset: Option<Dimension>,
    /// `lga`: body edge to each terminal's outer end (terminals set in
    /// from the edge). Absent: flush with the edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_inset: Option<Dimension>,
    /// Switches: diameter of the round actuator (plunger) on top.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actuator_diameter: Option<Dimension>,
}

impl PackageDimensions {
    /// Every present dimension with its JSON Pointer path under the
    /// Package document (for validation and provenance checks).
    pub fn entries(&self) -> Vec<(&'static str, &Dimension)> {
        let mut out = vec![
            ("/dimensions/body_width", &self.body_width),
            ("/dimensions/body_length", &self.body_length),
        ];
        let optional = [
            ("/dimensions/body_height", &self.body_height),
            ("/dimensions/overall_width", &self.overall_width),
            ("/dimensions/overall_length", &self.overall_length),
            ("/dimensions/overall_height", &self.overall_height),
            ("/dimensions/standoff", &self.standoff),
            ("/dimensions/lead_width", &self.lead_width),
            ("/dimensions/lead_length", &self.lead_length),
            ("/dimensions/lead_thickness", &self.lead_thickness),
            ("/dimensions/tab_width", &self.tab_width),
            ("/dimensions/tab_length", &self.tab_length),
            ("/dimensions/row_spacing", &self.row_spacing),
            ("/dimensions/hole_diameter", &self.hole_diameter),
            ("/dimensions/aux_pad_width", &self.aux_pad_width),
            ("/dimensions/aux_pad_length", &self.aux_pad_length),
            ("/dimensions/aux_row_x", &self.aux_row_x),
            ("/dimensions/aux_row_y", &self.aux_row_y),
            (
                "/dimensions/mounting_hole_diameter",
                &self.mounting_hole_diameter,
            ),
            (
                "/dimensions/mounting_hole_spacing",
                &self.mounting_hole_spacing,
            ),
            ("/dimensions/mounting_hole_inset", &self.mounting_hole_inset),
            ("/dimensions/mating_length", &self.mating_length),
            ("/dimensions/tail_length", &self.tail_length),
            ("/dimensions/pad_diameter", &self.pad_diameter),
            ("/dimensions/body_offset", &self.body_offset),
            ("/dimensions/lead_inset", &self.lead_inset),
            ("/dimensions/actuator_diameter", &self.actuator_diameter),
        ];
        out.extend(
            optional
                .into_iter()
                .filter_map(|(path, dim)| dim.as_ref().map(|d| (path, d))),
        );
        if let Some(ep) = &self.exposed_pad {
            out.push(("/dimensions/exposed_pad/width", &ep.width));
            out.push(("/dimensions/exposed_pad/length", &ep.length));
        }
        out
    }
}

/// Canonical Data Specification section 20: the package's physical
/// construction, as the source document states it -- a closed vocabulary
/// of facts that change how the part looks (and is handled), never a
/// guess from its size or name. Absent when the source doesn't say.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageConstruction {
    /// Transfer-molded plastic/epoxy body (most ICs, SOD/SMA diodes,
    /// DO-41 "molded plastic" axial diodes).
    Molded,
    /// Molded case around a solid-tantalum capacitor element (KEMET
    /// T49x and similar), anode-marked.
    TantalumMolded,
    /// Hermetically sealed glass body (e.g. DO-35 small-signal diodes).
    Glass,
    /// Fired ceramic body with plated end terminations (MLCC, ceramic
    /// chip inductors/fuses).
    Ceramic,
    /// Metal can (crystals, aluminium electrolytics).
    MetalCan,
}

/// The wide tab of a power package (SOT-223, TO-252/DPAK) -- soldered
/// to the board and electrically one of the device's terminals. How the
/// datasheet numbers it varies by vendor ("4", "TAB", or simply tied to
/// pin 2), which is why it is recorded here rather than derived; packages
/// whose outline is identical but whose tab is numbered differently are
/// separate Packages (as KiCad's `SOT-223-3_TabPin2` / `_TabPin4`).
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields = nullable))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageTab {
    /// Pad number of the tab as the source numbers it. It may repeat a
    /// lead's number when the source says the tab is that pin (a
    /// footprint then has two pads with that number).
    pub number: String,
    /// Lead positions cut short at the body, so they never reach the
    /// board and get no pad (TO-252's centre lead, which is the tab's own
    /// lead-frame finger). Numbers out of `1..=lead_count`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trimmed_leads: Vec<String>,
}

/// One pad of an explicit land pattern (`Package::pads`), in mm, top
/// view, X right and Y up (the family documents the origin). `width` is
/// the pad along X, `length` along Y. A plated hole has a `drill`
/// diameter; a slotted hole also a `drill_length` (the slot's overall
/// length, along the pad's longer side).
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields = nullable))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackagePad {
    pub number: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub length: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drill: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drill_length: Option<f64>,
    /// `false` for a non-plated hole (an alignment peg): `drill` is the
    /// hole, `width`/`length` its size, and the pad has no copper (its
    /// number may be empty).
    #[serde(default = "plated_default", skip_serializing_if = "is_plated")]
    pub plated: bool,
}

fn plated_default() -> bool {
    true
}

fn is_plated(p: &bool) -> bool {
    *p
}

/// Canonical Data Specification section 21.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryType {
    Parametric,
    Procedural,
    Handcrafted,
    External,
    None,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields = nullable))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageGeometrySpec {
    #[serde(rename = "type")]
    pub geometry_type: GeometryType,
    /// Name of the CAD-independent geometry generator (e.g. "lqfp"). Not
    /// a KiCad/STEP generator name — see openparts-mcad.
    #[serde(default)]
    pub generator: Option<String>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields = nullable))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub schema_version: String,
    pub kind: Kind,
    pub id: PackageId,
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub family: crate::family::PackageFamily,
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
    /// evenly and don't use this field). `bga` and the header families
    /// use it as `[rows, columns]` (a header's columns are its pins per
    /// row).
    #[serde(default)]
    pub lead_layout: Option<Vec<u32>>,
    /// The package's wide tab, for families that have one (`sot223`,
    /// `dpak`); `None` otherwise. Its size is in `dimensions.tab_*`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<PackageTab>,
    /// `module` only: pad numbers of an extra row of pads beyond the
    /// `lead_count` edge pins (the Raspberry Pi Pico's SWD debug pads
    /// "D1", "D2", "D3"), as the source numbers them, left to right in
    /// the top view, at `pitch`. Their size and place are in
    /// `dimensions.aux_*`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aux_pads: Vec<String>,
    /// Pad numbers in the generator's position order, when the source
    /// numbers the positions differently from the family convention
    /// (tactile switches: vendors number the four corners in different
    /// orders). Must hold exactly `lead_count` entries; empty means the
    /// convention's own numbers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pad_order: Vec<String>,
    /// Explicit land pattern (`usb_c`): every pad as the source's
    /// recommended PCB layout gives it, in the family's frame.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pads: Vec<PackagePad>,
    /// Physical construction as the source states it (section 20);
    /// `None` when not recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construction: Option<PackageConstruction>,
    #[serde(default)]
    pub geometry: Option<PackageGeometrySpec>,
    #[serde(default, deserialize_with = "deserialize_no_dup_map")]
    pub provenance: ProvenanceMap,
}
