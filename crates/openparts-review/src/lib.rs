//! openparts-review: human review of recorded facts (Canonical Data
//! Specification section 26.1) outside the Canonical Data itself.
//!
//! - [`SubmissionLog`]: the append-only JSON Lines file where
//!   openparts-server keeps review submissions it accepted
//!   (`OPENPARTS_REVIEW_DIR/submissions.jsonl`). The server never
//!   writes into openparts-data.
//! - [`FactIndex`] and [`fact_state`]: every reviewable fact of a
//!   dataset and its status, merging the committed review documents
//!   with pending submissions.
//! - [`export`]: turns submissions into `reviews/<kind>/<id>.yaml` in an
//!   openparts-data checkout (`openparts reviews export`), for a person
//!   to commit or open a pull request with.

mod export;
mod state;
mod submission;
mod time;

pub use export::{export, ExportError, ExportReport};
pub use state::{display_value, fact_state, FactIndex, FactState, FactStatus, StateRecord};
pub use submission::{Submission, SubmissionLog, SUBMISSIONS_FILE};
pub use time::{today_utc, utc_now};
