//! The dataset the server answers from, loaded once at startup into
//! memory (`openparts_data::Dataset`), plus an optional prebuilt
//! artifact bundle. Read-only: the server never writes Canonical Data.

use openparts_data::Dataset;
use std::path::Path;

#[derive(Debug, Default)]
pub struct Store {
    pub dataset: Dataset,
    /// Prebuilt artifacts (`OPENPARTS_ARTIFACT_DIR`), served in place of
    /// generating them per request.
    pub bundle: Option<crate::bundle::Bundle>,
    /// `GET /v1/search`'s index over `dataset.parts`.
    pub search: crate::search::SearchIndex,
    /// `GET /v1/catalog`'s facets over `dataset.parts`.
    pub catalog: crate::catalog::CatalogIndex,
    /// `POST /v1/bom/check` and `GET .../alternatives` (openparts-bom).
    pub bom: openparts_bom::BomIndex,
    /// Downloaded source documents (`OPENPARTS_SOURCE_CACHE`); `None`:
    /// documents aren't fetched, and the site links to the originals.
    pub source_cache: Option<crate::sources::SourceCache>,
    /// `GET /v1/pcbcraft/...` and `.../artifacts/pcbcraft`: exported
    /// once by [`crate::pcbcraft::PcbcraftService::prepare`].
    pub pcbcraft: crate::pcbcraft::PcbcraftService,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error(transparent)]
    Data(#[from] openparts_data::DataError),
    /// A file of the dataset failed to load: the server refuses to start
    /// on a partly loaded dataset.
    #[error("{path}: {error}")]
    File {
        path: String,
        error: openparts_data::DataError,
    },
}

pub fn discover_and_load(data_dir: &Path) -> Result<Store, LoadError> {
    let mut dataset = Dataset::load(data_dir)?;
    if !dataset.issues.is_empty() {
        let issue = dataset.issues.remove(0);
        return Err(LoadError::File {
            path: issue.path.display().to_string(),
            error: issue.error,
        });
    }
    tracing::info!(
        parts = dataset.parts.len(),
        devices = dataset.devices.len(),
        packages = dataset.packages.len(),
        sources = dataset.sources.len(),
        "loaded openparts-data"
    );
    Ok(Store {
        search: crate::search::SearchIndex::build(&dataset),
        catalog: crate::catalog::CatalogIndex::build(&dataset),
        bom: openparts_bom::BomIndex::build(&dataset),
        dataset,
        bundle: None,
        source_cache: None,
        pcbcraft: Default::default(),
    })
}
