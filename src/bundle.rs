//! Prebuilt artifacts: a bundle written by `openparts bundle`, in the
//! layout and manifest schema of `openparts_artifacts::bundle`, served
//! instead of building each artifact per request.
//!
//! The bundle is built where SolidCraft is installed (at dataset
//! release); with it, this server needs no SolidCraft at all.

use openparts_artifacts::bundle::{Manifest, FORMAT, FORMAT_VERSION};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

pub use openparts_artifacts::bundle::{Artifact, MANIFEST_FILE};

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not a bundle manifest: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{path}: expected format {FORMAT} version {FORMAT_VERSION}, found {format} version {version}")]
    Format {
        path: String,
        format: String,
        version: u32,
    },
    #[error("{path}: artifact_path {template:?} has no {{sha256}} placeholder")]
    Template { path: String, template: String },
}

#[derive(Debug, Default)]
pub struct Bundle {
    root: PathBuf,
    artifact_path: String,
    /// (manufacturer, mpn) -> kind -> artifact.
    parts: HashMap<(String, String), BTreeMap<String, Artifact>>,
    /// The conditions the bundle was built under, as the manifest
    /// records them.
    pub reproducibility: openparts_artifacts::bundle::Reproducibility,
}

impl Bundle {
    pub fn load(root: &Path) -> Result<Bundle, BundleError> {
        let path = root.join(MANIFEST_FILE);
        let shown = path.display().to_string();
        let text = std::fs::read_to_string(&path).map_err(|source| BundleError::Io {
            path: shown.clone(),
            source,
        })?;
        let manifest: Manifest =
            serde_json::from_str(&text).map_err(|source| BundleError::Json {
                path: shown.clone(),
                source,
            })?;
        if manifest.format != FORMAT || manifest.format_version != FORMAT_VERSION {
            return Err(BundleError::Format {
                path: shown,
                format: manifest.format,
                version: manifest.format_version,
            });
        }
        if !manifest.artifact_path.contains("{sha256}") {
            return Err(BundleError::Template {
                path: shown,
                template: manifest.artifact_path,
            });
        }
        let mut bundle = Bundle {
            root: root.to_path_buf(),
            artifact_path: manifest.artifact_path,
            parts: HashMap::new(),
            reproducibility: manifest.reproducibility,
        };
        let mut missing = 0;
        for part in manifest.parts {
            let artifacts: BTreeMap<String, Artifact> = part
                .artifacts
                .into_iter()
                .filter(|(_, a)| {
                    let present = bundle.path_of(a).is_some_and(|p| p.is_file());
                    missing += usize::from(!present);
                    present
                })
                .collect();
            bundle
                .parts
                .insert((part.manufacturer, part.mpn), artifacts);
        }
        if missing > 0 {
            tracing::warn!(
                missing,
                "bundle manifest lists artifacts whose files are missing; they will be generated live if possible"
            );
        }
        tracing::info!(
            root = %root.display(),
            parts = bundle.parts.len(),
            artifacts = bundle.parts.values().map(BTreeMap::len).sum::<usize>(),
            "loaded artifact bundle"
        );
        Ok(bundle)
    }

    /// The prebuilt `kind` artifact of a part, if the bundle has it.
    pub fn get(&self, manufacturer: &str, mpn: &str, kind: &str) -> Option<&Artifact> {
        self.parts
            .get(&(manufacturer.to_string(), mpn.to_string()))?
            .get(kind)
    }

    /// Where `artifact` lives; `None` for a digest that isn't one (so a
    /// manifest can't point outside the bundle).
    fn path_of(&self, artifact: &Artifact) -> Option<PathBuf> {
        let hex = &artifact.sha256;
        let valid = hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit());
        valid.then(|| self.root.join(self.artifact_path.replace("{sha256}", hex)))
    }

    /// Reads `artifact`'s bytes, checked against its digest: `None` if
    /// the file is missing, unreadable or not what the manifest says.
    pub async fn read(&self, artifact: &Artifact) -> Option<Vec<u8>> {
        let path = self.path_of(artifact)?;
        let bytes = match tokio::fs::read(&path).await {
            Ok(b) => b,
            Err(e) => {
                tracing::error!(path = %path.display(), error = %e, "cannot read bundled artifact");
                return None;
            }
        };
        if crate::routes::sha256_hex(&bytes) != artifact.sha256.to_ascii_lowercase() {
            tracing::error!(path = %path.display(), "bundled artifact does not match its digest");
            return None;
        }
        Some(bytes)
    }
}
