//! The connector families (`card_socket`, `barrel_jack`,
//! `terminal_block`, `fpc_connector`, `rf_coax`, `micro_usb_b`,
//! `mini_usb_b`, `usb_b`) and the electromechanical ones (`relay`,
//! `buzzer`, `trimmer`, `rotary_encoder`: no mating face), from an
//! explicit land pattern: every pad -- signal contacts, shield and hold-down
//! pads, plated (often slotted) tabs, non-plated peg holes -- is listed in
//! `Package::pads` as the source's recommended PCB layout gives it.
//!
//! Frame: top view, X right, Y up, the origin on the centre of the body
//! (the housing, shell or flange) outline `body_width` x `body_length`,
//! the mating face (card slot, plug opening, cable or wire entry, a
//! right-angle RF connector's barrel) toward +Y. `body_height` is the
//! body's height above the board (`standoff` lifts it). `lead_count` is
//! the number of plated pads (non-plated holes don't count); a pad
//! number may repeat (shield pads).
//!
//! An edge-mount RF connector (`connector.edge_mount`) straddles the
//! board: its axis lies on the board's mid-plane (a generator default
//! board thickness), the body beyond the PCB edge line.

use crate::defaults::{self, value};
use crate::{Body, BodyShape, McadError, MechanicalGeometry, Mounting, Point3, Size3};
use openparts_core::{Orientation, Package, PackageFamily as F};

pub fn generate_connector(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if !openparts_core::is_pad_body_family(package.family) {
        return Err(McadError::UnsupportedFamily(package.family));
    }
    if package.pads.is_empty() {
        return Err(McadError::MissingDimension("pads"));
    }
    let plated = package.pads.iter().filter(|p| p.plated).count();
    if plated != package.lead_count as usize {
        return Err(McadError::InvalidLeadCount(package.lead_count));
    }
    let d = &package.dimensions;
    let body_w = defaults::dim(&d.body_width, "dimensions.body_width")?;
    // An edge-mount RF connector whose flange depth isn't printed: the
    // frame's Y origin is the PCB edge line (`edge.pcb_edge: 0` on +y)
    // and the flange is drawn from it outward at the default depth.
    let edge_flange = edge_flange(package);
    let (body_l, body_y) = match (defaults::required(&d.body_length), edge_flange) {
        (Some(l), _) => (l, 0.0),
        (None, Some(edge)) => {
            let depth = defaults::connector::RF_FLANGE_DEPTH;
            (depth, edge + depth / 2.0)
        }
        (None, None) => return Err(McadError::MissingDimension("dimensions.body_length")),
    };
    let body_h = match defaults::stated(&d.body_height) {
        Some(h) => h,
        // An RF base's height is often not printed (3D only).
        None if package.family == F::RfCoax => defaults::connector::RF_BASE_HEIGHT,
        None => return Err(McadError::MissingDimension("dimensions.body_height")),
    };
    let features = connector_features(package, [body_w, body_l, body_h]);
    let details = if openparts_core::is_electromechanical_family(package.family) {
        crate::Details::Electromechanical(electromechanical_features(
            package,
            [body_w, body_l, body_h],
            features.tail_length,
        ))
    } else {
        crate::Details::Connector(features)
    };
    let z = if features.edge_mount {
        -defaults::connector::EDGE_BOARD_THICKNESS / 2.0
    } else {
        value(&d.standoff).unwrap_or(0.0) + body_h / 2.0
    };
    let mut with_lands = package.clone();
    with_lands.pads = default_lands(package)?;
    let leads = crate::land::from_pads_with(&with_lands, z - body_h / 2.0)?;
    if leads.iter().all(|l| l.mounting == Mounting::NonPlated) {
        return Err(McadError::MissingDimension("pads (a plated pad)"));
    }

    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],
        pitch: defaults::required(&package.pitch),
        details,
        body: Body {
            position: Point3 {
                x: 0.0,
                y: body_y,
                z,
            },
            size: Size3 {
                x: body_w,
                y: body_l,
                z: body_h,
            },
            shape: BodyShape::Box,
        },
        leads,
        markers: vec![],
        lead_form: None,
    })
}

/// The PCB edge line an edge-mount `rf_coax` flange stands on when its
/// depth (`body_length`) isn't printed: the stated `edge.pcb_edge` on the
/// +y side (the mating face). `None` for any other package.
pub fn edge_flange(package: &Package) -> Option<f64> {
    let edge = package.edge.as_ref()?;
    (package.family == F::RfCoax
        && package.connector.as_ref().is_some_and(|c| c.edge_mount)
        && edge.side == openparts_core::PackageSide::PlusY)
        .then_some(edge.pcb_edge)
        .flatten()
}

/// The pads with a land for every plated hole the source draws without
/// one: the hole (a slot along its `slot_axis`) grown by the default
/// annular ring, shrunk to keep the minimum copper gap to every other
/// pad (never below the minimum ring).
fn default_lands(package: &Package) -> Result<Vec<openparts_core::PackagePad>, McadError> {
    use defaults::connector as dc;
    let mut pads = package.pads.clone();
    for (i, (p, out)) in package.pads.iter().zip(pads.iter_mut()).enumerate() {
        if !(p.drill.is_some() && p.land_unstated()) {
            continue;
        }
        let (hole_x, hole_y) = hole_size(p).ok_or_else(|| {
            McadError::InconsistentDimensions(format!(
                "pad \"{}\": a slot without a land needs slot_axis",
                p.number
            ))
        })?;
        // The ring that keeps the gap to every other pad (its stated
        // land, or its bare hole).
        let mut ring = dc::ANNULAR_RING;
        for (j, q) in package.pads.iter().enumerate() {
            if j == i {
                continue;
            }
            let bare = q.land_unstated();
            let (qx, qy) = if bare {
                hole_size(q).unwrap_or((0.0, 0.0))
            } else {
                q.extent()
            };
            let gap_x = (q.x - p.x).abs() - (qx + hole_x) / 2.0;
            let gap_y = (q.y - p.y).abs() - (qy + hole_y) / 2.0;
            // Both rings grow into a gap between two bare holes.
            let share = if bare && q.plated { 2.0 } else { 1.0 };
            ring = ring.min((gap_x.max(gap_y) - dc::MIN_COPPER_GAP) / share);
        }
        // A non-plated hole has no copper: its size is the hole.
        let ring = if p.plated {
            ring.max(dc::MIN_RING)
        } else {
            0.0
        };
        out.width = hole_x + 2.0 * ring;
        out.length = hole_y + 2.0 * ring;
    }
    Ok(pads)
}

/// A hole's extent along X and Y: round, or a slot along its
/// `slot_axis` (`None`: a slot without one).
fn hole_size(p: &openparts_core::PackagePad) -> Option<(f64, f64)> {
    let d = p.drill?;
    match (p.drill_length, p.slot_axis) {
        (None, _) => Some((d, d)),
        (Some(l), Some(openparts_core::SlotAxis::X)) => Some((l, d)),
        (Some(l), Some(openparts_core::SlotAxis::Y)) => Some((d, l)),
        (Some(_), None) => None,
    }
}

/// What a connector's 3D model needs beyond its body and pads: each
/// value is the package's own when it states one, else a generator
/// default (`defaults::connector`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnectorFeatures {
    /// The mating direction drawn: the source's `connector.orientation`,
    /// else right angle (the family's usual form; 3D only).
    pub orientation: Orientation,
    /// An edge-launch RF connector: the body lies beyond the board edge.
    pub edge_mount: bool,
    /// Plated tabs and pins below the board's top surface.
    pub tail_length: f64,
    /// Metal shell sheet thickness (card sockets, micro-USB).
    pub shell_thickness: f64,
    /// Round mating interface diameter (RF barrel, barrel-jack opening).
    pub mating_diameter: f64,
    /// How far an RF connector's barrel reaches beyond its body.
    pub mating_length: f64,
    /// A barrel jack's centre pin.
    pub pin_diameter: f64,
}

pub fn connector_features(package: &Package, body: [f64; 3]) -> ConnectorFeatures {
    use defaults::connector as dc;
    let [body_w, body_l, body_h] = body;
    let d = &package.dimensions;
    let c = package.connector.as_ref();
    let orientation = c
        .and_then(|c| c.orientation)
        .unwrap_or(Orientation::RightAngle);
    let vertical = orientation == Orientation::Vertical && !c.is_some_and(|c| c.edge_mount);
    // The barrel stands on the body's top face (vertical) or leaves its
    // +Y face: its size follows that face.
    let across = if vertical {
        body_w.min(body_l)
    } else {
        body_w.min(body_h)
    };
    // A vertical connector's barrel reaches the stated overall height.
    let to_top = value(&d.overall_height)
        .filter(|_| vertical)
        .map(|top| top - value(&d.standoff).unwrap_or(0.0) - body_h)
        .filter(|l| *l > 0.0);
    let mating_diameter = value(&d.mating_diameter).unwrap_or(
        across
            * if package.family == F::BarrelJack {
                dc::BORE_RATIO
            } else {
                dc::BARREL_RATIO
            },
    );
    ConnectorFeatures {
        orientation,
        edge_mount: c.is_some_and(|c| c.edge_mount),
        tail_length: value(&d.tail_length).unwrap_or(dc::TAIL_LENGTH),
        shell_thickness: dc::SHELL_THICKNESS,
        mating_diameter,
        mating_length: value(&d.mating_length)
            .or(to_top)
            .unwrap_or(mating_diameter * dc::BARREL_LENGTH_RATIO),
        pin_diameter: value(&d.pin_diameter).unwrap_or(mating_diameter * dc::PIN_RATIO),
    }
}

/// What an electromechanical body carries on top (3D only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElectromechanicalFeatures {
    /// Plated pins below the board's top surface.
    pub tail_length: f64,
    /// A buzzer's round can (`can_diameter`); `None`: a square housing.
    pub can_diameter: Option<f64>,
    /// A trimmer's rotor, an encoder's shaft (`actuator_diameter`, else
    /// a default); 0 for a relay or buzzer.
    pub top_diameter: f64,
    /// How far the rotor or the bushing + shaft rise above the housing
    /// (`overall_height` - standoff - `body_height`, else a default).
    pub top_height: f64,
}

pub fn electromechanical_features(
    package: &Package,
    body: [f64; 3],
    tail_length: f64,
) -> ElectromechanicalFeatures {
    use defaults::electromechanical as de;
    let [body_w, body_l, body_h] = body;
    let d = &package.dimensions;
    let side = body_w.min(body_l);
    let above = value(&d.overall_height)
        .map(|top| top - value(&d.standoff).unwrap_or(0.0) - body_h)
        .filter(|h| *h > 0.0);
    let (top_diameter, top_height) = match package.family {
        F::Trimmer => (
            value(&d.actuator_diameter).unwrap_or(side * de::ROTOR_RATIO),
            above.unwrap_or(de::ROTOR_HEIGHT),
        ),
        F::RotaryEncoder => (
            value(&d.actuator_diameter)
                .unwrap_or(de::SHAFT_DIAMETER)
                .min(side * de::MAX_SHAFT_RATIO),
            above.unwrap_or(de::SHAFT_LENGTH),
        ),
        _ => (0.0, 0.0),
    };
    ElectromechanicalFeatures {
        tail_length,
        can_diameter: (package.family == F::Buzzer)
            .then(|| value(&d.can_diameter))
            .flatten(),
        top_diameter,
        top_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate;

    /// An Omron G5V-1-style relay: 12.5 x 7.5 x 10 mm, a 2 x 3 pin grid
    /// less one (5 plated holes).
    pub(crate) fn relay() -> Package {
        pkg("family: relay\nlead_count: 5\npitch: {unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {typ: 12.5, unit: mm}\n\
             \x20 body_length: {typ: 7.5, unit: mm}\n\
             \x20 body_height: {typ: 10.0, unit: mm}\n\
             pads:\n\
             \x20 - {number: '1', x: -5.08, y: -2.54, width: 1.6, length: 1.6, drill: 1.0}\n\
             \x20 - {number: '2', x: -5.08, y: 2.54, width: 1.6, length: 1.6, drill: 1.0}\n\
             \x20 - {number: '3', x: 2.54, y: 2.54, width: 1.6, length: 1.6, drill: 1.0}\n\
             \x20 - {number: '4', x: 5.08, y: 2.54, width: 1.6, length: 1.6, drill: 1.0}\n\
             \x20 - {number: '5', x: 5.08, y: -2.54, width: 1.6, length: 1.6, drill: 1.0}\n")
    }

    #[test]
    fn electromechanical_bodies_sit_over_their_land_pattern() {
        let g = generate(&relay()).unwrap();
        assert_eq!(g.leads.len(), 5);
        assert!((g.body.position.z - 5.0).abs() < 1e-9);
        let crate::Details::Electromechanical(f) = g.details else {
            panic!("electromechanical details");
        };
        assert_eq!((f.top_diameter, f.can_diameter), (0.0, None));
        // An encoder's shaft reaches the stated overall height; a
        // trimmer's rotor takes the default share of the housing.
        let mut enc = relay();
        enc.family = F::RotaryEncoder;
        enc.dimensions.overall_height = Some(openparts_core::Dimension {
            typ: Some(26.5),
            unit: "mm".into(),
            ..Default::default()
        });
        let crate::Details::Electromechanical(f) = generate(&enc).unwrap().details else {
            panic!();
        };
        assert!((f.top_height - 16.5).abs() < 1e-9);
        assert!(
            (f.top_diameter - 3.75).abs() < 1e-9,
            "capped at half the side"
        );
        let mut trim = relay();
        trim.family = F::Trimmer;
        let crate::Details::Electromechanical(f) = generate(&trim).unwrap().details else {
            panic!();
        };
        assert!((f.top_diameter - 7.5 * 0.75).abs() < 1e-9);
        assert!((f.top_height - 0.5).abs() < 1e-9);
    }

    fn pkg(yaml: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\n{yaml}"
        ))
        .unwrap()
    }

    /// A microSD socket: nine SMD contacts at 1.1 mm along the rear
    /// edge, four shield pads, one peg hole.
    pub(crate) fn micro_sd() -> Package {
        let mut pads = String::new();
        for i in 0..9 {
            let x = -4.4 + 1.1 * f64::from(i);
            pads += &format!(
                "\x20 - {{number: '{}', x: {x:.2}, y: -6.0, width: 0.7, length: 1.5}}\n",
                i + 1
            );
        }
        for (x, y) in [(-6.9, -4.5), (6.9, -4.5), (-6.9, 4.0), (6.9, 4.0)] {
            pads += &format!("\x20 - {{number: SH, x: {x}, y: {y}, width: 1.2, length: 1.4}}\n");
        }
        pads +=
            "\x20 - {number: '', x: 0, y: 2, width: 1.0, length: 1.0, drill: 1.0, plated: false}\n";
        pkg(&format!(
            "family: card_socket\nlead_count: 13\npitch: {{typ: 1.1, unit: mm}}\n\
             dimensions:\n\
             \x20 body_width: {{typ: 14.0, unit: mm}}\n\
             \x20 body_length: {{typ: 15.0, unit: mm}}\n\
             \x20 body_height: {{typ: 1.8, unit: mm}}\n\
             connector: {{mechanism: push_push, interface: micro_sd}}\n\
             pads:\n{pads}"
        ))
    }

    #[test]
    fn card_socket_pads_are_the_land_pattern_body_centred() {
        let g = generate(&micro_sd()).unwrap();
        assert_eq!(g.leads.len(), 14);
        assert_eq!(g.lands.len(), 0, "the leads are the pads");
        assert_eq!(
            g.leads.iter().filter(|l| l.number == "SH").count(),
            4,
            "shield pads keep their repeated number"
        );
        assert_eq!(g.leads[13].mounting, Mounting::NonPlated);
        assert_eq!((g.body.position.x, g.body.position.y), (0.0, 0.0));
        assert!((g.body.position.z - 0.9).abs() < 1e-9);
        assert_eq!(g.pitch, Some(1.1));
        let crate::Details::Connector(f) = g.details else {
            panic!("connector details");
        };
        assert_eq!(f.orientation, Orientation::RightAngle);
    }

    #[test]
    fn lead_count_counts_plated_pads_and_pads_are_required() {
        let mut p = micro_sd();
        p.lead_count = 14;
        assert!(matches!(generate(&p), Err(McadError::InvalidLeadCount(14))));
        let mut p = micro_sd();
        p.pads.clear();
        assert!(generate(&p).is_err());
        let mut p = micro_sd();
        p.dimensions.body_height = None;
        assert!(generate(&p).is_err());
    }

    #[test]
    fn barrel_jack_slots_and_defaults() {
        let p = pkg("family: barrel_jack\nlead_count: 3\npitch: {unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {typ: 9.0, unit: mm}\n\
             \x20 body_length: {typ: 14.0, unit: mm}\n\
             \x20 body_height: {typ: 11.0, unit: mm}\n\
             \x20 pin_diameter: {typ: 2.0, unit: mm}\n\
             pads:\n\
             \x20 - {number: '1', x: 0, y: 6.0, width: 3.0, length: 1.6, drill: 1.0, drill_length: 2.4}\n\
             \x20 - {number: '2', x: 0, y: 0.0, width: 3.0, length: 1.6, drill: 1.0, drill_length: 2.4}\n\
             \x20 - {number: '3', x: 4.7, y: 3.0, width: 1.6, length: 3.0, drill: 1.0, drill_length: 2.4}\n");
        let g = generate(&p).unwrap();
        assert!(g.leads.iter().all(|l| l.mounting == Mounting::ThroughHole));
        assert_eq!(g.leads[0].slot, Some(2.4));
        let crate::Details::Connector(f) = g.details else {
            panic!()
        };
        assert_eq!(f.pin_diameter, 2.0);
        assert!((f.mating_diameter - 9.0 * defaults::connector::BORE_RATIO).abs() < 1e-9);
        assert_eq!(f.tail_length, defaults::connector::TAIL_LENGTH);
    }

    /// Samtec SMA-J-P-X-ST-EM1's recommended layout (rev G): the flange
    /// depth is not printed ("1.73 REF" is the gap between the leg pairs,
    /// which straddle the board), so the frame starts at the PCB edge.
    /// The two bottom lands are this test's own, not the print's.
    pub(crate) fn sma_edge_mount() -> Package {
        pkg("family: rf_coax\nlead_count: 5\npitch: {unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {typ: 6.35, unit: mm}\n\
             \x20 body_length: {unit: mm}\n\
             \x20 body_height: {typ: 6.35, unit: mm}\n\
             connector: {gender: female, orientation: right_angle, interface: sma, edge_mount: true}\n\
             pads:\n\
             \x20 - {number: '1', x: 0, y: -1.9, width: 1.27, length: 3.6}\n\
             \x20 - {number: '2', x: -2.825, y: -2.1, width: 1.35, length: 4.2}\n\
             \x20 - {number: '2', x: 2.825, y: -2.1, width: 1.35, length: 4.2}\n\
             \x20 - {number: '2', x: -2.825, y: -2.1, width: 1.35, length: 4.2, side: bottom}\n\
             \x20 - {number: '2', x: 2.825, y: -2.1, width: 1.35, length: 4.2, side: bottom}\n\
             edge: {side: +y, pcb_edge: 0}\n")
    }

    #[test]
    fn an_edge_mount_rf_connector_sits_on_the_board_mid_plane() {
        let g = generate(&sma_edge_mount()).unwrap();
        assert!((g.body.position.z + defaults::connector::EDGE_BOARD_THICKNESS / 2.0).abs() < 1e-9);
        let crate::Details::Connector(f) = g.details else {
            panic!()
        };
        assert!(f.edge_mount);
        // The unprinted flange: the default depth, from the edge outward.
        let depth = defaults::connector::RF_FLANGE_DEPTH;
        assert!((g.body.size.y - depth).abs() < 1e-9);
        assert!((g.body.position.y - depth / 2.0).abs() < 1e-9);
        // Bottom lands hang under the board.
        let bottom: Vec<_> = g
            .leads
            .iter()
            .filter(|l| l.side == openparts_core::PadSide::Bottom)
            .collect();
        assert_eq!(bottom.len(), 2);
        assert!(bottom
            .iter()
            .all(|l| l.position.z < -defaults::connector::EDGE_BOARD_THICKNESS));
    }

    #[test]
    fn a_connector_states_its_depth_unless_an_edge_flange_starts_at_the_edge() {
        let mut p = sma_edge_mount();
        p.edge = None;
        assert!(matches!(
            generate(&p),
            Err(McadError::MissingDimension("dimensions.body_length"))
        ));
        // A hole cannot be on the bottom side.
        let mut p = sma_edge_mount();
        p.pads[3].drill = Some(0.5);
        p.pads[3].width = 1.35;
        assert!(generate(&p).is_err());
    }

    #[test]
    fn holes_without_a_land_get_the_default_ring_shrunk_to_the_gap() {
        // Terminal block holes at 2.54 mm with no land; a slot along Y.
        let p = pkg(
            "family: terminal_block\nlead_count: 3\npitch: {typ: 2.54, unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {typ: 8.0, unit: mm}\n\
             \x20 body_length: {typ: 6.0, unit: mm}\n\
             \x20 body_height: {typ: 9.0, unit: mm}\n\
             pads:\n\
             \x20 - {number: '1', x: -2.54, y: 0, drill: 1.0}\n\
             \x20 - {number: '2', x: 0, y: 0, drill: 1.0}\n\
             \x20 - {number: '3', x: 6.0, y: 0, drill: 1.0, drill_length: 2.0, slot_axis: y}\n",
        );
        let g = generate(&p).unwrap();
        let ring = defaults::connector::ANNULAR_RING;
        // 2.54 - 1.0 = 1.54 between holes, 0.25 kept: (1.54 - 0.25) / 2 = 0.645
        // > 0.35, so the full ring.
        assert!((g.leads[0].size.x - (1.0 + 2.0 * ring)).abs() < 1e-9);
        let slot = &g.leads[2];
        assert!((slot.size.x - 1.7).abs() < 1e-9 && (slot.size.y - 2.7).abs() < 1e-9);
        assert_eq!(slot.slot, Some(2.0));
        // Holes 1.2 apart: (1.2 - 1.0 - 0.25) < 0: the minimum ring.
        let mut tight = p.clone();
        tight.pads[1].x = -1.34;
        let g = generate(&tight).unwrap();
        let min = defaults::connector::MIN_RING;
        assert!((g.leads[0].size.x - (1.0 + 2.0 * min)).abs() < 1e-9);
        // A slot without a land needs its axis.
        let mut bad = p.clone();
        bad.pads[2].slot_axis = None;
        assert!(generate(&bad).is_err());
    }

    #[test]
    fn an_rf_base_without_a_height_and_a_barrel_to_the_overall_height() {
        let p = pkg("family: rf_coax\nlead_count: 2\npitch: {unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {typ: 5.88, unit: mm}\n\
             \x20 body_length: {typ: 5.88, unit: mm}\n\
             \x20 overall_height: {typ: 6.0, unit: mm}\n\
             connector: {orientation: vertical, interface: mcx}\n\
             pads:\n\
             \x20 - {number: '1', x: 0, y: 0, width: 1.65, length: 1.65}\n\
             \x20 - {number: '2', x: 2.22, y: 2.22, width: 2.1, length: 2.1}\n");
        let g = generate(&p).unwrap();
        let base = defaults::connector::RF_BASE_HEIGHT;
        assert!((g.body.size.z - base).abs() < 1e-9);
        let crate::Details::Connector(f) = g.details else {
            panic!()
        };
        assert!((f.mating_length - (6.0 - base)).abs() < 1e-9);
        assert!((f.mating_diameter - 5.88 * defaults::connector::BARREL_RATIO).abs() < 1e-9);
    }

    #[test]
    fn only_connector_families_use_this_generator() {
        let mut p = micro_sd();
        p.family = F::UsbC;
        assert!(generate_connector(&p).is_err());
    }
}
