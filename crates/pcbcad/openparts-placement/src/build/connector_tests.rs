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
    // Samtec SMA-J-P-X-ST-EM1's layout (rev G): the flange depth is not
    // printed ("1.73 REF" is the gap between the leg pairs), so the frame
    // starts at the PCB edge line. The bottom lands are this test's own.
    let p = pkg("family: rf_coax\nlead_count: 5\npitch: {unit: mm}\n\
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
         edge: {side: +y, pcb_edge: 0}\n\
         provenance:\n  /edge: [{source: x/Y, page: 1}]\n  /pads: [{source: x/Y, page: 1}]\n  \
         /connector: [{source: x/Y, page: 1}]\n");
    let (g, m) = model(&p);
    assert!(aligned(&g, &m));
    let e = m.edge.as_ref().unwrap();
    assert_eq!(e.requirement, openparts_core::EdgeRequirement::Required);
    // The flange, drawn at the default depth from the edge line, reaches
    // that depth past it.
    let depth = openparts_mcad_flange_depth();
    assert!((e.overhang.unwrap() - depth).abs() < 1e-3, "{e:?}");
    assert!(m
        .basis
        .body
        .defaults
        .contains(&"/dimensions/body_length".to_string()));
    // The body sits on the board's mid-plane, partly below the top face.
    assert!(m.body.regions[0].z_min < 0.0);
    // Bottom lands: the opposite copper, mask and paste; no gap is counted
    // between a top land and the bottom land under it.
    let bottom: Vec<_> = m.pads.iter().filter(|p| p.is_bottom()).collect();
    assert_eq!(bottom.len(), 2);
    assert!(bottom
        .iter()
        .all(|p| p.layers.mask == MaskLayers::Bottom && p.layers.paste));
    assert!(m.min_pad_gap.unwrap() > 0.5, "{:?}", m.min_pad_gap);
}

fn openparts_mcad_flange_depth() -> f64 {
    // `defaults::connector::RF_FLANGE_DEPTH` (private to openparts-mcad).
    1.5
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
    for family in ["micro_usb_b", "mini_usb_b", "fpc_connector"] {
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
fn holes_without_a_land_say_the_land_is_a_default() {
    let p = pkg(
        "family: terminal_block\nlead_count: 2\npitch: {typ: 3.5, unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {typ: 7.0, unit: mm}\n\
         \x20 body_length: {typ: 7.0, unit: mm}\n\
         \x20 body_height: {typ: 8.0, unit: mm}\n\
         pads:\n\
         \x20 - {number: '1', x: -1.75, y: 0, drill: 1.1}\n\
         \x20 - {number: '2', x: 1.75, y: 0, drill: 1.1}\n",
    );
    let (g, m) = model(&p);
    assert!(aligned(&g, &m));
    assert_eq!(m.basis.pads.kind, BasisKind::Datasheet);
    assert_eq!(m.basis.pads.defaults, ["/pads/land"]);
    assert!((pad(&m, "2").size.x - 1.8).abs() < 1e-9);
    assert_eq!(pad(&m, "1").drill, Some(1.1));
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
        F::MiniUsbB,
        F::UsbB,
    ] {
        assert_eq!(zero_orientation_row(f).0, "connector");
    }
    assert_eq!(zero_orientation_row(F::TerminalBlock).0, "terminal block");
}

/// Stated land shapes (Canonical Data Specification 20.7) reach the
/// model: an MMCX jack's round centre pad and notched ground pads keep
/// the family frame (pin 1 is the centroid).
#[test]
fn stated_land_shapes_reach_the_model() {
    let p = pkg("family: rf_coax\nlead_count: 2\npitch: {unit: mm}\n\
         connector: {gender: female, orientation: vertical, interface: mmcx}\n\
         dimensions:\n  body_width: {nominal: 3.45, unit: mm}\n  body_length: {nominal: 3.45, unit: mm}\n\
         pads:\n\
         \x20 - {number: '1', x: 0, y: 0, width: 1.2, length: 1.2, shape: circle}\n\
         \x20 - {number: '2', x: 1.3, y: 1.3, width: 1.35, length: 1.35, cutout: {x: 0, y: 0, diameter: 2.5}}\n");
    let (_, m) = model(&p);
    assert_eq!(m.frame.zero_orientation.pin1.as_deref(), Some("1"));
    assert_eq!(pad(&m, "1").shape, PadShape::Circle);
    let g = pad(&m, "2");
    assert_eq!(g.shape, PadShape::Polygon);
    let poly = g.polygon.as_ref().unwrap();
    assert!(poly.len() > 6, "the arc is segmented");
    assert!((g.size.x - 1.35).abs() < 1e-9);
}

/// A chamfered land's corners turn with the model: a 2020 LED whose pad 1
/// is lower right turns 180°, so its cut top-right corner ends up bottom
/// left.
#[test]
fn chamfer_corners_turn_with_the_model() {
    let p = pkg("family: smd_led\nlead_count: 4\npitch: {unit: mm}\n\
         dimensions:\n  body_width: {nominal: 2.2, unit: mm}\n  body_length: {nominal: 2.0, unit: mm}\n\
         \x20 overall_height: {nominal: 0.84, unit: mm}\n\
         pads:\n\
         \x20 - {number: '1', x: 0.915, y: -0.55, width: 0.7, length: 0.7, shape: chamfered, chamfer: 0.2, corners: [top_right]}\n\
         \x20 - {number: '2', x: 0.915, y: 0.55, width: 0.7, length: 0.7}\n\
         \x20 - {number: '3', x: -0.915, y: 0.55, width: 0.7, length: 0.7}\n\
         \x20 - {number: '4', x: -0.915, y: -0.55, width: 0.7, length: 0.7}\n");
    let (g, m) = model(&p);
    assert!(upper_left(&m));
    assert_eq!(m.model3d.rotation, 180.0);
    assert!(aligned(&g, &m));
    let p1 = pad(&m, "1");
    assert_eq!(p1.shape, PadShape::Chamfered);
    assert_eq!(
        p1.chamfer.as_ref().unwrap().corners,
        vec![PadCorner::BottomLeft]
    );
    assert_eq!(m.frame.zero_orientation.family, "SMD LED");
}

/// An SMD electrolytic keeps its frame (+ terminal, pad 1, on the left)
/// and lists the can among the body's sources.
#[test]
fn an_smd_electrolytic_keeps_the_two_terminal_frame() {
    let p = pkg("family: smd_electrolytic\nlead_count: 2\npitch: {unit: mm}\n\
         dimensions:\n  body_width: {nominal: 6.6, unit: mm}\n  body_length: {nominal: 6.6, unit: mm}\n\
         \x20 overall_height: {nominal: 5.8, unit: mm}\n  can_diameter: {nominal: 6.3, unit: mm}\n\
         pads:\n\
         \x20 - {number: '1', x: -2.5, y: 0, width: 3.2, length: 1.6}\n\
         \x20 - {number: '2', x: 2.5, y: 0, width: 3.2, length: 1.6}\n");
    let (g, m) = model(&p);
    assert!(upper_left(&m));
    assert_eq!(m.model3d.rotation, 0.0);
    assert!(aligned(&g, &m));
    assert_eq!(m.frame.zero_orientation.family, "two-terminal");
}

/// A MiniMELF keeps the two-terminal frame (pad 1 on the left) and its
/// height is the diameter it lies on the board with.
#[test]
fn a_melf_keeps_the_two_terminal_frame() {
    let p = pkg("family: melf\nlead_count: 2\npitch: {unit: mm}\n\
         dimensions:\n  body_width: {min: 1.4, max: 1.6, unit: mm}\n  body_length: {min: 3.3, max: 3.7, unit: mm}\n\
         pads:\n\
         \x20 - {number: '1', x: -1.875, y: 0, width: 1.25, length: 2.0}\n\
         \x20 - {number: '2', x: 1.875, y: 0, width: 1.25, length: 2.0}\n");
    let (g, m) = model(&p);
    assert!(upper_left(&m));
    assert_eq!(m.model3d.rotation, 0.0);
    // The lands are the stated pads, the body centred between them.
    assert!((pad(&m, "1").position.x + 1.875).abs() < 1e-9);
    assert!((pad(&m, "2").size.y - 2.0).abs() < 1e-9);
    assert!(m.body.regions[0].center.x.abs() < 1e-9);
    assert_eq!(g.body.shape, openparts_mcad::BodyShape::CylinderX);
    assert_eq!(m.frame.zero_orientation.family, "two-terminal");
    assert!((m.body.height - 1.5).abs() < 1e-9, "{:?}", m.body.height);
}

/// The electromechanical families (20.14) place like the connectors:
/// their stated pads, pin 1 upper left, the 3D body over the lands, the
/// pins' tails below the board.
#[test]
fn electromechanical_families_place_their_pads_and_body() {
    for family in ["relay", "buzzer", "trimmer", "rotary_encoder"] {
        let p = pkg(&format!(
            "family: {family}\nlead_count: 3\npitch: {{unit: mm}}\n\
             dimensions:\n\
             \x20 body_width: {{typ: 9.5, unit: mm}}\n\
             \x20 body_length: {{typ: 9.5, unit: mm}}\n\
             \x20 body_height: {{typ: 4.8, unit: mm}}\n\
             pads:\n\
             \x20 - {{number: '1', x: -2.5, y: 2.5, width: 1.6, length: 1.6, drill: 0.9}}\n\
             \x20 - {{number: '2', x: 0, y: -2.5, width: 1.6, length: 1.6, drill: 0.9}}\n\
             \x20 - {{number: '3', x: 2.5, y: 2.5, width: 1.6, length: 1.6, drill: 0.9}}\n"
        ));
        let (g, m) = model(&p);
        assert_eq!(m.pads.len(), 3, "{family}");
        assert!(upper_left(&m), "{family}");
        assert!(aligned(&g, &m), "{family}");
        assert_eq!(zero_orientation_row(p.family).0, "electromechanical");
    }
}
