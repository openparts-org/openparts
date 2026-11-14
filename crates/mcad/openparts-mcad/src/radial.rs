use crate::defaults;
use crate::{
    Body, BodyShape, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Mounting, Point3,
    Size3,
};
use openparts_core::Package;

/// Generates Mechanical Geometry for a radial electrolytic capacitor:
/// a cylindrical can (`BodyShape::Cylinder`, not an axis-aligned box --
/// see `MechanicalGeometry::Body`'s docs for why the exact shape is
/// kept in the shared geometry model rather than pre-approximated)
/// with 2 through-hole leads spaced `pitch` apart at the can's base.
///
/// Numbering/polarity convention: pin 1 (positive, on the left) and
/// pin 2 (negative, on the right), per IPC-7351B -- confirmed directly
/// against the same real KiCad reference footprint's own pad order
/// (`(pad 1 thru_hole rect (at 0 0) ...) (pad 2 thru_hole circle (at 2
/// 0) ...)`). That reference file is also where the pin1-squared/
/// pin2-round pad-shape polarity convention this generator relies on
/// comes from -- applied generically in
/// `openparts_pcbcad::build_footprint` (first lead of a through-hole
/// part is squared), not hardcoded here.
pub fn generate_radial(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if package.lead_count != 2 {
        return Err(McadError::InvalidLeadCount(package.lead_count));
    }

    let pitch = defaults::dim(&package.pitch, "pitch.nominal")?;
    // Both represent the can's diameter -- a true radial can has no
    // separate width/length, but the schema has no dedicated
    // "diameter" field, so this generator expects (without enforcing)
    // that the two agree.
    let diameter = defaults::dim(
        &package.dimensions.body_width,
        "dimensions.body_width.nominal",
    )?;
    let body_h = defaults::stated(&package.dimensions.body_height)
        .unwrap_or(crate::defaults::radial::BODY_HEIGHT);

    let body = Body {
        position: Point3 {
            x: 0.0,
            y: 0.0,
            z: body_h / 2.0,
        },
        size: Size3 {
            x: diameter,
            y: diameter,
            z: body_h,
        },
        shape: BodyShape::Cylinder,
    };

    let pad_size = Size3 {
        x: crate::defaults::radial::PAD_DIAMETER,
        y: crate::defaults::radial::PAD_DIAMETER,
        z: crate::defaults::radial::LEAD_HEIGHT,
    };
    let leads = vec![
        Lead {
            number: "1".to_string(),
            position: Point3 {
                x: -pitch / 2.0,
                y: 0.0,
                z: 0.0,
            },
            size: pad_size,
            mounting: Mounting::ThroughHole,
            drill: Some(crate::defaults::radial::DRILL_DIAMETER),
            slot: None,
        },
        Lead {
            number: "2".to_string(),
            position: Point3 {
                x: pitch / 2.0,
                y: 0.0,
                z: 0.0,
            },
            size: pad_size,
            mounting: Mounting::ThroughHole,
            drill: Some(crate::defaults::radial::DRILL_DIAMETER),
            slot: None,
        },
    ];

    let markers = vec![Marker {
        kind: MarkerKind::NegativeStripe,
        position: Point3 {
            x: pitch / 2.0,
            y: 0.0,
            z: body_h,
        },
    }];

    Ok(MechanicalGeometry {
        family: package.family,

        pitch: crate::defaults::required(&package.pitch),

        details: crate::Details::None,
        body,
        leads,
        markers,
        lead_form: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Dimension, Kind, Package, PackageDimensions};
    use std::collections::BTreeMap;

    fn radial_d5() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/RADIAL-D5.0-P2.00-TEST"),
            family: openparts_core::PackageFamily::Radial,
            lead_count: 2,
            pitch: Dimension {
                nominal: Some(2.0),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    nominal: Some(5.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    nominal: Some(5.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_height: Some(Dimension {
                    nominal: Some(11.0),
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
    fn generates_two_through_hole_leads() {
        let geometry = generate_radial(&radial_d5()).unwrap();
        assert_eq!(geometry.leads.len(), 2);
        for lead in &geometry.leads {
            assert_eq!(lead.mounting, Mounting::ThroughHole);
            assert_eq!(lead.drill, Some(crate::defaults::radial::DRILL_DIAMETER));
        }
    }

    #[test]
    fn pin_1_is_positive_on_the_left() {
        let geometry = generate_radial(&radial_d5()).unwrap();
        let pin1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        let pin2 = geometry.leads.iter().find(|l| l.number == "2").unwrap();
        assert!(pin1.position.x < 0.0);
        assert!(pin2.position.x > 0.0);
        assert!((pin1.position.x + pin2.position.x).abs() < 1e-9);
        assert!((pin2.position.x - pin1.position.x - 2.0).abs() < 1e-9); // pitch
    }

    #[test]
    fn body_is_a_cylinder_not_a_box() {
        let geometry = generate_radial(&radial_d5()).unwrap();
        assert_eq!(geometry.body.shape, BodyShape::Cylinder);
        assert_eq!(geometry.body.size.x, geometry.body.size.y); // diameter
    }

    #[test]
    fn missing_nominal_is_an_error_not_an_invented_value() {
        let mut package = radial_d5();
        package.dimensions.body_width.nominal = None;
        let err = generate_radial(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }

    #[test]
    fn rejects_wrong_lead_count() {
        let mut package = radial_d5();
        package.lead_count = 3;
        let err = generate_radial(&package).unwrap_err();
        assert!(matches!(err, McadError::InvalidLeadCount(3)));
    }
}
