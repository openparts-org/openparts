//! openparts-step: Mechanical Geometry -> STEP, through SolidCraft.
//!
//! `openparts-solidcraft` turns the geometry into a detailed, parametric
//! SolidCraft model (drafted bodies, formed leads, plating, polarity
//! marks) and the locally installed SolidCraft exports it as AP242 STEP
//! with a colour on every body. SolidCraft writes no time stamp, so the
//! same input gives byte-identical STEP (Architecture Specification
//! section 24, content addressing).

pub use openparts_solidcraft::ModelInput;

#[derive(Debug, thiserror::Error)]
pub enum StepError {
    #[error(transparent)]
    Generate(#[from] openparts_solidcraft::Error),
    #[error("SolidCraft wrote STEP that is not UTF-8")]
    NotUtf8,
}

/// The detailed model of `input` as a complete STEP file, its product
/// named after `product_name`.
pub fn generate_step(input: &ModelInput<'_>, product_name: &str) -> Result<String, StepError> {
    let mut files =
        openparts_solidcraft::generate(input, product_name, &[openparts_solidcraft::Format::Step])?;
    String::from_utf8(files.remove(0)).map_err(|_| StepError::NotUtf8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{
        ComponentCategory, Dimension, Kind, Package, PackageDimensions, PackageId,
    };

    fn dim(v: f64) -> Dimension {
        Dimension {
            nominal: Some(v),
            min: None,
            max: None,
            unit: "mm".into(),
        }
    }

    fn soic8() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: PackageId::from("standards/SOIC8-TEST"),
            family: "soic".into(),
            lead_count: 8,
            pitch: dim(1.27),
            dimensions: PackageDimensions {
                body_width: dim(3.9),
                body_length: dim(4.9),
                body_height: Some(dim(1.5)),
                exposed_pad: None,
                overall_width: None,
                overall_length: None,
                overall_height: None,
                standoff: None,
                lead_width: None,
                lead_length: None,
                lead_thickness: None,
                tab_width: None,
                tab_length: None,
                row_spacing: None,
                hole_diameter: None,
                aux_pad_width: None,
                aux_pad_length: None,
                aux_row_x: None,
                aux_row_y: None,
                mounting_hole_diameter: None,
                mounting_hole_spacing: None,
                mounting_hole_inset: None,
            },
            lead_layout: None,
            tab: None,
            aux_pads: vec![],
            construction: None,
            geometry: None,
            provenance: Default::default(),
        }
    }

    fn step_of(package: &Package) -> Option<String> {
        if !openparts_solidcraft::available() {
            eprintln!("SolidCraft is not installed; skipping");
            return None;
        }
        let geometry = openparts_mcad::generate(package).unwrap();
        let input = ModelInput {
            package,
            geometry: &geometry,
            category: Some(ComponentCategory::Microcontroller),
            marked_pin: None,
        };
        Some(generate_step(&input, "TEST").unwrap())
    }

    #[test]
    fn produces_a_well_formed_step_file_with_a_coloured_solid_per_body() {
        let Some(step) = step_of(&soic8()) else {
            return;
        };
        assert!(step.starts_with("ISO-10303-21;"));
        assert!(step.trim_end().ends_with("END-ISO-10303-21;"));
        assert!(step.contains("PRODUCT('TEST'"));
        // The body and the 8 leads, every one coloured.
        let solids = step.matches("MANIFOLD_SOLID_BREP(").count();
        assert_eq!(solids, 9);
        assert_eq!(step.matches("STYLED_ITEM(").count(), solids);
    }

    #[test]
    fn the_same_input_gives_byte_identical_step() {
        let Some(a) = step_of(&soic8()) else { return };
        let b = step_of(&soic8()).unwrap();
        assert_eq!(a, b);
    }
}
