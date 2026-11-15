//! Alternatives and BOM checks against the real openparts-data, and the
//! BOM parsers against fixtures built from the Pico board
//! (examples/pico-board): its KiCad BOM export, netlist, a generic
//! assembly-house CSV and a legacy BOM-script CSV with generic values.

use openparts_bom::alternatives::AlternativeClass;
use openparts_bom::matcher::{FootprintStatus, IssueCode, LineStatus, MatchKind};
use openparts_bom::parse::BomFormat;
use openparts_bom::pins::PadStatus;
use openparts_bom::rules::{CheckStatus, Verdict};
use openparts_bom::{parse_bom, AlternativesIndex, AlternativesResponse, BomIndex};
use openparts_core::{Device, PartId};
use openparts_data::Dataset;
use std::path::PathBuf;
use std::sync::OnceLock;

fn dataset() -> Option<&'static Dataset> {
    static DS: OnceLock<Option<Dataset>> = OnceLock::new();
    DS.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../openparts-data");
        root.exists().then(|| Dataset::load(&root).unwrap())
    })
    .as_ref()
}

/// A dataset of one's own, to change.
fn fresh_dataset() -> Option<Dataset> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../openparts-data");
    root.exists().then(|| Dataset::load(&root).unwrap())
}

fn index() -> Option<(&'static Dataset, &'static BomIndex)> {
    static IX: OnceLock<Option<BomIndex>> = OnceLock::new();
    let ds = dataset()?;
    let ix = IX.get_or_init(|| Some(BomIndex::build(ds))).as_ref()?;
    Some((ds, ix))
}

fn alternatives(ds: &Dataset, ix: &AlternativesIndex, id: &str) -> AlternativesResponse {
    let i = ds
        .parts
        .iter()
        .position(|p| p.id.0 == id)
        .unwrap_or_else(|| panic!("no part {id}"));
    ix.alternatives(ds, i)
}

fn fixture(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let bytes = std::fs::read(&p).unwrap();
    openparts_bom::decode(&bytes).unwrap()
}

/// The Pico board's parts, from examples/pico-board/board.yaml.
fn pico_parts() -> Vec<(String, String)> {
    let board = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/pico-board/board.yaml"),
    )
    .unwrap();
    let mut out = Vec::new();
    let mut inside = false;
    for line in board.lines() {
        if line.starts_with("components:") {
            inside = true;
            continue;
        }
        if inside {
            if !line.starts_with("  ") {
                break;
            }
            let Some((reference, id)) = line.trim().split_once(':') else {
                continue;
            };
            let id = id.split('#').next().unwrap().trim().to_string();
            out.push((reference.trim().to_string(), id));
        }
    }
    assert_eq!(out.len(), 33);
    out
}

// ---------------------------------------------------------- alternatives

#[test]
fn a_0603_100nf_x7r_cap_has_other_0603_100nf_caps_as_drop_ins() {
    let Some((ds, ix)) = index() else { return };
    let r = alternatives(ds, &ix.alternatives, "samsung/CL10B104KB8NNNC");
    let drop_ins: Vec<_> = r
        .alternatives
        .iter()
        .filter(|a| a.class == AlternativeClass::DropIn)
        .collect();
    let mpns: Vec<&str> = drop_ins.iter().map(|a| a.part.mpn.as_str()).collect();
    for want in ["C0603C104K5RACTU", "GRM188R71H104KA93D", "885012206095"] {
        assert!(mpns.contains(&want), "{want} not a drop-in: {mpns:?}");
    }
    for a in &drop_ins {
        assert_eq!(a.part.size_code.as_deref(), Some("0603"), "{}", a.part.mpn);
        assert_eq!(a.part.parameters["capacitance"], "100 nF");
        assert_ne!(a.verdict, Verdict::Worse);
        assert!(a.pins.as_ref().unwrap().compatible);
        let cap = a
            .parameters
            .iter()
            .find(|c| c.key == "capacitance")
            .unwrap();
        assert_eq!(cap.status, CheckStatus::Pass);
    }
    // 0805 100 nF parts are functional equivalents, not drop-ins.
    let functional: Vec<_> = r
        .alternatives
        .iter()
        .filter(|a| a.class == AlternativeClass::Functional)
        .collect();
    assert!(!functional.is_empty());
    assert!(functional
        .iter()
        .all(|a| a.part.size_code.as_deref() != Some("0603")));
    // No other capacitance, and no X5R in place of X7R.
    for a in &r.alternatives {
        assert_eq!(a.part.parameters["capacitance"], "100 nF");
        if a.class == AlternativeClass::DropIn {
            assert_ne!(
                a.part.parameters.get("dielectric").map(String::as_str),
                Some("X5R")
            );
        }
    }
}

#[test]
fn resistor_10k_0603_alternatives() {
    let Some((ds, ix)) = index() else { return };
    let r = alternatives(ds, &ix.alternatives, "yageo/RC0603FR-0710KL");
    let drop_ins: Vec<&str> = r
        .alternatives
        .iter()
        .filter(|a| a.class == AlternativeClass::DropIn)
        .map(|a| a.part.mpn.as_str())
        .collect();
    assert!(
        drop_ins.contains(&"ERJ-3EKF1002V") || drop_ins.contains(&"ERJ3EKF1002V"),
        "{drop_ins:?}"
    );
    assert!(drop_ins.contains(&"RK73H1JTTD1002F"), "{drop_ins:?}");
    for a in &r.alternatives {
        assert_eq!(a.part.parameters["resistance"], "10 kΩ", "{}", a.part.mpn);
        if a.class == AlternativeClass::DropIn {
            assert_eq!(a.part.size_code.as_deref(), Some("0603"));
        }
    }
    // A candidate that doesn't state the tolerance is unverified, not a pass.
    for a in r
        .alternatives
        .iter()
        .filter(|a| a.verdict == Verdict::Unverified)
    {
        assert!(a
            .parameters
            .iter()
            .any(|c| c.status == CheckStatus::Unverified));
    }
    // Ranked: verified drop-ins before unverified ones.
    let first_unverified = r
        .alternatives
        .iter()
        .position(|a| a.verdict == Verdict::Unverified && a.class == AlternativeClass::DropIn);
    let last_meets = r
        .alternatives
        .iter()
        .rposition(|a| a.verdict == Verdict::Meets && a.class == AlternativeClass::DropIn);
    if let (Some(u), Some(m)) = (first_unverified, last_meets) {
        assert!(m < u);
    }
}

#[test]
fn sot223_regulators_with_another_pinout_are_flagged() {
    let Some((ds, ix)) = index() else { return };
    // LM1117: 1 ADJ/GND, 2 VOUT, 3 VIN; UA78M33: 1 INPUT, 2 COMMON, 3 OUTPUT.
    let r = alternatives(ds, &ix.alternatives, "ti/LM1117MPX-3.3/NOPB");
    let ua = r
        .alternatives
        .iter()
        .find(|a| a.part.mpn == "UA78M33CDCYR")
        .expect("UA78M33 on the same SOT-223 land");
    assert_eq!(ua.class, AlternativeClass::FootprintCompatible);
    let pins = ua.pins.as_ref().unwrap();
    assert!(!pins.compatible);
    assert_eq!(pins.differing, vec!["1", "2", "3", "4"]);
    // The NCP1117 and AZ1117 share LM1117's pinout: drop-ins (their
    // parameters unverified where the LM1117 states none).
    for mpn in ["NCP1117ST33T3G", "AZ1117CH-3.3TRG1"] {
        let a = r.alternatives.iter().find(|a| a.part.mpn == mpn).unwrap();
        assert_eq!(a.class, AlternativeClass::DropIn, "{mpn}");
        assert!(a
            .pins
            .as_ref()
            .unwrap()
            .pads
            .iter()
            .all(|p| p.status != PadStatus::Differs));
    }
}

/// `ti/TLV75533PDBVR`'s Device with its pins rearranged.
fn with_pins(ds: &Dataset, device: &str, pins: &[(&str, &str, openparts_core::PinType)]) -> Device {
    let mut d = ds.devices[device].clone();
    for (n, name, t) in pins {
        let p = d.pins.get_mut(*n).unwrap();
        p.name = name.to_string();
        p.pin_type = *t;
    }
    d
}

#[test]
fn sot23_5_ldos_with_different_pinouts_are_not_drop_ins() {
    let Some(mut ds) = fresh_dataset() else {
        return;
    };
    use openparts_core::PinType as T;
    // The same SOT-23-5 land, three pinouts: AP2112K (1 VIN, 2 GND,
    // 3 EN, 4 NC, 5 VOUT); one with VIN and VOUT swapped (as LDOs in the
    // same package do differ between vendors); an adjustable one with
    // a feedback pin where the AP2112K has NC.
    let tlv = ds
        .parts
        .iter()
        .position(|p| p.id.0 == "ti/TLV75533PDBVR")
        .unwrap();
    let mut swapped_part = ds.parts[tlv].clone();
    swapped_part.id = PartId("test/SWAPPED-LDO".into());
    swapped_part.mpn = "SWAPPED-LDO".into();
    let mut swapped = with_pins(
        &ds,
        "ti/TLV755P-DBV",
        &[("1", "VOUT", T::PowerOut), ("5", "VIN", T::Power)],
    );
    swapped.id = "test/SWAPPED".into();
    swapped_part.device = swapped.id.clone();
    let mut adj_part = ds.parts[tlv].clone();
    adj_part.id = PartId("test/ADJ-LDO".into());
    adj_part.mpn = "ADJ-LDO".into();
    let mut adj = with_pins(&ds, "ti/TLV755P-DBV", &[("4", "FB", T::Input)]);
    adj.id = "test/ADJ".into();
    adj_part.device = adj.id.clone();
    ds.devices.insert(swapped.id.0.clone(), swapped);
    ds.devices.insert(adj.id.0.clone(), adj);
    ds.parts.push(swapped_part);
    ds.parts.push(adj_part);
    let ix = AlternativesIndex::build(&ds);
    let r = alternatives(&ds, &ix, "diodes/AP2112K-3.3TRG1");
    let get = |mpn: &str| r.alternatives.iter().find(|a| a.part.mpn == mpn).unwrap();

    let s = get("SWAPPED-LDO");
    assert_eq!(s.class, AlternativeClass::FootprintCompatible);
    assert!(s.footprint.same_land_pattern);
    let pins = s.pins.as_ref().unwrap();
    assert_eq!(pins.differing, vec!["1", "5"]);
    let pad1 = &pins.pads[0];
    assert_eq!(pad1.original.as_ref().unwrap().name, "VIN");
    assert_eq!(pad1.candidate.as_ref().unwrap().name, "VOUT");

    let a = get("ADJ-LDO");
    assert_eq!(a.class, AlternativeClass::FootprintCompatible);
    assert_eq!(a.pins.as_ref().unwrap().differing, vec!["4"]);

    // The real TLV75533 has the AP2112K's pinout (IN/GND/EN/NC/OUT):
    // pin-compatible on the same land, but a lower input rating and
    // output current keep it from being a drop-in.
    let t = get("TLV75533PDBVR");
    assert_eq!(t.class, AlternativeClass::NearMiss);
    let pins = t.pins.as_ref().unwrap();
    assert!(pins.compatible);
    assert!(pins
        .pads
        .iter()
        .any(|p| p.pad == "1" && p.status == PadStatus::Equivalent));
    let vin = t
        .parameters
        .iter()
        .find(|c| c.key == "input_voltage_max")
        .unwrap();
    assert_eq!(vin.status, CheckStatus::Fail);
}

#[test]
fn a_listed_replacement_ranks_first() {
    let Some(mut ds) = fresh_dataset() else {
        return;
    };
    let i = ds
        .parts
        .iter()
        .position(|p| p.id.0 == "samsung/CL10B104KB8NNNC")
        .unwrap();
    ds.parts[i].lifecycle.replacement = vec![PartId("samsung/CL21B104KBCNNNC".into())];
    let ix = AlternativesIndex::build(&ds);
    let r = ix.alternatives(&ds, i);
    let first = &r.alternatives[0];
    assert!(first.listed_replacement);
    assert_eq!(first.part.mpn, "CL21B104KBCNNNC");
    // Still checked: another size, so a functional equivalent.
    assert_eq!(first.class, AlternativeClass::Functional);
    assert!(!first.footprint.same_land_pattern);
}

// ------------------------------------------------------------------ BOM

fn check_pico(name: &str, format: BomFormat) -> openparts_bom::BomReport {
    let (ds, ix) = index().unwrap();
    let bom = parse_bom(&fixture(name)).unwrap();
    assert_eq!(bom.format, format, "{name}");
    ix.check(ds, &bom)
}

/// Every Pico component, matched exactly to its board.yaml part.
fn assert_every_pico_part_matches_exactly(report: &openparts_bom::BomReport) {
    let parts = pico_parts();
    let mut seen = 0;
    for line in &report.lines {
        assert_eq!(
            line.kind,
            MatchKind::Exact,
            "row {}: {:?}",
            line.row,
            line.references
        );
        assert_ne!(line.status, LineStatus::NotFound);
        let best = &line.matches[0].part;
        let id = format!("{}/{}", best.manufacturer, best.mpn);
        for r in &line.references {
            let want = &parts.iter().find(|(x, _)| x == r).unwrap().1;
            // board.yaml names parts by id; the MPN may differ (`/` -> `-`).
            let want_part = ds_part(want);
            assert_eq!(&id, &want_part, "{r}");
            seen += 1;
        }
        assert_eq!(line.matches.len(), 1, "{:?}", line.references);
    }
    assert_eq!(seen, 33);
}

fn ds_part(id: &str) -> String {
    let ds = dataset().unwrap();
    let p = ds.parts.iter().find(|p| p.id.0 == id).unwrap();
    format!("{}/{}", p.manufacturer.0, p.mpn)
}

#[test]
fn pico_kicad_bom_export_matches_exactly() {
    if index().is_none() {
        return;
    }
    let r = check_pico("pico-kicad-bom.csv", BomFormat::KicadCsv);
    assert_eq!(r.summary.lines, 16);
    assert_eq!(r.summary.components, 33);
    assert_eq!(r.summary.exact, 16);
    assert_every_pico_part_matches_exactly(&r);
    // The 0603 caps are active and verified: nothing to review.
    let c1 = r.lines.iter().find(|l| l.references[0] == "C1").unwrap();
    assert_eq!(c1.status, LineStatus::Ok);
    assert_eq!(c1.quantity, 11);
    // Lifecycle unknown: review.
    let u2 = r.lines.iter().find(|l| l.references[0] == "U2").unwrap();
    assert_eq!(u2.status, LineStatus::Review);
    assert!(u2
        .issues
        .iter()
        .any(|i| i.code == IssueCode::LifecycleUnknown));
}

#[test]
fn pico_netlist_matches_exactly() {
    if index().is_none() {
        return;
    }
    let r = check_pico("pico.net", BomFormat::KicadNetlist);
    assert_eq!(r.summary.lines, 16);
    assert_eq!(r.summary.components, 33);
    assert_every_pico_part_matches_exactly(&r);
    let c = r.lines.iter().find(|l| l.references[0] == "C1").unwrap();
    assert_eq!(c.references.len(), 11);
    assert_eq!(c.references[10], "C11");
}

#[test]
fn pico_generic_csv_matches_exactly() {
    if index().is_none() {
        return;
    }
    let r = check_pico("pico-generic.csv", BomFormat::GenericCsv);
    assert_eq!(r.summary.components, 33);
    assert_every_pico_part_matches_exactly(&r);
    // The manufacturer column (display names) agrees with every match.
    assert!(r.lines.iter().all(|l| !l
        .issues
        .iter()
        .any(|i| i.code == IssueCode::ManufacturerMismatch)));
}

#[test]
fn pico_legacy_bom_matches_passives_by_value_and_size() {
    if index().is_none() {
        return;
    }
    let r = check_pico("pico-kicad-legacy.csv", BomFormat::KicadCsv);
    assert_eq!(r.summary.components, 33);
    let line = |r0: &str| r.lines.iter().find(|l| l.references[0] == r0).unwrap();
    // 100nF + C_0603 -> 100 nF 0603 capacitors, the Samsung one among them.
    let c1 = line("C1");
    assert_eq!(c1.kind, MatchKind::ValueFootprint);
    assert!(c1
        .matches
        .iter()
        .all(|m| m.part.parameters["capacitance"] == "100 nF"
            && m.part.size_code.as_deref() == Some("0603")));
    assert!(c1.matches.iter().any(|m| m.part.mpn == "CL10B104KB8NNNC"));
    // 15pF C0G: the dielectric narrows it.
    let c16 = line("C16");
    assert!(c16
        .matches
        .iter()
        .all(|m| m.part.parameters["dielectric"] == "C0G"));
    assert!(c16.matches.iter().any(|m| m.part.mpn == "CL10C150JB8NNNC"));
    for (r0, ohms) in [
        ("R1", "27 Ω"),
        ("R3", "1 kΩ"),
        ("R5", "5.1 kΩ"),
        ("R7", "330 Ω"),
    ] {
        let l = line(r0);
        assert_eq!(l.kind, MatchKind::ValueFootprint, "{r0}");
        assert!(
            l.matches
                .iter()
                .all(|m| m.part.parameters["resistance"] == ohms),
            "{r0}"
        );
    }
    // The ICs still match by MPN, and their KiCad footprints agree.
    for r0 in ["U1", "U2", "U3"] {
        let l = line(r0);
        assert_eq!(l.kind, MatchKind::Exact, "{r0}");
        assert_eq!(
            l.matches[0].footprint.status,
            FootprintStatus::Match,
            "{r0}: {:?}",
            l.matches[0].footprint
        );
    }
    // The LED drawn on an 0805 footprint is a 0603 part.
    let d1 = line("D1");
    assert_eq!(d1.matches[0].footprint.status, FootprintStatus::Mismatch);
    assert_eq!(d1.status, LineStatus::Warning);
    assert!(d1
        .issues
        .iter()
        .any(|i| i.code == IssueCode::FootprintMismatch
            && i.detail.as_deref() == Some("0805 / 0603")));
}

#[test]
fn bom_lines_are_matched_by_normalized_mpn_and_variant() {
    let Some((ds, ix)) = index() else { return };
    let csv = "MPN,Manufacturer,Qty\n\
               rc0603fr-07 10kl,Yageo,4\n\
               AP2112K-3.3,Diodes Incorporated,1\n\
               NOPE-12345-XYZ,,1\n\
               W25Q128JVSIQ,Texas Instruments,1\n";
    let r = ix.check(ds, &parse_bom(csv).unwrap());
    assert_eq!(r.format, BomFormat::GenericCsv);
    assert_eq!(r.lines[0].kind, MatchKind::Normalized);
    assert_eq!(r.lines[0].matches[0].part.mpn, "RC0603FR-0710KL");
    assert_eq!(r.lines[0].quantity, 4);
    assert_eq!(r.lines[1].kind, MatchKind::Variant);
    assert_eq!(r.lines[1].matches[0].part.mpn, "AP2112K-3.3TRG1");
    assert_eq!(r.lines[1].status, LineStatus::Review);
    assert_eq!(r.lines[2].status, LineStatus::NotFound);
    assert!(r.lines[2]
        .issues
        .iter()
        .any(|i| i.code == IssueCode::NotFound));
    // The MPN is Winbond's, the BOM says TI: matched, flagged.
    assert_eq!(r.lines[3].status, LineStatus::Warning);
    assert!(r.lines[3]
        .issues
        .iter()
        .any(|i| i.code == IssueCode::ManufacturerMismatch));
    assert_eq!(r.summary.not_found, 1);
    assert_eq!(r.summary.lines, 4);
}

#[test]
fn eol_and_nrnd_lines_get_alternatives() {
    let Some((ds, ix)) = index() else { return };
    // Every EOL/NRND part of the dataset gets a lifecycle warning, and
    // the alternatives shown for it are never EOL/obsolete themselves.
    let problems: Vec<&openparts_core::Part> = ds
        .parts
        .iter()
        .filter(|p| {
            matches!(
                p.lifecycle.status,
                openparts_core::LifecycleStatus::Nrnd
                    | openparts_core::LifecycleStatus::EolAnnounced
            )
        })
        .collect();
    assert!(!problems.is_empty());
    let mut csv = String::from("Reference,Value,Footprint,MPN\n");
    for (k, p) in problems.iter().enumerate() {
        csv.push_str(&format!("U{},{},,\"{}\"\n", k + 1, p.mpn, p.mpn));
    }
    let r = ix.check(ds, &parse_bom(&csv).unwrap());
    let mut with_alternatives = 0;
    for l in &r.lines {
        assert_eq!(l.status, LineStatus::Warning, "{}", l.mpn);
        assert!(l
            .issues
            .iter()
            .any(|i| matches!(i.code, IssueCode::LifecycleEol | IssueCode::LifecycleNrnd)));
        for a in &l.alternatives {
            assert!(!matches!(
                a.part.lifecycle,
                openparts_core::LifecycleStatus::EolAnnounced
                    | openparts_core::LifecycleStatus::Obsolete
            ));
        }
        if !l.alternatives.is_empty() {
            with_alternatives += 1;
        }
    }
    assert!(with_alternatives > 0);
}
