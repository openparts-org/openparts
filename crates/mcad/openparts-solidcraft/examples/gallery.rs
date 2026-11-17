//! Development gallery: builds the detailed model of one part per
//! (package, category) pair in an `openparts-data` checkout and writes
//! its script, STEP and an isometric PNG to an output directory.
//!
//! ```sh
//! cargo run -p openparts-solidcraft --example gallery -- ../openparts-data /tmp/gallery [filter]
//! ```

use openparts_core::{ComponentCategory, Dimension, Kind, Package, PackageDimensions, PackageId};
use openparts_mcad::generate as generate_geometry;
use openparts_solidcraft::{build_script, run, Format, ModelInput};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = PathBuf::from(
        args.get(1)
            .map(String::as_str)
            .unwrap_or("../openparts-data"),
    );
    let out = PathBuf::from(
        args.get(2)
            .map(String::as_str)
            .unwrap_or("/tmp/openparts-gallery"),
    );
    let filter = args.get(3).cloned().unwrap_or_default();
    std::fs::create_dir_all(&out).unwrap();

    let dataset = openparts_data::Dataset::load(&data).unwrap();
    for issue in &dataset.issues {
        println!("{}: load: {}", issue.path.display(), issue.error);
    }
    // One part per package id, so every distinct package is shown once.
    let mut picks = BTreeMap::new();
    for part in &dataset.parts {
        picks
            .entry(part.package.to_string())
            .or_insert(part.clone());
    }
    let (packages, devices) = (&dataset.packages, &dataset.devices);

    // (package, category, polarity pin, mpn)
    let mut jobs: Vec<(Package, Option<ComponentCategory>, Option<String>, String)> = picks
        .into_iter()
        .filter_map(|(package_id, part)| {
            let package = packages.get(&package_id)?.clone();
            let device = devices.get(&part.device.to_string());
            let category = device.map(|d| d.category);
            let marked = device
                .and_then(openparts_solidcraft::polarity_pin)
                .map(str::to_string);
            Some((package, category, marked, part.mpn))
        })
        .collect();
    // Families the dataset has no parts for yet.
    jobs.extend(synthetic().into_iter().map(|(p, c, m)| (p, c, None, m)));

    for (package, category, marked, mpn) in &jobs {
        let (package, category) = (package, *category);
        let label = format!("{}_{:?}_{}", package.family, category, mpn).replace('/', "_");
        if !label.to_lowercase().contains(&filter.to_lowercase()) {
            continue;
        }
        let geometry = match generate_geometry(package) {
            Ok(g) => g,
            Err(e) => {
                println!("{label}: geometry: {e}");
                continue;
            }
        };
        let input = ModelInput {
            package,
            geometry: &geometry,
            category,
            marked_pin: marked.as_deref(),
        };
        let script = match build_script(&input) {
            Ok(s) => s,
            Err(e) => {
                println!("{label}: {e}");
                continue;
            }
        };
        std::fs::write(out.join(format!("{label}.json")), script.to_string_pretty()).unwrap();
        let started = std::time::Instant::now();
        match run(&script, mpn, &[Format::Step, Format::Png]) {
            Ok(files) => {
                std::fs::write(out.join(format!("{label}.step")), &files[0]).unwrap();
                std::fs::write(out.join(format!("{label}.png")), &files[1]).unwrap();
                println!("{label}: ok ({:.1}s)", started.elapsed().as_secs_f64());
            }
            Err(e) => println!("{label}: {e}"),
        }
    }
}

fn dim(v: f64) -> Dimension {
    Dimension {
        typ: Some(v),
        min: None,
        max: None,
        unit: "mm".into(),
    }
}

#[allow(clippy::too_many_arguments)]
fn package(
    id: &str,
    family: &str,
    leads: u32,
    pitch: f64,
    w: f64,
    l: f64,
    h: f64,
    layout: Option<Vec<u32>>,
) -> Package {
    Package {
        schema_version: "0.1".into(),
        kind: Kind::Package,
        id: PackageId::from(id),
        family: family.parse().unwrap(),
        lead_count: leads,
        pitch: dim(pitch),
        dimensions: PackageDimensions {
            body_width: dim(w),
            body_length: dim(l),
            body_height: Some(dim(h)),
            ..Default::default()
        },
        lead_layout: layout,
        tab: None,
        aux_pads: vec![],
        pad_order: vec![],
        actuation: None,
        terminal_shape: None,
        connector: None,
        edge: None,
        keepouts: vec![],
        pad_rows: vec![],
        pad_grids: vec![],
        shield: None,
        smt: None,
        pads: vec![],
        construction: None,
        geometry: None,
        provenance: Default::default(),
        review: None,
    }
}

/// A tabbed power package (SOT-223 / TO-252) with JEDEC-typical tabs.
fn tabbed(id: &str, family: &str, pitch: f64, w: f64, l: f64, h: f64, tab_w: f64) -> Package {
    let mut p = package(id, family, 3, pitch, w, l, h, None);
    p.dimensions.tab_width = Some(dim(tab_w));
    p.tab = Some(openparts_core::PackageTab {
        number: if family == "dpak" { "2" } else { "4" }.into(),
        trimmed_leads: if family == "dpak" {
            vec!["2".into()]
        } else {
            vec![]
        },
    });
    p
}

fn synthetic() -> Vec<(Package, Option<ComponentCategory>, String)> {
    vec![
        (
            tabbed("synthetic/SOT223", "sot223", 2.3, 3.5, 6.5, 1.6, 3.0),
            Some(ComponentCategory::Other),
            "SYNTH-SOT223".into(),
        ),
        (
            tabbed("synthetic/DPAK", "dpak", 2.285, 6.1, 6.54, 2.25, 5.21),
            Some(ComponentCategory::Transistor),
            "SYNTH-DPAK".into(),
        ),
        (
            package("synthetic/DIP8", "dip", 8, 2.54, 7.62, 9.27, 3.3, None),
            Some(ComponentCategory::Microcontroller),
            "SYNTH-DIP8".into(),
        ),
        (
            package("synthetic/SIP8", "sip", 8, 2.54, 19.0, 3.0, 6.0, None),
            Some(ComponentCategory::Other),
            "SYNTH-SIP8".into(),
        ),
        (
            package(
                "synthetic/BGA64",
                "bga",
                64,
                0.8,
                7.0,
                7.0,
                1.2,
                Some(vec![8, 8]),
            ),
            Some(ComponentCategory::Microcontroller),
            "SYNTH-BGA64".into(),
        ),
        (
            yaml(SYNTH_CHIP_ARRAY),
            Some(ComponentCategory::Resistor),
            "SYNTH-YC124".into(),
        ),
        (
            {
                let mut p = yaml(SYNTH_CHIP_ARRAY);
                p.terminal_shape = Some(openparts_core::TerminalShape::Concave);
                p
            },
            Some(ComponentCategory::Resistor),
            "SYNTH-YC124-CONCAVE".into(),
        ),
        (
            yaml(SYNTH_PAD_UP),
            Some(ComponentCategory::AudioAmplifier),
            "SYNTH-HTSSOP-DAD".into(),
        ),
        (
            yaml(SYNTH_SO6),
            Some(ComponentCategory::Optocoupler),
            "SYNTH-SO6".into(),
        ),
        (
            yaml(SYNTH_MIC),
            Some(ComponentCategory::Microphone),
            "SYNTH-LGA-MIC".into(),
        ),
    ]
}

fn yaml(text: &str) -> Package {
    serde_yaml::from_str(text).unwrap()
}

/// The schema-gap capabilities (Canonical Data Specification 20.7,
/// 20.15, pad-up exposed pads, empty `pad_order` positions, LGA pads
/// beyond the perimeter).
const SYNTH_CHIP_ARRAY: &str = "schema_version: '0.1'\nkind: package\nid: synthetic/YC124\n\
    family: chip_array\nlead_count: 8\nterminal_shape: convex\npitch: {nominal: 0.5, unit: mm}\n\
    dimensions: {body_width: {nominal: 1.0, unit: mm}, body_length: {nominal: 2.0, unit: mm},\n\
    \x20 body_height: {nominal: 0.35, unit: mm}, lead_width: {nominal: 0.3, unit: mm}, lead_length: {nominal: 0.2, unit: mm}}\n\
    pads:\n\
    - {number: '1', x: -0.5, y: 0.75, width: 0.5, length: 0.3}\n\
    - {number: '2', x: -0.5, y: 0.25, width: 0.5, length: 0.3}\n\
    - {number: '3', x: -0.5, y: -0.25, width: 0.5, length: 0.3}\n\
    - {number: '4', x: -0.5, y: -0.75, width: 0.5, length: 0.3}\n\
    - {number: '5', x: 0.5, y: -0.75, width: 0.5, length: 0.3}\n\
    - {number: '6', x: 0.5, y: -0.25, width: 0.5, length: 0.3}\n\
    - {number: '7', x: 0.5, y: 0.25, width: 0.5, length: 0.3}\n\
    - {number: '8', x: 0.5, y: 0.75, width: 0.5, length: 0.3}\n";

const SYNTH_PAD_UP: &str = "schema_version: '0.1'\nkind: package\nid: synthetic/HTSSOP-DAD\n\
    family: tssop\nlead_count: 32\npitch: {nominal: 0.65, unit: mm}\n\
    dimensions: {body_width: {nominal: 6.1, unit: mm}, body_length: {nominal: 11.0, unit: mm},\n\
    \x20 body_height: {nominal: 1.0, unit: mm}, overall_width: {nominal: 8.1, unit: mm},\n\
    \x20 exposed_pad: {width: {nominal: 3.6, unit: mm}, length: {nominal: 7.5, unit: mm}, face: top}}\n";

const SYNTH_SO6: &str = "schema_version: '0.1'\nkind: package\nid: synthetic/SO6\n\
    family: soic\nlead_count: 6\npitch: {nominal: 1.27, unit: mm}\npad_order: ['1', '', '3', '4', '5', '6']\n\
    dimensions: {body_width: {nominal: 3.7, unit: mm}, body_length: {nominal: 4.4, unit: mm},\n\
    \x20 body_height: {nominal: 2.1, unit: mm}, overall_width: {nominal: 7.0, unit: mm}}\n";

const SYNTH_MIC: &str = "schema_version: '0.1'\nkind: package\nid: synthetic/LGA-MIC\n\
    family: lga\nlead_count: 5\nlead_layout: [2, 0, 2, 0]\npitch: {nominal: 1.0, unit: mm}\n\
    dimensions: {body_width: {nominal: 3.5, unit: mm}, body_length: {nominal: 2.65, unit: mm},\n\
    \x20 body_height: {nominal: 0.98, unit: mm}, lead_width: {nominal: 0.5, unit: mm}, lead_length: {nominal: 0.6, unit: mm}}\n\
    pads:\n\
    - {number: '1', x: -1.3, y: 0.5, width: 0.6, length: 0.5}\n\
    - {number: '2', x: -1.3, y: -0.5, width: 0.6, length: 0.5}\n\
    - {number: '3', x: 1.3, y: -0.5, width: 0.6, length: 0.5}\n\
    - {number: '4', x: 1.3, y: 0.5, width: 0.6, length: 0.5}\n\
    - {number: '5', x: 0, y: 0, width: 1.3, length: 1.3, shape: annular, inner_diameter: 0.7}\n\
    - {number: '', x: 0, y: 0, width: 0.5, length: 0.5, drill: 0.5, plated: false}\n";
