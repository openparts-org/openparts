//! Pending review submissions: one JSON object per line, appended,
//! never rewritten.

use openparts_core::{FactReview, ReviewTargetKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// The log's file name inside the review directory.
pub const SUBMISSIONS_FILE: &str = "submissions.jsonl";

/// One accepted review submission: the record as it would be committed,
/// plus what it is about and when it arrived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    /// Unique per submission (16 hex digits).
    pub id: String,
    /// When the server accepted it (RFC 3339 UTC).
    pub received: String,
    pub target_kind: ReviewTargetKind,
    pub target: String,
    pub record: FactReview,
}

impl Submission {
    /// A new submission with a fresh id, received now.
    pub fn new(target_kind: ReviewTargetKind, target: &str, record: FactReview) -> Submission {
        let received = crate::utc_now();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seed = format!(
            "{nanos}\0{}\0{target}\0{}\0{}",
            std::process::id(),
            record.pointer,
            record.reviewer
        );
        let id = Sha256::digest(seed.as_bytes())[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Submission {
            id,
            received,
            target_kind,
            target: target.to_string(),
            record,
        }
    }
}

/// `<dir>/submissions.jsonl`.
#[derive(Debug, Clone)]
pub struct SubmissionLog {
    path: PathBuf,
}

impl SubmissionLog {
    /// The log in `dir` (created if missing).
    pub fn open(dir: &Path) -> std::io::Result<SubmissionLog> {
        std::fs::create_dir_all(dir)?;
        Ok(SubmissionLog {
            path: dir.join(SUBMISSIONS_FILE),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The log's size in bytes (0 before the first submission).
    pub fn len_bytes(&self) -> u64 {
        std::fs::metadata(&self.path).map_or(0, |m| m.len())
    }

    /// Every submission, oldest first, and the number of lines that
    /// didn't parse (skipped: a torn last line after a crash, say).
    pub fn read_all(&self) -> std::io::Result<(Vec<Submission>, usize)> {
        let file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((vec![], 0)),
            Err(e) => return Err(e),
        };
        let mut out = Vec::new();
        let mut bad = 0;
        for line in std::io::BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Submission>(&line) {
                Ok(s) => out.push(s),
                Err(_) => bad += 1,
            }
        }
        Ok((out, bad))
    }

    /// Appends `s` as one line and flushes it to disk.
    pub fn append(&self, s: &Submission) -> std::io::Result<()> {
        let mut line = serde_json::to_string(s).map_err(std::io::Error::other)?;
        line.push('\n');
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(line.as_bytes())?;
        file.sync_data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::ReviewStatus;

    #[test]
    fn appends_and_reads_back_skipping_torn_lines() {
        let dir = std::env::temp_dir().join(format!("openparts-review-log-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let log = SubmissionLog::open(&dir).unwrap();
        assert_eq!(log.read_all().unwrap(), (vec![], 0));
        let record = FactReview {
            pointer: "/pitch".into(),
            status: ReviewStatus::Confirmed,
            reviewer: "alice".into(),
            date: "2026-11-15".into(),
            sources: vec!["ex/DS1".into()],
            value_hash: format!("sha256:{}", "0".repeat(64)),
            comment: None,
            corrected_value: None,
        };
        let a = Submission::new(ReviewTargetKind::Package, "standards/T", record.clone());
        let b = Submission::new(ReviewTargetKind::Package, "standards/T", record);
        assert_ne!(a.id, b.id);
        log.append(&a).unwrap();
        log.append(&b).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(log.path())
            .unwrap()
            .write_all(b"{\"id\": \"torn")
            .unwrap();
        let (read, bad) = log.read_all().unwrap();
        assert_eq!(read, vec![a, b]);
        assert_eq!(bad, 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
