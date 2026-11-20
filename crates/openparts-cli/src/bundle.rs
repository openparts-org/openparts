//! `openparts bundle`: prebuilds every part's artifacts for distribution,
//! in the layout described in [`openparts_artifacts::bundle`].
//!
//! The bundle is built where SolidCraft is installed (at dataset
//! release) and shipped to `openparts-server`, which then serves it
//! without running SolidCraft itself.

use anyhow::Context;
use openparts_artifacts::bundle::{
    self, Artifact, DatasetRevision, GitRevision, LoadError, Manifest, PartEntry, PartError,
    Reproducibility, Status, Summary,
};
use openparts_artifacts::{ArtifactKind, BuildError, PartBuild};
use openparts_core::Part;
use openparts_data::Dataset;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Writes `bytes` under its digest (once: an existing file with that
/// name already holds these bytes) and describes it.
fn store(
    out: &Path,
    bytes: &[u8],
    kind: ArtifactKind,
    filename: String,
) -> anyhow::Result<Artifact> {
    let sha256 = hex(&Sha256::digest(bytes));
    let path = out.join(bundle::artifact_path(&sha256));
    if !path.is_file() {
        // Written aside and renamed, so a concurrent writer of the same
        // bytes or an interrupted run never leaves a partial file.
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let tmp = path.with_file_name(format!(
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
        content_type: kind.content_type().to_string(),
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
    let put = |entry: &mut PartEntry, build: &PartBuild, kind: ArtifactKind, bytes: &[u8]| {
        let artifact = store(out, bytes, kind, build.file_name(kind))?;
        entry.artifacts.insert(kind.id().to_string(), artifact);
        anyhow::Ok(())
    };
    let result = (|| -> anyhow::Result<Result<(), BuildError>> {
        let build = match PartBuild::from_dataset(dataset, part, None) {
            Ok(b) => b,
            Err(e) => return Ok(Err(e)),
        };
        for kind in [
            ArtifactKind::KicadSymbol,
            ArtifactKind::KicadFootprint,
            ArtifactKind::FootprintSvg,
            ArtifactKind::SymbolSvg,
        ] {
            match build.render(kind) {
                Ok(bytes) => put(&mut entry, &build, kind, &bytes)?,
                Err(e) => return Ok(Err(e)),
            }
        }
        match build.models(&ArtifactKind::MODELS) {
            Ok(models) => {
                for (kind, bytes) in ArtifactKind::MODELS.into_iter().zip(&models) {
                    put(&mut entry, &build, kind, bytes)?;
                }
                Ok(Ok(()))
            }
            Err(e) => Ok(Err(e)),
        }
    })()?;
    entry.status = match &result {
        Ok(()) => Status::Ok,
        Err(_) if entry.artifacts.is_empty() => Status::Failed,
        Err(_) => Status::Partial,
    };
    if let Err(e) = result {
        entry.errors.push(PartError {
            stage: e.stage().id().to_string(),
            message: e.to_string(),
        });
    }
    Ok(entry)
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

pub struct Options {
    pub data: PathBuf,
    pub out: PathBuf,
    pub jobs: usize,
    pub strict: bool,
}

/// Builds every part's entry with `jobs` workers, in dataset order.
fn bundle_all(dataset: &Dataset, out: &Path, jobs: usize) -> anyhow::Result<Vec<PartEntry>> {
    let total = dataset.parts.len();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<PartEntry>>> = Mutex::new(vec![None; total]);
    let first_error: Mutex<Option<anyhow::Error>> = Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= total || first_error.lock().unwrap().is_some() {
                    break;
                }
                let part = &dataset.parts[i];
                let started = Instant::now();
                match bundle_part(dataset, part, out) {
                    Ok(entry) => {
                        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                        let note = match entry.errors.first() {
                            None => "ok".to_string(),
                            Some(e) => format!("{}: {}", e.stage, first_line(&e.message)),
                        };
                        eprintln!(
                            "[{n}/{total}] {}/{} {note} ({:.1}s)",
                            part.manufacturer,
                            part.mpn,
                            started.elapsed().as_secs_f64()
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
    Ok(results
        .into_inner()
        .unwrap()
        .into_iter()
        .flatten()
        .collect())
}

fn reproducibility(dataset: &Dataset, data: &Path, solidcraft: String) -> Reproducibility {
    let mut schema_versions = BTreeSet::new();
    for part in &dataset.parts {
        schema_versions.insert(part.schema_version.clone());
        if let Ok((d, p)) = dataset.resolve(part) {
            schema_versions.insert(d.schema_version.clone());
            schema_versions.insert(p.schema_version.clone());
        }
    }
    let version = env!("CARGO_PKG_VERSION").to_string();
    let generator = [
        "openparts-cli",
        "openparts-core",
        "openparts-validator",
        "openparts-artifacts",
        "openparts-mcad",
        "openparts-solidcraft",
        "openparts-pcbcad",
        "openparts-kicad",
    ]
    .into_iter()
    .map(|c| (c.to_string(), version.clone()))
    .collect();
    Reproducibility {
        dataset: DatasetRevision {
            git: git_revision(data),
        },
        schema_versions,
        generator,
        generator_revision: git_revision(Path::new(env!("CARGO_MANIFEST_DIR"))),
        toolchain: BTreeMap::from([
            ("solidcraft".to_string(), solidcraft),
            ("rustc".to_string(), crate::rustc_version()),
        ]),
        silicon_revision: None,
        not_byte_reproducible: vec![ArtifactKind::SolidCraft.id().to_string()],
    }
}

fn summary(parts: &[PartEntry]) -> Summary {
    let mut digests = BTreeSet::new();
    let mut total_bytes = 0;
    for artifact in parts.iter().flat_map(|p| p.artifacts.values()) {
        if digests.insert(artifact.sha256.clone()) {
            total_bytes += artifact.size;
        }
    }
    let count = |s: Status| parts.iter().filter(|p| p.status == s).count();
    Summary {
        parts: parts.len(),
        ok: count(Status::Ok),
        partial: count(Status::Partial),
        failed: count(Status::Failed),
        artifacts: digests.len(),
        total_bytes,
    }
}

pub fn run(opts: Options) -> anyhow::Result<()> {
    let started = Instant::now();
    let solidcraft = openparts_solidcraft::version()
        .context("openparts bundle builds every 3D model, so it needs SolidCraft installed")?;
    let dataset = Dataset::load(&opts.data)?;
    std::fs::create_dir_all(opts.out.join("artifacts").join("sha256"))
        .with_context(|| format!("creating {}", opts.out.display()))?;

    let total = dataset.parts.len();
    let jobs = opts.jobs.max(1).min(total.max(1));
    eprintln!(
        "bundling {total} part(s) from {} with {jobs} job(s) ({solidcraft})",
        opts.data.display()
    );
    let parts = bundle_all(&dataset, &opts.out, jobs)?;
    let manifest = Manifest {
        format: bundle::FORMAT.to_string(),
        format_version: bundle::FORMAT_VERSION,
        artifact_path: bundle::ARTIFACT_PATH.to_string(),
        reproducibility: reproducibility(&dataset, &opts.data, solidcraft),
        summary: summary(&parts),
        parts,
        load_errors: dataset
            .issues
            .iter()
            .map(|i| LoadError {
                path: i.path.display().to_string(),
                message: i.error.to_string(),
            })
            .collect(),
    };
    let manifest_path = opts.out.join(bundle::MANIFEST_FILE);
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
        let a = store(&out, b"hello", ArtifactKind::Stl, "a.txt".into()).unwrap();
        let b = store(&out, b"hello", ArtifactKind::Stl, "b.txt".into()).unwrap();
        assert_eq!(
            a.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(a.sha256, b.sha256);
        assert_eq!(a.size, 5);
        assert_eq!(
            std::fs::read(out.join(bundle::artifact_path(&a.sha256))).unwrap(),
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
    fn a_part_that_does_not_resolve_is_recorded_not_dropped() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../openparts-data");
        if !data.is_dir() {
            return;
        }
        let dataset = Dataset::load(&data).unwrap();
        let mut orphan = dataset.parts[0].clone();
        orphan.device = openparts_core::DeviceId::from("nobody/NOTHING");
        let out =
            std::env::temp_dir().join(format!("openparts-bundle-orphan-{}", std::process::id()));
        let entry = bundle_part(&dataset, &orphan, &out).unwrap();
        assert_eq!(entry.status, Status::Failed);
        assert_eq!(entry.errors[0].stage, "resolve");
    }
}
