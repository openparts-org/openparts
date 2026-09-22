//! openparts-librepcb: LibrePCB adapter (Architecture Specification
//! section 12/13 — one PCB CAD adapter behind the openparts-pcbcad IR).
//!
//! TODO: not implemented for the first vertical slice, which only
//! requires the KiCad adapter (see openparts-kicad). "Not all Adapters
//! need to ship in the initial release" (section 6).

use openparts_pcbcad::{PcbFootprint, PcbSymbol};

#[derive(Debug, thiserror::Error)]
pub enum LibrePcbError {
    #[error("openparts-librepcb is not implemented yet")]
    NotImplemented,
}

pub fn render_symbol(_symbol: &PcbSymbol) -> Result<String, LibrePcbError> {
    Err(LibrePcbError::NotImplemented)
}

pub fn render_footprint(_footprint: &PcbFootprint) -> Result<String, LibrePcbError> {
    Err(LibrePcbError::NotImplemented)
}
