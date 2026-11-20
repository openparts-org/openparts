//! The data documents behind a part, for the site's review badge and
//! "Edit on GitHub" links (Canonical Data Specification section 26.1):
//! `GET /v1/parts/{manufacturer}/{mpn}/documents` answers each of the
//! part's, its device's and its package's file in openparts-data and
//! `review` block. The catalog carries the same review summary per
//! row ([`ReviewSummary`]) and filters on it (`review=`).
//!
//! Review itself happens in pull requests to openparts-data; nothing
//! here writes anything.

use crate::error::ApiError;
use crate::store::Store;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use openparts_core::{Device, DocumentReview, Kind, Package, Part};
use openparts_data::Dataset;
use serde::Serialize;
use std::sync::Arc;

pub fn router() -> Router<Arc<Store>> {
    Router::new()
        .route("/v1/parts/:manufacturer/:mpn/documents", get(get_documents))
        .route("/v1/config", get(get_config))
}

/// Whether a part's documents are reviewed: all three, some, or none.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    /// The part, its device and its package are all reviewed.
    Reviewed,
    /// Some of them are.
    Partly,
    /// None is (all AI-extracted candidates).
    Unreviewed,
}

impl ReviewState {
    pub fn wire(self) -> &'static str {
        match self {
            ReviewState::Reviewed => "reviewed",
            ReviewState::Partly => "partly",
            ReviewState::Unreviewed => "unreviewed",
        }
    }

    pub fn of(flags: [bool; 3]) -> ReviewState {
        match flags.iter().filter(|r| **r).count() {
            3 => ReviewState::Reviewed,
            0 => ReviewState::Unreviewed,
            _ => ReviewState::Partly,
        }
    }
}

/// A catalog row's review status.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ReviewSummary {
    pub state: ReviewState,
    pub part: bool,
    pub device: bool,
    pub package: bool,
}

impl ReviewSummary {
    pub fn of(part: &Part, device: Option<&Device>, package: Option<&Package>) -> ReviewSummary {
        let (p, d, k) = (
            part.review.is_some(),
            device.is_some_and(|d| d.review.is_some()),
            package.is_some_and(|k| k.review.is_some()),
        );
        ReviewSummary {
            state: ReviewState::of([p, d, k]),
            part: p,
            device: d,
            package: k,
        }
    }
}

/// One data document.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct DocumentInfo {
    pub id: String,
    /// The file, relative to the openparts-data repository root (`/`
    /// separated), e.g. `parts/raspberrypi/rp2040/RP2040.yaml`.
    pub path: Option<String>,
    /// The document's `review` block; `None`: unreviewed.
    pub review: Option<DocumentReview>,
}

/// `GET /v1/parts/{manufacturer}/{mpn}/documents`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct PartDocuments {
    pub part: DocumentInfo,
    pub device: DocumentInfo,
    pub package: DocumentInfo,
    pub review: ReviewState,
}

/// What the site may use on this server.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct ServerConfig {
    /// `GET /v1/sources/{id}/document` serves PDFs
    /// (`OPENPARTS_SOURCE_CACHE` is set); otherwise the site links to
    /// the original documents.
    pub source_documents: bool,
    /// The PCBCraft catalog (`GET /v1/pcbcraft`): whether it is ready
    /// and whether its revisions persist across restarts.
    pub pcbcraft: crate::pcbcraft::PcbcraftStatus,
}

pub fn repo_path(ds: &Dataset, kind: Kind, id: &str) -> Option<String> {
    ds.path_of(kind, id).map(|p| {
        p.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    })
}

fn info(ds: &Dataset, kind: Kind, id: &str, review: Option<&DocumentReview>) -> DocumentInfo {
    DocumentInfo {
        id: id.to_string(),
        path: repo_path(ds, kind, id),
        review: review.cloned(),
    }
}

async fn get_documents(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<PartDocuments>, ApiError> {
    let ds = &store.dataset;
    let part = ds
        .part(&manufacturer, &mpn)
        .ok_or_else(|| ApiError::NotFound(format!("no part {manufacturer}/{mpn}")))?;
    let device = ds.devices.get(&part.device.0);
    let package = ds.packages.get(&part.package.0);
    Ok(Json(PartDocuments {
        part: info(ds, Kind::Part, &part.id.0, part.review.as_ref()),
        device: info(
            ds,
            Kind::Device,
            &part.device.0,
            device.and_then(|d| d.review.as_ref()),
        ),
        package: info(
            ds,
            Kind::Package,
            &part.package.0,
            package.and_then(|k| k.review.as_ref()),
        ),
        review: ReviewSummary::of(part, device, package).state,
    }))
}

async fn get_config(State(store): State<Arc<Store>>) -> Json<ServerConfig> {
    Json(ServerConfig {
        source_documents: store.source_cache.is_some(),
        pcbcraft: store.pcbcraft.status(),
    })
}

#[cfg(feature = "schema")]
pub fn subschemas(gen: &mut schemars::SchemaGenerator) {
    gen.subschema_for::<PartDocuments>();
    gen.subschema_for::<ServerConfig>();
}
