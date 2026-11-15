//! The connector families (`card_socket`, `barrel_jack`,
//! `terminal_block`, `fpc_connector`, `rf_coax`, `micro_usb_b`), from an
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
    if !openparts_core::is_connector_family(package.family) {
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
    let body_l = defaults::dim(&d.body_length, "dimensions.body_length")?;
    let body_h = defaults::stated(&d.body_height)
        .ok_or(McadError::MissingDimension("dimensions.body_height"))?;
    let features = connector_features(package, body_w, body_h);
    let z = if features.edge_mount {
        -defaults::connector::EDGE_BOARD_THICKNESS / 2.0
    } else {
        value(&d.standoff).unwrap_or(0.0) + body_h / 2.0
    };
    let leads = crate::land::from_pads_with(package, z - body_h / 2.0)?;
    if leads.iter().all(|l| l.mounting == Mounting::NonPlated) {
        return Err(McadError::MissingDimension("pads (a plated pad)"));
    }

    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],
        pitch: defaults::required(&package.pitch),
        details: crate::Details::Connector(features),
        body: Body {
            position: Point3 { x: 0.0, y: 0.0, z },
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

pub fn connector_features(package: &Package, body_w: f64, body_h: f64) -> ConnectorFeatures {
    use defaults::connector as dc;
    let d = &package.dimensions;
    let c = package.connector.as_ref();
    let across = body_w.min(body_h);
    let mating_diameter = value(&d.mating_diameter).unwrap_or(
        across
            * if package.family == F::BarrelJack {
                dc::BORE_RATIO
            } else {
                dc::BARREL_RATIO
            },
    );
    ConnectorFeatures {
        orientation: c
            .and_then(|c| c.orientation)
            .unwrap_or(Orientation::RightAngle),
        edge_mount: c.is_some_and(|c| c.edge_mount),
        tail_length: value(&d.tail_length).unwrap_or(dc::TAIL_LENGTH),
        shell_thickness: dc::SHELL_THICKNESS,
        mating_diameter,
        mating_length: value(&d.mating_length).unwrap_or(mating_diameter * dc::BARREL_LENGTH_RATIO),
        pin_diameter: value(&d.pin_diameter).unwrap_or(mating_diameter * dc::PIN_RATIO),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate;

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

    #[test]
    fn an_edge_mount_rf_connector_sits_on_the_board_mid_plane() {
        let p = pkg("family: rf_coax\nlead_count: 3\npitch: {unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {typ: 6.35, unit: mm}\n\
             \x20 body_length: {typ: 1.73, unit: mm}\n\
             \x20 body_height: {typ: 6.35, unit: mm}\n\
             connector: {gender: female, orientation: right_angle, interface: sma, edge_mount: true}\n\
             pads:\n\
             \x20 - {number: '1', x: 0, y: -3.0, width: 1.27, length: 4.2}\n\
             \x20 - {number: '2', x: -2.865, y: -3.0, width: 1.27, length: 4.2}\n\
             \x20 - {number: '2', x: 2.865, y: -3.0, width: 1.27, length: 4.2}\n\
             edge: {side: +y, pcb_edge: -0.8}\n");
        let g = generate(&p).unwrap();
        assert!((g.body.position.z + defaults::connector::EDGE_BOARD_THICKNESS / 2.0).abs() < 1e-9);
        let crate::Details::Connector(f) = g.details else {
            panic!()
        };
        assert!(f.edge_mount);
    }

    #[test]
    fn only_connector_families_use_this_generator() {
        let mut p = micro_sd();
        p.family = F::UsbC;
        assert!(generate_connector(&p).is_err());
    }
}
