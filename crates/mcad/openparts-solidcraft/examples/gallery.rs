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
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

fn yaml_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            yaml_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "yaml") {
            out.push(p);
        }
    }
}

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

    let mut packages = HashMap::new();
    let mut files = Vec::new();
    yaml_files(&data.join("packages"), &mut files);
    for f in files.drain(..) {
        let p = openparts_data::load_package(&f).unwrap();
        packages.insert(p.id.to_string(), p);
    }
    let mut devices = HashMap::new();
    yaml_files(&data.join("devices"), &mut files);
    for f in files.drain(..) {
        let d = openparts_data::load_device(&f).unwrap();
        devices.insert(d.id.to_string(), d);
    }

    // One part per package id, so every distinct package is shown once.
    let mut picks = BTreeMap::new();
    yaml_files(&data.join("parts"), &mut files);
    for f in files.drain(..) {
        let part = openparts_data::load_part(&f).unwrap();
        picks.entry(part.package.to_string()).or_insert(part);
    }

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
        nominal: Some(v),
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
        family: family.into(),
        lead_count: leads,
        pitch: dim(pitch),
        dimensions: PackageDimensions {
            body_width: dim(w),
            body_length: dim(l),
            body_height: Some(dim(h)),
            exposed_pad: None,
            overall_width: None,
            overall_length: None,
            overall_height: None,
            standoff: None,
            lead_width: None,
            lead_length: None,
            lead_thickness: None,
            tab_width: None,
            tab_length: None,
            row_spacing: None,
            hole_diameter: None,
            aux_pad_width: None,
            aux_pad_length: None,
            aux_row_x: None,
            aux_row_y: None,
            mounting_hole_diameter: None,
            mounting_hole_spacing: None,
            mounting_hole_inset: None,
            mating_length: None,
            tail_length: None,
            pad_diameter: None,
            body_offset: None,
        },
        lead_layout: layout,
        tab: None,
        aux_pads: vec![],
        construction: None,
        geometry: None,
        provenance: Default::default(),
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
    ]
}
