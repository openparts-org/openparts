//! End-to-end test of the First Vertical Slice (Architecture
//! Specification section 32): Canonical YAML -> Parser -> Validator ->
//! Effective Model -> KiCad + STEP, against real fixture data (Raspberry
//! Pi RP2040, and a Yageo chip resistor for the "chip" family) in the
//! sibling `openparts-data` repository.
//!
//! KiCad output is checked by reading it back with the same parsers
//! openparts-kicad's own unit tests use (Testing and Quality
//! Specification section 9, "Independent read-back" / "Semantic
//! checks") -- not just a substring match. STEP output only gets
//! structural checks, since no independent STEP reader exists yet; see
//! the NOTE in openparts-step for why that's recorded honestly rather
//! than claimed as verified.

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

    // The footprint pad and the mechanical geometry lead agree on
    // position for every pin -- the IR conversion doesn't silently drop
    // or reorder anything.
    for lead in &geometry.leads {
        let pad = parsed_pads
            .iter()
            .find(|p| p.number == lead.number)
            .unwrap();
        assert!((pad.x - lead.position.x).abs() < 1e-9);
        assert!((pad.y - lead.position.y).abs() < 1e-9);
    }

    let step_text = openparts_step::generate_step(&geometry, &part.mpn).expect("step");
    assert!(step_text.starts_with("ISO-10303-21;\n"));
    // 1 body + 56 leads = 57 boxes.
    assert_eq!(step_text.matches("MANIFOLD_SOLID_BREP(").count(), 57);

    let stl_text = openparts_stl::generate_stl(&geometry, &part.mpn).expect("stl");
    assert!(stl_text.starts_with("solid RP2040\n"));
    // 57 boxes * 12 facets each.
    assert_eq!(stl_text.matches("facet normal").count(), 57 * 12);
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

    let step_text = openparts_step::generate_step(&geometry, &part.mpn).unwrap();
    // 1 body + 2 terminals = 3 boxes.
    assert_eq!(step_text.matches("MANIFOLD_SOLID_BREP(").count(), 3);
}
