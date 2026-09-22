//! Local Client Cache (Architecture Specification section 25): a normal
//! user only keeps the components they actually use, not the whole
//! dataset. Metadata (Part/Device/Package JSON) is cached by request
//! path; Artifacts are cached content-addressed by their SHA-256 hash
//! (section 24), which is what makes cache entries safely shareable and
//! trivially verifiable on read.

use crate::error::ClientError;
use std::path::{Path, PathBuf};

pub fn default_cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".cache").join("openparts")
}

pub fn write(cache_dir: &Path, rel_path: &str, content: &str) -> Result<(), ClientError> {
    let path = cache_dir.join(rel_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ClientError::CacheIo {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(&path, content).map_err(|source| ClientError::CacheIo {
        path: path.display().to_string(),
        source,
    })
}

pub fn read(cache_dir: &Path, rel_path: &str) -> Option<String> {
    std::fs::read_to_string(cache_dir.join(rel_path)).ok()
}

pub fn metadata_key(kind: &str, id: &str) -> String {
    format!("metadata/{kind}/{id}.json")
}

pub fn artifact_pointer_key(
    manufacturer: &str,
    mpn: &str,
    kind: &str,
    revision: Option<&str>,
) -> String {
    match revision {
        Some(rev) => format!("metadata/artifact-pointers/{manufacturer}/{mpn}/{kind}@{rev}.hash"),
        None => format!("metadata/artifact-pointers/{manufacturer}/{mpn}/{kind}.hash"),
    }
}

pub fn artifact_content_key(hash: &str) -> String {
    format!("artifacts/sha256/{hash}")
}
