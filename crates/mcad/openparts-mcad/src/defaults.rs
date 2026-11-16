//! The one place package-shape defaults live (generators keep no
//! numeric defaults of their own), and how they combine with
//! the package's own dimensions: a datasheet value always wins over a
//! family default. Footprint pads (through `Lead`) and the detailed 3D
//! model (through `LeadForm`) are both built from what this module
//! resolves, so they cannot disagree.
//!
//! Every default here is a generator implementation detail for a
//! geometric approximation -- JEDEC-typical where a JEDEC outline exists
//! -- and is never written back into Canonical Data as a fact (Canonical
//! Data Specification section 7: never invent a nominal).

use crate::{LeadForm, McadError};
use openparts_core::{Dimension, Package};

/// Defaults for one gull-wing family.
pub(crate) struct GullWingDefaults {
    /// Molded body thickness (A2).
    pub body_height: f64,
    /// Lead width (b).
    pub lead_width: f64,
    /// Body edge to lead toe, i.e. (E - E1) / 2.
    pub protrusion: f64,
    /// Lead-frame thickness (c).
    pub thickness: f64,
    /// Flat foot length (L).
    pub foot_length: f64,
    /// Standoff (A1).
    pub standoff: f64,
}

/// LQFP/TQFP (JEDEC MS-026): every MS-026 body size has E = E1 + 2.0 mm,
/// b 0.22 (0.5 mm pitch), c 0.09..0.20, L 0.45..0.75, A1 0.05..0.15.
pub(crate) const QFP: GullWingDefaults = GullWingDefaults {
    body_height: 1.4,
    lead_width: 0.22,
    protrusion: 1.0,
    thickness: 0.15,
    foot_length: 0.6,
    standoff: 0.1,
};

/// SOIC/SOP (JEDEC MS-012): E 6.0 over E1 3.9, b 0.31..0.51,
/// c 0.17..0.25, L 0.40..1.27, A1 0.10..0.25; height is MS-012's A max.
pub(crate) const SOIC: GullWingDefaults = GullWingDefaults {
    body_height: 1.75,
    lead_width: 0.4,
    protrusion: 1.05,
    thickness: 0.2,
    foot_length: 0.8,
    standoff: 0.15,
};

/// TSSOP (JEDEC MO-153, 4.4 mm body): E 6.4 BSC over E1 4.4, A2 0.80..1.05,
/// b 0.19..0.30, c 0.09..0.20, L 0.45..0.75, A1 0.05..0.15.
pub(crate) const TSSOP: GullWingDefaults = GullWingDefaults {
    body_height: 1.0,
    lead_width: 0.25,
    protrusion: 1.0,
    thickness: 0.15,
    foot_length: 0.6,
    standoff: 0.1,
};

/// SSOP (JEDEC MO-150, 5.3 mm body): E 7.4..8.2 over E1 5.3, A2 1.65..1.85,
/// b 0.22..0.38, c 0.09..0.25, L 0.55..0.95, A1 0.05 min.
pub(crate) const SSOP: GullWingDefaults = GullWingDefaults {
    body_height: 1.75,
    lead_width: 0.3,
    protrusion: 1.25,
    thickness: 0.15,
    foot_length: 0.75,
    standoff: 0.1,
};

/// MSOP (JEDEC MO-187, 3.0 mm body): E 4.9 BSC over E1 3.0, A2 0.75..0.95,
/// b 0.22..0.38 (0.65 pitch), c 0.08..0.23, L 0.40..0.80, A1 0.00..0.15.
pub(crate) const MSOP: GullWingDefaults = GullWingDefaults {
    body_height: 0.85,
    lead_width: 0.3,
    protrusion: 0.95,
    thickness: 0.15,
    foot_length: 0.6,
    standoff: 0.08,
};

/// SOT-223 (JEDEC TO-261AA): HE 6.7..7.3 over E 3.3..3.7, A2 ~1.6,
/// b 0.66..0.84, c 0.23..0.35, L 0.75 min, A1 0.02..0.10.
pub(crate) const SOT223: GullWingDefaults = GullWingDefaults {
    body_height: 1.6,
    lead_width: 0.75,
    protrusion: 1.75,
    thickness: 0.3,
    foot_length: 0.9,
    standoff: 0.06,
};

/// TO-252 / DPAK (JEDEC TO-252AA): H 9.40..10.42 over a 5.97..6.22 body
/// with the tab reaching L3 0.88..1.27 past it, A 2.18..2.39, b 0.64..0.89,
/// c 0.46..0.61, A1 0.00..0.13. `protrusion` is the lead side only
/// (H - D - L3 ~ 10.1 - 6.1 - 1.0).
pub(crate) const DPAK: GullWingDefaults = GullWingDefaults {
    body_height: 2.25,
    lead_width: 0.76,
    protrusion: 3.0,
    thickness: 0.5,
    foot_length: 1.0,
    standoff: 0.05,
};

/// TO-252 / DPAK tab defaults beyond `DPAK`.
pub(crate) struct TabDefaults {
    /// Body edge to tab end (L3).
    pub length: f64,
    /// Tab metal thickness (c2).
    pub thickness: f64,
    /// Soldered tab length under the body, as a fraction of the body
    /// size across the leads (TO-252 drawings give only a minimum, e.g.
    /// 4.32 min under a ~6.1 body).
    pub under_body_ratio: f64,
}

pub(crate) const DPAK_TAB: TabDefaults = TabDefaults {
    length: 1.0,
    thickness: 0.5,
    under_body_ratio: 0.7,
};

/// TO-263 / D2PAK (JEDEC TO-263AB): H 14.61..15.88 over an 8.38..9.65
/// body with the tab reaching L3 ~1.27 past it, A 4.06..4.83,
/// b 0.51..0.99, c 0.38..0.74, L 1.78..2.79, A1 0.00..0.25.
/// `protrusion` is the lead side only (H - D - L3 ~ 15.2 - 9.0 - 1.3).
pub(crate) const D2PAK: GullWingDefaults = GullWingDefaults {
    body_height: 4.45,
    lead_width: 0.81,
    protrusion: 4.9,
    thickness: 0.5,
    foot_length: 2.3,
    standoff: 0.1,
};

pub(crate) const D2PAK_TAB: TabDefaults = TabDefaults {
    length: 1.3,
    thickness: 1.3,
    under_body_ratio: 0.75,
};

/// SOD-123 class (bent leads): HE 3.55..3.85 over a 2.55..2.85 body,
/// A 0.95..1.35, b 0.45..0.65, c 0.08..0.15, L ~0.3, A1 0.00..0.10.
/// `lead_width` applies only when the body is SOD-123-sized: smaller
/// bodies scale it (see [`sod_lead_width`]).
pub(crate) const SOD: GullWingDefaults = GullWingDefaults {
    body_height: 1.1,
    lead_width: 0.55,
    protrusion: 0.5,
    thickness: 0.12,
    foot_length: 0.3,
    standoff: 0.05,
};

/// SOD-123F/FL, SOD-323F (flat leads out of the body underside):
/// A 0.80..1.08, b 0.70..0.90, c 0.10..0.20, HE - E ~1.0.
pub(crate) const SOD_FLAT: GullWingDefaults = GullWingDefaults {
    body_height: 0.95,
    lead_width: 0.8,
    protrusion: 0.5,
    thickness: 0.15,
    foot_length: 0.5,
    standoff: 0.0,
};

/// DO-214AC (SMA): overall 4.93..5.28 over a 3.99..4.50 body,
/// A 1.98..2.29, b 1.25..1.65, c 0.15..0.31, L 0.76..1.52.
pub(crate) const DO214: GullWingDefaults = GullWingDefaults {
    body_height: 2.1,
    lead_width: 1.45,
    protrusion: 0.4,
    thickness: 0.2,
    foot_length: 1.1,
    standoff: 0.0,
};

/// Lead width when the package states none, for the 2-terminal molded
/// families: the family default, but no more than `ratio` of the body
/// width (an SOD-523's leads are far narrower than an SOD-123's).
pub(crate) fn two_terminal_lead_width(default: f64, body_w: f64, ratio: f64) -> f64 {
    default.min(body_w * ratio)
}

/// TO-220 (JEDEC TO-220AB), vertical through-hole: a 9.66..10.66 wide,
/// 4.4..4.6 thick molding, 8.6..9.4 tall, its tab rising to 15.1..15.75
/// with a 3.5..3.9 hole; b 0.7..0.9, c 0.4..0.6. Drill 1.1 and land 1.9
/// as KiCad's `Package_TO_SOT_THT.pretty/TO-220-3_Vertical` (pad
/// 1.905 x 2.0, drill 1.1). Body centre 0.95 behind the lead row,
/// toward the tab (the same footprint's fab outline, y -3.15..1.25).
pub(crate) struct To220Defaults {
    pub body_height: f64,
    pub drill: f64,
    pub pad_diameter: f64,
    pub body_offset: f64,
    pub lead_width: f64,
    pub lead_thickness: f64,
    /// Tab: thickness (J1 1.15..1.40), top above the body underside
    /// (overall 15.1..15.75), hole 3.5..3.9 centred 2.8 below the top.
    pub tab_thickness: f64,
    pub overall_height: f64,
    pub hole_diameter: f64,
    pub hole_inset: f64,
    /// Lead length below the board top (3D only).
    pub tail_length: f64,
}

pub(crate) const TO220: To220Defaults = To220Defaults {
    body_height: 9.0,
    drill: 1.1,
    pad_diameter: 1.9,
    body_offset: 0.95,
    lead_width: 0.8,
    lead_thickness: 0.5,
    tab_thickness: 1.3,
    overall_height: 15.4,
    hole_diameter: 3.7,
    hole_inset: 2.8,
    tail_length: 3.5,
};

/// SOT-23 class (JEDEC TO-236): HE 2.4 over E 1.3, b 0.3..0.5,
/// c 0.08..0.20, L ~0.4, A1 0.01..0.10.
pub(crate) const SOT: GullWingDefaults = GullWingDefaults {
    body_height: 1.0,
    lead_width: 0.4,
    protrusion: 0.55,
    thickness: 0.12,
    foot_length: 0.4,
    standoff: 0.05,
};

/// QFN/DFN (JEDEC MO-220-typical): A 0.9, L 0.4, b 0.25, A3 0.2 REF.
pub(crate) struct FlushDefaults {
    pub body_height: f64,
    pub lead_length: f64,
    pub lead_width: f64,
    pub thickness: f64,
}

pub(crate) const QFN: FlushDefaults = FlushDefaults {
    body_height: 0.9,
    lead_length: 0.4,
    lead_width: 0.25,
    thickness: 0.2,
};

/// Sensor LGA (Bosch, ST): A ~0.9..1.0, terminals ~0.35 x 0.25, plated
/// pads ~0.05 thick (no lead frame).
pub(crate) const LGA: FlushDefaults = FlushDefaults {
    body_height: 0.95,
    lead_length: 0.35,
    lead_width: 0.25,
    thickness: 0.05,
};

/// Castellated-module (`module`) presentation defaults.
pub(crate) struct ModuleDefaults {
    /// Board thickness when the package states no `body_height`: the
    /// common 1.6 mm FR-4 board.
    pub board_thickness: f64,
    /// Copper pad thickness (1 oz copper, 35 um) -- drawing only.
    pub pad_thickness: f64,
}

pub(crate) const MODULE: ModuleDefaults = ModuleDefaults {
    board_thickness: 1.6,
    pad_thickness: 0.035,
};

/// Pin header / socket defaults (`header`, `socket`, `shrouded_header`,
/// `right_angle_header`).
pub(crate) struct HeaderDefaults {
    /// Annular ring around the recommended hole when the source gives no
    /// land: 0.35 mm, KiCad's 2.54 mm pin-header footprints' 1.7 mm pad
    /// on a 1.0 mm drill (`Connector_PinHeader_2.54mm.pretty`).
    pub annular_ring: f64,
    /// Copper gap the defaulted ring is reduced to keep between
    /// neighbouring lands: 0.25 mm (KiCad's 1.27 mm pin headers leave
    /// 0.27 mm: 1.0 mm pads on a 1.27 mm grid).
    pub min_gap: f64,
    /// Smallest ring the reduction may leave; below it the hole doesn't
    /// fit the grid and generation fails.
    pub min_ring: f64,
    /// The 3D model's values when the package states none, for a
    /// 2.54 mm grid (scaled by pitch / 2.54 for other grids): the common
    /// 0.64 mm (0.025 in) square post standing 6.0 mm above a 2.54 mm
    /// housing with a 3.0 mm tail, and an 8.5 mm socket housing --
    /// typical values (e.g. Würth WR-PHD 61300211121 / 61300211821),
    /// drawing only, never a fact about a part.
    pub pin_width: f64,
    pub mating_length: f64,
    pub tail_length: f64,
    pub header_height: f64,
    pub socket_height: f64,
    /// `shrouded_header` drawing values when the source states none,
    /// for a 2.54 mm grid (scaled like the above): a 7.0 mm housing
    /// (JST XH's printed height), 0.6 mm shroud walls, a cavity floor at
    /// 35 % of the housing height and post tips 0.8 mm below the rim.
    /// Wire-to-board drawings dimension the housing outline but not its
    /// walls, floor or post height: these only shape the 3D model.
    pub shroud_height: f64,
    pub shroud_wall: f64,
    pub shroud_floor_ratio: f64,
    pub shroud_post_recess: f64,
}

pub(crate) const HEADER: HeaderDefaults = HeaderDefaults {
    annular_ring: 0.35,
    min_gap: 0.25,
    min_ring: 0.1,
    pin_width: 0.64,
    mating_length: 6.0,
    tail_length: 3.0,
    header_height: 2.54,
    socket_height: 8.5,
    shroud_height: 7.0,
    shroud_wall: 0.6,
    shroud_floor_ratio: 0.35,
    shroud_post_recess: 0.8,
};

/// `bga` generator defaults.
pub(crate) mod bga {
    /// Generator defaults used only when the datasheet doesn't supply a
    /// value -- implementation details for a geometric approximation, not
    /// facts about any specific part (same policy as every other family's
    /// own defaults). BGA balls/pads are much smaller than a perimeter
    /// package's gull-wing leads, so these are distinct, smaller constants.
    pub const BODY_HEIGHT: f64 = 1.2;
    pub const PAD_DIAMETER: f64 = 0.4;
    pub const LEAD_HEIGHT: f64 = 0.3;
}

/// `axial` generator defaults.
pub(crate) mod axial {
    /// Drill diameter and pad size, both taken directly from a real KiCad
    /// reference footprint (`Diode_THT.pretty/
    /// D_DO-41_SOD81_P10.16mm_Horizontal.kicad_mod`, `KiCad/kicad-footprints`):
    /// `(pad ... (size 2.2 2.2) (drill 1.1) ...)`.
    pub const DRILL_DIAMETER: f64 = 1.1;
    pub const PAD_DIAMETER: f64 = 2.2;
    pub const LEAD_HEIGHT: f64 = 3.0;
}

/// `sip` generator defaults.
pub(crate) mod sip {
    /// Drill diameter and pad size, both taken directly from a real KiCad
    /// reference footprint (`Package_SIP.pretty/SIP-8_19x3mm_P2.54mm.kicad_mod`,
    /// `KiCad/kicad-footprints`): `(pad ... (size 1.45 1.45) (drill 0.85) ...)`
    ///.
    pub const DRILL_DIAMETER: f64 = 0.85;
    pub const PAD_DIAMETER: f64 = 1.45;
    pub const LEAD_HEIGHT: f64 = 3.0;
    /// Generator default when the datasheet doesn't supply a body height --
    /// same policy as every other family's own defaults, not a fact about
    /// any specific part.
    pub const BODY_HEIGHT: f64 = 3.0;
}

/// `chip` generator defaults.
pub(crate) mod chip {
    /// Generator default: typical end-termination length for a small
    /// 2-terminal chip component, used when the package states no
    /// `lead_length`. Not a datasheet fact -- an approximation, same policy
    /// as `defaults.rs`.
    pub const TERMINATION_LENGTH: f64 = 0.3;
    /// A termination never covers more than this share of the body
    /// length (0201: 0.15 of 0.6; EIA chips are ~0.25).
    pub const MAX_TERMINATION_RATIO: f64 = 0.3;
    pub const BODY_HEIGHT: f64 = 0.45;
}

/// `tact` generator defaults.
pub(crate) mod tact {
    /// THT pin length below the board (3D only).
    pub const TAIL_LENGTH: f64 = 3.0;
    /// Annular ring around a THT tact-switch drill when no land is given.
    pub const THT_RING: f64 = 0.4;
    /// Lead thickness when unstated (SMD feet), drawing only.
    pub const SMD_LEAD_T: f64 = 0.25;
}

/// `diode` generator defaults.
pub(crate) mod diode {
    /// Lead-width cap as a fraction of the body width, when unstated.
    pub const SOD_LEAD_RATIO: f64 = 0.4;
    pub const DO214_LEAD_RATIO: f64 = 0.55;
}

/// `dip` generator defaults.
pub(crate) mod dip {
    /// Drill diameter and pad size, both taken directly from a real KiCad
    /// reference footprint (`Package_DIP.pretty/DIP-8_W7.62mm.kicad_mod`,
    /// `KiCad/kicad-footprints`): `(pad ... (size 1.6 1.6) (drill 0.8) ...)`
    ///.
    pub const DRILL_DIAMETER: f64 = 0.8;
    pub const PAD_DIAMETER: f64 = 1.6;
    pub const LEAD_HEIGHT: f64 = 3.0;
    /// Generator default when the datasheet doesn't supply a body height --
    /// same policy as every other family's own defaults, not a fact about
    /// any specific part.
    pub const BODY_HEIGHT: f64 = 3.3;
}

/// IPC-7351B land calculation (density level N) -- see `ipc.rs`.
pub(crate) mod ipc {
    use crate::ipc::Fillets;
    /// Fabrication and placement tolerances, as KiCad's footprint
    /// generator uses them.
    pub const FABRICATION: f64 = 0.1;
    pub const PLACEMENT: f64 = 0.05;
    /// Gull-wing, pitch > 0.625 mm (IPC-7351B table 3-2, N).
    pub const GULL_WING: Fillets = Fillets {
        toe: 0.35,
        heel: 0.35,
        side: 0.03,
    };
    /// Gull-wing, pitch <= 0.625 mm (table 3-3, N).
    pub const GULL_WING_FINE: Fillets = Fillets {
        toe: 0.35,
        heel: 0.35,
        side: -0.02,
    };
    /// Flat no-lead: QFN, DFN, SON (table 3-15, N).
    pub const FLAT_NO_LEAD: Fillets = Fillets {
        toe: 0.3,
        heel: 0.0,
        side: -0.04,
    };
    /// Flat leads out of the body underside (SOD-123F class).
    pub const FLAT_LEAD: Fillets = Fillets {
        toe: 0.3,
        heel: 0.0,
        side: 0.03,
    };
    /// Inward-L / J-bend under the body: DO-214 SMA/SMB/SMC (table 3-11, N).
    pub const INWARD_L: Fillets = Fillets {
        toe: 0.15,
        heel: 0.5,
        side: -0.05,
    };
    /// Rectangular chip terminations, 1608 (0603) and larger (table 3-5, N).
    pub const CHIP: Fillets = Fillets {
        toe: 0.35,
        heel: -0.05,
        side: 0.0,
    };
    /// Smaller chips (1005 / 0402 and below, table 3-6, N).
    pub const CHIP_SMALL: Fillets = Fillets {
        toe: 0.1,
        heel: 0.0,
        side: 0.0,
    };
    /// Smallest gap between two opposite lands (a tiny chip's).
    pub const MIN_GAP: f64 = 0.2;
    /// Gap kept between a terminal land and an exposed pad.
    pub const EP_CLEARANCE: f64 = 0.2;
}

/// Explicit land patterns (`Package::pads`).
pub(crate) mod land {
    /// Copper thickness drawn for an SMD pad (3D only).
    pub const PAD_T: f64 = 0.1;
}

/// `usb_c` generator defaults.
pub(crate) mod usb_c {
    /// Shell tabs below the board, and shell sheet thickness (3D only).
    pub const TAIL_LENGTH: f64 = 1.0;
    pub const SHELL_THICKNESS: f64 = 0.2;
}

/// Connector families' generator defaults (3D only: the footprint is the
/// stated land pattern).
pub(crate) mod connector {
    /// Plated pins and tabs below the board's top surface.
    pub const TAIL_LENGTH: f64 = 1.5;
    /// Metal shell sheet (card sockets, micro-USB).
    pub const SHELL_THICKNESS: f64 = 0.2;
    /// An RF connector's barrel diameter, of the body's smaller cross
    /// dimension (width or height), when no `mating_diameter` is stated.
    pub const BARREL_RATIO: f64 = 0.7;
    /// A barrel jack's plug opening, of the same.
    pub const BORE_RATIO: f64 = 0.6;
    /// An RF barrel's length beyond the body, in barrel diameters.
    pub const BARREL_LENGTH_RATIO: f64 = 1.5;
    /// A barrel jack's centre pin, of the opening.
    pub const PIN_RATIO: f64 = 0.33;
    /// The board an edge-mount connector straddles (its axis on the
    /// board's mid-plane): 1.6 mm, the common FR-4 thickness.
    pub const EDGE_BOARD_THICKNESS: f64 = 1.6;
    /// A plated hole the source draws without a land: the land is the
    /// hole (or slot) grown by this ring on every side, as the header
    /// families' default, shrunk where it would come closer than
    /// `MIN_COPPER_GAP` to another pad.
    pub const ANNULAR_RING: f64 = 0.35;
    pub const MIN_COPPER_GAP: f64 = 0.25;
    /// The smallest ring kept when shrinking.
    pub const MIN_RING: f64 = 0.1;
    /// An RF connector's base (flange) height when the source doesn't
    /// print it.
    pub const RF_BASE_HEIGHT: f64 = 1.5;
}

/// `smd_electrolytic` generator defaults (3D only: the footprint is the
/// stated land pattern). Sources dimension the plate's outline, the can
/// and the terminals' outline, not the plate's thickness or the size of
/// its corner cuts.
pub(crate) mod smd_electrolytic {
    /// The base plate's thickness, of the overall height, kept within
    /// the limits below.
    pub const PLATE_THICKNESS_RATIO: f64 = 0.1;
    pub const PLATE_THICKNESS_MIN: f64 = 0.4;
    pub const PLATE_THICKNESS_MAX: f64 = 1.2;
    /// The terminals' thickness under the plate.
    pub const TERMINAL_THICKNESS: f64 = 0.2;
    /// A terminal's length under the plate, of the plate width, when the
    /// source doesn't state it.
    pub const TERMINAL_LENGTH_RATIO: f64 = 0.35;
    /// The + side's corner cuts, of the plate's smaller side.
    pub const CHAMFER_RATIO: f64 = 0.15;
}

/// `radial` generator defaults.
pub(crate) mod radial {
    /// Engineering defaults used only when the datasheet doesn't supply a
    /// value -- generator implementation details for a geometric
    /// approximation, never written back into Canonical Data as a fact
    /// (same policy as this crate's other generators' defaults).
    ///
    /// Electrolytic can heights vary hugely by capacitance/voltage (a few
    /// mm to 30+ mm); this is a rough mid-range fallback, not a
    /// representative "typical" value.
    pub const BODY_HEIGHT: f64 = 11.0;
    /// Drill diameter and pad size, both taken directly from a real
    /// official KiCad footprint (`Capacitor_THT.pretty/
    /// CP_Radial_D5.0mm_P2.00mm.kicad_mod`, `KiCad/kicad-footprints`):
    /// `(pad ... (size 1.6 1.6) (drill 0.8) ...)`.
    pub const DRILL_DIAMETER: f64 = 0.8;
    pub const PAD_DIAMETER: f64 = 1.6;
    pub const LEAD_HEIGHT: f64 = 3.0;
}

/// `to92` generator defaults.
pub(crate) mod to92 {
    /// Engineering defaults used only when the datasheet doesn't supply a
    /// value -- same policy as every other generator's own defaults, not a
    /// fact about any specific part.
    pub const BODY_HEIGHT: f64 = 4.6;
    /// Drill diameter and pad size, both taken directly from a real KiCad
    /// reference footprint (`Package_TO_SOT_THT.pretty/TO-92.kicad_mod`,
    /// `KiCad/kicad-footprints`): `(pad ... (size 1.3 1.3) (drill 0.75) ...)`
    ///.
    pub const DRILL_DIAMETER: f64 = 0.75;
    pub const PAD_DIAMETER: f64 = 1.3;
    pub const LEAD_HEIGHT: f64 = 3.0;
}

/// The value a generator uses for an optional dimension: the stated
/// nominal, else the middle of a stated min..max range (many outline
/// tables give only limits). The midpoint is an approximation inside
/// the datasheet's own range, used for drawing only -- never stored.
/// `None` when neither is stated (a lone min or max isn't enough).
pub(crate) fn value(dim: &Option<Dimension>) -> Option<f64> {
    let d = dim.as_ref()?;
    d.typ.or(match (d.min, d.max) {
        (Some(min), Some(max)) => Some((min + max) / 2.0),
        _ => None,
    })
}

/// The value a generator draws a body/pitch dimension at -- the one
/// rule every generator uses: the stated nominal, else the middle of a
/// stated min..max range, else the single stated limit (outline tables
/// often give a height only as "1.80 max"). Drawing only: the derived
/// value is never written back into Canonical Data, so packages record
/// exactly what the datasheet prints (Canonical Data Specification 20.1).
pub(crate) fn required(dim: &Dimension) -> Option<f64> {
    dim.typ.or(match (dim.min, dim.max) {
        (Some(min), Some(max)) => Some((min + max) / 2.0),
        (min, max) => min.or(max),
    })
}

/// [`required`], or `MissingDimension(what)` when nothing is stated.
pub(crate) fn dim(dim: &Dimension, what: &'static str) -> Result<f64, crate::McadError> {
    required(dim).ok_or(crate::McadError::MissingDimension(what))
}

/// [`required`] for an optional dimension (e.g. body height).
pub(crate) fn stated(dim: &Option<Dimension>) -> Option<f64> {
    dim.as_ref().and_then(required)
}

/// Which body axis a row of leads exits across.
#[derive(Clone, Copy)]
pub(crate) enum Axis {
    /// Leads on the left/right edges; span is `overall_width`.
    X,
    /// Leads on the bottom/top edges; span is `overall_length`.
    Y,
}

/// Resolved gull-wing dimensions for one package.
pub(crate) struct GullWing {
    pub body_height: f64,
    pub lead_width: f64,
    pub thickness: f64,
    pub foot_length: f64,
    pub standoff: f64,
    defaults: &'static GullWingDefaults,
}

impl GullWing {
    pub fn resolve(package: &Package, defaults: &'static GullWingDefaults) -> GullWing {
        let d = &package.dimensions;
        let standoff = value(&d.standoff).unwrap_or(defaults.standoff);
        GullWing {
            body_height: body_height(package, standoff, defaults.body_height),
            lead_width: value(&d.lead_width).unwrap_or(defaults.lead_width),
            thickness: value(&d.lead_thickness).unwrap_or(defaults.thickness),
            foot_length: value(&d.lead_length).unwrap_or(defaults.foot_length),
            standoff,
            defaults,
        }
    }

    /// Body edge to lead toe across `axis`: half the stated overall span
    /// minus half the body, else the family default.
    pub fn protrusion(&self, package: &Package, axis: Axis) -> Result<f64, McadError> {
        let d = &package.dimensions;
        let (span, body, name) = match axis {
            Axis::X => (&d.overall_width, &d.body_width, "dimensions.overall_width"),
            Axis::Y => (
                &d.overall_length,
                &d.body_length,
                "dimensions.overall_length",
            ),
        };
        let (Some(span), Some(body)) = (value(span), required(body)) else {
            return Ok(self.defaults.protrusion);
        };
        let protrusion = (span - body) / 2.0;
        if protrusion <= 0.0 {
            return Err(McadError::InconsistentDimensions(format!(
                "{name} ({span}) must exceed the body size ({body})"
            )));
        }
        Ok(protrusion)
    }

    /// The family's default lead width (before any datasheet value).
    pub fn default_lead_width(&self) -> f64 {
        self.defaults.lead_width
    }

    pub fn lead_form(&self) -> LeadForm {
        LeadForm::GullWing {
            thickness: self.thickness,
            foot_length: self.foot_length,
            standoff: self.standoff,
        }
    }
}

/// Resolved flush-terminal (QFN/DFN) dimensions for one package.
pub(crate) struct Flush {
    pub body_height: f64,
    pub lead_length: f64,
    pub lead_width: f64,
    pub thickness: f64,
}

impl Flush {
    pub fn resolve(package: &Package, defaults: &FlushDefaults) -> Flush {
        let d = &package.dimensions;
        Flush {
            body_height: body_height(package, 0.0, defaults.body_height),
            lead_length: value(&d.lead_length).unwrap_or(defaults.lead_length),
            lead_width: value(&d.lead_width).unwrap_or(defaults.lead_width),
            thickness: value(&d.lead_thickness).unwrap_or(defaults.thickness),
        }
    }

    pub fn lead_form(&self) -> LeadForm {
        LeadForm::Flush {
            thickness: self.thickness,
        }
    }
}

/// Body thickness: `body_height`, else the overall height less the
/// standoff, else `default` -- each read with [`required`] (a lone "max"
/// counts), like every other drawn dimension.
pub(crate) fn body_height(package: &Package, standoff: f64, default: f64) -> f64 {
    let d = &package.dimensions;
    // A height given only as a minimum is a floor, not a size to draw at:
    // keep the family default unless it falls below that floor.
    let floor_only = |dim: &Option<Dimension>| {
        dim.as_ref()
            .filter(|x| x.typ.is_none() && x.max.is_none())
            .and_then(|x| x.min)
    };
    if let Some(min) = floor_only(&d.body_height) {
        return default.max(min);
    }
    stated(&d.body_height)
        .or_else(|| {
            if let Some(min) = floor_only(&d.overall_height) {
                return Some((default + standoff).max(min) - standoff);
            }
            stated(&d.overall_height).map(|a| a - standoff)
        })
        .filter(|h| *h > 0.0)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dim(nominal: Option<f64>, min: Option<f64>, max: Option<f64>) -> Option<Dimension> {
        Some(Dimension {
            typ: nominal,
            min,
            max,
            unit: "mm".into(),
        })
    }

    #[test]
    fn value_prefers_nominal_then_midpoint_never_a_lone_limit() {
        assert_eq!(value(&dim(Some(0.2), Some(0.1), Some(0.5))), Some(0.2));
        assert_eq!(value(&dim(None, Some(0.1), Some(0.5))), Some(0.3));
        assert_eq!(value(&dim(None, None, Some(0.5))), None);
        assert_eq!(value(&None), None);
    }
}
