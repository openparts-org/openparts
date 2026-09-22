//! openparts-stl: STL exporter (Architecture Specification section 15).
//!
//! TODO: not implemented for the first vertical slice, which only
//! requires STEP (see openparts-step) and KiCad outputs.

use openparts_mcad::MechanicalGeometry;

#[derive(Debug, thiserror::Error)]
pub enum StlError {
    #[error("openparts-stl is not implemented yet")]
    NotImplemented,
}

pub fn generate_stl(_geometry: &MechanicalGeometry) -> Result<Vec<u8>, StlError> {
    Err(StlError::NotImplemented)
}
