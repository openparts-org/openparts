//! Provenance checks shared by every document kind.

use crate::Diagnostic;
use openparts_core::{ProvenanceMap, SourceId};
use std::collections::BTreeSet;

pub(crate) fn check_provenance(
    entity: &str,
    provenance: &ProvenanceMap,
    known_sources: &BTreeSet<SourceId>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (path, entries) in provenance {
        if !path.starts_with('/') {
            out.push(Diagnostic::error(
                "provenance.invalid_path",
                entity,
                Some(path),
                format!(
                    "provenance path \"{path}\" is not JSON-Pointer-shaped (must start with \"/\")"
                ),
            ));
        }
        for entry in entries {
            if !known_sources.contains(&entry.source) {
                out.push(Diagnostic::error(
                    "ref.unknown_source",
                    entity,
                    Some(path),
                    format!(
                        "provenance for \"{path}\" cites source \"{}\" which was not supplied for validation",
                        entry.source
                    ),
                ));
            }
        }
    }
    out
}
