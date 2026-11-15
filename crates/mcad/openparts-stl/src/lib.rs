//! openparts-stl: Mechanical Geometry -> binary STL, through SolidCraft
//! (the same detailed model `openparts-step` exports, tessellated by
//! SolidCraft). STL has no colours or units; coordinates are millimetres.

pub use openparts_solidcraft::ModelInput;

#[derive(Debug, thiserror::Error)]
pub enum StlError {
    #[error(transparent)]
    Generate(#[from] openparts_solidcraft::Error),
    #[error("SolidCraft wrote a malformed binary STL")]
    Malformed,
}

/// The detailed model of `input` as a binary STL.
pub fn generate_stl(input: &ModelInput<'_>, product_name: &str) -> Result<Vec<u8>, StlError> {
    let mut files =
        openparts_solidcraft::generate(input, product_name, &[openparts_solidcraft::Format::Stl])?;
    let stl = files.remove(0);
    if triangle_count(&stl).is_none() {
        return Err(StlError::Malformed);
    }
    Ok(stl)
}

/// The triangle count of a well-formed binary STL (80-byte header, a
/// little-endian count, 50 bytes per triangle), `None` if malformed.
pub fn triangle_count(stl: &[u8]) -> Option<u32> {
    let n = u32::from_le_bytes(stl.get(80..84)?.try_into().ok()?);
    (stl.len() == 84 + 50 * n as usize).then_some(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{
        ComponentCategory, Dimension, Kind, Package, PackageDimensions, PackageId,
    };

    fn dim(v: f64) -> Dimension {
        Dimension {
            typ: Some(v),
            min: None,
            max: None,
            unit: "mm".into(),
        }
    }

    #[test]
    fn produces_a_well_formed_binary_stl() {
        if !openparts_solidcraft::available_for_test(module_path!()) {
            return;
        }
        let package = Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: PackageId::from("standards/CHIP0603-TEST"),
            family: openparts_core::PackageFamily::Chip,
            lead_count: 2,
            pitch: Dimension {
                typ: None,
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: dim(0.8),
                body_length: dim(1.6),
                body_height: Some(dim(0.45)),
                ..Default::default()
            },
            lead_layout: None,
            tab: None,
            aux_pads: vec![],
            pad_order: vec![],
            connector: None,
            edge: None,
            keepouts: vec![],
            pad_rows: vec![],
            pad_grids: vec![],
            shield: None,
            smt: None,
            pads: vec![],
            construction: None,
            geometry: None,
            provenance: Default::default(),
            review: None,
        };
        let geometry = openparts_mcad::generate(&package).unwrap();
        let input = ModelInput {
            package: &package,
            geometry: &geometry,
            category: Some(ComponentCategory::Resistor),
            marked_pin: None,
        };
        let stl = generate_stl(&input, "TEST").unwrap();
        // Core, overcoat and two end caps: at least 12 triangles each.
        assert!(triangle_count(&stl).unwrap() >= 4 * 12);
    }

    #[test]
    fn rejects_truncated_stl() {
        let mut stl = vec![0u8; 84 + 50];
        stl[80] = 2; // claims two triangles, holds one
        assert_eq!(triangle_count(&stl), None);
        stl[80] = 1;
        assert_eq!(triangle_count(&stl), Some(1));
    }
}
