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
        // Pin 1 at the centroid (an RF connector's centre contact): no
        // turn places it, the generator's turn is kept.
        let at_centroid = m
            .pin1
            .as_ref()
            .is_some_and(|q| q.position.x.abs() < 1e-6 && q.position.y.abs() < 1e-6);
        assert!(
            upper_left(&m) || at_centroid,
            "{}: pin 1 at {:?}",
            p.id.0,
            m.pin1
        );
        assert!(centred(&m), "{}", p.id.0);
        assert_3d_alignment(&g, &m);
        n += 1;
    }
    assert!(n > 100);
}

/// A device pin's differential pair reaches its pad, with the partner's
/// pad; the hash does not change with it (pins are not hashed).
#[test]
fn pads_carry_the_device_diff_pairs() {
    let device: openparts_core::Device = serde_yaml::from_str(
        "schema_version: '0.1'\nkind: device\nid: ex/TEST\nmanufacturer: ex\ncategory: interface\n\
         pins:\n\
         \x20 '1': {name: DP, type: io, diff_pair: {partner: '2', polarity: p, standard: usb2}}\n\
         \x20 '2': {name: DM, type: io, diff_pair: {partner: '1', polarity: n, standard: usb2}}\n\
         \x20 '3': {name: VDD, type: power}\n",
    )
    .unwrap();
    let p = soic8();
    let g = openparts_mcad::generate(&p).unwrap();
    let with = build(&p, Some(&device), &g, Density::N);
    let pair = pad(&with, "1")
        .pin
        .as_ref()
        .unwrap()
        .diff_pair
        .as_ref()
        .unwrap();
    assert_eq!(pair.partner, "2");
    assert_eq!(pair.partner_pad.as_deref(), Some("2"));
    assert_eq!(pair.polarity, openparts_core::DiffPolarity::P);
    assert_eq!(pair.standard, openparts_core::DiffStandard::Usb2);
    assert!(pad(&with, "3").pin.as_ref().unwrap().diff_pair.is_none());
    let json = serde_json::to_value(&with).unwrap();
    let second = &json["pads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["number"] == "2")
        .unwrap()["pin"];
    assert_eq!(
        second["diff_pair"],
        serde_json::json!({"partner": "1", "partner_pad": "1", "polarity": "n", "standard": "usb2"})
    );
    assert_eq!(with.hash, model(&p, Density::N).hash);
}

/// Toshiba P-HSOP8-0405-1.27-001 (TB67H450FNG): SOIC-8 with a centred
/// exposed pad.
fn hsop8() -> Package {
    pkg(
        "family: soic\nlead_count: 8\npitch: {nominal: 1.27, unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {min: 3.81, max: 3.99, unit: mm}\n\
         \x20 body_length: {min: 4.8, max: 5.0, unit: mm}\n\
         \x20 overall_width: {min: 5.79, max: 6.2, unit: mm}\n\
         \x20 exposed_pad:\n\
         \x20   width: {min: 2.18, max: 2.39, unit: mm}\n\
         \x20   length: {min: 2.18, max: 2.39, unit: mm}\n",
    )
}

#[test]
fn an_exposed_pad_soic_keeps_pin_1_upper_left_and_marks_its_thermal_pad() {
    let m = model(&hsop8(), Density::N);
    assert!(upper_left(&m) && centred(&m));
    assert_eq!(m.model3d.rotation, 0.0);
    let ep = pad(&m, "EP");
    assert!(ep.exposed && ep.position.x.abs() < 1e-9 && ep.position.y.abs() < 1e-9);
    // The signal lands stay clear of it.
    let p1 = pad(&m, "1");
    assert!(p1.position.x + p1.size.x / 2.0 < -ep.size.x / 2.0);
}

/// An ESP32-WROOM-style module: castellated rows on three edges, a
/// segmented ground pad and a shield can.
fn wroom() -> Package {
    pkg("family: module\nlead_count: 39\npitch: {nominal: 1.27, unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {nominal: 18, unit: mm}\n\
         \x20 body_length: {nominal: 25.5, unit: mm}\n\
         \x20 body_height: {nominal: 0.8, unit: mm}\n\
         \x20 overall_height: {nominal: 3.1, unit: mm}\n\
         pad_rows:\n\
         \x20 - {side: -x, first: 1, count: 14, pitch: 1.27, start: 7.49, inset: 0.25, pad: [0.9, 1.5]}\n\
         \x20 - {side: -y, first: 15, count: 10, pitch: 1.27, start: 3.285, inset: 0.25, pad: [0.9, 1.5]}\n\
         \x20 - {side: +x, first: 25, count: 14, pitch: 1.27, start: 1.5, inset: 0.25, pad: [0.9, 1.5]}\n\
         pad_grids:\n\
         \x20 - {numbers: ['39'], columns: 3, rows: 3, pitch: [1.4, 1.4], pad: [0.9, 0.9], x: -1.5, y: -2.46}\n\
         shield: {x: 0, y: -2.9, width: 15.8, length: 17.6}\n")
}

#[test]
fn a_rows_module_is_level_a_castellated_at_its_edges_and_as_tall_as_its_can() {
    let m = model(&wroom(), Density::N);
    assert!(upper_left(&m));
    assert_eq!(m.model3d.rotation, 0.0);
    assert!(pad(&m, "1").castellated && pad(&m, "15").castellated);
    let ground: Vec<_> = m.pads.iter().filter(|p| p.number == "39").collect();
    assert_eq!(ground.len(), 9);
    assert!(ground.iter().all(|p| !p.castellated));
    assert!((m.body.height - 3.1).abs() < 1e-9);
    assert!(m.body.regions.iter().any(|r| r.name == "shield"));
}

/// JST SH-style SMT headers: top entry (`shrouded_header`) and side
/// entry (`right_angle_header`), 4 circuits.
fn smt_header(family: &str, offset: f64, mech_x: f64) -> Package {
    pkg(&format!(
        "family: {family}\nlead_count: 4\nlead_layout: [1, 4]\npitch: {{nominal: 1.0, unit: mm}}\n\
         dimensions:\n\
         \x20 body_width: {{nominal: 2.9, unit: mm}}\n\
         \x20 body_length: {{nominal: 6.0, unit: mm}}\n\
         \x20 body_height: {{nominal: 4.25, unit: mm}}\n\
         \x20 body_offset: {{nominal: {offset}, unit: mm}}\n\
         smt: {{pad: [1.55, 0.6], mech_pad: [1.8, 1.2], mech_pad_at: [{mech_x}, 2.8]}}\n"
    ))
}

#[test]
fn smt_headers_are_level_a_with_smd_signal_and_mechanical_pads() {
    for (family, offset, mech_x) in [
        ("shrouded_header", 1.775, 2.525),
        ("right_angle_header", -2.45, -3.875),
    ] {
        let m = model(&smt_header(family, offset, mech_x), Density::N);
        assert!(upper_left(&m), "{family}");
        assert!(m.pads.iter().all(|p| p.kind == PadKind::Smd), "{family}");
        assert_eq!(m.pads.iter().filter(|p| p.number == "MP").count(), 2);
        assert!(m.tht.is_none());
        assert_eq!(m.body.regions.len(), 1, "{family}: no pin region");
    }
}

fn device(category: &str, pins: &str) -> openparts_core::Device {
    serde_yaml::from_str(&format!(
        "schema_version: '0.1'\nkind: device\nid: t/T\nmanufacturer: t\ncategory: {category}\npins:\n{pins}"
    ))
    .unwrap()
}

#[test]
fn polarity_comes_from_the_device_pin_names() {
    use openparts_core::PolarTerminal;
    let p = chip0603();
    let g = openparts_mcad::generate(&p).unwrap();
    // Everlight numbers the anode 1: the cathode is pad 2.
    let led = device(
        "led",
        "  \"1\": { name: Anode, type: passive }\n  \"2\": { name: Cathode, type: passive }\n",
    );
    let m = build(&p, Some(&led), &g, Density::N);
    let pol = m.polarity.as_ref().expect("an LED is polarised");
    assert_eq!(pol.pad, "2");
    assert_eq!(pol.terminal, PolarTerminal::Cathode);
    assert_eq!(pol.mark, PolarityMark::Band);
    assert_eq!(pol.position, pad(&m, "2").position);
    // A tantalum's band is its + terminal.
    let tantalum = device(
        "capacitor",
        "  \"1\": { name: \"+\", type: passive }\n  \"2\": { name: \"-\", type: passive }\n",
    );
    let pol = build(&p, Some(&tantalum), &g, Density::N).polarity.unwrap();
    assert_eq!(
        (pol.pad.as_str(), pol.terminal),
        ("1", PolarTerminal::Positive)
    );
    // An MLCC, a bidirectional TVS, no device: no mark.
    let mlcc = device(
        "capacitor",
        "  \"1\": { name: \"1\", type: passive }\n  \"2\": { name: \"2\", type: passive }\n",
    );
    assert!(build(&p, Some(&mlcc), &g, Density::N).polarity.is_none());
    let tvs = device(
        "diode",
        "  \"1\": { name: Pin1, type: passive }\n  \"2\": { name: Pin2, type: passive }\n",
    );
    assert!(build(&p, Some(&tvs), &g, Density::N).polarity.is_none());
    assert!(model(&p, Density::N).polarity.is_none());
}

#[test]
fn polarity_is_only_for_two_terminal_families() {
    let p = sot23();
    let g = openparts_mcad::generate(&p).unwrap();
    let d = device(
        "diode",
        "  \"1\": { name: Anode, type: passive }\n  \"2\": { name: NC, type: nc }\n  \"3\": { name: Cathode, type: passive }\n",
    );
    assert!(build(&p, Some(&d), &g, Density::N).polarity.is_none());
}

#[test]
fn a_side_actuator_is_a_region_inside_the_courtyard_and_a_plunger_sets_the_height() {
    // B3U-3100P-like: two terminals, a ground foot, a knob on +Y.
    let p = pkg(
        "family: tact_switch_smd\nlead_count: 3\npitch: {nominal: 3.4, unit: mm}\n\
         actuation: side\npad_order: [\"2\", \"1\", \"3\"]\ndimensions:\n\
         \x20 body_width: {nominal: 3.0, unit: mm}\n\
         \x20 body_length: {nominal: 2.5, unit: mm}\n\
         \x20 body_height: {nominal: 1.2, unit: mm}\n\
         \x20 overall_width: {nominal: 4.0, unit: mm}\n\
         \x20 lead_width: {nominal: 1.4, unit: mm}\n\
         \x20 ground_terminal_offset: {nominal: 1.9, unit: mm}\n\
         \x20 actuator_length: {nominal: 0.7, unit: mm}\n\
         \x20 actuator_width: {nominal: 1.7, unit: mm}\n\
         \x20 actuator_thickness: {nominal: 1.0, unit: mm}\n",
    );
    let m = model(&p, Density::N);
    let act = m
        .body
        .regions
        .iter()
        .find(|r| r.name == "actuator")
        .unwrap();
    assert_eq!(
        (act.size.x.min(act.size.y), act.size.x.max(act.size.y)),
        (0.7, 1.7)
    );
    assert_eq!((act.z_min, act.z_max), (0.1, 1.1));
    // The courtyard reaches past the knob's tip, wherever the turn put it.
    let reach = |pts: &[Point2]| {
        pts.iter()
            .map(|p| p.x.abs().max(p.y.abs()))
            .fold(0.0, f64::max)
    };
    assert!(reach(&m.courtyard.polygons[0]) > reach(&act.outline));
    assert_eq!(m.pads.len(), 3);

    let top = pkg(
        "family: tact_switch_tht\nlead_count: 4\npitch: {nominal: 4.5, unit: mm}\n\
         dimensions:\n\
         \x20 body_width: {nominal: 6.0, unit: mm}\n\
         \x20 body_length: {nominal: 6.0, unit: mm}\n\
         \x20 body_height: {nominal: 3.4, unit: mm}\n\
         \x20 overall_height: {nominal: 5.0, unit: mm}\n\
         \x20 row_spacing: {nominal: 6.5, unit: mm}\n\
         \x20 hole_diameter: {nominal: 1.0, unit: mm}\n",
    );
    assert_eq!(model(&top, Density::N).body.height, 5.0);
}
