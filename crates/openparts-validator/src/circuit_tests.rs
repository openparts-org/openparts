//! Tests of the circuit checks (`circuit.rs`).

use super::circuit::*;
use crate::Diagnostic;
use openparts_core::{Circuit, Device, SourceId};
use std::collections::{BTreeMap, BTreeSet};

fn device() -> Device {
    serde_yaml::from_str(
        "schema_version: '0.2'\nkind: device\nid: t/MCU\nmanufacturer: t\ncategory: microcontroller\npins:\n\
         \x20 '1': {name: VDD, type: power}\n\
         \x20 '2': {name: VDD, type: power}\n\
         \x20 '3': {name: XIN, type: clock}\n\
         \x20 '4': {name: GND, type: ground}\n",
    )
    .unwrap()
}

/// A circuit in memory (as openparts-data decodes a file).
fn circuit(roles: &str, extra: &str) -> Circuit {
    let text = format!(
        "schema_version: '0.2'\nkind: circuit\nid: t/mcu-power\ntitle: T\nhost: t/MCU\n\
         nets:\n  GND: {{kind: ground}}\n  VDD: {{kind: power}}\n\
         connect:\n  VDD: {{net: VDD}}\n  GND: {{net: GND, level: {{value: required, word: shall}}}}\n\
         roles:\n{roles}\n{extra}\
         provenance:\n  /connect: [{{source: t/DS, page: 1}}]\n  /roles: [{{source: t/DS, page: 2}}]\n"
    );
    serde_yaml::from_str(&text).unwrap()
}

fn check(c: &Circuit) -> Vec<Diagnostic> {
    let mut devices = BTreeMap::new();
    devices.insert("t/MCU".to_string(), device());
    let parts: BTreeSet<String> = ["t/XTAL".to_string()].into();
    let sources: BTreeSet<SourceId> = [SourceId::from("t/DS")].into();
    let circuits = BTreeMap::new();
    validate_circuit(
        c,
        &CircuitContext {
            devices: &devices,
            parts: &parts,
            sources: &sources,
            circuits: &circuits,
        },
    )
}

fn errors(d: &[Diagnostic]) -> Vec<&str> {
    d.iter()
        .filter(|d| d.is_error())
        .map(|d| d.rule_id)
        .collect()
}

const DECOUPLING: &str = "  C_VDD:\n    per_pin: VDD\n    requires: {category: capacitor, capacitance: {typ: 100, unit: nF}}\n    \
                          pins: {'1': $pin, '2': GND}\n    level: {value: recommended, word: should}\n";

#[test]
fn a_valid_circuit_passes() {
    let d = check(&circuit(DECOUPLING, ""));
    assert!(errors(&d).is_empty(), "{d:?}");
}

#[test]
fn host_pins_must_exist() {
    let d = check(&circuit(
        &DECOUPLING.replace("per_pin: VDD", "per_pin: VCC"),
        "",
    ));
    assert_eq!(errors(&d), ["circuit.pin"]);
    let d = check(&circuit(
        &DECOUPLING.replace("'2': GND", "'2': host.XOUT"),
        "",
    ));
    assert_eq!(errors(&d), ["circuit.pin"]);
}

#[test]
fn parameter_keys_and_units_are_the_catalogs() {
    let d = check(&circuit(&DECOUPLING.replace("unit: nF", "unit: V"), ""));
    assert_eq!(errors(&d), ["parameter.unit"]);
    let d = check(&circuit(
        &DECOUPLING.replace("capacitance:", "capacity_x:"),
        "",
    ));
    assert_eq!(errors(&d), ["parameter.unknown_key"]);
}

#[test]
fn connections_need_declared_nets_and_per_pin() {
    let d = check(&circuit(&DECOUPLING.replace("'2': GND", "'2': AGND"), ""));
    assert_eq!(errors(&d), ["circuit.connection"]);
    let d = check(&circuit(&DECOUPLING.replace("    per_pin: VDD\n", ""), ""));
    assert_eq!(errors(&d), ["circuit.connection"]);
}

#[test]
fn fixed_parts_must_exist_and_words_fit_levels() {
    let x = "  X1:\n    requires: {part: t/NOPE}\n    pins: {'1': host.XIN}\n    level: {value: required, word: should}\n";
    let d = check(&circuit(x, ""));
    assert_eq!(errors(&d), ["circuit.part", "circuit.level"]);
}

#[test]
fn every_fact_cites_a_source() {
    let mut c = circuit(DECOUPLING, "");
    c.provenance.remove("/roles");
    let d = check(&c);
    assert_eq!(errors(&d), ["provenance.missing", "provenance.missing"]);
    let mut c = circuit(DECOUPLING, "");
    c.provenance.insert(
        "/roles".into(),
        vec![openparts_core::ProvenanceEntry {
            source: "t/OTHER".into(),
            page: None,
            locator: Some("x".repeat(130)),
        }],
    );
    let d = check(&c);
    assert_eq!(errors(&d), ["ref.unknown_source"]);
    assert!(d.iter().any(|d| d.rule_id == "provenance.locator_long"));
}

#[test]
fn formulas_need_their_inputs() {
    for (f, rule) in [
        (
            "formulas:\n  f:\n    expr: C1 * k\n    inputs:\n      C1: {role: C_VDD, param: capacitance}\n",
            "circuit.formula",
        ),
        (
            "formulas:\n  f:\n    expr: C1 * 2\n    inputs:\n      C1: {role: C_VDD, param: resistance}\n",
            "circuit.formula",
        ),
    ] {
        let mut c = circuit(DECOUPLING, f);
        c.provenance
            .insert("/formulas".into(), c.provenance["/roles"].clone());
        assert_eq!(errors(&check(&c)), [rule]);
    }
}

#[test]
fn circuits_live_under_their_publisher() {
    assert!(validate_circuit_location("t/a", "circuits/t/a.yaml").is_empty());
    assert!(!validate_circuit_location("t/a", "circuits/u/a.yaml").is_empty());
}
