//! openparts-altium: Altium adapter (Architecture Specification
//! section 12/13).
//!
//! TODO: not implemented for the first vertical slice; see
//! openparts-librepcb for the same rationale.

use openparts_pcbcad::{PcbFootprint, PcbSymbol};

#[derive(Debug, thiserror::Error)]
pub enum AltiumError {
    #[error("openparts-altium is not implemented yet")]
    NotImplemented,
}

pub fn render_symbol(_symbol: &PcbSymbol) -> Result<String, AltiumError> {
    Err(AltiumError::NotImplemented)
}

pub fn render_footprint(_footprint: &PcbFootprint) -> Result<String, AltiumError> {
    Err(AltiumError::NotImplemented)
}
