//! The whole `openparts-data` tree in memory: every document by id, the
//! file each came from, and the files that failed to load. The one
//! loader the CLI, the server, the tests and the gallery share.

use crate::{check_kind, DataError};
use openparts_core::{Device, Kind, Manufacturer, Package, Part, Source, SourceId};
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A file that could not be loaded; the rest of the dataset still is.
#[derive(Debug)]
pub struct LoadIssue {
    /// Relative to the dataset root.
    pub path: PathBuf,
    pub error: DataError,
}

/// Why a part's device or package could not be found.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("device {0} not found")]
    Device(String),
    #[error("package {0} not found")]
    Package(String),
}

#[derive(Debug, Default)]
pub struct Dataset {
    pub root: PathBuf,
    pub manufacturers: BTreeMap<String, Manufacturer>,
    /// Sorted by (manufacturer, MPN).
    pub parts: Vec<Part>,
    pub devices: BTreeMap<String, Device>,
    pub packages: BTreeMap<String, Package>,
    pub sources: BTreeMap<String, Source>,
    /// Every loaded document's (kind, id) -> its file (relative to
    /// `root`). Ids are unique per kind only: a Part and its Device may
    /// share one.
    pub paths: BTreeMap<(Kind, String), PathBuf>,
    pub issues: Vec<LoadIssue>,
}

impl Dataset {
    /// Loads every `*.yaml` under `root` by its `kind`. Files of other
    /// kinds (errata, rejected entries) are skipped; files that fail to
    /// parse are recorded in `issues`. Only an unreadable `root` is an
    /// error.
    pub fn load(root: impl AsRef<Path>) -> Result<Dataset, DataError> {
        let root = root.as_ref();
        let mut files = Vec::new();
        yaml_files(root, &mut files).map_err(|source| DataError::Io {
            path: root.display().to_string(),
            source,
        })?;
        let mut ds = Dataset {
            root: root.to_path_buf(),
            ..Dataset::default()
        };
        for path in files {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            if let Err(error) = ds.load_file(&path, &rel) {
                ds.issues.push(LoadIssue { path: rel, error });
            }
        }
        ds.parts
            .sort_by(|a, b| (&a.manufacturer.0, &a.mpn).cmp(&(&b.manufacturer.0, &b.mpn)));
        Ok(ds)
    }

    fn load_file(&mut self, path: &Path, rel: &Path) -> Result<(), DataError> {
        let text = std::fs::read_to_string(path).map_err(|source| DataError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let kind: Option<Kind> = serde_yaml::from_str::<serde_yaml::Value>(&text)
            .map_err(|source| yaml_error(path, source))?
            .get("kind")
            .and_then(|k| serde_yaml::from_value(k.clone()).ok());
        let rel = rel.to_path_buf();
        match kind {
            Some(Kind::Part) => {
                let doc: Part = parse(path, &text, Kind::Part, |d: &Part| d.kind)?;
                self.paths
                    .insert((doc.kind, doc.id.to_string()), rel.clone());
                self.parts.push(doc);
            }
            Some(Kind::Device) => {
                let doc: Device = parse(path, &text, Kind::Device, |d: &Device| d.kind)?;
                self.paths
                    .insert((doc.kind, doc.id.to_string()), rel.clone());
                self.devices.insert(doc.id.to_string(), doc);
            }
            Some(Kind::Package) => {
                let doc: Package = parse(path, &text, Kind::Package, |d: &Package| d.kind)?;
                self.paths
                    .insert((doc.kind, doc.id.to_string()), rel.clone());
                self.packages.insert(doc.id.to_string(), doc);
            }
            Some(Kind::Source) => {
                let doc: Source = parse(path, &text, Kind::Source, |d: &Source| d.kind)?;
                self.paths
                    .insert((doc.kind, doc.id.to_string()), rel.clone());
                self.sources.insert(doc.id.to_string(), doc);
            }
            Some(Kind::Manufacturer) => {
                let doc: Manufacturer =
                    parse(path, &text, Kind::Manufacturer, |d: &Manufacturer| d.kind)?;
                self.paths
                    .insert((doc.kind, doc.id.to_string()), rel.clone());
                self.manufacturers.insert(doc.id.to_string(), doc);
            }
            _ => {}
        }
        Ok(())
    }

    /// The file a document of `kind` with `id` was loaded from.
    pub fn path_of(&self, kind: Kind, id: &str) -> Option<&Path> {
        self.paths
            .get(&(kind, id.to_string()))
            .map(PathBuf::as_path)
    }

    /// The part `manufacturer`/`mpn`.
    pub fn part(&self, manufacturer: &str, mpn: &str) -> Option<&Part> {
        self.parts
            .binary_search_by(|p| {
                (p.manufacturer.0.as_str(), p.mpn.as_str()).cmp(&(manufacturer, mpn))
            })
            .ok()
            .map(|i| &self.parts[i])
    }

    /// The device and package `part` refers to.
    pub fn resolve(&self, part: &Part) -> Result<(&Device, &Package), ResolveError> {
        let device = self
            .devices
            .get(&part.device.0)
            .ok_or_else(|| ResolveError::Device(part.device.0.clone()))?;
        let package = self
            .packages
            .get(&part.package.0)
            .ok_or_else(|| ResolveError::Package(part.package.0.clone()))?;
        Ok((device, package))
    }

    /// Every loaded Source id (what the validator checks citations
    /// against).
    pub fn source_ids(&self) -> BTreeSet<SourceId> {
        self.sources.values().map(|s| s.id.clone()).collect()
    }
}

fn yaml_error(path: &Path, source: serde_yaml::Error) -> DataError {
    DataError::Yaml {
        path: path.display().to_string(),
        source,
    }
}

fn parse<T: DeserializeOwned>(
    path: &Path,
    text: &str,
    expected: Kind,
    kind_of: impl Fn(&T) -> Kind,
) -> Result<T, DataError> {
    let doc: T = crate::parse_document(path, text)?;
    check_kind(path, expected, kind_of(&doc))?;
    Ok(doc)
}

/// Every `*.yaml` under `dir`, recursively, in sorted order. Hidden
/// directories (`.git`, `.github`, editor and agent worktrees under
/// `.claude/`) are not part of the dataset and are skipped.
pub fn yaml_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        let hidden = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        if path.is_dir() && hidden {
            continue;
        }
        if path.is_dir() {
            yaml_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "yaml") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_the_real_dataset_and_resolves_every_part() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../openparts-data");
        if !root.exists() {
            return;
        }
        let ds = Dataset::load(&root).unwrap();
        assert!(ds.issues.is_empty(), "{:?}", ds.issues);
        assert!(!ds.parts.is_empty());
        for part in &ds.parts {
            ds.resolve(part).unwrap();
            assert!(ds.part(&part.manufacturer.0, &part.mpn).is_some());
        }
    }
}
