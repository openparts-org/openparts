use crate::defaults;
use crate::{Body, BodyShape, Lead, McadError, MechanicalGeometry, Mounting, Point3, Size3};
use openparts_core::Package;

/// Generator default: typical end-termination length for a small
/// 2-terminal chip component, used when the package states no
/// `lead_length`. Not a datasheet fact -- an approximation, same policy
/// as `defaults.rs`.
const DEFAULT_TERMINATION_LENGTH_MM: f64 = 0.3;
const DEFAULT_BODY_HEIGHT_MM: f64 = 0.45;

/// Generates Mechanical Geometry for a 2-terminal chip component
/// (resistor, capacitor, ...): a rectangular body with a wrap-around
/// terminal at each end. `dimensions.body_length` is the long axis
/// (terminal-to-terminal, X); `dimensions.body_width` is the short axis
/// (Y). Terminals are keyed `"1"`/`"2"` -- passive 2-terminal parts have
/// no meaningful polarity/pin-1 orientation, so no marker is emitted.
pub fn generate_chip(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if !package.family.eq_ignore_ascii_case("chip") {
        return Err(McadError::UnsupportedFamily(package.family.clone()));
    }
    if package.lead_count != 2 {
        return Err(McadError::InvalidLeadCount(package.lead_count));
    }

    let body_len = defaults::dim(
        &package.dimensions.body_length,
        "dimensions.body_length.nominal",
    )?;
    let body_w = defaults::dim(
        &package.dimensions.body_width,
        "dimensions.body_width.nominal",
    )?;
    let body_h =
        defaults::stated(&package.dimensions.body_height).unwrap_or(DEFAULT_BODY_HEIGHT_MM);

    let body = Body {
        position: Point3 {
            x: 0.0,
            y: 0.0,
            z: body_h / 2.0,
        },
        size: Size3 {
            x: body_len,
            y: body_w,
            z: body_h,
        },
        shape: BodyShape::Box,
    };

    let term_len = defaults::value(&package.dimensions.lead_length)
        .unwrap_or(DEFAULT_TERMINATION_LENGTH_MM)
        .min(body_len / 2.0);
    let term_size = Size3 {
        x: term_len,
        y: body_w,
        z: body_h,
    };

    let leads = vec![
        Lead {
            number: "1".to_string(),
            position: Point3 {
                x: -(body_len / 2.0 - term_len / 2.0),
                y: 0.0,
                z: body_h / 2.0,
            },
            size: term_size,
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
        },
        Lead {
            number: "2".to_string(),
            position: Point3 {
                x: body_len / 2.0 - term_len / 2.0,
                y: 0.0,
                z: body_h / 2.0,
            },
            size: term_size,
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
        },
    ];

    Ok(MechanicalGeometry {
        body,
        leads,
        markers: vec![],
        lead_form: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Dimension, Kind, Package, PackageDimensions};
    use std::collections::BTreeMap;

    fn chip_0603() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/CHIP-0603-TEST"),
            family: "chip".into(),
            lead_count: 2,
            pitch: Dimension {
                nominal: None,
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    nominal: Some(0.8),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    nominal: Some(1.6),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_height: Some(Dimension {
                    nominal: Some(0.45),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                }),
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
                mating_length: None,
                tail_length: None,
                pad_diameter: None,
                body_offset: None,
                lead_inset: None,
                actuator_diameter: None,
            },
            lead_layout: None,
            tab: None,
            aux_pads: vec![],
            pad_order: vec![],
            pads: vec![],
            construction: None,
            geometry: None,
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn generates_two_terminals() {
        let geometry = generate_chip(&chip_0603()).unwrap();
        assert_eq!(geometry.leads.len(), 2);
        let numbers: std::collections::BTreeSet<&str> =
            geometry.leads.iter().map(|l| l.number.as_str()).collect();
        assert!(numbers.contains("1"));
        assert!(numbers.contains("2"));
    }

    #[test]
    fn terminals_are_at_opposite_ends() {
        let geometry = generate_chip(&chip_0603()).unwrap();
        let t1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        let t2 = geometry.leads.iter().find(|l| l.number == "2").unwrap();
        assert!(t1.position.x < 0.0);
        assert!(t2.position.x > 0.0);
        assert!(
            (t1.position.x + t2.position.x).abs() < 1e-9,
            "terminals should be symmetric"
        );
    }

    #[test]
    fn rejects_wrong_lead_count() {
        let mut package = chip_0603();
        package.lead_count = 3;
        let err = generate_chip(&package).unwrap_err();
        assert!(matches!(err, McadError::InvalidLeadCount(3)));
    }
}
