//! End-to-end test of the First Vertical Slice (Architecture
//! Specification section 32): Canonical YAML -> Parser -> Validator ->
//! Effective Model -> KiCad + STEP, against real fixture data (Raspberry
//! Pi RP2040, and a Yageo chip resistor for the "chip" family) in the
//! sibling `openparts-data` repository.
//!
//! KiCad output is checked by reading it back with the same parsers
//! openparts-kicad's own unit tests use (Testing and Quality
//! Specification section 9, "Independent read-back" / "Semantic
//! checks") -- not just a substring match. STEP/STL output (built by
//! SolidCraft) only gets structural checks here -- solid and colour
//! counts, STL framing -- and is skipped when SolidCraft isn't installed.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../openparts-data")
}

fn load_rp2040() -> (
    openparts_core::Part,
    openparts_core::Device,
    openparts_core::Package,
    BTreeSet<openparts_core::SourceId>,
) {
    let data = data_dir();
    let part =
        openparts_data::load_part(data.join("parts/raspberrypi/rp2040/RP2040.yaml")).unwrap();
    let device = openparts_data::load_device(data.join("devices/raspberrypi/RP2040.yaml")).unwrap();
    let package =
        openparts_data::load_package(data.join("packages/standards/qfn/QFN56-RP2040.yaml"))
            .unwrap();
    let source =
        openparts_data::load_source(data.join("sources/raspberrypi/RP-008371-DS.yaml")).unwrap();

    let mut known_sources = BTreeSet::new();
    known_sources.insert(source.id);

    (part, device, package, known_sources)
}

#[test]
fn rp2040_fixture_passes_validation() {
    let (part, device, package, known_sources) = load_rp2040();
    let diags = openparts_validator::validate_part(&part, &device, &package, &known_sources);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
}

#[test]
fn full_pipeline_generates_semantically_correct_kicad_and_step_for_rp2040() {
    let (part, device, package, known_sources) = load_rp2040();
    let diags = openparts_validator::validate_part(&part, &device, &package, &known_sources);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let model = openparts_core::build_effective_model(part.clone(), &device, package.clone(), None)
        .expect("effective model");

    // Uses the family dispatcher (not generate_qfn directly), matching
    // what the CLI actually calls.
    let geometry = openparts_mcad::generate(&model.package).expect("geometry");
    // 56 perimeter leads; no Exposed Pad lead, since its physical
    // dimensions are not confirmed in the source datasheet (the Device
    // still records the "EP" pin electrically -- see the symbol check
    // below).
    assert_eq!(geometry.leads.len(), 56);

    let symbol = openparts_pcbcad::build_symbol(&part.mpn, &model.device);
    let symbol_text = openparts_kicad::render_symbol(&symbol);
    let parsed_pins = openparts_kicad::parse_symbol_pins(&symbol_text);
    // 56 perimeter pins + the EP pin, since the symbol is built from the
    // Device (electrical facts), independent of geometry confirmation.
    assert_eq!(parsed_pins.len(), 57);
    let pin1 = parsed_pins.iter().find(|p| p.number == "1").unwrap();
    assert_eq!(pin1.name, "IOVDD");
    assert_eq!(pin1.electrical_type, "power_in");
    let ep_pin = parsed_pins.iter().find(|p| p.number == "EP").unwrap();
    assert_eq!(ep_pin.name, "GND");

    let footprint = openparts_pcbcad::build_footprint(&part.mpn, &geometry);
    let footprint_text = openparts_kicad::render_footprint(&footprint);
    let parsed_pads = openparts_kicad::parse_footprint_pads(&footprint_text);
    assert_eq!(parsed_pads.len(), 56);

    // The footprint pads are the geometry's lands (IPC-7351B, from the
    // leads), position for position -- the IR conversion doesn't
    // silently drop or reorder anything.
    assert!(!geometry.lands.is_empty(), "QFN gets IPC lands");
    for lead in &geometry.lands {
        let pad = parsed_pads
            .iter()
            .find(|p| p.number == lead.number)
            .unwrap();
        assert!((pad.x - lead.position.x).abs() < 1e-9);
        assert!((pad.y - lead.position.y).abs() < 1e-9);
    }

    if !openparts_solidcraft::available_for_test(module_path!()) {
        return;
    }
    let input = openparts_step::ModelInput {
        package: &model.package,
        geometry: &geometry,
        category: Some(model.device.category),
        marked_pin: openparts_solidcraft::polarity_pin(&model.device),
    };
    let step_text = openparts_step::generate_step(&input, &part.mpn).expect("step");
    assert!(step_text.starts_with("ISO-10303-21;"));
    // The body plus 56 terminals inlaid in it, each its own coloured solid.
    assert_eq!(step_text.matches("MANIFOLD_SOLID_BREP(").count(), 57);
    assert_eq!(step_text.matches("STYLED_ITEM(").count(), 57);

    let stl = openparts_stl::generate_stl(&input, &part.mpn).expect("stl");
    assert!(openparts_stl::triangle_count(&stl).unwrap() >= 57 * 12);
}

#[test]
fn known_revision_with_no_documented_overrides_matches_the_base_device() {
    // RP2040's B0/B1/B2 silicon revisions are real and documented, but
    // the datasheet documents no pinout/pin-behavior difference between
    // them -- selecting one must not invent a difference that isn't
    // there (Testing and Quality Specification section 4: never
    // synthesize a value that wasn't actually given).
    let (part, device, package, known_sources) = load_rp2040();
    let diags = openparts_validator::validate_part(&part, &device, &package, &known_sources);
    assert!(diags.is_empty());

    let base_model =
        openparts_core::build_effective_model(part.clone(), &device, package.clone(), None)
            .unwrap();
    let b2_model =
        openparts_core::build_effective_model(part, &device, package, Some("rev-b2")).unwrap();

    assert_eq!(base_model.device.pins, b2_model.device.pins);
    assert_eq!(b2_model.applied_revision.as_deref(), Some("rev-b2"));
}

#[test]
fn unresolvable_dependency_stops_generation_instead_of_guessing() {
    // A revision that does not exist must be rejected, not silently
    // ignored (Testing and Quality Specification section 7: "一意に安全
    // な生成結果を決められない操作は停止する").
    let (part, device, package, _) = load_rp2040();
    let err =
        openparts_core::build_effective_model(part, &device, package, Some("rev-does-not-exist"))
            .unwrap_err();
    assert!(matches!(
        err,
        openparts_core::EffectiveModelError::UnknownRevision { .. }
    ));
}

#[test]
fn full_pipeline_generates_correct_kicad_and_step_for_a_chip_resistor() {
    // Exercises the "chip" (2-terminal) family through the same
    // dispatcher, proving the pipeline isn't LQFP/QFN-specific.
    let data = data_dir();
    let part =
        openparts_data::load_part(data.join("parts/yageo/rc0603/RC0603FR-0710KL.yaml")).unwrap();
    let device =
        openparts_data::load_device(data.join("devices/yageo/RC0603-GENERAL-PURPOSE.yaml"))
            .unwrap();
    let package =
        openparts_data::load_package(data.join("packages/standards/chip/CHIP0603-RC.yaml"))
            .unwrap();
    let source = openparts_data::load_source(data.join("sources/yageo/PYU-RC_GROUP.yaml")).unwrap();
    let mut known_sources = BTreeSet::new();
    known_sources.insert(source.id);

    let diags = openparts_validator::validate_part(&part, &device, &package, &known_sources);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let model = openparts_core::build_effective_model(part.clone(), &device, package.clone(), None)
        .unwrap();
    let geometry = openparts_mcad::generate(&model.package).expect("geometry");
    assert_eq!(geometry.leads.len(), 2);

    let footprint = openparts_pcbcad::build_footprint(&part.mpn, &geometry);
    let footprint_text = openparts_kicad::render_footprint(&footprint);
    let parsed_pads = openparts_kicad::parse_footprint_pads(&footprint_text);
    assert_eq!(parsed_pads.len(), 2);

    if !openparts_solidcraft::available_for_test(module_path!()) {
        return;
    }
    let input = openparts_step::ModelInput {
        package: &model.package,
        geometry: &geometry,
        category: Some(model.device.category),
        marked_pin: openparts_solidcraft::polarity_pin(&model.device),
    };
    let step_text = openparts_step::generate_step(&input, &part.mpn).unwrap();
    // A thick-film resistor: alumina core, black overcoat, 2 end caps.
    assert_eq!(step_text.matches("MANIFOLD_SOLID_BREP(").count(), 4);
}

/// Every package in the dataset: its SMD footprint pads (the geometry's
/// lead outlines) never overlap each other, and where a package states
/// its lead-tip span (`overall_width`/`overall_length`) the pads reach
/// exactly across it -- toe to toe, or lead toe to tab end on TO-252. Guards the
/// switch from generator defaults to datasheet lead dimensions, which
/// moved real footprint pads.
#[test]
fn dataset_footprint_pads_do_not_overlap_and_toes_follow_the_lead_span() {
    let dataset = openparts_data::Dataset::load(data_dir()).unwrap();
    assert!(dataset.issues.is_empty(), "{:?}", dataset.issues);
    assert!(!dataset.packages.is_empty());
    // Every package must generate: a package the generators can't draw
    // is a data or generator bug, not something to skip silently. The
    // only exceptions are listed here, each with its reason.
    const KNOWN_UNDRAWABLE: &[(&str, &str)] =
        &[("standards/RADIAL-ECS-200-9X", "source states no lead pitch")];
    let mut failures = Vec::new();
    for package in dataset.packages.values() {
        let geometry = match openparts_mcad::generate(package) {
            Ok(g) => g,
            Err(e) => {
                if !KNOWN_UNDRAWABLE
                    .iter()
                    .any(|(id, _)| *id == package.id.to_string())
                {
                    failures.push(format!("{}: {e}", package.id));
                }
                continue;
            }
        };
        let rect = |l: &openparts_mcad::Lead| {
            let (hx, hy) = (l.size.x / 2.0, l.size.y / 2.0);
            (
                l.position.x - hx,
                l.position.x + hx,
                l.position.y - hy,
                l.position.y + hy,
            )
        };
        // Through-hole pads are KiCad reference-footprint copper sizes,
        // not lead outlines, and aren't checked here.
        // Both the leads and the footprint lands made from them.
        let smd: Vec<_> = geometry
            .leads
            .iter()
            .filter(|l| l.mounting == openparts_mcad::Mounting::Smd)
            .collect();
        let lands: Vec<_> = geometry
            .lands
            .iter()
            .filter(|l| l.mounting == openparts_mcad::Mounting::Smd)
            .collect();
        for (i, a) in lands.iter().enumerate() {
            for b in &lands[i + 1..] {
                let (ra, rb) = (rect(a), rect(b));
                let overlap = ra.0 < rb.1 - 1e-9
                    && rb.0 < ra.1 - 1e-9
                    && ra.2 < rb.3 - 1e-9
                    && rb.2 < ra.3 - 1e-9;
                if overlap && a.number != b.number {
                    failures.push(format!(
                        "{}: lands {} and {} overlap",
                        package.id, a.number, b.number
                    ));
                }
            }
        }
        for (i, a) in smd.iter().enumerate() {
            for b in &smd[i + 1..] {
                let (ra, rb) = (rect(a), rect(b));
                let overlap = ra.0 < rb.1 - 1e-9
                    && rb.0 < ra.1 - 1e-9
                    && ra.2 < rb.3 - 1e-9
                    && rb.2 < ra.3 - 1e-9;
                assert!(
                    !overlap,
                    "{}: pads {} and {} overlap",
                    package.id, a.number, b.number
                );
            }
        }
        if !matches!(
            geometry.lead_form,
            Some(openparts_mcad::LeadForm::GullWing { .. })
        ) {
            continue;
        }
        // Toe to toe (or, on a TO-252, lead toe to tab end) across each
        // axis equals the stated span; for symmetric packages that puts
        // each toe at half of it.
        let dims = &package.dimensions;
        let extent = |lo: fn(&openparts_mcad::Lead) -> f64,
                      hi: fn(&openparts_mcad::Lead) -> f64| {
            geometry.leads.iter().map(hi).fold(f64::MIN, f64::max)
                - geometry.leads.iter().map(lo).fold(f64::MAX, f64::min)
        };
        // The two-terminal `sod` frame runs its lead span (overall_length)
        // along X, like `chip`; every other gull-wing family along Y.
        let terminal_axis_x = package.family == openparts_core::PackageFamily::Sod;
        let (x_span, y_span) = if terminal_axis_x {
            (&dims.overall_length, &dims.overall_width)
        } else {
            (&dims.overall_width, &dims.overall_length)
        };
        for (span, outer) in [
            (
                x_span,
                extent(
                    |l| l.position.x - l.size.x / 2.0,
                    |l| l.position.x + l.size.x / 2.0,
                ),
            ),
            (
                y_span,
                extent(
                    |l| l.position.y - l.size.y / 2.0,
                    |l| l.position.y + l.size.y / 2.0,
                ),
            ),
        ] {
            let Some(span) = span.as_ref() else { continue };
            let expected = span
                .nominal
                .or(span.min.zip(span.max).map(|(a, b)| (a + b) / 2.0));
            if let Some(expected) = expected {
                assert!(
                    (outer - expected).abs() < 1e-9,
                    "{}: toe-to-toe {outer}, lead span {expected}",
                    package.id
                );
            }
        }
    }
    assert!(
        failures.is_empty(),
        "packages that fail to generate:\n{}",
        failures.join("\n")
    );
}

/// Every package's footprint: the courtyard encloses every pad and the
/// silkscreen, and no silkscreen line runs over copper.
#[test]
fn dataset_footprints_have_a_courtyard_around_pads_and_silk_clear_of_copper() {
    use openparts_pcbcad::{FpLayer, FpShape};
    let dataset = openparts_data::Dataset::load(data_dir()).unwrap();
    let mut failures = Vec::new();
    for package in dataset.packages.values() {
        let Ok(geometry) = openparts_mcad::generate(package) else {
            continue;
        };
        let fp = openparts_pcbcad::build_footprint("X", &geometry);
        let c = fp.courtyard.as_ref().expect("a courtyard");
        let (cx0, cx1) = c
            .outline
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
        let (cy0, cy1) = c
            .outline
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        let inside = |x: f64, y: f64| {
            x >= cx0 - 1e-6 && x <= cx1 + 1e-6 && y >= cy0 - 1e-6 && y <= cy1 + 1e-6
        };
        for pad in &fp.pads {
            let (hx, hy) = (pad.size.0 / 2.0, pad.size.1 / 2.0);
            if !inside(pad.position.x - hx, pad.position.y - hy)
                || !inside(pad.position.x + hx, pad.position.y + hy)
            {
                failures.push(format!(
                    "{}: pad {} outside the courtyard",
                    package.id, pad.number
                ));
            }
        }
        for g in fp.graphics.iter().filter(|g| g.layer == FpLayer::Silk) {
            if let FpShape::Line(a, b) = &g.shape {
                if !inside(a.x, a.y) || !inside(b.x, b.y) {
                    failures.push(format!("{}: silkscreen outside the courtyard", package.id));
                }
                // Sample the line: no point within a pad.
                for i in 0..=20 {
                    let t = i as f64 / 20.0;
                    let (x, y) = (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
                    if let Some(p) = fp.pads.iter().find(|p| {
                        (x - p.position.x).abs() < p.size.0 / 2.0
                            && (y - p.position.y).abs() < p.size.1 / 2.0
                    }) {
                        failures.push(format!("{}: silkscreen over pad {}", package.id, p.number));
                        break;
                    }
                }
            }
        }
    }
    failures.dedup();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
