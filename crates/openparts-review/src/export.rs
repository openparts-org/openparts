//! Pending submissions -> review documents in an openparts-data
//! checkout (`openparts reviews export`). Writes files only; committing
//! them is left to a person.

use crate::{FactIndex, Submission};
use openparts_core::ReviewDocument;
use openparts_data::{review_path, Dataset};
use openparts_validator::review_record_problems;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error(transparent)]
    Data(#[from] openparts_data::DataError),
    #[error("{0}: the dataset doesn't load cleanly; fix it first")]
    Dataset(String),
    #[error("writing {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// What [`export`] did.
#[derive(Debug, Default, PartialEq)]
pub struct ExportReport {
    /// Review documents written, relative to the dataset root.
    pub written: Vec<PathBuf>,
    /// Records added.
    pub added: usize,
    /// Records that replaced the same reviewer's earlier one.
    pub replaced: usize,
    /// Submissions already in the dataset.
    pub unchanged: usize,
    /// Submissions a later one of the same reviewer and fact replaces.
    pub superseded: usize,
    /// Submissions of a value that has changed since (`<id> <target>
    /// <pointer>`): reviewed against old data, so not exported.
    pub stale: Vec<String>,
    /// Submissions of a fact the dataset doesn't have (or malformed).
    pub skipped: Vec<String>,
}

const HEADER: &str = "# Human reviews (Canonical Data Specification section 26.1), written by\n\
# `openparts reviews export`. Each record confirms or disputes one fact\n\
# against the location its provenance cites; `value_hash` ties it to the\n\
# value reviewed (the validator warns `review.stale` once that changes).\n";

/// Adds `submissions` (oldest first) to the review documents of the
/// openparts-data checkout at `data_dir`: one record per reviewer and
/// fact, a later submission replacing the same reviewer's earlier
/// record. Only submissions of the fact as it is now are taken.
pub fn export(data_dir: &Path, submissions: &[Submission]) -> Result<ExportReport, ExportError> {
    let ds = Dataset::load(data_dir)?;
    if let Some(issue) = ds.issues.first() {
        return Err(ExportError::Dataset(format!(
            "{}: {}",
            issue.path.display(),
            issue.error
        )));
    }
    let facts = FactIndex::build(&ds);
    let mut report = ExportReport::default();
    let mut docs: BTreeMap<String, ReviewDocument> = BTreeMap::new();
    let mut touched = BTreeSet::new();
    // Only each reviewer's latest submission per fact matters.
    let mut latest: BTreeMap<(&str, &str, &str, &str), usize> = BTreeMap::new();
    for (i, s) in submissions.iter().enumerate() {
        let key = (
            s.target_kind.as_str(),
            s.target.as_str(),
            s.record.pointer.as_str(),
            s.record.reviewer.as_str(),
        );
        latest.insert(key, i);
    }
    let keep: BTreeSet<usize> = latest.into_values().collect();
    report.superseded = submissions.len() - keep.len();
    for s in keep.into_iter().map(|i| &submissions[i]) {
        let what = format!("{} {} {}", s.id, s.target, s.record.pointer);
        let Some(fact) = facts.fact(s.target_kind, &s.target, &s.record.pointer) else {
            report.skipped.push(what);
            continue;
        };
        if !review_record_problems(&s.record).is_empty() {
            report.skipped.push(what);
            continue;
        }
        if fact.hash != s.record.value_hash {
            report.stale.push(what);
            continue;
        }
        let id = ReviewDocument::id_for(s.target_kind, &s.target);
        let doc = docs.entry(id.clone()).or_insert_with(|| {
            ds.reviews
                .get(&id)
                .cloned()
                .unwrap_or_else(|| ReviewDocument::new(s.target_kind, &s.target))
        });
        let r = &s.record;
        match doc
            .reviews
            .iter_mut()
            .find(|e| e.pointer == r.pointer && e.reviewer == r.reviewer)
        {
            Some(e)
                if e.status == r.status
                    && e.value_hash == r.value_hash
                    && e.comment == r.comment
                    && e.corrected_value == r.corrected_value =>
            {
                report.unchanged += 1;
            }
            Some(e) => {
                *e = r.clone();
                report.replaced += 1;
                touched.insert(id);
            }
            None => {
                doc.reviews.push(r.clone());
                report.added += 1;
                touched.insert(id);
            }
        }
    }
    for id in touched {
        let mut doc = docs.remove(&id).expect("touched docs exist");
        doc.reviews
            .sort_by(|a, b| (&a.pointer, &a.reviewer).cmp(&(&b.pointer, &b.reviewer)));
        let rel = ds
            .path_of(openparts_core::Kind::Review, &id)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(review_path(doc.target_kind, &doc.target)));
        let path = data_dir.join(&rel);
        let write_err = |source| ExportError::Write {
            path: path.display().to_string(),
            source,
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(write_err)?;
        }
        let yaml = serde_yaml::to_string(&doc).map_err(|e| write_err(std::io::Error::other(e)))?;
        std::fs::write(&path, format!("{HEADER}{yaml}")).map_err(write_err)?;
        report.written.push(rel);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{FactReview, ReviewStatus, ReviewTargetKind};

    fn fixture(dir: &Path) {
        let files = [
            (
                "sources/ex/DS1.yaml",
                "schema_version: '0.1'\nkind: source\nid: ex/DS1\npublisher: ex\ndocument_id: DS1\n\
                 document_type: datasheet\nrevision: '1'\ndate: '2026'\nurl: 'https://example.com/ds1.pdf'\n",
            ),
            (
                "packages/standards/T.yaml",
                "schema_version: '0.1'\nkind: package\nid: standards/T\nfamily: soic\nlead_count: 8\n\
                 pitch: {nominal: 1.27, unit: mm}\ndimensions:\n\
                 \x20 body_width: {nominal: 3.9, unit: mm}\n\
                 \x20 body_length: {nominal: 4.9, unit: mm}\n\
                 provenance:\n\
                 \x20 /pitch: [{source: ex/DS1, page: 2, locator: 'Package outline, e'}]\n",
            ),
        ];
        for (rel, text) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }

    fn submission(reviewer: &str, status: ReviewStatus, hash: &str) -> Submission {
        Submission::new(
            ReviewTargetKind::Package,
            "standards/T",
            FactReview {
                pointer: "/pitch".into(),
                status,
                reviewer: reviewer.into(),
                date: "2026-11-15".into(),
                sources: vec!["ex/DS1".into()],
                value_hash: hash.into(),
                comment: (status == ReviewStatus::Disputed).then(|| "Table says 1.25".into()),
                corrected_value: None,
            },
        )
    }

    #[test]
    fn writes_one_record_per_reviewer_and_skips_stale_submissions() {
        let dir =
            std::env::temp_dir().join(format!("openparts-review-export-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        fixture(&dir);
        let ds = Dataset::load(&dir).unwrap();
        let hash = FactIndex::build(&ds)
            .fact(ReviewTargetKind::Package, "standards/T", "/pitch")
            .unwrap()
            .hash
            .clone();
        let old = format!("sha256:{}", "0".repeat(64));
        let subs = [
            submission("alice", ReviewStatus::Disputed, &hash),
            submission("alice", ReviewStatus::Confirmed, &hash),
            submission("bob", ReviewStatus::Confirmed, &old),
        ];
        let report = export(&dir, &subs).unwrap();
        assert_eq!(
            report.written,
            [PathBuf::from("reviews/package/standards/T.yaml")]
        );
        assert_eq!((report.added, report.superseded), (1, 1));
        assert_eq!(report.stale.len(), 1);

        let ds = Dataset::load(&dir).unwrap();
        assert!(ds.issues.is_empty(), "{:?}", ds.issues);
        let doc = ds
            .reviews_of(ReviewTargetKind::Package, "standards/T")
            .unwrap();
        assert_eq!(doc.reviews.len(), 1);
        assert_eq!(doc.reviews[0].status, ReviewStatus::Confirmed);
        let text = std::fs::read_to_string(dir.join(&report.written[0])).unwrap();
        assert!(text.starts_with("# Human reviews"), "{text}");

        // Exporting again changes nothing.
        let again = export(&dir, &subs).unwrap();
        assert!(again.written.is_empty());
        assert_eq!(again.unchanged, 1);
        // A reviewer changing their mind later replaces their record.
        let later = [submission("alice", ReviewStatus::Disputed, &hash)];
        let changed = export(&dir, &later).unwrap();
        assert_eq!((changed.replaced, changed.written.len()), (1, 1));
        std::fs::remove_dir_all(&dir).ok();
    }
}
