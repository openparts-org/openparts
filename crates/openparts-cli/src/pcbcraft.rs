//! `openparts pcbcraft`: an openparts-data checkout (or some of its
//! parts) as PCBCraft native libraries (openparts-pcbcraft).

use anyhow::{bail, Context};
use openparts_artifacts::pcbcraft as shared;
use openparts_core::Part;
use openparts_data::Dataset;
use openparts_pcbcraft::ledger::Ledger;
use openparts_pcbcraft::{Library, Refusal};
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
    let settings = shared::Settings {
        library_id: o.library_id.clone(),
        library_name: o.library_name.clone(),
        license: o.license.clone(),
        source_reference: shared::source_reference(&match &o.dataset_revision {
            Some(r) => r.clone(),
            None => dataset_revision(&o.data)?,
        }),
        locator_base: o.locator_base.trim_end_matches('/').to_string(),
        group_by: match o.group_by {
            GroupBy::None => shared::GroupBy::None,
            GroupBy::CategoryGroup => shared::GroupBy::CategoryGroup,
        },
    };

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
    for exported in shared::export(&ds, &parts, &settings, &mut ledger)? {
        let lib = exported.library;
        lib.check_limits().map_err(|e| {
            anyhow::anyhow!(
                "library {}: {e}; split it with --group-by category-group or export fewer parts",
                lib.id
            )
        })?;
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
