//! `openparts pcbcraft`: an openparts-data checkout (or some of its
//! parts) as PCBCraft native libraries (openparts-pcbcraft).

use anyhow::{bail, Context};
use openparts_artifacts::PartBuild;
use openparts_core::{ComponentCategory, Part};
use openparts_data::Dataset;
use openparts_pcbcad::openparts_placement::{self, Density};
use openparts_pcbcraft::ledger::Ledger;
use openparts_pcbcraft::{DeviceInput, Input, Library, PackageInput, PartInput, Refusal};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum GroupBy {
    /// One library for everything (at most 8,192 files: PCBCraft's
    /// per-source limit).
    None,
    /// One library per top-level category (passive, semiconductor...).
    CategoryGroup,
}

pub struct Options {
    pub data: PathBuf,
    pub out: Option<PathBuf>,
    pub bundle: Option<PathBuf>,
    pub license: String,
    pub ledger: Option<PathBuf>,
    pub library_id: String,
    pub library_name: String,
    pub group_by: GroupBy,
    pub dataset_revision: Option<String>,
    pub locator_base: String,
    pub parts: Vec<String>,
    pub strict: bool,
}

/// Percent-encodes one URL path segment (an MPN may hold `/`).
fn segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn dataset_revision(data: &Path) -> anyhow::Result<String> {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(data)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let rev = git(&["rev-parse", "HEAD"]).with_context(|| {
        format!(
            "{} is not a git checkout: pass --dataset-revision",
            data.display()
        )
    })?;
    let dirty =
        git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    Ok(if dirty { format!("{rev}-dirty") } else { rev })
}

fn slug(s: &str) -> String {
    s.replace('_', "-")
}

pub fn run(o: Options) -> anyhow::Result<()> {
    if o.out.is_none() == o.bundle.is_none() {
        bail!("give exactly one of --out <dir> or --bundle <file>");
    }
    let ds = Dataset::load(&o.data).with_context(|| format!("loading {}", o.data.display()))?;
    let parts: Vec<&Part> = if o.parts.is_empty() {
        ds.parts.iter().collect()
    } else {
        o.parts
            .iter()
            .map(|want| {
                ds.parts
                    .iter()
                    .find(|p| p.id.0 == *want || format!("{}/{}", p.manufacturer.0, p.mpn) == *want)
                    .with_context(|| format!("no part {want} in {}", o.data.display()))
            })
            .collect::<anyhow::Result<_>>()?
    };
    let reference = format!(
        "openparts-data {}; openparts-pcbcraft {}",
        match &o.dataset_revision {
            Some(r) => r.clone(),
            None => dataset_revision(&o.data)?,
        },
        env!("CARGO_PKG_VERSION")
    );
    let base = o.locator_base.trim_end_matches('/');

    // Inputs per library group.
    let mut groups: BTreeMap<Option<ComponentCategory>, (Input, Vec<Refusal>)> = BTreeMap::new();
    for part in parts {
        let device = ds.devices.get(&part.device.0);
        let group = match o.group_by {
            GroupBy::None => None,
            GroupBy::CategoryGroup => device.map(|d| d.category.root()),
        };
        let (input, refused) = groups.entry(group).or_default();
        let build = match PartBuild::from_dataset(&ds, part, None) {
            Ok(b) => b,
            Err(e) => {
                refused.push(Refusal {
                    part: part.id.0.clone(),
                    reason: format!("not buildable: {e}"),
                });
                continue;
            }
        };
        let device = &build.model.device;
        let package = &build.model.package;
        if !input.devices.iter().any(|d| d.id == device.id.0) {
            input.devices.push(DeviceInput {
                id: device.id.0.clone(),
                name: device.id.0.clone(),
                category: device.category,
                symbol: openparts_pcbcad::build_symbol(&device.id.0, device),
                locator: format!("{base}/v1/devices/{}", device.id.0),
            });
        }
        if !input.packages.iter().any(|p| p.id == package.id.0) {
            // The package's own footprint: its Placement Model without
            // any device's pins.
            let footprint = build.geometry().map_err(|e| e.to_string()).map(|g| {
                let model = openparts_placement::build(package, None, g, Density::N);
                openparts_pcbcad::footprint_from_placement(&package.id.0, &model)
            });
            input.packages.push(PackageInput {
                id: package.id.0.clone(),
                name: package.id.0.clone(),
                footprint,
                locator: format!("{base}/v1/packages/{}", package.id.0),
            });
        }
        let effective =
            openparts_core::parameter::effective_parameters(&part.parameters, &device.parameters);
        input.parts.push(PartInput {
            id: part.id.0.clone(),
            mpn: part.mpn.clone(),
            manufacturer: ds
                .manufacturers
                .get(&part.manufacturer.0)
                .map_or_else(|| part.manufacturer.0.clone(), |m| m.name.clone()),
            device: device.id.0.clone(),
            package: package.id.0.clone(),
            footprint: build.footprint().map_err(|e| e.to_string()),
            datasheet: build.datasheet.clone(),
            locator: format!(
                "{base}/v1/parts/{}/{}",
                segment(&part.manufacturer.0),
                segment(&part.mpn)
            ),
            parameters: effective
                .into_iter()
                .map(|(k, (v, _))| (k.to_string(), v.clone()))
                .collect(),
        });
    }

    let mut ledger = match &o.ledger {
        Some(path) if path.exists() => Ledger::from_json(
            &std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?,
        )
        .with_context(|| format!("reading {}", path.display()))?,
        _ => Ledger::default(),
    };

    let mut libraries: Vec<Library> = Vec::new();
    let mut refusals: Vec<Refusal> = Vec::new();
    for (group, (input, refused)) in &groups {
        let (id, name) = match group {
            None => (o.library_id.clone(), o.library_name.clone()),
            Some(c) => (
                format!("{}-{}", o.library_id, slug(c.name())),
                format!("{} {}", o.library_name, slug(c.name()).replace('-', " ")),
            ),
        };
        let options = openparts_pcbcraft::Options {
            library_id: id.clone(),
            library_name: name,
            license: o.license.clone(),
            source_reference: reference.clone(),
        };
        let lib = openparts_pcbcraft::export(input, &options, ledger.library_mut(&id))?;
        lib.check_limits().map_err(|e| {
            anyhow::anyhow!(
                "library {id}: {e}; split it with --group-by category-group or export fewer parts"
            )
        })?;
        refusals.extend(refused.iter().cloned());
        refusals.extend(lib.refusals.iter().cloned());
        libraries.push(lib);
    }

    let grouped = o.group_by != GroupBy::None;
    if let Some(out) = &o.out {
        if out.exists() && std::fs::read_dir(out)?.next().is_some() {
            bail!(
                "{} exists and is not empty: PCBCraft libraries are never overwritten",
                out.display()
            );
        }
        for lib in &libraries {
            let root = if grouped {
                out.join(&lib.id)
            } else {
                out.clone()
            };
            for (path, text) in &lib.documents {
                let file = root.join(path);
                std::fs::create_dir_all(file.parent().expect("relative path"))?;
                std::fs::write(&file, text)
                    .with_context(|| format!("writing {}", file.display()))?;
            }
            println!(
                "{}: {} parts, {} files",
                root.display(),
                lib.parts,
                lib.documents.len()
            );
        }
    }
    if let Some(bundle) = &o.bundle {
        for lib in &libraries {
            let path = if grouped {
                std::fs::create_dir_all(bundle)?;
                bundle.join(format!("{}.pclib.json", lib.id))
            } else {
                bundle.clone()
            };
            let text = lib.bundle();
            if text.len() > openparts_pcbcraft::MAX_BUNDLE_BYTES {
                bail!(
                    "{}: the bundle is {} bytes, over PCBCraft's 16 MiB; use --group-by category-group or a folder (--out)",
                    lib.id,
                    text.len()
                );
            }
            use std::io::Write;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .with_context(|| format!("creating {} (never overwritten)", path.display()))?
                .write_all(text.as_bytes())?;
            println!(
                "{}: {} parts, {} documents",
                path.display(),
                lib.parts,
                lib.documents.len()
            );
        }
    }
    if let Some(path) = &o.ledger {
        std::fs::write(path, ledger.to_json())
            .with_context(|| format!("writing {}", path.display()))?;
    }
    for r in &refusals {
        eprintln!("refused {}: {}", r.part, r.reason);
    }
    eprintln!(
        "{} parts written, {} refused",
        libraries.iter().map(|l| l.parts).sum::<usize>(),
        refusals.len()
    );
    if o.strict && !refusals.is_empty() {
        bail!("{} parts were refused", refusals.len());
    }
    Ok(())
}
