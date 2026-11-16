//! Tactile switches: a square housing with an actuator and two or four
//! terminals at its corners, plus an optional ground terminal -- SMD
//! gull-wing feet (`tact_switch_smd`) or straight through-hole pins
//! (`tact_switch_tht`).
//!
//! Frame: terminals on the left and right of the housing, `pitch` apart
//! along Y; positions numbered like SOIC-4 -- 1 top-left, 2 bottom-left,
//! 3 bottom-right, 4 top-right (two terminals: 1 left, 2 right, on the X
//! axis). An odd `lead_count` (3 or 5) adds a ground terminal as the last
//! position, on the Y axis below the housing centre
//! (`ground_terminal_offset` along -Y). Vendors number the positions
//! differently: `pad_order` gives the source's numbers in this position
//! order. `body_width` is the housing along X, `body_length` along Y,
//! `body_height` its height.
//!
//! The actuator is pushed from the top (`overall_height` its top above
//! the board, `actuator_diameter` its diameter) or, with `actuation:
//! side`, from the side: it leaves the housing's +Y face by
//! `actuator_length`, round (`actuator_diameter`) or rectangular
//! (`actuator_width` x `actuator_thickness`), its axis
//! `actuator_elevation` above the board (else the housing's mid-height).
//! Actuator sizes are 3D only.
//!
//! - SMD: `overall_width` is the toe-to-toe span (X); each pad is the
//!   foot, `lead_length` in from the toe (else from the housing edge),
//!   `lead_width` wide. A ground foot runs from the housing edge to its
//!   tip at `ground_terminal_offset`, `ground_terminal_width` (else
//!   `lead_width`) wide.
//! - THT: `row_spacing` is the X distance between the pin columns,
//!   `hole_diameter` the drill, `pad_diameter` the land (else drill +
//!   2 x 0.4 mm); `standoff` lifts the housing (stand-off bumps). With
//!   `pads` (the source's PCB holes, drills of their own, in this frame)
//!   the pins stand in those holes instead: a side-operated switch's
//!   holes rarely form a rectangle.

use crate::defaults::{self, value};
use crate::{Body, BodyShape, Lead, McadError, MechanicalGeometry, Mounting, Point3, Size3};
use openparts_core::{Actuation, Package, PackageFamily as F};

pub fn generate_tact_switch(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let tht = match package.family {
        F::TactSwitchSmd => false,
        F::TactSwitchTht => true,
        other => return Err(McadError::UnsupportedFamily(other)),
    };
    let n = package.lead_count;
    if !(2..=5).contains(&n) {
        return Err(McadError::InvalidLeadCount(n));
    }
    // Corner terminals (2 or 4); an odd count adds the ground terminal.
    let corners_n = n & !1;
    let ground = n % 2 == 1;
    let d = &package.dimensions;
    let body_w = defaults::required(&d.body_width)
        .ok_or(McadError::MissingDimension("dimensions.body_width"))?;
    let body_l = defaults::required(&d.body_length)
        .ok_or(McadError::MissingDimension("dimensions.body_length"))?;
    let body_h = defaults::stated(&d.body_height)
        .ok_or(McadError::MissingDimension("dimensions.body_height"))?;
    let from_pads = tht && !package.pads.is_empty();
    let ground_y = match defaults::stated(&d.ground_terminal_offset) {
        Some(y) if y > 0.0 => Some(y),
        Some(_) => {
            return Err(McadError::InconsistentDimensions(
                "ground_terminal_offset must be positive".into(),
            ))
        }
        None if ground && !from_pads => {
            return Err(McadError::MissingDimension(
                "dimensions.ground_terminal_offset",
            ))
        }
        None => None,
    };
    let dy = if corners_n == 4 && !from_pads {
        defaults::dim(&package.pitch, "pitch")? / 2.0
    } else {
        0.0
    };
    // Position order: top-left, bottom-left, bottom-right, top-right.
    let corners: Vec<(f64, f64)> = if corners_n == 4 {
        vec![(-1.0, dy), (-1.0, -dy), (1.0, -dy), (1.0, dy)]
    } else {
        vec![(-1.0, 0.0), (1.0, 0.0)]
    };
    // Position numbers; `pad_order` is applied by `crate::generate`.
    let number = |i: usize| (i + 1).to_string();

    let (standoff, leads) = if from_pads {
        let standoff = value(&d.standoff).unwrap_or(0.0);
        (standoff, pad_pins(package, standoff)?)
    } else if tht {
        let span =
            value(&d.row_spacing).ok_or(McadError::MissingDimension("dimensions.row_spacing"))?;
        let drill = value(&d.hole_diameter)
            .ok_or(McadError::MissingDimension("dimensions.hole_diameter"))?;
        let land = value(&d.pad_diameter).unwrap_or(drill + 2.0 * crate::defaults::tact::THT_RING);
        let standoff = value(&d.standoff).unwrap_or(0.0);
        let pin = |number: String, x: f64, y: f64| Lead {
            number,
            position: Point3 { x, y, z: 0.0 },
            size: Size3 {
                x: land,
                y: land,
                z: standoff,
            },
            mounting: Mounting::ThroughHole,
            drill: Some(drill),
            slot: None,
            shape: crate::LandShape::Rect,
            side: openparts_core::PadSide::Top,
        };
        let mut leads: Vec<Lead> = corners
            .iter()
            .enumerate()
            .map(|(i, &(sx, y))| pin(number(i), sx * span / 2.0, y))
            .collect();
        if let (true, Some(gy)) = (ground, ground_y) {
            leads.push(pin(number(corners_n as usize), 0.0, -gy));
        }
        (standoff, leads)
    } else {
        let span = value(&d.overall_width)
            .ok_or(McadError::MissingDimension("dimensions.overall_width"))?;
        if span <= body_w {
            return Err(McadError::InconsistentDimensions(format!(
                "overall_width ({span}) must exceed body_width ({body_w})"
            )));
        }
        let toe = span / 2.0;
        let foot = value(&d.lead_length).unwrap_or(toe - body_w / 2.0).min(toe);
        let width =
            value(&d.lead_width).ok_or(McadError::MissingDimension("dimensions.lead_width"))?;
        let t = value(&d.lead_thickness).unwrap_or(crate::defaults::tact::SMD_LEAD_T);
        let foot_lead = |number: String, position: Point3, x: f64, y: f64| Lead {
            number,
            position,
            size: Size3 { x, y, z: t },
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
            shape: crate::LandShape::Rect,
            side: openparts_core::PadSide::Top,
        };
        let mut leads: Vec<Lead> = corners
            .iter()
            .enumerate()
            .map(|(i, &(sx, y))| {
                let at = Point3 {
                    x: sx * (toe - foot / 2.0),
                    y,
                    z: t / 2.0,
                };
                foot_lead(number(i), at, foot, width)
            })
            .collect();
        if let (true, Some(tip)) = (ground, ground_y) {
            if tip <= body_l / 2.0 {
                return Err(McadError::InconsistentDimensions(format!(
                    "ground_terminal_offset ({tip}) must reach past the housing edge ({})",
                    body_l / 2.0
                )));
            }
            let gw = value(&d.ground_terminal_width).unwrap_or(width);
            let at = Point3 {
                x: 0.0,
                y: -(tip + body_l / 2.0) / 2.0,
                z: t / 2.0,
            };
            leads.push(foot_lead(
                number(corners_n as usize),
                at,
                gw,
                tip - body_l / 2.0,
            ));
        }
        (0.0, leads)
    };

    let mut geometry = MechanicalGeometry {
        family: package.family,
        lands: vec![],

        pitch: crate::defaults::required(&package.pitch),

        details: crate::Details::None,
        body: Body {
            position: Point3 {
                x: 0.0,
                y: 0.0,
                z: standoff + body_h / 2.0,
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
    };
    let mut features = switch_features(package, &geometry);
    features.ground_lead = (ground && !from_pads).then_some(corners_n as usize);
    features.side = side_actuator(package, &geometry)?;
    geometry.details = crate::Details::Switch(features);
    Ok(geometry)
}

/// THT pins standing in the source's PCB holes (`pads`): one per plated
/// hole, the land as stated or the hole grown by `pad_diameter` / the
/// default ring. Non-plated holes (locating bosses) stay footprint-only.
fn pad_pins(package: &Package, standoff: f64) -> Result<Vec<Lead>, McadError> {
    let d = &package.dimensions;
    let mut pads = package.pads.clone();
    for p in &mut pads {
        let Some(drill) = p.drill else {
            return Err(McadError::InconsistentDimensions(format!(
                "pad \"{}\": a tact_switch_tht pad needs a drill",
                p.number
            )));
        };
        if p.plated && p.land_unstated() {
            let land = value(&d.pad_diameter)
                .filter(|l| *l > drill)
                .unwrap_or(drill + 2.0 * crate::defaults::tact::THT_RING);
            p.width = land;
            p.length = land;
        }
    }
    let mut with_lands = package.clone();
    with_lands.pads = pads;
    let leads = crate::land::from_pads_with(&with_lands, standoff)?;
    let plated = leads
        .iter()
        .filter(|l| l.mounting == Mounting::ThroughHole)
        .count();
    if plated != package.lead_count as usize {
        return Err(McadError::InconsistentDimensions(format!(
            "{plated} plated holes in pads for lead_count {}",
            package.lead_count
        )));
    }
    Ok(leads)
}

/// A side-operated switch's actuator (`actuation: side`).
fn side_actuator(
    package: &Package,
    g: &MechanicalGeometry,
) -> Result<Option<SideActuator>, McadError> {
    let d = &package.dimensions;
    if package.actuation != Some(Actuation::Side) {
        return Ok(None);
    }
    let b = &g.body;
    let bottom = b.position.z - b.size.z / 2.0;
    let length = defaults::stated(&d.actuator_length)
        .ok_or(McadError::MissingDimension("dimensions.actuator_length"))?;
    let (width, thickness, round) = match (value(&d.actuator_width), value(&d.actuator_thickness)) {
        (Some(w), Some(t)) => (w, t, false),
        (None, None) => {
            let dia = value(&d.actuator_diameter).unwrap_or(b.size.x.min(b.size.z) * 0.5);
            (dia, dia, true)
        }
        _ => {
            return Err(McadError::InconsistentDimensions(
                "actuator_width and actuator_thickness go together".into(),
            ))
        }
    };
    let elevation = defaults::stated(&d.actuator_elevation).unwrap_or(bottom + b.size.z / 2.0);
    if length <= 0.0 || width <= 0.0 || thickness <= 0.0 || elevation - thickness / 2.0 < 0.0 {
        return Err(McadError::InconsistentDimensions(format!(
            "side actuator ({width} x {thickness}, {length} long, axis {elevation} above the board) \
             must be positive and above the board"
        )));
    }
    Ok(Some(SideActuator {
        width,
        thickness,
        round,
        length,
        elevation,
    }))
}

/// The actuator and lead sizes a switch's 3D model needs: the package's
/// own values, else drawing defaults (a centred plunger half the housing
/// wide rising 1 mm above it; 0.25 mm feet; THT pins 70 % of the drill,
/// 3 mm below the board).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchFeatures {
    pub actuator_diameter: f64,
    /// Actuator top above the board (a top actuator).
    pub actuator_top: f64,
    pub lead_thickness: f64,
    pub tail_length: f64,
    /// Index in `leads` of an SMD or THT ground terminal the generator
    /// placed below the housing (its foot runs along Y).
    pub ground_lead: Option<usize>,
    /// The side actuator of a side-operated switch (`None`: on top).
    pub side: Option<SideActuator>,
}

/// A side-operated switch's actuator, leaving the housing's +Y face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SideActuator {
    /// Size along X (a round actuator's diameter).
    pub width: f64,
    /// Size along Z (a round actuator's diameter).
    pub thickness: f64,
    pub round: bool,
    /// Beyond the housing's +Y face.
    pub length: f64,
    /// Its axis above the board.
    pub elevation: f64,
}

pub fn switch_features(package: &Package, geometry: &MechanicalGeometry) -> SwitchFeatures {
    let d = &package.dimensions;
    let b = &geometry.body;
    let top = b.position.z + b.size.z / 2.0;
    SwitchFeatures {
        actuator_diameter: value(&d.actuator_diameter)
            .unwrap_or(b.size.x.min(b.size.y) * 0.5)
            .min(b.size.x.min(b.size.y)),
        actuator_top: defaults::stated(&d.overall_height)
            .filter(|h| *h > top)
            .unwrap_or(top + 1.0),
        lead_thickness: value(&d.lead_thickness).unwrap_or(crate::defaults::tact::SMD_LEAD_T),
        tail_length: value(&d.tail_length).unwrap_or(crate::defaults::tact::TAIL_LENGTH),
        ground_lead: None,
        side: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(yaml: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nlead_count: 4\n{yaml}"
        ))
        .unwrap()
    }

    fn tht() -> Package {
        pkg(
            "family: tact_switch_tht\npitch: {nominal: 4.5, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 6.0, unit: mm}\n\
             \x20 body_length: {nominal: 6.0, unit: mm}\n\
             \x20 body_height: {nominal: 3.5, unit: mm}\n\
             \x20 row_spacing: {nominal: 6.5, unit: mm}\n\
             \x20 hole_diameter: {nominal: 1.0, unit: mm}\n",
        )
    }

    #[test]
    fn tht_pins_sit_at_the_corners_in_soic_order() {
        let g = generate_tact_switch(&tht()).unwrap();
        let at: Vec<(f64, f64)> = g
            .leads
            .iter()
            .map(|l| (l.position.x, l.position.y))
            .collect();
        assert_eq!(
            at,
            [(-3.25, 2.25), (-3.25, -2.25), (3.25, -2.25), (3.25, 2.25)]
        );
        assert!(g
            .leads
            .iter()
            .all(|l| l.drill == Some(1.0) && (l.size.x - 1.8).abs() < 1e-9));
    }

    #[test]
    fn pad_order_renumbers_the_positions() {
        let mut p = tht();
        p.pad_order = vec!["1".into(), "3".into(), "4".into(), "2".into()];
        let g = crate::generate(&p).unwrap();
        let numbers: Vec<&str> = g.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, ["1", "3", "4", "2"]);
        p.pad_order.pop();
        assert!(crate::generate(&p).is_err());
    }

    #[test]
    fn smd_feet_end_at_the_overall_width() {
        let p = pkg(
            "family: tact_switch_smd\npitch: {nominal: 4.5, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 6.0, unit: mm}\n\
             \x20 body_length: {nominal: 6.0, unit: mm}\n\
             \x20 body_height: {nominal: 3.5, unit: mm}\n\
             \x20 overall_width: {nominal: 9.0, unit: mm}\n\
             \x20 lead_width: {nominal: 0.7, unit: mm}\n\
             \x20 lead_length: {nominal: 1.0, unit: mm}\n",
        );
        let g = crate::generate(&p).unwrap();
        let l4 = &g.leads[3];
        assert!((l4.position.x + l4.size.x / 2.0 - 4.5).abs() < 1e-9);
        assert!((l4.size.x - 1.0).abs() < 1e-9 && (l4.size.y - 0.7).abs() < 1e-9);
        assert_eq!(l4.mounting, Mounting::Smd);
    }

    #[test]
    fn a_fifth_terminal_is_the_ground_pin_below_the_centre() {
        let mut p = tht();
        p.lead_count = 5;
        assert!(generate_tact_switch(&p).is_err(), "needs its offset");
        p.dimensions.ground_terminal_offset = Some(openparts_core::Dimension {
            typ: Some(4.1),
            min: None,
            max: None,
            unit: "mm".into(),
        });
        p.pad_order = ["4", "2", "1", "3", "5"].map(String::from).to_vec();
        let g = crate::generate(&p).unwrap();
        let l5 = g.leads.iter().find(|l| l.number == "5").unwrap();
        assert_eq!((l5.position.x, l5.position.y), (0.0, -4.1));
        assert_eq!(l5.drill, Some(1.0));
        let crate::Details::Switch(f) = g.details else {
            panic!()
        };
        assert_eq!(f.ground_lead, Some(4));
    }

    fn b3u(extra: &str) -> Package {
        pkg(&format!(
            "family: tact_switch_smd\npitch: {{nominal: 3.4, unit: mm}}\n{extra}dimensions:\n\
             \x20 body_width: {{nominal: 3.0, unit: mm}}\n\
             \x20 body_length: {{nominal: 2.5, unit: mm}}\n\
             \x20 body_height: {{nominal: 1.2, unit: mm}}\n\
             \x20 overall_width: {{nominal: 4.0, unit: mm}}\n\
             \x20 lead_width: {{nominal: 1.4, unit: mm}}\n\
             \x20 ground_terminal_offset: {{nominal: 1.9, unit: mm}}\n\
             \x20 ground_terminal_width: {{nominal: 0.5, unit: mm}}\n\
             \x20 actuator_length: {{nominal: 0.7, unit: mm}}\n\
             \x20 actuator_width: {{nominal: 1.7, unit: mm}}\n\
             \x20 actuator_thickness: {{nominal: 1.0, unit: mm}}\n"
        ))
    }

    #[test]
    fn an_smd_ground_foot_runs_from_the_housing_to_its_tip() {
        let mut p = b3u("");
        p.lead_count = 3;
        let g = generate_tact_switch(&p).unwrap();
        assert_eq!(g.leads.len(), 3);
        let gnd = &g.leads[2];
        assert_eq!(gnd.number, "3");
        assert!((gnd.position.y - gnd.size.y / 2.0 + 1.9).abs() < 1e-9);
        assert!((gnd.position.y + gnd.size.y / 2.0 + 1.25).abs() < 1e-9);
        assert!((gnd.size.x - 0.5).abs() < 1e-9 && gnd.position.x == 0.0);
        // A tip inside the housing is an error.
        p.dimensions.ground_terminal_offset.as_mut().unwrap().typ = Some(1.0);
        assert!(generate_tact_switch(&p).is_err());
    }

    #[test]
    fn a_side_actuator_leaves_the_plus_y_face() {
        let mut p = b3u("actuation: side\n");
        p.lead_count = 2;
        let g = generate_tact_switch(&p).unwrap();
        let crate::Details::Switch(f) = g.details else {
            panic!()
        };
        let a = f.side.unwrap();
        assert!(!a.round);
        assert_eq!((a.width, a.thickness, a.length), (1.7, 1.0, 0.7));
        assert!((a.elevation - 0.6).abs() < 1e-9, "mid-height default");
        // Without actuation: side the actuator stays on top.
        let mut top = b3u("");
        top.lead_count = 2;
        let crate::Details::Switch(f) = generate_tact_switch(&top).unwrap().details else {
            panic!()
        };
        assert!(f.side.is_none());
        // A side actuator needs its length.
        p.dimensions.actuator_length = None;
        assert!(generate_tact_switch(&p).is_err());
    }

    #[test]
    fn tht_pads_place_the_pins() {
        // B3F-3100-like: switch pins 4.5 apart in front, bracket pins
        // 7 apart (larger drill) 2.5 behind them.
        let p = pkg(
            "family: tact_switch_tht\npitch: {nominal: 2.5, unit: mm}\nactuation: side\n\
             dimensions:\n\
             \x20 body_width: {nominal: 7.3, unit: mm}\n\
             \x20 body_length: {nominal: 6.25, unit: mm}\n\
             \x20 body_height: {nominal: 7.4, unit: mm}\n\
             \x20 actuator_diameter: {nominal: 3.5, unit: mm}\n\
             \x20 actuator_length: {nominal: 0.9, unit: mm}\n\
             \x20 actuator_elevation: {nominal: 4.0, unit: mm}\n\
             pads:\n\
             \x20 - {number: '2', x: -2.25, y: 0.875, drill: 1.0}\n\
             \x20 - {number: '1', x: 2.25, y: 0.875, drill: 1.0}\n\
             \x20 - {number: '4', x: -3.5, y: -1.625, drill: 1.5}\n\
             \x20 - {number: '3', x: 3.5, y: -1.625, drill: 1.5}\n",
        );
        let g = crate::generate(&p).unwrap();
        assert_eq!(g.leads.len(), 4);
        let l4 = g.leads.iter().find(|l| l.number == "4").unwrap();
        assert_eq!(
            (l4.position.x, l4.position.y, l4.drill),
            (-3.5, -1.625, Some(1.5))
        );
        assert!((l4.size.x - 2.3).abs() < 1e-9, "drill + default ring");
        assert!(g.lands.is_empty(), "the pins are the lands");
        let crate::Details::Switch(f) = g.details else {
            panic!()
        };
        let a = f.side.unwrap();
        assert!(a.round && a.width == 3.5 && a.elevation == 4.0);
        let mut five = p.clone();
        five.lead_count = 5;
        assert!(crate::generate(&five).is_err(), "one pin per plated hole");
    }
}
