//! Human review of recorded facts (Canonical Data Specification section
//! 26.1): each fact's status, the queue of facts left to review, and
//! the one write path of this server -- accepting a review submission.
//!
//! The server never writes Canonical Data. A submission is appended to
//! `OPENPARTS_REVIEW_DIR/submissions.jsonl` (openparts-review's
//! [`SubmissionLog`]); `openparts reviews export` later turns the log
//! into `reviews/*.yaml` in an openparts-data checkout, which a person
//! commits. Writes are opt-in: without `OPENPARTS_REVIEW_TOKEN` (and a
//! review directory) `POST /v1/reviews` answers `403` and the site
//! stays read-only.
//!
//! - `GET  /v1/reviews/config`: whether submissions are accepted.
//! - `GET  /v1/parts/{manufacturer}/{mpn}/reviews`: every fact of the
//!   part, its device and its package with its status, and counts.
//! - `GET  /v1/reviews/queue?category=&manufacturer=&mpn=&status=&offset=&limit=`:
//!   facts to review, parts with electrical parameters first.
//! - `POST /v1/reviews` (`Authorization: Bearer <token>`): submit one
//!   review.

use crate::store::Store;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use openparts_core::{
    ComponentCategory, FactReview, Part, ReviewDocument, ReviewStatus, ReviewTargetKind,
};
use openparts_review::{display_value, fact_state, FactStatus, Submission, SubmissionLog};
use openparts_validator::Fact;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::path::Path as FsPath;
use std::sync::{Arc, RwLock};

/// Largest accepted submission body.
pub const MAX_BODY_BYTES: usize = 16 * 1024;
/// The log stops growing at this size (export and rotate it).
pub const MAX_LOG_BYTES: u64 = 64 * 1024 * 1024;
/// Shortest token that enables writes.
pub const MIN_TOKEN_LEN: usize = 16;

pub fn routes() -> Router<Arc<Store>> {
    Router::new()
        .route("/v1/reviews/config", get(config))
        .route("/v1/reviews/queue", get(queue))
        .route(
            "/v1/reviews",
            post(submit).layer(DefaultBodyLimit::max(MAX_BODY_BYTES)),
        )
        .route("/v1/parts/:manufacturer/:mpn/reviews", get(part_reviews))
}

/// The pending submissions, and how to accept more.
#[derive(Default)]
pub struct ReviewService {
    writer: Option<Writer>,
    /// Submissions in the log, by (kind, target), oldest first.
    pending: RwLock<BTreeMap<(ReviewTargetKind, String), Vec<Submission>>>,
}

struct Writer {
    token_digest: [u8; 32],
    log: SubmissionLog,
}

impl std::fmt::Debug for ReviewService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReviewService")
            .field("writes_enabled", &self.writer.is_some())
            .finish_non_exhaustive()
    }
}

fn digest(s: &str) -> [u8; 32] {
    Sha256::digest(s.as_bytes()).into()
}

impl ReviewService {
    /// From `OPENPARTS_REVIEW_DIR` and `OPENPARTS_REVIEW_TOKEN`.
    pub fn from_env() -> std::io::Result<ReviewService> {
        let dir = std::env::var_os("OPENPARTS_REVIEW_DIR").map(std::path::PathBuf::from);
        let token = std::env::var("OPENPARTS_REVIEW_TOKEN").ok();
        ReviewService::new(dir.as_deref(), token)
    }

    /// Pending submissions are read from `dir` when given; writes are
    /// enabled only with both `dir` and a `token` of at least
    /// [`MIN_TOKEN_LEN`] characters.
    pub fn new(dir: Option<&FsPath>, token: Option<String>) -> std::io::Result<ReviewService> {
        let Some(dir) = dir else {
            if token.is_some() {
                tracing::warn!("OPENPARTS_REVIEW_TOKEN is set but OPENPARTS_REVIEW_DIR isn't: review submissions stay disabled");
            }
            return Ok(ReviewService::default());
        };
        let log = SubmissionLog::open(dir)?;
        let (submissions, bad) = log.read_all()?;
        if bad > 0 {
            tracing::warn!(bad, path = %log.path().display(), "unreadable review submissions skipped");
        }
        let mut pending: BTreeMap<_, Vec<Submission>> = BTreeMap::new();
        for s in submissions {
            pending
                .entry((s.target_kind, s.target.clone()))
                .or_default()
                .push(s);
        }
        let writer = match token {
            Some(t) if t.chars().count() >= MIN_TOKEN_LEN => Some(Writer {
                token_digest: digest(&t),
                log,
            }),
            Some(_) => {
                tracing::error!(
                    "OPENPARTS_REVIEW_TOKEN is shorter than {MIN_TOKEN_LEN} characters: review submissions stay disabled"
                );
                None
            }
            None => None,
        };
        tracing::info!(
            writes = writer.is_some(),
            dir = %dir.display(),
            "review submissions"
        );
        Ok(ReviewService {
            writer,
            pending: RwLock::new(pending),
        })
    }

    pub fn writes_enabled(&self) -> bool {
        self.writer.is_some()
    }

    fn pending_of(&self, kind: ReviewTargetKind, target: &str) -> Vec<Submission> {
        self.pending
            .read()
            .map(|p| p.get(&(kind, target.to_string())).cloned())
            .ok()
            .flatten()
            .unwrap_or_default()
    }
}

/// A fact's review status on the wire.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactReviewStatus {
    Unreviewed,
    Confirmed,
    Disputed,
}

impl From<FactStatus> for FactReviewStatus {
    fn from(s: FactStatus) -> Self {
        match s {
            FactStatus::Unreviewed => FactReviewStatus::Unreviewed,
            FactStatus::Confirmed => FactReviewStatus::Confirmed,
            FactStatus::Disputed => FactReviewStatus::Disputed,
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    Unreviewed,
    Disputed,
    Confirmed,
    All,
}

/// Where a fact's value is cited, with the Source it cites.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct FactEvidence {
    pub source: String,
    pub page: Option<u32>,
    pub locator: Option<String>,
    /// `<PUBLISHER> <document id>`, empty for an unknown source.
    pub title: String,
    pub revision: String,
    pub date: String,
    pub url: String,
}

/// One current review of a fact.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct ReviewView {
    pub reviewer: String,
    pub date: String,
    pub status: ReviewStatus,
    pub comment: Option<String>,
    pub corrected_value: Option<String>,
    /// Submitted, not yet committed to openparts-data.
    pub pending: bool,
}

/// One fact and its review status.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct FactView {
    pub target_kind: ReviewTargetKind,
    pub target: String,
    /// The provenance key (JSON Pointer).
    pub pointer: String,
    /// The value as short text (`3.9 mm`).
    pub value: String,
    /// What a submission must quote (`sha256:<hex>`).
    pub value_hash: String,
    pub evidence: Vec<FactEvidence>,
    pub status: FactReviewStatus,
    /// The status rests on pending submissions only.
    pub pending: bool,
    /// Reviews of an earlier value (they don't count).
    pub stale: u32,
    pub reviews: Vec<ReviewView>,
}

/// `GET /v1/parts/{manufacturer}/{mpn}/reviews`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct PartReviews {
    pub manufacturer: String,
    pub mpn: String,
    pub total: u32,
    pub confirmed: u32,
    pub disputed: u32,
    pub unreviewed: u32,
    /// Facts whose status rests on pending submissions only.
    pub pending: u32,
    /// Part facts first, then the device's, then the package's.
    pub facts: Vec<FactView>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct ReviewConfig {
    /// `POST /v1/reviews` accepts submissions (with the token).
    pub writes_enabled: bool,
    /// `GET /v1/sources/{id}/document` fetches documents.
    pub documents_enabled: bool,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct QueueItem {
    /// A part the fact belongs to (a device or package fact is shared
    /// by every part using it; this is the first in queue order).
    pub manufacturer: String,
    pub mpn: String,
    pub category: Option<ComponentCategory>,
    pub fact: FactView,
}

/// `GET /v1/reviews/queue`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct ReviewQueue {
    pub total: u32,
    pub offset: u32,
    pub limit: u32,
    pub items: Vec<QueueItem>,
}

/// `POST /v1/reviews`'s body.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewSubmission {
    pub target_kind: ReviewTargetKind,
    pub target: String,
    pub pointer: String,
    pub status: ReviewStatus,
    pub reviewer: String,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub corrected_value: Option<String>,
    /// The `value_hash` of the fact as the reviewer saw it.
    pub value_hash: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct SubmitResponse {
    pub id: String,
    pub fact: FactView,
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(serde_json::json!({ "error": message.into() }))).into_response()
}

fn evidence_of(store: &Store, fact: &Fact) -> Vec<FactEvidence> {
    fact.evidence
        .iter()
        .map(|e| {
            let source = store.dataset.sources.get(&e.source.0);
            FactEvidence {
                source: e.source.0.clone(),
                page: e.page,
                locator: e.locator.clone(),
                title: source.map_or_else(String::new, |s| {
                    format!("{} {}", s.publisher.to_uppercase(), s.document_id)
                }),
                revision: source.map_or_else(String::new, |s| s.revision.clone()),
                date: source.map_or_else(String::new, |s| s.date.clone()),
                url: source.map_or_else(String::new, |s| s.url.clone()),
            }
        })
        .collect()
}

/// The facts of one document with their status.
fn views_of(store: &Store, kind: ReviewTargetKind, target: &str) -> Vec<FactView> {
    let Some(facts) = store.facts.facts(kind, target) else {
        return vec![];
    };
    let committed = store
        .dataset
        .reviews_of(kind, target)
        .map_or(&[][..], |d| d.reviews.as_slice());
    let pending = store.review.pending_of(kind, target);
    let pending: Vec<&Submission> = pending.iter().collect();
    facts
        .iter()
        .map(|f| view(store, kind, target, f, committed, &pending))
        .collect()
}

fn view(
    store: &Store,
    kind: ReviewTargetKind,
    target: &str,
    fact: &Fact,
    committed: &[FactReview],
    pending: &[&Submission],
) -> FactView {
    let state = fact_state(fact, committed, pending);
    FactView {
        target_kind: kind,
        target: target.to_string(),
        pointer: fact.pointer.clone(),
        value: display_value(fact.value.as_ref()),
        value_hash: fact.hash.clone(),
        evidence: evidence_of(store, fact),
        status: state.status.into(),
        pending: state.pending,
        stale: state.stale as u32,
        reviews: state
            .current
            .into_iter()
            .map(|c| ReviewView {
                reviewer: c.record.reviewer,
                date: c.record.date,
                status: c.record.status,
                comment: c.record.comment,
                corrected_value: c.record.corrected_value,
                pending: c.pending,
            })
            .collect(),
    }
}

/// Part facts, then the device's, then the package's; electrical
/// parameters first within each.
fn part_facts(store: &Store, part: &Part) -> Vec<FactView> {
    let mut out = Vec::new();
    for (kind, id) in [
        (ReviewTargetKind::Part, &part.id.0),
        (ReviewTargetKind::Device, &part.device.0),
        (ReviewTargetKind::Package, &part.package.0),
    ] {
        let mut views = views_of(store, kind, id);
        views.sort_by_key(|v| !v.pointer.starts_with("/parameters/"));
        out.extend(views);
    }
    out
}

async fn config(State(store): State<Arc<Store>>) -> Json<ReviewConfig> {
    Json(ReviewConfig {
        writes_enabled: store.review.writes_enabled(),
        documents_enabled: store.source_cache.is_some(),
    })
}

async fn part_reviews(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Response {
    let Some(part) = store.dataset.part(&manufacturer, &mpn) else {
        return error(
            StatusCode::NOT_FOUND,
            format!("no part {manufacturer}/{mpn}"),
        );
    };
    let facts = part_facts(&store, part);
    let count = |s: FactReviewStatus| facts.iter().filter(|f| f.status == s).count() as u32;
    Json(PartReviews {
        manufacturer,
        mpn,
        total: facts.len() as u32,
        confirmed: count(FactReviewStatus::Confirmed),
        disputed: count(FactReviewStatus::Disputed),
        unreviewed: count(FactReviewStatus::Unreviewed),
        pending: facts.iter().filter(|f| f.pending).count() as u32,
        facts,
    })
    .into_response()
}

#[derive(Debug, Deserialize)]
struct QueueParams {
    category: Option<ComponentCategory>,
    manufacturer: Option<String>,
    /// With `manufacturer`: only this part's facts.
    mpn: Option<String>,
    status: Option<QueueStatus>,
    offset: Option<u32>,
    limit: Option<u32>,
}

/// `GET /v1/reviews/queue`: facts with the wanted status (unreviewed by
/// default), each once, in priority order: parts with electrical
/// parameters first (their parameters are the facts most often read
/// off a table by hand), then by manufacturer and MPN; within a part
/// its own facts, then its device's and its package's.
async fn queue(State(store): State<Arc<Store>>, Query(q): Query<QueueParams>) -> Response {
    let ds = &store.dataset;
    let wanted = q.status.unwrap_or(QueueStatus::Unreviewed);
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let offset = q.offset.unwrap_or(0);
    let mut parts: Vec<&Part> = ds
        .parts
        .iter()
        .filter(|p| {
            q.manufacturer
                .as_deref()
                .is_none_or(|m| m.is_empty() || p.manufacturer.0 == m)
        })
        .filter(|p| q.mpn.as_deref().is_none_or(|m| m.is_empty() || p.mpn == m))
        .filter(|p| match q.category {
            None => true,
            Some(c) => ds
                .devices
                .get(&p.device.0)
                .is_some_and(|d| d.category.is_within(c)),
        })
        .collect();
    let has_parameters = |p: &Part| {
        !p.parameters.is_empty()
            || ds
                .devices
                .get(&p.device.0)
                .is_some_and(|d| !d.parameters.is_empty())
    };
    parts.sort_by_key(|p| !has_parameters(p));
    let mut seen = HashSet::new();
    let mut total = 0u32;
    let mut items = Vec::new();
    for part in parts {
        for fact in part_facts(&store, part) {
            if !seen.insert((fact.target_kind, fact.target.clone(), fact.pointer.clone())) {
                continue;
            }
            let keep = match wanted {
                QueueStatus::All => true,
                QueueStatus::Unreviewed => fact.status == FactReviewStatus::Unreviewed,
                QueueStatus::Disputed => fact.status == FactReviewStatus::Disputed,
                QueueStatus::Confirmed => fact.status == FactReviewStatus::Confirmed,
            };
            if !keep {
                continue;
            }
            if total >= offset && items.len() < limit as usize {
                items.push(QueueItem {
                    manufacturer: part.manufacturer.0.clone(),
                    mpn: part.mpn.clone(),
                    category: ds.devices.get(&part.device.0).map(|d| d.category),
                    fact,
                });
            }
            total += 1;
        }
    }
    Json(ReviewQueue {
        total,
        offset,
        limit,
        items,
    })
    .into_response()
}

/// The bearer token of `headers`, compared in constant time.
fn authorized(headers: &HeaderMap, token_digest: &[u8; 32]) -> bool {
    let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let Some(given) = value.strip_prefix("Bearer ") else {
        return false;
    };
    let given = digest(given.trim());
    given
        .iter()
        .zip(token_digest)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().replace("\r\n", "\n"))
        .filter(|s| !s.is_empty())
}

/// `POST /v1/reviews`: one review of one fact, appended to the log.
async fn submit(State(store): State<Arc<Store>>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(writer) = &store.review.writer else {
        return error(
            StatusCode::FORBIDDEN,
            "review submissions are disabled on this server (no OPENPARTS_REVIEW_TOKEN)",
        );
    };
    if !authorized(&headers, &writer.token_digest) {
        return error(StatusCode::UNAUTHORIZED, "missing or wrong review token");
    }
    let s: ReviewSubmission = match serde_json::from_slice(&body) {
        Ok(s) => s,
        Err(e) => {
            return error(
                StatusCode::BAD_REQUEST,
                format!("malformed submission: {e}"),
            )
        }
    };
    let Some(fact) = store.facts.fact(s.target_kind, &s.target, &s.pointer) else {
        return error(
            StatusCode::NOT_FOUND,
            format!(
                "{} {} has no fact {}",
                s.target_kind.as_str(),
                s.target,
                s.pointer
            ),
        );
    };
    if s.value_hash != fact.hash {
        return error(
            StatusCode::CONFLICT,
            "the fact has changed since it was loaded: reload and review it again",
        );
    }
    let mut sources: Vec<openparts_core::SourceId> = Vec::new();
    for e in &fact.evidence {
        if !sources.contains(&e.source) {
            sources.push(e.source.clone());
        }
    }
    let record = FactReview {
        pointer: s.pointer.clone(),
        status: s.status,
        reviewer: s.reviewer.trim().to_string(),
        date: openparts_review::today_utc(),
        sources,
        value_hash: s.value_hash.clone(),
        comment: clean(s.comment),
        corrected_value: clean(s.corrected_value),
    };
    let problems = openparts_validator::review_record_problems(&record);
    if !problems.is_empty() {
        return error(StatusCode::BAD_REQUEST, problems.join("; "));
    }
    if writer.log.len_bytes() > MAX_LOG_BYTES {
        return error(
            StatusCode::INSUFFICIENT_STORAGE,
            "the review log is full: export it (openparts reviews export) and rotate it",
        );
    }
    let submission = Submission::new(s.target_kind, &s.target, record);
    {
        let Ok(mut pending) = store.review.pending.write() else {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "review state unavailable",
            );
        };
        if let Err(e) = writer.log.append(&submission) {
            tracing::error!(error = %e, "appending a review submission");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not store the submission",
            );
        }
        pending
            .entry((submission.target_kind, submission.target.clone()))
            .or_default()
            .push(submission.clone());
    }
    tracing::info!(
        id = %submission.id,
        target = %ReviewDocument::id_for(s.target_kind, &s.target),
        pointer = %s.pointer,
        reviewer = %submission.record.reviewer,
        "review submitted"
    );
    let views = views_of(&store, s.target_kind, &s.target);
    let Some(fact) = views.into_iter().find(|v| v.pointer == s.pointer) else {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "fact vanished");
    };
    (
        StatusCode::CREATED,
        Json(SubmitResponse {
            id: submission.id,
            fact,
        }),
    )
        .into_response()
}

#[cfg(all(test, feature = "schema"))]
pub(crate) fn subschemas(gen: &mut schemars::SchemaGenerator) {
    gen.subschema_for::<PartReviews>();
    gen.subschema_for::<ReviewQueue>();
    gen.subschema_for::<ReviewConfig>();
    gen.subschema_for::<SubmitResponse>();
}
