//! openparts-gltf: glTF exporter (Architecture Specification section 15,
//! Design Principle 4.6 Visual Model).
//!
//! TODO: not implemented for the first vertical slice; see
//! openparts-stl for the same rationale.

use openparts_mcad::MechanicalGeometry;

#[derive(Debug, thiserror::Error)]
pub enum GltfError {
    #[error("openparts-gltf is not implemented yet")]
    NotImplemented,
}

pub fn generate_gltf(_geometry: &MechanicalGeometry) -> Result<Vec<u8>, GltfError> {
    Err(GltfError::NotImplemented)
}
