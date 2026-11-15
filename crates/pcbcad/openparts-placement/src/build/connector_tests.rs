//! The connector families (Canonical Data Specification 20.6): their
//! zero orientation, slots and peg holes, edge, tails and 3D alignment.

use super::*;
use openparts_core::Package;

fn pkg(yaml: &str) -> Package {
    serde_yaml::from_str(&format!(
        "schema_version: '0.1'\nkind: package\nid: standards/TEST\n{yaml}"
    ))
    .unwrap()
}

fn model(p: &Package) -> (openparts_mcad::MechanicalGeometry, PlacementModel) {
    let g = openparts_mcad::generate(p).unwrap();
    let m = build(p, None, &g, Density::N);
    (g, m)
}

fn pad<'a>(m: &'a PlacementModel, n: &str) -> &'a Pad {
    m.pads.iter().find(|p| p.number == n).unwrap()
}

/// Pin 1 upper left of the origin, per the Level A rule.
fn upper_left(m: &PlacementModel) -> bool {
    let p = m.pin1.as_ref().unwrap().position;
    p.x <= 1e-9 && p.y >= -1e-9 && (p.x < -1e-9 || p.y > 1e-9)
}

/// Every land and the body centre land where `model3d` puts them.
fn aligned(g: &openparts_mcad::MechanicalGeometry, m: &PlacementModel) -> bool {
    let (s, c) = m.model3d.rotation.to_radians().sin_cos();
    let place = |x: f64, y: f64| {
        (
            c * x - s * y + m.model3d.offset[0],
            s * x + c * y + m.model3d.offset[1],
        )
    };
    let lands_ok = g.leads.iter().zip(&m.pads).all(|(l, p)| {
        let (x, y) = place(l.position.x, l.position.y);
        (x - p.position.x).abs() < 1e-3 && (y - p.position.y).abs() < 1e-3
    });
    let (bx, by) = place(g.body.position.x, g.body.position.y);
    let b = &m.body.regions[0].center;
    lands_ok && (bx - b.x).abs() < 1e-3 && (by - b.y).abs() < 1e-3
}

/// A microSD socket: contacts 1..8 from the left along the rear edge
/// (pin 1 lower left of the land centroid), card slot toward +Y, two
/// shell pads per side, one peg hole.
fn micro_sd() -> Package {
    let mut pads = String::new();
    for i in 0..8 {
        let x = -3.85 + 1.1 * f64::from(i);
        pads += &format!(
            "\x20 - {{number: '{}', x: {x:.2}, y: -5.5, width: 0.7, length: 1.6}}\n",
            i + 1
        );
    }
    pads += "\x20 - {number: SH, x: -6.4, y: -4.0, width: 1.2, length: 1.6}\n\
             \x20 - {number: SH, x: 6.4, y: -4.0, width: 1.2, length: 1.6}\n\
             \x20 - {number: SH, x: -6.4, y: 4.5, width: 1.2, length: 1.6}\n\
             \x20 - {number: SH, x: 6.4, y: 4.5, width: 1.2, length: 1.6}\n\
             \x20 - {number: '', x: 4.0, y: 0, width: 1.0, length: 1.0, drill: 1.0, plated: false}\n";
    pkg(&format!(
        "family: card_socket\nlead_count: 12\npitch: {{typ: 1.1, unit: mm}}\n\
         dimensions:\n\
         \x20 body_width: {{typ: 12.0, unit: mm}}\n\
         \x20 body_length: {{typ: 12.0, unit: mm}}\n\
         \x20 body_height: {{typ: 1.5, unit: mm}}\n\
         connector: {{mechanism: push_push, interface: micro_sd}}\n\
         pads:\n{pads}"
    ))
}

#[test]
fn a_card_socket_turns_so_contact_1_is_upper_left() {
    let p = micro_sd();
    let (g, m) = model(&p);
    assert!(upper_left(&m));
    // Pin 1 started lower left: the first turn that fixes it is -90°.
    assert_eq!(m.model3d.rotation, 270.0);
    assert!(aligned(&g, &m));
    assert_eq!(m.basis.pads.kind, BasisKind::Datasheet);
    let peg = m.pads.iter().find(|p| p.kind == PadKind::Npth).unwrap();
    assert_eq!(peg.layers.copper, CopperLayers::None);
    assert_eq!(m.pads.iter().filter(|p| p.number == "SH").count(), 4);
    assert_eq!(m.pitch, Some(1.1));
    assert!(m.tht.is_none(), "an all-SMD socket has no tails");
}

#[test]
fn a_barrel_jack_keeps_its_slots_and_tails() {
    let p = pkg("family: barrel_jack\nlead_count: 3\npitch: {unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {typ: 9.0, unit: mm}\n\
         \x20 body_length: {typ: 14.0, unit: mm}\n\
         \x20 body_height: {typ: 11.0, unit: mm}\n\
         \x20 tail_length: {typ: 3.5, unit: mm}\n\
         pads:\n\
         \x20 - {number: '1', x: 0, y: -6.0, width: 3.0, length: 1.6, drill: 1.0, drill_length: 2.4}\n\
         \x20 - {number: '2', x: 0, y: 0.0, width: 3.0, length: 1.6, drill: 1.0, drill_length: 2.4}\n\
         \x20 - {number: '3', x: 4.7, y: -3.0, width: 1.6, length: 3.0, drill: 1.0, drill_length: 2.4}\n\
         provenance:\n  /dimensions/tail_length: [{source: x/Y, page: 1}]\n");
    let (g, m) = model(&p);
    assert!(upper_left(&m));
    assert!(aligned(&g, &m));
    let one = pad(&m, "1");
    assert_eq!(one.shape, PadShape::Oval);
    assert_eq!(one.slot, Some(2.4));
    assert_eq!(m.tht.as_ref().unwrap().tail_length, 3.5);
    assert_eq!(m.basis.tht.as_ref().unwrap().kind, BasisKind::Datasheet);
}

#[test]
fn a_terminal_block_keeps_pole_1_on_the_left() {
    let p = pkg(
        "family: terminal_block\nlead_count: 3\npitch: {typ: 5.0, unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {typ: 15.0, unit: mm}\n\
         \x20 body_length: {typ: 8.0, unit: mm}\n\
         \x20 body_height: {typ: 10.0, unit: mm}\n\
         connector: {orientation: right_angle}\n\
         pads:\n\
         \x20 - {number: '1', x: -5.0, y: -1.0, width: 2.4, length: 2.4, drill: 1.3}\n\
         \x20 - {number: '2', x: 0.0, y: -1.0, width: 2.4, length: 2.4, drill: 1.3}\n\
         \x20 - {number: '3', x: 5.0, y: -1.0, width: 2.4, length: 2.4, drill: 1.3}\n\
         provenance:\n  /connector: [{source: x/Y, page: 1}]\n",
    );
    let (g, m) = model(&p);
    assert_eq!(m.model3d.rotation, 0.0);
    assert!(upper_left(&m));
    assert!(aligned(&g, &m));
    assert_eq!(pad(&m, "1").shape, PadShape::Rect, "square pin-1 THT pad");
    assert_eq!(pad(&m, "2").shape, PadShape::Circle);
    assert!(pad(&m, "1").position.x < pad(&m, "3").position.x);
}

#[test]
fn an_edge_mount_rf_connector_reports_its_edge_and_overhang() {
    // Samtec SMA-J-P-X-ST-EM1-like: pads end 0.10 before the PCB edge,
    // the flange beyond it.
    let p = pkg("family: rf_coax\nlead_count: 3\npitch: {unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {typ: 6.35, unit: mm}\n\
         \x20 body_length: {typ: 1.73, unit: mm}\n\
         \x20 body_height: {typ: 6.35, unit: mm}\n\
         connector: {gender: female, orientation: right_angle, interface: sma, edge_mount: true}\n\
         pads:\n\
         \x20 - {number: '1', x: 0, y: -3.065, width: 1.27, length: 4.2}\n\
         \x20 - {number: '2', x: -2.865, y: -3.065, width: 1.27, length: 4.2}\n\
         \x20 - {number: '2', x: 2.865, y: -3.065, width: 1.27, length: 4.2}\n\
         edge: {side: +y, pcb_edge: -0.865}\n\
         provenance:\n  /edge: [{source: x/Y, page: 1}]\n  /pads: [{source: x/Y, page: 1}]\n  \
         /connector: [{source: x/Y, page: 1}]\n");
    let (g, m) = model(&p);
    // The centre contact (pin 1) is the land centroid: no turn places
    // it upper left, so the family frame stays (barrel toward +Y).
    assert_eq!(m.model3d.rotation, 0.0);
    let p1 = m.pin1.as_ref().unwrap().position;
    assert!(p1.x.abs() < 1e-9 && p1.y.abs() < 1e-9);
    assert!(aligned(&g, &m));
    let e = m.edge.as_ref().unwrap();
    assert_eq!(e.requirement, openparts_core::EdgeRequirement::Required);
    // The flange (1.73 deep, centred at the origin) reaches 0.865 + 0.865
    // = 1.73 past the edge line.
    assert!((e.overhang.unwrap() - 1.73).abs() < 1e-3, "{e:?}");
    // The body sits on the board's mid-plane, partly below the top face.
    assert!(m.body.regions[0].z_min < 0.0);
}

#[test]
fn micro_usb_and_fpc_turn_contact_1_upper_left() {
    let five = "pads:\n\
        \x20 - {number: '1', x: 1.3, y: -2.0, width: 0.4, length: 1.35}\n\
        \x20 - {number: '2', x: 0.65, y: -2.0, width: 0.4, length: 1.35}\n\
        \x20 - {number: '3', x: 0.0, y: -2.0, width: 0.4, length: 1.35}\n\
        \x20 - {number: '4', x: -0.65, y: -2.0, width: 0.4, length: 1.35}\n\
        \x20 - {number: '5', x: -1.3, y: -2.0, width: 0.4, length: 1.35}\n\
        \x20 - {number: S, x: -3.6, y: 0.5, width: 1.8, length: 1.9, drill: 0.7, drill_length: 1.2}\n\
        \x20 - {number: S, x: 3.6, y: 0.5, width: 1.8, length: 1.9, drill: 0.7, drill_length: 1.2}\n";
    for family in ["micro_usb_b", "fpc_connector"] {
        let extra = if family == "fpc_connector" {
            "connector: {contact_side: bottom}\n"
        } else {
            ""
        };
        let p = pkg(&format!(
            "family: {family}\nlead_count: 7\npitch: {{typ: 0.65, unit: mm}}\n\
             dimensions:\n\
             \x20 body_width: {{typ: 7.5, unit: mm}}\n\
             \x20 body_length: {{typ: 5.0, unit: mm}}\n\
             \x20 body_height: {{typ: 2.5, unit: mm}}\n\
             {extra}{five}"
        ));
        let (g, m) = model(&p);
        // Contact 1 starts lower right: turned 180°.
        assert_eq!(m.model3d.rotation, 180.0, "{family}");
        assert!(upper_left(&m), "{family}");
        assert!(aligned(&g, &m), "{family}");
        assert!(m.tht.is_some(), "{family}: shell tabs go through");
    }
}

#[test]
fn zero_orientation_rows_name_the_connector_families() {
    use openparts_core::PackageFamily as F;
    for f in [
        F::CardSocket,
        F::BarrelJack,
        F::FpcConnector,
        F::RfCoax,
        F::MicroUsbB,
    ] {
        assert_eq!(zero_orientation_row(f).0, "connector");
    }
    assert_eq!(zero_orientation_row(F::TerminalBlock).0, "terminal block");
}
