//! End-to-end test of the First Vertical Slice (Architecture
//! Specification section 32): Canonical YAML -> Parser -> Validator ->
//! Effective Model -> KiCad + STEP, against the `exemplar` fixture data
//! in the sibling `openparts-data` repository.
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

fn load_fixture() -> (
    openparts_core::Part,
    openparts_core::Device,
    openparts_core::Package,
    BTreeSet<openparts_core::SourceId>,
) {
    let data = data_dir();
    let part = openparts_data::load_part(data.join("parts/exemplar/ex48/EX48F100Q6.yaml")).unwrap();
    let device = openparts_data::load_device(data.join("devices/exemplar/EX48F100.yaml")).unwrap();
    let package =
        openparts_data::load_package(data.join("packages/standards/lqfp/LQFP48-7x7-P0.5.yaml"))
            .unwrap();
    let source =
        openparts_data::load_source(data.join("sources/exemplar/EX-DS-0001.yaml")).unwrap();

    let mut known_sources = BTreeSet::new();
    known_sources.insert(source.id);

    (part, device, package, known_sources)
}

#[test]
fn fixture_passes_validation() {
    let (part, device, package, known_sources) = load_fixture();
    let diags = openparts_validator::validate_part(&part, &device, &package, &known_sources);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
}

#[test]
fn full_pipeline_generates_semantically_correct_kicad_and_step() {
    let (part, device, package, known_sources) = load_fixture();
    let diags = openparts_validator::validate_part(&part, &device, &package, &known_sources);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let model = openparts_core::build_effective_model(part.clone(), &device, package.clone(), None)
        .expect("effective model");

    let geometry = openparts_mcad::generate_lqfp(&model.package).expect("geometry");
    assert_eq!(geometry.leads.len(), 48);

    let symbol = openparts_pcbcad::build_symbol(&part.mpn, &model.device);
    let symbol_text = openparts_kicad::render_symbol(&symbol);
    let parsed_pins = openparts_kicad::parse_symbol_pins(&symbol_text);
    assert_eq!(parsed_pins.len(), 48);
    let pin1 = parsed_pins.iter().find(|p| p.number == "1").unwrap();
    assert_eq!(pin1.name, "VBAT");
    assert_eq!(pin1.electrical_type, "power_in");

    let footprint = openparts_pcbcad::build_footprint(&part.mpn, &geometry);
    let footprint_text = openparts_kicad::render_footprint(&footprint);
    let parsed_pads = openparts_kicad::parse_footprint_pads(&footprint_text);
    assert_eq!(parsed_pads.len(), 48);

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
    // 1 body + 48 leads = 49 boxes.
    assert_eq!(step_text.matches("MANIFOLD_SOLID_BREP(").count(), 49);
}

#[test]
fn revision_override_changes_only_the_targeted_pin() {
    let (part, device, package, known_sources) = load_fixture();
    let diags = openparts_validator::validate_part(&part, &device, &package, &known_sources);
    assert!(diags.is_empty());

    let base_model =
        openparts_core::build_effective_model(part.clone(), &device, package.clone(), None)
            .unwrap();
    let rev_b_model =
        openparts_core::build_effective_model(part, &device, package, Some("rev-b")).unwrap();

    assert_eq!(base_model.device.pins["42"].name, "VSS");
    assert_eq!(rev_b_model.device.pins["42"].name, "PDR_ON");

    // Every other pin is identical between the two Effective Models.
    for (number, base_pin) in &base_model.device.pins {
        if number == "42" {
            continue;
        }
        assert_eq!(
            &rev_b_model.device.pins[number], base_pin,
            "pin {number} changed unexpectedly"
        );
    }

    // The rendered symbol reflects the override.
    let symbol = openparts_pcbcad::build_symbol("EX48F100Q6", &rev_b_model.device);
    let text = openparts_kicad::render_symbol(&symbol);
    let pins = openparts_kicad::parse_symbol_pins(&text);
    assert_eq!(
        pins.iter().find(|p| p.number == "42").unwrap().name,
        "PDR_ON"
    );
}

#[test]
fn unresolvable_dependency_stops_generation_instead_of_guessing() {
    // A revision that does not exist must be rejected, not silently
    // ignored (Testing and Quality Specification section 7: "一意に安全
    // な生成結果を決められない操作は停止する").
    let (part, device, package, _) = load_fixture();
    let err =
        openparts_core::build_effective_model(part, &device, package, Some("rev-does-not-exist"))
            .unwrap_err();
    assert!(matches!(
        err,
        openparts_core::EffectiveModelError::UnknownRevision { .. }
    ));
}
