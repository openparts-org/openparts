//! BOM checking and alternative parts (openparts-bom):
//!
//! - `GET /v1/parts/{manufacturer}/{mpn}/alternatives`: candidate
//!   replacements, classified (drop-in, footprint-compatible with a
//!   different pinout, functional equivalent, near miss) and ranked,
//!   with the parameter comparison and the per-pad pin diff;
//! - `POST /v1/bom/check`: a BOM (KiCad BOM CSV, KiCad netlist, or a
//!   CSV with MPN/manufacturer columns) as the request body, each line
//!   matched to parts and checked. At most
//!   [`openparts_bom::MAX_BYTES`] (larger: `413`) and
//!   [`openparts_bom::MAX_ROWS`] rows; input that isn't a BOM is `400`.

use crate::error::ApiError;
use crate::store::Store;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use openparts_bom::{AlternativesResponse, BomError, BomReport};
use std::sync::Arc;

pub fn router() -> Router<Arc<Store>> {
    Router::new()
        .route(
            "/v1/parts/:manufacturer/:mpn/alternatives",
            get(alternatives),
        )
        .route(
            "/v1/bom/check",
            post(bom_check).layer(DefaultBodyLimit::max(openparts_bom::MAX_BYTES)),
        )
}

/// `GET /v1/parts/{manufacturer}/{mpn}/alternatives`
async fn alternatives(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<AlternativesResponse>, ApiError> {
    let part = store
        .dataset
        .part(&manufacturer, &mpn)
        .ok_or_else(|| ApiError::NotFound(format!("no part {manufacturer}/{mpn}")))?;
    let index = &store.bom.alternatives;
    let i = index
        .position(part)
        .ok_or_else(|| ApiError::NotFound(format!("no part {manufacturer}/{mpn}")))?;
    Ok(Json(index.alternatives(&store.dataset, i)))
}

/// `POST /v1/bom/check` (body: the BOM file as text).
async fn bom_check(
    State(store): State<Arc<Store>>,
    body: Bytes,
) -> Result<Json<BomReport>, ApiError> {
    let bad = |e: BomError| ApiError::BadRequest(e.to_string());
    let text = openparts_bom::decode(&body).map_err(bad)?;
    let bom = openparts_bom::parse_bom(&text).map_err(bad)?;
    // Matching a few thousand lines takes a moment: off the async workers.
    let report = tokio::task::spawn_blocking(move || store.bom.check(&store.dataset, &bom))
        .await
        .map_err(|e| ApiError::GenerationFailed(e.to_string()))?;
    Ok(Json(report))
}

#[cfg(all(test, feature = "schema"))]
pub(crate) fn subschemas(gen: &mut schemars::SchemaGenerator) {
    gen.subschema_for::<AlternativesResponse>();
    gen.subschema_for::<BomReport>();
}
