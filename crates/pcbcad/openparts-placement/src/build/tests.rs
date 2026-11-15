use super::*;
use openparts_core::Package;

fn pkg(yaml: &str) -> Package {
    serde_yaml::from_str(&format!(
        "schema_version: '0.1'\nkind: package\nid: standards/TEST\n{yaml}"
    ))
    .unwrap()
}

fn model(p: &Package, density: Density) -> PlacementModel {
    let g = openparts_mcad::generate(p).unwrap();
    build(p, None, &g, density)
}

fn pad<'a>(m: &'a PlacementModel, n: &str) -> &'a Pad {
    m.pads.iter().find(|p| p.number == n).unwrap()
}

fn soic8() -> Package {
    pkg(
        "family: soic\nlead_count: 8\npitch: {nominal: 1.27, unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {nominal: 3.9, unit: mm}\n\
         \x20 body_length: {nominal: 4.9, unit: mm}\n\
         \x20 overall_width: {min: 5.80, max: 6.20, unit: mm}\n\
         \x20 lead_width: {min: 0.31, max: 0.51, unit: mm}\n\
         \x20 lead_length: {min: 0.40, max: 1.27, unit: mm}\n\
         provenance:\n\
         \x20 /dimensions/lead_width: [{source: ti/TEST, page: 3}]\n",
    )
}

fn sot23() -> Package {
    pkg(
        "family: sot\nlead_count: 3\nlead_layout: [2, 1]\npitch: {nominal: 0.95, unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {nominal: 1.3, unit: mm}\n\
         \x20 body_length: {nominal: 2.9, unit: mm}\n\
         \x20 overall_width: {nominal: 2.4, unit: mm}\n",
    )
}

fn chip0603() -> Package {
    pkg("family: chip\nlead_count: 2\npitch: {unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {nominal: 1.6, unit: mm}\n\
         \x20 body_length: {nominal: 0.8, unit: mm}\n\
         \x20 body_height: {nominal: 0.45, unit: mm}\n\
         \x20 lead_length: {nominal: 0.3, unit: mm}\n")
}

fn crystal() -> Package {
    // ABM8-272: 4 terminals, pin 1 lower left in the source's top view.
    pkg("family: lga\nlead_count: 4\nlead_layout: [2, 0, 2, 0]\npad_order: [\"4\", \"1\", \"2\", \"3\"]\n\
         pitch: {nominal: 1.5, unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {nominal: 3.2, unit: mm}\n\
         \x20 body_length: {nominal: 2.5, unit: mm}\n\
         \x20 lead_width: {nominal: 1.2, unit: mm}\n\
         \x20 lead_length: {nominal: 0.9, unit: mm}\n")
}

fn header_2x5() -> Package {
    pkg("family: header\nlead_count: 10\nlead_layout: [2, 5]\n\
         pitch: {nominal: 2.54, unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {nominal: 5.08, unit: mm}\n\
         \x20 body_length: {nominal: 12.7, unit: mm}\n\
         \x20 row_spacing: {nominal: 2.54, unit: mm}\n\
         \x20 hole_diameter: {nominal: 1.0, unit: mm}\n")
}

/// GCT USB4105's land pattern (source frame: opening toward +Y).
fn usb_c() -> Package {
    pkg("family: usb_c\nlead_count: 4\npitch: {unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {nominal: 8.94, unit: mm}\n\
         \x20 body_length: {nominal: 7.35, unit: mm}\n\
         \x20 body_height: {nominal: 3.31, unit: mm}\n\
         \x20 body_offset: {nominal: -1.075, unit: mm}\n\
         pads:\n\
         \x20 - {number: A1, x: 3.2, y: -4.755, width: 0.6, length: 1.15}\n\
         \x20 - {number: A12, x: -3.2, y: -4.755, width: 0.6, length: 1.15}\n\
         \x20 - {number: SHELL, x: 4.32, y: -4.18, width: 1.0, length: 2.1, drill: 0.6, drill_length: 1.7}\n\
         \x20 - {number: SHELL, x: -4.32, y: 0.0, width: 1.0, length: 1.8, drill: 0.6, drill_length: 1.4}\n\
         edge: {side: +y, pcb_edge: 2.6}\n\
         keepouts:\n\
         \x20 - {purpose: mechanical, restricts: [components], rect: {x: 0, y: 4, width: 2, length: 1}}\n\
         provenance:\n\
         \x20 /edge: [{source: gct/USB4105-DWG, page: 1}]\n\
         \x20 /pads: [{source: gct/USB4105-DWG, page: 1}]\n\
         \x20 /keepouts: [{source: gct/USB4105-DWG, page: 1}]\n")
}

/// Pin 1 upper left of the origin, per the Level A rule.
fn upper_left(m: &PlacementModel) -> bool {
    let p = m.pin1.as_ref().unwrap().position;
    p.x <= 1e-9 && p.y >= -1e-9 && (p.x < -1e-9 || p.y > 1e-9)
}

/// The extents of the plated pads are centred on the origin.
fn centred(m: &PlacementModel) -> bool {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in m.pads.iter().filter(|p| p.plated) {
        let (lo, hi) = p.bounds();
        x0 = x0.min(lo.x);
        y0 = y0.min(lo.y);
        x1 = x1.max(hi.x);
        y1 = y1.max(hi.y);
    }
    ((x0 + x1) / 2.0).abs() < 1e-3 && ((y0 + y1) / 2.0).abs() < 1e-3
}

#[test]
fn soic_keeps_its_generator_turn_pin_1_top_of_the_left_row() {
    let m = model(&soic8(), Density::N);
    assert_eq!(m.model3d.rotation, 0.0);
    let (p1, p4, p5) = (pad(&m, "1"), pad(&m, "4"), pad(&m, "5"));
    assert!(p1.position.x < 0.0 && p1.position.y > 0.0);
    assert!((p1.position.x - p4.position.x).abs() < 1e-9 && p4.position.y < 0.0);
    assert!(p5.position.x > 0.0 && p5.position.y < 0.0);
    assert!(centred(&m));
}

#[test]
fn sot23_turns_so_pins_1_and_2_run_down_the_left() {
    let m = model(&sot23(), Density::N);
    assert_eq!(m.model3d.rotation, 270.0);
    let (p1, p2, p3) = (pad(&m, "1"), pad(&m, "2"), pad(&m, "3"));
    assert!(p1.position.x < 0.0 && p1.position.y > 0.0, "{p1:?}");
    assert!((p1.position.x - p2.position.x).abs() < 1e-9 && p2.position.y < 0.0);
    assert!(p3.position.x > 0.0 && p3.position.y.abs() < 1e-9);
    // Lands turn with it: longer across the body (along X).
    assert!(p1.size.x > p1.size.y);
    assert!(upper_left(&m) && centred(&m));
}

#[test]
fn two_terminal_chips_have_pin_1_on_the_left() {
    let m = model(&chip0603(), Density::N);
    let p1 = pad(&m, "1");
    assert!(p1.position.x < 0.0 && p1.position.y.abs() < 1e-9);
    assert_eq!(m.model3d.rotation, 0.0);
}

#[test]
fn a_crystal_with_pin_1_lower_left_turns_a_quarter() {
    let m = model(&crystal(), Density::N);
    assert!(upper_left(&m), "{:?}", m.pin1);
    assert_eq!(m.model3d.rotation, 270.0);
}

#[test]
fn headers_keep_pin_1_upper_left() {
    let m = model(&header_2x5(), Density::N);
    let p1 = pad(&m, "1");
    let p2 = pad(&m, "2");
    assert!(p1.position.x < 0.0 && p1.position.y > 0.0);
    assert!(p2.position.x > 0.0 && (p2.position.y - p1.position.y).abs() < 1e-9);
    assert_eq!(m.model3d.rotation, 0.0);
    assert_eq!(p1.shape, PadShape::Rect);
    assert_eq!(p2.shape, PadShape::Circle);
    assert!(m.tht.is_some());
}

#[test]
fn usb_c_turns_so_a1_is_upper_left_and_the_opening_faces_down() {
    let p = usb_c();
    let m = model(&p, Density::N);
    assert_eq!(m.model3d.rotation, 180.0);
    assert!(upper_left(&m));
    assert!(centred(&m));
    // Land centroid in the source frame: x 0, y (0.9 + -5.33) / 2.
    let cy = (0.9 + -5.33) / 2.0;
    assert!((m.model3d.offset[1] - cy).abs() < 1e-4, "{:?}", m.model3d);
    // The mating face (+y in the source) now faces -y.
    let e = m.edge.as_ref().unwrap();
    assert_eq!(e.side, Side::MinusY);
    // From the centroid to the edge line: 2.6 - cy.
    assert!((e.pcb_edge.unwrap() - (2.6 - cy)).abs() < 1e-4);
    // The shell's front face is 2.6 in front of the front slots.
    assert!(e.overhang.unwrap().abs() < 1e-4, "{e:?}");
    assert_eq!(m.basis.edge.as_ref().unwrap().kind, BasisKind::Datasheet);
    assert_eq!(m.basis.pads.kind, BasisKind::Datasheet);
    // The keep-out turns with the part.
    let k = &m.keepouts[0];
    let poly = k.polygon.as_ref().unwrap();
    assert!(poly.iter().all(|q| q.y < 0.0));
    // Slots stay along the land's longer side.
    let shell = m.pads.iter().find(|p| p.number == "SHELL").unwrap();
    assert_eq!(shell.shape, PadShape::Oval);
    assert!(shell.size.y > shell.size.x);
}

#[test]
fn courtyard_levels_grow_l_n_m() {
    let p = soic8();
    let width = |d| {
        let m = model(&p, d);
        let poly = &m.courtyard.polygons[0];
        let xs = poly.iter().map(|q| q.x);
        let (lo, hi) = (
            xs.clone().fold(f64::MAX, f64::min),
            xs.fold(f64::MIN, f64::max),
        );
        (m.courtyard.excess, hi - lo, m.hash)
    };
    let (l, n, m) = (width(Density::L), width(Density::N), width(Density::M));
    assert_eq!((l.0, n.0, m.0), (0.10, 0.25, 0.50));
    assert!(l.1 < n.1 && n.1 < m.1, "{l:?} {n:?} {m:?}");
    // Each level is its own model.
    assert!(l.2 != n.2 && n.2 != m.2);
    // On the 0.05 grid, and enclosing the lands by at least the excess.
    let mm = model(&p, Density::N);
    let land_x = mm
        .pads
        .iter()
        .map(|q| q.bounds().1.x)
        .fold(f64::MIN, f64::max);
    for q in &mm.courtyard.polygons[0] {
        assert!(((q.x / 0.05).round() * 0.05 - q.x).abs() < 1e-9);
    }
    let right = mm.courtyard.polygons[0]
        .iter()
        .map(|q| q.x)
        .fold(f64::MIN, f64::max);
    assert!(right >= land_x + 0.25 - 1e-9);
    assert_eq!(mm.basis.courtyard.kind, BasisKind::Ipc7351);
    // Through-hole parts: the generator default.
    let h = model(&header_2x5(), Density::N);
    assert_eq!(h.courtyard.excess, 0.5);
    assert_eq!(h.basis.courtyard.kind, BasisKind::GeneratorDefault);
}

#[test]
fn hash_is_stable_and_tracks_geometry_only() {
    let p = soic8();
    let a = model(&p, Density::N);
    let b = model(&p, Density::N);
    assert_eq!(a.hash, b.hash);
    assert!(a.hash.starts_with("sha256:") && a.hash.len() == 7 + 64);
    // A golden value: the hash covers the JSON form, so a change here
    // means every generated footprint changes (bump deliberately).
    assert_eq!(a.hash, GOLDEN_SOIC8);
    // Where a number came from doesn't change the copper.
    let mut other = p.clone();
    other.provenance.clear();
    assert_eq!(model(&other, Density::N).hash, a.hash);
    // A different lead width does.
    let mut wider = p.clone();
    wider.dimensions.lead_width.as_mut().unwrap().max = Some(0.6);
    assert_ne!(model(&wider, Density::N).hash, a.hash);
    // The hash is of the model without itself.
    assert_eq!(crate::hash::content_hash(&a), a.hash);
}

const GOLDEN_SOIC8: &str =
    "sha256:c945e8a1039cfd2f827f6524a40be608fdb3813cad736cae2667bee4335d2d7f";

#[test]
fn soic_basis_points_at_its_provenance() {
    let m = model(&soic8(), Density::N);
    assert_eq!(m.basis.pads.kind, BasisKind::Ipc7351);
    assert!(m
        .basis
        .pads
        .sources
        .iter()
        .any(|s| s.pointer == "/dimensions/lead_width" && s.page == Some(3)));
    assert!(m
        .basis
        .body
        .defaults
        .contains(&"/dimensions/body_height".to_string()));
    assert_eq!(m.pitch, Some(1.27));
    assert!(m.min_pad_gap.unwrap() > 0.5 && m.min_pad_gap.unwrap() < 1.27);
}

#[test]
fn json_round_trips() {
    let m = model(&usb_c(), Density::M);
    let json = serde_json::to_string(&m).unwrap();
    let back: PlacementModel = serde_json::from_str(&json).unwrap();
    assert_eq!(back, m);
}

/// The 3D model (built in the generator frame) lines up with the pads
/// under `model3d`: every land and the body centre map onto the model's.
fn assert_3d_alignment(g: &openparts_mcad::MechanicalGeometry, m: &PlacementModel) {
    let place = |x: f64, y: f64| {
        let (s, c) = m.model3d.rotation.to_radians().sin_cos();
        (
            c * x - s * y + m.model3d.offset[0],
            s * x + c * y + m.model3d.offset[1],
        )
    };
    let lands = if g.lands.is_empty() {
        &g.leads
    } else {
        &g.lands
    };
    for (land, pad) in lands.iter().zip(&m.pads) {
        let (x, y) = place(land.position.x, land.position.y);
        assert!(
            (x - pad.position.x).abs() < 1e-3 && (y - pad.position.y).abs() < 1e-3,
            "{:?}: land {} at ({x}, {y}), pad at {:?}",
            m.package,
            land.number,
            pad.position
        );
    }
    let (bx, by) = place(g.body.position.x, g.body.position.y);
    let body = &m.body.regions[0].center;
    assert!((bx - body.x).abs() < 1e-3 && (by - body.y).abs() < 1e-3);
}

#[test]
fn the_3d_model_lines_up_with_the_pads() {
    for p in [soic8(), sot23(), crystal(), usb_c(), header_2x5()] {
        let g = openparts_mcad::generate(&p).unwrap();
        assert_3d_alignment(&g, &build(&p, None, &g, Density::N));
    }
}

/// Every package of the real dataset (when it is checked out next to
/// the repository) gets a model in its Level A orientation.
#[test]
fn every_dataset_package_is_level_a() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../openparts-data");
    if !root.exists() {
        return;
    }
    let ds = openparts_data::Dataset::load(&root).unwrap();
    let mut n = 0;
    for p in ds.packages.values() {
        let Ok(g) = openparts_mcad::generate(p) else {
            continue;
        };
        let m = build(p, None, &g, Density::N);
        assert!(upper_left(&m), "{}: pin 1 at {:?}", p.id.0, m.pin1);
        assert!(centred(&m), "{}", p.id.0);
        assert_3d_alignment(&g, &m);
        n += 1;
    }
    assert!(n > 100);
}
