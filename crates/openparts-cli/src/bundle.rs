//! `openparts bundle`: prebuilds every part's artifacts for distribution.
//!
//! Walks an `openparts-data` checkout, validates every Part, builds its
//! Effective Model (base revision) and writes each artifact once, content
//! addressed (Architecture Specification section 24):
//!
//! ```text
//! <out>/
//! ├── manifest.json
//! └── artifacts/sha256/<hex>
//! ```
//!
//! `manifest.json` maps (manufacturer, mpn) to each artifact kind's
//! digest, size and content type, lists every part that failed (and
//! why) instead of dropping it, and records the conditions the bundle was
//! built under (section 23: generator versions, SolidCraft version,
//! dataset revision, schema versions). It has no time stamp, so the same
//! dataset and toolchain give the same bundle, byte for byte.
//!
//! The bundle is built where SolidCraft is installed (at dataset
//! release) and shipped to `openparts-server`, which then serves it
//! without running SolidCraft itself.

use anyhow::Context;
use openparts_core::{Device, Package, Part, SourceId};
use openparts_solidcraft::Format;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Version of the bundle layout and `manifest.json` schema.
pub const BUNDLE_FORMAT_VERSION: u32 = 1;
pub const MANIFEST_FILE: &str = "manifest.json";

/// The 3D exports, all from one SolidCraft run per part: (format,
/// artifact kind, content type).
const MODEL_FORMATS: [(Format, &str, &str); 4] = [
    (Format::Step, "step", "model/step"),
    (Format::Stl, "stl", "model/stl"),
    (Format::ThreeMf, "3mf", "model/3mf"),
    (Format::Design, "solidcraft", "application/octet-stream"),
];
const KICAD_CONTENT_TYPE: &str = "text/plain; charset=utf-8";

#[derive(Serialize)]
pub struct Manifest {
    pub format: &'static str,
    pub format_version: u32,
    /// Where each artifact lives, relative to the bundle root, with
    /// `{sha256}` standing for its hex digest.
    pub artifact_path: &'static str,
    pub reproducibility: Reproducibility,
    pub summary: Summary,
    /// Sorted by (manufacturer, mpn).
    pub parts: Vec<PartEntry>,
    /// Dataset files that could not be loaded at all (a Part among them
    /// has no manufacturer/MPN to be listed under).
    pub load_errors: Vec<LoadError>,
}

#[derive(Serialize)]
pub struct Reproducibility {
    pub dataset: DatasetRevision,
    /// `schema_version`s of the Parts/Devices/Packages bundled.
    pub schema_versions: BTreeSet<String>,
    /// Generator crate versions (one workspace version today).
    pub generator: BTreeMap<&'static str, &'static str>,
    /// The openparts source checkout the generators were built from,
    /// when it is a git repository.
    pub generator_revision: Option<GitRevision>,
    pub toolchain: BTreeMap<&'static str, String>,
    /// The Effective Model's revision: always the base device.
    pub silicon_revision: Option<String>,
    /// Artifact kinds that are not byte-identical across rebuilds of
    /// the same input: SolidCraft's saved design keeps per-feature build
    /// timings in its cache. Every other kind is.
    pub not_byte_reproducible: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct DatasetRevision {
    pub git: Option<GitRevision>,
}

#[derive(Serialize)]
pub struct GitRevision {
    pub commit: String,
    /// Uncommitted changes were present.
    pub dirty: bool,
}

#[derive(Serialize)]
pub struct Summary {
    pub parts: usize,
    pub ok: usize,
    pub partial: usize,
    pub failed: usize,
    pub artifacts: usize,
    pub total_bytes: u64,
}

#[derive(Serialize)]
pub struct PartEntry {
    pub manufacturer: String,
    pub mpn: String,
    pub part_id: String,
    pub device: String,
    pub package: String,
    pub status: Status,
    /// Artifact kind (`step`, `stl`, `3mf`, `solidcraft`,
    /// `kicad-symbol`, `kicad-footprint`) -> artifact.
    pub artifacts: BTreeMap<&'static str, Artifact>,
    pub errors: Vec<PartError>,
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Every artifact was built.
    Ok,
    /// Some were built; `errors` says why the rest were not.
    Partial,
    /// None were built.
    Failed,
}

#[derive(Serialize, Clone)]
pub struct Artifact {
    pub sha256: String,
    pub size: u64,
    pub content_type: &'static str,
    /// A file name to save it under.
    pub filename: String,
}

#[derive(Serialize)]
pub struct PartError {
    /// `resolve`, `validation`, `effective-model`, `geometry` or `3d-model`.
    pub stage: &'static str,
    pub message: String,
}

#[derive(Serialize)]
pub struct LoadError {
    pub path: String,
    pub message: String,
}

struct Dataset {
    parts: Vec<Part>,
    devices: HashMap<String, Device>,
    packages: HashMap<String, Package>,
    sources: BTreeSet<SourceId>,
    load_errors: Vec<LoadError>,
}

fn yaml_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            yaml_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "yaml") {
            out.push(path);
        }
    }
    Ok(())
}

fn load_dataset(data: &Path) -> anyhow::Result<Dataset> {
    let mut files = Vec::new();
    yaml_files(data, &mut files).with_context(|| format!("reading {}", data.display()))?;
    let mut dataset = Dataset {
        parts: Vec::new(),
        devices: HashMap::new(),
        packages: HashMap::new(),
        sources: BTreeSet::new(),
        load_errors: Vec::new(),
    };
    for path in files {
        let rel = path
            .strip_prefix(data)
            .unwrap_or(&path)
            .display()
            .to_string();
        let result = (|| -> anyhow::Result<()> {
            let text = std::fs::read_to_string(&path)?;
            let value: serde_yaml::Value = serde_yaml::from_str(&text)?;
            match value.get("kind").and_then(|k| k.as_str()) {
                Some("part") => dataset.parts.push(openparts_data::load_part(&path)?),
                Some("device") => {
                    let d = openparts_data::load_device(&path)?;
                    dataset.devices.insert(d.id.to_string(), d);
                }
                Some("package") => {
                    let p = openparts_data::load_package(&path)?;
                    dataset.packages.insert(p.id.to_string(), p);
                }
                Some("source") => {
                    dataset
                        .sources
                        .insert(openparts_data::load_source(&path)?.id);
                }
                // Manufacturers, errata, rejected entries, scripts' own
                // YAML: nothing to bundle.
                _ => {}
            }
            Ok(())
        })();
        if let Err(e) = result {
            dataset.load_errors.push(LoadError {
                path: rel,
                message: format!("{e:#}"),
            });
        }
    }
    dataset
        .parts
        .sort_by(|a, b| (&a.manufacturer.0, &a.mpn).cmp(&(&b.manufacturer.0, &b.mpn)));
    Ok(dataset)
}

fn git_revision(dir: &Path) -> Option<GitRevision> {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let commit = git(&["rev-parse", "HEAD"])?;
    let dirty = git(&["status", "--porcelain", "--", "."]).is_some_and(|s| !s.is_empty());
    Some(GitRevision { commit, dirty })
}

/// Writes `bytes` under its digest (once: an existing file with that
/// name already holds these bytes) and describes it.
fn store(
    out: &Path,
    bytes: &[u8],
    content_type: &'static str,
    filename: String,
) -> anyhow::Result<Artifact> {
    let sha256 = hex(&Sha256::digest(bytes));
    let dir = out.join("artifacts").join("sha256");
    let path = dir.join(&sha256);
    if !path.is_file() {
        // Written aside and renamed, so a concurrent writer of the same
        // bytes or an interrupted run never leaves a partial file.
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let tmp = dir.join(format!(
            ".tmp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(Artifact {
        sha256,
        size: bytes.len() as u64,
        content_type,
        filename,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Builds and stores one part's artifacts. I/O errors writing the bundle
/// abort the whole run; anything wrong with the part itself is recorded
/// in its entry.
fn bundle_part(dataset: &Dataset, part: &Part, out: &Path) -> anyhow::Result<PartEntry> {
    let mut entry = PartEntry {
        manufacturer: part.manufacturer.0.clone(),
        mpn: part.mpn.clone(),
        part_id: part.id.to_string(),
        device: part.device.to_string(),
        package: part.package.to_string(),
        status: Status::Failed,
        artifacts: BTreeMap::new(),
        errors: Vec::new(),
    };
    let fail = |mut entry: PartEntry, stage, message: String| {
        entry.errors.push(PartError { stage, message });
        entry.status = if entry.artifacts.is_empty() {
            Status::Failed
        } else {
            Status::Partial
        };
        Ok(entry)
    };

    let device = dataset.devices.get(&part.device.to_string());
    let package = dataset.packages.get(&part.package.to_string());
    let (device, package) = match (device, package) {
        (Some(d), Some(p)) => (d, p),
        (d, _) => {
            let missing = if d.is_none() {
                format!("device {} is not in the dataset", part.device)
            } else {
                format!("package {} is not in the dataset", part.package)
            };
            return fail(entry, "resolve", missing);
        }
    };

    let diags = openparts_validator::validate_part(part, device, package, &dataset.sources);
    if !diags.is_empty() {
        let message = diags
            .iter()
            .map(|d| {
                format!(
                    "[{:?}] {} {} {}: {}",
                    d.severity,
                    d.rule_id,
                    d.entity,
                    d.field_path.as_deref().unwrap_or(""),
                    d.message
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return fail(entry, "validation", message);
    }

    let model =
        match openparts_core::build_effective_model(part.clone(), device, package.clone(), None) {
            Ok(m) => m,
            Err(e) => return fail(entry, "effective-model", e.to_string()),
        };
    let file = crate::safe_filename(&part.mpn);

    let symbol = openparts_pcbcad::build_symbol(&part.mpn, &model.device);
    let symbol_text = openparts_kicad::render_symbol(&symbol);
    entry.artifacts.insert(
        "kicad-symbol",
        store(
            out,
            symbol_text.as_bytes(),
            KICAD_CONTENT_TYPE,
            format!("{file}.kicad_sym"),
        )?,
    );

    let geometry = match openparts_mcad::generate(&model.package) {
        Ok(g) => g,
        Err(e) => return fail(entry, "geometry", e.to_string()),
    };
    let footprint = openparts_pcbcad::build_footprint(&part.mpn, &geometry);
    let footprint_text = openparts_kicad::render_footprint(&footprint);
    entry.artifacts.insert(
        "kicad-footprint",
        store(
            out,
            footprint_text.as_bytes(),
            KICAD_CONTENT_TYPE,
            format!("{file}.kicad_mod"),
        )?,
    );

    let input = openparts_solidcraft::ModelInput {
        package: &model.package,
        geometry: &geometry,
        category: Some(model.device.category),
    };
    let formats: Vec<Format> = MODEL_FORMATS.iter().map(|(f, _, _)| *f).collect();
    let models = match openparts_solidcraft::generate(&input, &part.mpn, &formats) {
        Ok(m) => m,
        Err(e) => return fail(entry, "3d-model", e.to_string()),
    };
    for ((format, kind, content_type), bytes) in MODEL_FORMATS.iter().zip(&models) {
        let filename = format!("{file}.{}", format.extension());
        entry
            .artifacts
            .insert(kind, store(out, bytes, content_type, filename)?);
    }
    entry.status = Status::Ok;
    Ok(entry)
}

pub struct Options {
    pub data: PathBuf,
    pub out: PathBuf,
    pub jobs: usize,
    pub strict: bool,
}

pub fn run(opts: Options) -> anyhow::Result<()> {
    let started = Instant::now();
    let solidcraft = openparts_solidcraft::version()
        .context("openparts bundle builds every 3D model, so it needs SolidCraft installed")?;
    let dataset = load_dataset(&opts.data)?;
    std::fs::create_dir_all(opts.out.join("artifacts").join("sha256"))
        .with_context(|| format!("creating {}", opts.out.display()))?;

    let total = dataset.parts.len();
    let jobs = opts.jobs.max(1).min(total.max(1));
    eprintln!(
        "bundling {total} part(s) from {} with {jobs} job(s) ({solidcraft})",
        opts.data.display()
    );

    // Workers take the next part index until none remain.
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<PartEntry>>> = Mutex::new((0..total).map(|_| None).collect());
    let first_error: Mutex<Option<anyhow::Error>> = Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= total || first_error.lock().unwrap().is_some() {
                    break;
                }
                let part = &dataset.parts[i];
                let part_started = Instant::now();
                match bundle_part(&dataset, part, &opts.out) {
                    Ok(entry) => {
                        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                        let note = match entry.status {
                            Status::Ok => "ok".to_string(),
                            _ => format!(
                                "{}: {}",
                                entry.errors[0].stage,
                                first_line(&entry.errors[0].message)
                            ),
                        };
                        eprintln!(
                            "[{n}/{total}] {}/{} {note} ({:.1}s)",
                            part.manufacturer,
                            part.mpn,
                            part_started.elapsed().as_secs_f64()
                        );
                        results.lock().unwrap()[i] = Some(entry);
                    }
                    Err(e) => {
                        first_error.lock().unwrap().get_or_insert(e);
                    }
                }
            });
        }
    });
    if let Some(e) = first_error.into_inner().unwrap() {
        return Err(e.context("writing the bundle"));
    }
    let parts: Vec<PartEntry> = results
        .into_inner()
        .unwrap()
        .into_iter()
        .flatten()
        .collect();

    let mut schema_versions = BTreeSet::new();
    for part in &dataset.parts {
        schema_versions.insert(part.schema_version.clone());
        if let Some(d) = dataset.devices.get(&part.device.to_string()) {
            schema_versions.insert(d.schema_version.clone());
        }
        if let Some(p) = dataset.packages.get(&part.package.to_string()) {
            schema_versions.insert(p.schema_version.clone());
        }
    }
    let version = env!("CARGO_PKG_VERSION");
    let generator = BTreeMap::from([
        ("openparts-cli", version),
        ("openparts-core", version),
        ("openparts-validator", version),
        ("openparts-mcad", version),
        ("openparts-solidcraft", version),
        ("openparts-pcbcad", version),
        ("openparts-kicad", version),
    ]);
    let toolchain = BTreeMap::from([
        ("solidcraft", solidcraft),
        ("rustc", crate::rustc_version()),
    ]);

    let mut digests = BTreeSet::new();
    let mut total_bytes = 0;
    for artifact in parts.iter().flat_map(|p| p.artifacts.values()) {
        if digests.insert(artifact.sha256.clone()) {
            total_bytes += artifact.size;
        }
    }
    let count = |s: Status| parts.iter().filter(|p| p.status == s).count();
    let summary = Summary {
        parts: parts.len(),
        ok: count(Status::Ok),
        partial: count(Status::Partial),
        failed: count(Status::Failed),
        artifacts: digests.len(),
        total_bytes,
    };
    let manifest = Manifest {
        format: "openparts-bundle",
        format_version: BUNDLE_FORMAT_VERSION,
        artifact_path: "artifacts/sha256/{sha256}",
        reproducibility: Reproducibility {
            dataset: DatasetRevision {
                git: git_revision(&opts.data),
            },
            schema_versions,
            generator,
            generator_revision: git_revision(Path::new(env!("CARGO_MANIFEST_DIR"))),
            toolchain,
            silicon_revision: None,
            not_byte_reproducible: vec!["solidcraft"],
        },
        summary,
        parts,
        load_errors: dataset.load_errors,
    };
    let manifest_path = opts.out.join(MANIFEST_FILE);
    let mut json = serde_json::to_string_pretty(&manifest)?;
    json.push('\n');
    std::fs::write(&manifest_path, json)
        .with_context(|| format!("writing {}", manifest_path.display()))?;

    let s = &manifest.summary;
    println!(
        "bundled {} part(s): {} ok, {} partial, {} failed; {} artifact(s), {} bytes, {} load error(s); {:.1}s -> {}",
        s.parts,
        s.ok,
        s.partial,
        s.failed,
        s.artifacts,
        s.total_bytes,
        manifest.load_errors.len(),
        started.elapsed().as_secs_f64(),
        manifest_path.display()
    );
    for part in manifest.parts.iter().filter(|p| p.status != Status::Ok) {
        for e in &part.errors {
            println!(
                "  {}/{} [{}] {}",
                part.manufacturer,
                part.mpn,
                e.stage,
                first_line(&e.message)
            );
        }
    }
    for e in &manifest.load_errors {
        println!("  {} [load] {}", e.path, first_line(&e.message));
    }
    if opts.strict && (s.ok != s.parts || !manifest.load_errors.is_empty()) {
        anyhow::bail!("--strict: not every part was bundled");
    }
    Ok(())
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifacts_are_stored_once_under_their_digest() {
        let out =
            std::env::temp_dir().join(format!("openparts-bundle-test-{}", std::process::id()));
        std::fs::create_dir_all(out.join("artifacts/sha256")).unwrap();
        let a = store(&out, b"hello", "text/plain", "a.txt".into()).unwrap();
        let b = store(&out, b"hello", "text/plain", "b.txt".into()).unwrap();
        assert_eq!(
            a.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(a.sha256, b.sha256);
        assert_eq!(a.size, 5);
        assert_eq!(
            std::fs::read(out.join("artifacts/sha256").join(&a.sha256)).unwrap(),
            b"hello"
        );
        // Only the one file, no leftover temporaries.
        assert_eq!(
            std::fs::read_dir(out.join("artifacts/sha256"))
                .unwrap()
                .count(),
            1
        );
        std::fs::remove_dir_all(&out).unwrap();
    }

    #[test]
    fn bundles_the_real_dataset_and_lists_failures() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../openparts-data");
        if !data.is_dir() {
            return;
        }
        let dataset = load_dataset(&data).unwrap();
        assert!(!dataset.parts.is_empty());
        // A part whose device is missing is recorded, not dropped.
        let mut orphan = dataset.parts[0].clone();
        orphan.device = openparts_core::DeviceId::from("nobody/NOTHING");
        let out =
            std::env::temp_dir().join(format!("openparts-bundle-orphan-{}", std::process::id()));
        let entry = bundle_part(&dataset, &orphan, &out).unwrap();
        assert!(entry.status == Status::Failed);
        assert_eq!(entry.errors[0].stage, "resolve");
    }
}
