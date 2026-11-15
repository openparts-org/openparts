//! `openparts data`: the data files themselves -- `fmt` (the canonical
//! 0.2 layout, `--check` for CI), `migrate --to 0.2` and `equiv` (the
//! migration's equivalence proof).

use anyhow::{bail, Context};
use openparts_data::{equiv, format, migrate};
use std::path::{Path, PathBuf};

/// The `*.yaml` files `paths` names: files as given, directories walked.
fn files(paths: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            openparts_data::yaml_files(p, &mut out)?;
        } else {
            out.push(p.clone());
        }
    }
    Ok(out)
}

fn read(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

/// Re-writes every file in the canonical layout; with `check`, only
/// lists the files that would change (and fails if any would).
pub fn fmt(paths: &[PathBuf], check: bool) -> anyhow::Result<()> {
    let mut changed = Vec::new();
    let mut errors = 0;
    for path in files(paths)? {
        let text = read(&path)?;
        match format::format(&text) {
            Ok(out) if out == text => {}
            Ok(out) => {
                if !check {
                    std::fs::write(&path, out)?;
                }
                changed.push(path);
            }
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                errors += 1;
            }
        }
    }
    for p in &changed {
        println!("{}", p.display());
    }
    if errors > 0 {
        bail!("{errors} file(s) could not be formatted");
    }
    if check && !changed.is_empty() {
        bail!(
            "{} file(s) are not formatted: run `openparts data fmt`",
            changed.len()
        );
    }
    if !check {
        eprintln!("{} file(s) re-formatted", changed.len());
    }
    Ok(())
}

pub fn migrate(paths: &[PathBuf], to: &str) -> anyhow::Result<()> {
    if to != format::SCHEMA_VERSION {
        bail!("only --to {} is supported", format::SCHEMA_VERSION);
    }
    let mut stats = migrate::Stats::default();
    for path in files(paths)? {
        let text = read(&path)?;
        match migrate::migrate_text(&text, &mut stats) {
            Ok(Some(out)) => std::fs::write(&path, out)?,
            Ok(None) => {}
            Err(e) => bail!("{}: {e}", path.display()),
        }
    }
    println!(
        "{} file(s) migrated to {}; {} condition(s) moved into `at` on {} value(s)",
        stats.files,
        format::SCHEMA_VERSION,
        stats.conditions,
        stats.values
    );
    Ok(())
}

pub fn equiv(before: &Path, after: &Path) -> anyhow::Result<()> {
    let report = equiv::compare_trees(before, after).map_err(anyhow::Error::msg)?;
    for d in &report.differences {
        println!("{}: {}", d.file, d.what);
    }
    println!(
        "{} file(s) compared, {} condition(s) justified by their old locators, {} difference(s)",
        report.files,
        report.conditions,
        report.differences.len()
    );
    if !report.differences.is_empty() {
        bail!("the trees differ");
    }
    Ok(())
}
