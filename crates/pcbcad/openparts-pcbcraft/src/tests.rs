use super::*;
use crate::ledger::Ledger;
use openparts_core::parameter::{ParameterValue as Pv, ParameterValues};
use openparts_mcad::Mounting;
use openparts_pcbcad::{
    Courtyard, FpGraphic, FpLayer, FpShape, Graphic, GraphicKind, Pad, PadShape, PadSide,
    PinElectricalType, PinOrientation, Point2, SymbolPin, SymbolUnit,
};

fn p(x: f64, y: f64) -> Point2 {
    Point2 { x, y }
}

fn pin(number: &str, name: &str, x: f64, y: f64, o: PinOrientation, hidden: bool) -> SymbolPin {
    SymbolPin {
        number: number.into(),
        name: name.into(),
        electrical_type: if hidden {
            PinElectricalType::Passive
        } else {
            PinElectricalType::PowerIn
        },
        position: p(x, y),
        length: 2.54,
        orientation: o,
        unit: 1,
        hidden,
    }
}

fn two_pin_symbol() -> PcbSymbol {
    let mut a = pin("1", "~", -5.08, 0.0, PinOrientation::Left, false);
    let mut b = pin("2", "~", 5.08, 0.0, PinOrientation::Right, false);
    a.electrical_type = PinElectricalType::Passive;
    b.electrical_type = PinElectricalType::Passive;
    PcbSymbol {
        name: "R".into(),
        reference: "R".into(),
        value: "R".into(),
        footprint: String::new(),
        datasheet: String::new(),
        units: vec![SymbolUnit { index: 1 }],
        pins: vec![a, b],
        graphics: vec![Graphic {
            kind: GraphicKind::Rectangle {
                start: p(-2.54, -1.27),
                end: p(2.54, 1.27),
            },
        }],
    }
}

fn smd(number: &str, x: f64, y: f64) -> Pad {
    Pad {
        number: number.into(),
        position: p(x, y),
        size: (0.8, 0.95),
        shape: PadShape::Rect,
        mounting: Mounting::Smd,
        drill: None,
        slot: None,
        side: PadSide::Top,
    }
}

fn chip_footprint() -> PcbFootprint {
    PcbFootprint {
        name: "0603".into(),
        pads: vec![smd("1", -0.8, 0.0), smd("2", 0.8, 0.0)],
        graphics: vec![FpGraphic {
            layer: FpLayer::Fab,
            width: 0.1,
            shape: FpShape::Polygon(vec![p(-0.8, -0.4), p(0.8, -0.4), p(0.8, 0.4), p(-0.8, 0.4)]),
        }],
        courtyard: Some(Courtyard {
            outline: vec![p(-1.5, -0.75), p(1.5, -0.75), p(1.5, 0.75), p(-1.5, 0.75)],
        }),
        model: None,
        placement_hash: None,
    }
}

fn value(typ: Option<f64>, min: Option<f64>, max: Option<f64>, unit: &str) -> ParameterValues {
    ParameterValues::one(Pv {
        typ,
        min,
        max,
        unit: Some(unit.into()),
        ..Pv::default()
    })
}

fn resistor_input() -> Input {
    let mut parameters = Parameters::new();
    parameters.insert("resistance".into(), value(Some(10.0), None, None, "kΩ"));
    parameters.insert("tolerance".into(), value(None, Some(-1.0), Some(1.0), "%"));
    parameters.insert("power_rating".into(), value(Some(0.1), None, None, "W"));
    Input {
        devices: vec![DeviceInput {
            id: "yageo/RC0603".into(),
            name: "yageo/RC0603".into(),
            category: ComponentCategory::Resistor,
            symbol: two_pin_symbol(),
            locator: "https://example.org/v1/devices/yageo/RC0603".into(),
        }],
        packages: vec![PackageInput {
            id: "standards/0603".into(),
            name: "standards/0603".into(),
            footprint: Ok(chip_footprint()),
            locator: "https://example.org/v1/packages/standards/0603".into(),
        }],
        parts: vec![PartInput {
            id: "yageo/RC0603FR-0710KL".into(),
            mpn: "RC0603FR-0710KL".into(),
            manufacturer: "Yageo".into(),
            device: "yageo/RC0603".into(),
            package: "standards/0603".into(),
            footprint: Ok(chip_footprint()),
            datasheet: Some("https://example.org/rc.pdf".into()),
            locator: "https://example.org/v1/parts/yageo/RC0603FR-0710KL".into(),
            parameters,
        }],
    }
}

fn options(reference: &str) -> Options {
    Options {
        library_id: "openparts".into(),
        library_name: "OpenParts".into(),
        license: "CC0-1.0".into(),
        source_reference: reference.into(),
    }
}

#[test]
fn a_resistor_matches_its_golden_bundle() {
    let mut ledger = LedgerLibrary::default();
    let lib = export(
        &resistor_input(),
        &options("openparts-data abc; test"),
        &mut ledger,
    )
    .unwrap();
    assert!(lib.refusals.is_empty(), "{:?}", lib.refusals);
    // The golden library folder, file for file.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/resistor");
    if std::env::var_os("OPENPARTS_BLESS").is_some() {
        let _ = std::fs::remove_dir_all(&root);
        for (path, text) in &lib.documents {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
    }
    for (path, text) in &lib.documents {
        let golden = std::fs::read_to_string(root.join(path))
            .unwrap_or_else(|_| panic!("{path}: no golden file (OPENPARTS_BLESS=1 writes them)"));
        assert_eq!(
            text, &golden,
            "{path} differs (OPENPARTS_BLESS=1 updates it)"
        );
    }
    // The bundle carries exactly those documents.
    let bundle: serde_json::Value = serde_json::from_str(&lib.bundle()).unwrap();
    assert_eq!(bundle["format"], "pcbcraft-library-bundle/1");
    assert_eq!(
        bundle["documents"].as_object().unwrap().len(),
        lib.documents.len()
    );
    lib.check_limits().unwrap();
}

#[test]
fn the_ledger_keeps_revisions_and_bytes_across_runs() {
    let mut ledger = Ledger::default();
    let first = export(
        &resistor_input(),
        &options("run 1"),
        ledger.library_mut("openparts"),
    )
    .unwrap();
    // Round-trip the ledger through its file form.
    let mut ledger = Ledger::from_json(&ledger.to_json()).unwrap();
    let second = export(
        &resistor_input(),
        &options("run 2"),
        ledger.library_mut("openparts"),
    )
    .unwrap();
    assert_eq!(first.documents, second.documents);

    // A changed land: new footprint, generic-part and orderable-part
    // revisions; the symbol keeps its own.
    let mut changed = resistor_input();
    let mut fp = chip_footprint();
    fp.pads[0].size = (0.9, 0.95);
    changed.packages[0].footprint = Ok(fp.clone());
    changed.parts[0].footprint = Ok(fp);
    let third = export(&changed, &options("run 3"), ledger.library_mut("openparts")).unwrap();
    let doc = |lib: &Library, path: &str| -> serde_json::Value {
        serde_json::from_str(&lib.documents[path]).unwrap()
    };
    let fp3 = doc(&third, "footprints/standards-0603.json");
    assert_eq!(fp3["revision"], 2);
    assert_eq!(fp3["source"]["reference"], "run 3");
    assert_eq!(doc(&third, "parts/yageo-rc0603.json")["revision"], 2);
    assert_eq!(
        doc(&third, "parts/yageo-rc0603.json")["packages"]["standards-0603"]["footprint"]
            ["revision"],
        2
    );
    assert_eq!(
        doc(&third, "manufacturer-parts/yageo-rc0603fr-0710kl.json")["revision"],
        2
    );
    assert_eq!(doc(&third, "symbols/yageo-rc0603.json")["revision"], 1);
    // Back to the first content: its first revisions and bytes again.
    let fourth = export(
        &resistor_input(),
        &options("run 4"),
        ledger.library_mut("openparts"),
    )
    .unwrap();
    assert_eq!(first.documents, fourth.documents);
}

#[test]
fn specifications_are_canonical_base_unit_decimals() {
    let mut ledger = LedgerLibrary::default();
    let lib = export(&resistor_input(), &options("r"), &mut ledger).unwrap();
    let m: serde_json::Value =
        serde_json::from_str(&lib.documents["manufacturer-parts/yageo-rc0603fr-0710kl.json"])
            .unwrap();
    let s = &m["specifications"];
    assert_eq!(s["pcbcraft:resistance"]["value"], "10000");
    assert_eq!(s["pcbcraft:tolerance"]["value"], "0.01");
    assert_eq!(s["openparts:power-rating"]["value"], "0.1");
    assert_eq!(s["openparts:power-rating"]["unit"], "W");
    let part: serde_json::Value =
        serde_json::from_str(&lib.documents["parts/yageo-rc0603.json"]).unwrap();
    assert_eq!(part["category"]["primary_value"], "pcbcraft:resistance");
}

fn refused(f: impl FnOnce(&mut PcbFootprint)) -> String {
    let mut input = resistor_input();
    let mut fp = chip_footprint();
    f(&mut fp);
    input.parts[0].footprint = Ok(fp.clone());
    input.packages[0].footprint = Ok(fp);
    let lib = export(&input, &options("r"), &mut LedgerLibrary::default()).unwrap();
    assert_eq!(lib.parts, 0);
    assert!(!lib.documents.keys().any(|k| k.starts_with("parts/")));
    lib.refusals[0].reason.clone()
}

#[test]
fn unrepresentable_footprints_are_refused_with_a_reason() {
    let r = refused(|fp| {
        fp.pads[0].shape = PadShape::Chamfered {
            size: 0.1,
            corners: vec![],
        }
    });
    assert!(r.contains("chamfered"), "{r}");
    let r = refused(|fp| fp.pads[0].shape = PadShape::Polygon(vec![]));
    assert!(r.contains("polygon"), "{r}");
    let r = refused(|fp| fp.pads[0].side = PadSide::Bottom);
    assert!(r.contains("bottom-side"), "{r}");
    let r = refused(|fp| {
        fp.pads[0].mounting = Mounting::ThroughHole;
        fp.pads[0].drill = Some(0.5);
        fp.pads[0].slot = Some(1.0);
    });
    assert!(r.contains("slotted"), "{r}");
    let r = refused(|fp| {
        fp.pads[0].mounting = Mounting::ThroughHole;
        fp.pads[0].drill = Some(0.9);
    });
    assert!(r.contains("not smaller"), "{r}");
    let r = refused(|fp| fp.pads[1].number = "3".into());
    assert!(r.contains("pin 2 has no pad"), "{r}");
    let r = refused(|fp| fp.pads[1].number = "TOOLONGPAD".into());
    assert!(r.contains("pad number"), "{r}");
}

#[test]
fn stacked_pins_and_repeated_pads_map_to_one_terminal() {
    let mut input = resistor_input();
    // Pin 3 stacked under pin 2 (same name, same place).
    let mut s = two_pin_symbol();
    s.pins[1].name = "GND".into();
    s.pins.push(SymbolPin {
        number: "3".into(),
        ..pin("3", "GND", 5.08, 0.0, PinOrientation::Right, true)
    });
    input.devices[0].symbol = s;
    let mut fp = chip_footprint();
    fp.pads.push(smd("3", 0.0, 1.0));
    fp.pads.push(smd("1", 0.0, -1.0)); // pin 1's second land
    let mut nc = smd("MH", 0.0, 2.0);
    nc.mounting = Mounting::NonPlated;
    nc.drill = Some(0.5);
    fp.pads.push(nc);
    input.parts[0].footprint = Ok(fp.clone());
    input.packages[0].footprint = Ok(fp);
    let lib = export(&input, &options("r"), &mut LedgerLibrary::default()).unwrap();
    assert!(lib.refusals.is_empty(), "{:?}", lib.refusals);
    let part: serde_json::Value =
        serde_json::from_str(&lib.documents["parts/yageo-rc0603.json"]).unwrap();
    let map = &part["packages"]["standards-0603"]["mapping"];
    assert_eq!(map["1"], serde_json::json!(["1", "1_2"]));
    assert_eq!(map["2"], serde_json::json!(["2", "3"]));
    let sym: serde_json::Value =
        serde_json::from_str(&lib.documents["symbols/yageo-rc0603.json"]).unwrap();
    assert_eq!(sym["terminals"].as_array().unwrap().len(), 2);
    let fp: serde_json::Value =
        serde_json::from_str(&lib.documents["footprints/standards-0603.json"]).unwrap();
    assert_eq!(fp["format"], "pcbcraft-footprint/4");
    assert_eq!(fp["non_plated_holes"][0]["diameter_nm"], 500000);
    // Y points down in PCBCraft.
    assert_eq!(fp["pads"][3]["center"]["y_nm"], 1000000);
}

#[test]
fn a_large_symbol_is_split_into_units() {
    let mut input = resistor_input();
    let mut s = two_pin_symbol();
    s.pins = (0..40)
        .map(|i| {
            let mut q = pin(
                &format!("{}", i + 1),
                &format!("P{i}"),
                -12.7,
                50.0 - 2.54 * i as f64,
                PinOrientation::Left,
                false,
            );
            q.electrical_type = PinElectricalType::Bidirectional;
            q
        })
        .collect();
    input.devices[0].symbol = s;
    let mut fp = chip_footprint();
    fp.pads = (0..40)
        .map(|i| smd(&format!("{}", i + 1), i as f64, 0.0))
        .collect();
    input.parts[0].footprint = Ok(fp.clone());
    input.packages[0].footprint = Ok(fp);
    let lib = export(&input, &options("r"), &mut LedgerLibrary::default()).unwrap();
    assert!(lib.refusals.is_empty(), "{:?}", lib.refusals);
    let sym: serde_json::Value =
        serde_json::from_str(&lib.documents["symbols/yageo-rc0603.json"]).unwrap();
    assert_eq!(sym["format"], "pcbcraft-symbol/4");
    let units = sym["units"].as_array().unwrap();
    // One family of 40 (`P0`..`P39`): units of 16 (left, then right).
    assert_eq!(units.len(), 3);
    // Pin 1 (the topmost) first, on the 1.27 mm grid within 12.7 mm.
    assert_eq!(units[0]["pins"][0]["terminal"], "1");
    for u in units {
        for pin in u["pins"].as_array().unwrap() {
            for v in pin["at_nm"].as_array().unwrap() {
                let v = v.as_i64().unwrap();
                assert!(v % 1_270_000 == 0 && v.abs() <= 12_700_000);
            }
        }
    }
}

#[test]
fn ids_are_collision_safe_and_stable() {
    let ledger = LedgerLibrary::default();
    let ids = assign_ids(
        &ledger,
        "part",
        &[
            ("a".into(), "TI/LM358".into()),
            ("b".into(), "ti/lm358".into()),
            ("c".into(), "st/LM358".into()),
        ],
    );
    assert_ne!(ids["a"], ids["b"]);
    assert!(ids["a"].starts_with("ti-lm358-"));
    assert_eq!(ids["c"], "st-lm358");
    for id in ids.values() {
        check_id(id).unwrap();
    }
    let long = "x".repeat(100);
    let ids = assign_ids(&ledger, "part", &[("k".into(), long)]);
    check_id(&ids["k"]).unwrap();
}

#[test]
fn options_are_checked() {
    let mut o = options("r");
    o.license = " ".into();
    assert!(export(&resistor_input(), &o, &mut LedgerLibrary::default()).is_err());
    let mut o = options("r");
    o.library_id = "Open Parts".into();
    assert!(export(&resistor_input(), &o, &mut LedgerLibrary::default()).is_err());
}

#[test]
fn a_part_s_documents_are_its_dependency_closure() {
    let mut input = resistor_input();
    // A second device whose records the resistor doesn't need.
    let mut other = input.devices[0].clone();
    other.id = "yageo/RC0402".into();
    other.name = other.id.clone();
    input.devices.push(other);
    let mut part = input.parts[0].clone();
    part.id = "yageo/RC0402FR-0710KL".into();
    part.mpn = "RC0402FR-0710KL".into();
    part.device = "yageo/RC0402".into();
    input.parts.push(part);
    let lib = export(&input, &options("r"), &mut LedgerLibrary::default()).unwrap();
    let docs = lib.part_documents("yageo/RC0603FR-0710KL").unwrap();
    let paths: Vec<&str> = docs.keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        [
            "categories/resistor.json",
            "footprints/standards-0603.json",
            "library.json",
            "manufacturer-parts/yageo-rc0603fr-0710kl.json",
            "parts/yageo-rc0603.json",
            "symbols/yageo-rc0603.json",
        ]
    );
    for (path, text) in &docs {
        assert_eq!(&lib.documents[path], text);
    }
    assert!(lib.part_documents("yageo/none").is_none());
}
