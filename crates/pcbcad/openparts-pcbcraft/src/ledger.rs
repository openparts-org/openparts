//! The revision ledger: what each exported record's id and revisions
//! were, so a later export keeps them.
//!
//! PCBCraft revisions are immutable: two copies of one (library, kind,
//! id, revision) with different content are both quarantined. The
//! ledger keeps, per record, the OpenParts key it was made from and
//! every revision it was published at with the SHA-256 of its content
//! (the canonical document with the revision and the source reference
//! blanked, so neither the revision itself nor the export run's
//! dataset revision changes the digest). An export reuses a revision
//! whose digest matches -- re-emitting that revision's own source
//! reference, so the bytes are identical -- and otherwise allocates the
//! next one.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const LEDGER_FORMAT: &str = "openparts-pcbcraft-ledger/1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    pub format: String,
    /// Library id -> its records.
    pub libraries: BTreeMap<String, LedgerLibrary>,
}

impl Default for Ledger {
    fn default() -> Self {
        Ledger {
            format: LEDGER_FORMAT.into(),
            libraries: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedgerLibrary {
    /// `<kind>/<id>` (kind: `symbol`, `footprint`, `category`, `part`,
    /// `manufacturer-part`) -> the record.
    pub records: BTreeMap<String, LedgerRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedgerRecord {
    /// The OpenParts object the record is made from
    /// (`device:raspberrypi/RP2040`, `part:yageo/RC0603FR-0710KL`...).
    pub key: String,
    pub revisions: Vec<LedgerRevision>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedgerRevision {
    pub revision: u32,
    pub sha256: String,
    /// The source reference this revision was published with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("invalid ledger JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported ledger format {0:?} (expected {LEDGER_FORMAT})")]
    Format(String),
}

impl Ledger {
    pub fn from_json(text: &str) -> Result<Ledger, LedgerError> {
        let ledger: Ledger = serde_json::from_str(text)?;
        if ledger.format != LEDGER_FORMAT {
            return Err(LedgerError::Format(ledger.format));
        }
        Ok(ledger)
    }

    pub fn to_json(&self) -> String {
        crate::record::render(self)
    }

    pub fn library_mut(&mut self, id: &str) -> &mut LedgerLibrary {
        self.libraries.entry(id.to_string()).or_default()
    }
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

impl LedgerLibrary {
    /// The ids a kind's keys already have.
    pub(crate) fn ids(&self, kind: &str) -> BTreeMap<String, String> {
        let prefix = format!("{kind}/");
        self.records
            .iter()
            .filter_map(|(k, r)| Some((r.key.clone(), k.strip_prefix(&prefix)?.to_string())))
            .collect()
    }

    pub(crate) fn taken(&self, kind: &str, id: &str) -> Option<&str> {
        self.records
            .get(&format!("{kind}/{id}"))
            .map(|r| r.key.as_str())
    }

    /// The revision for content `digest` of `kind/id` (made from
    /// `key`), and the source reference to publish it with.
    pub(crate) fn revision(
        &mut self,
        kind: &str,
        id: &str,
        key: &str,
        digest: &str,
        reference: Option<&str>,
    ) -> (u32, Option<String>) {
        let record = self
            .records
            .entry(format!("{kind}/{id}"))
            .or_insert_with(|| LedgerRecord {
                key: key.to_string(),
                revisions: vec![],
            });
        if let Some(r) = record.revisions.iter().find(|r| r.sha256 == digest) {
            return (r.revision, r.reference.clone());
        }
        let revision = record
            .revisions
            .iter()
            .map(|r| r.revision)
            .max()
            .unwrap_or(0)
            + 1;
        record.revisions.push(LedgerRevision {
            revision,
            sha256: digest.to_string(),
            reference: reference.map(str::to_string),
        });
        (revision, reference.map(str::to_string))
    }
}
