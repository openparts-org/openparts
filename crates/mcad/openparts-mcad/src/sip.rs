use crate::defaults;
use crate::{Body, BodyShape, Lead, McadError, MechanicalGeometry, Mounting, Point3, Size3};
use openparts_core::Package;

/// Generates Mechanical Geometry for a SIP-style through-hole package:
/// a rectangular body with all `lead_count` leads in a single straight
/// row on one edge, evenly spaced by `pitch` -- unlike
/// [`crate::generate_to92`], there is no stagger (TO-92's staggered
/// middle pin is a JEDEC feature specific to that 3-pin package, not a
/// general SIP feature). Confirmed against a real KiCad SIP-8 reference
/// footprint (`SIP-8_19x3mm_P2.54mm.kicad_mod`): pin 1 squared, the
/// rest round, evenly spaced by `pitch`, all through-hole -- matching
/// this project's existing pin1-vs-rest polarity convention already
/// applied by `openparts_pcbcad::build_footprint`.
///
/// Numbering: pin 1 at the leftmost position, increasing left to right.
pub fn generate_sip(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let lead_count = package.lead_count;
    if lead_count == 0 {
        return Err(McadError::InvalidLeadCount(lead_count));
    }

    let pitch = defaults::dim(&package.pitch, "pitch.typ")?;
    let body_w = defaults::dim(&package.dimensions.body_width, "dimensions.body_width.typ")?;
    let body_l = defaults::dim(
        &package.dimensions.body_length,
        "dimensions.body_length.typ",
    )?;
    let body_h = defaults::stated(&package.dimensions.body_height)
        .unwrap_or(crate::defaults::sip::BODY_HEIGHT);

    let body = Body {
        position: Point3 {
            x: 0.0,
            y: 0.0,
            z: body_h / 2.0,
        },
        size: Size3 {
            x: body_w,
            y: body_l,
            z: body_h,
        },
        shape: BodyShape::Box,
    };

    let half_span = (lead_count as f64 - 1.0) * pitch / 2.0;

    let pad_size = Size3 {
        x: crate::defaults::sip::PAD_DIAMETER,
        y: crate::defaults::sip::PAD_DIAMETER,
        z: crate::defaults::sip::LEAD_HEIGHT,
    };

    let leads = (0..lead_count)
        .map(|i| {
            let x = -half_span + i as f64 * pitch;
            Lead {
                number: (i + 1).to_string(),
                position: Point3 { x, y: 0.0, z: 0.0 },
                size: pad_size,
                mounting: Mounting::ThroughHole,
                drill: Some(crate::defaults::sip::DRILL_DIAMETER),
                slot: None,
                shape: crate::LandShape::Rect,
                side: openparts_core::PadSide::Top,
            }
        })
        .collect();

    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],

        pitch: crate::defaults::required(&package.pitch),

        details: crate::Details::None,
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

    fn sip8() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/SIP8-TEST"),
            family: openparts_core::PackageFamily::Sip,
            lead_count: 8,
            pitch: Dimension {
                typ: Some(2.54),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    typ: Some(19.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    typ: Some(3.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                ..Default::default()
            },
            lead_layout: None,
            tab: None,
            aux_pads: vec![],
            pad_order: vec![],
            actuation: None,
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
            provenance: BTreeMap::new(),
            review: None,
        }
    }

    #[test]
    fn generates_expected_lead_count_and_numbering() {
        let geometry = generate_sip(&sip8()).unwrap();
        assert_eq!(geometry.leads.len(), 8);
        let numbers: Vec<&str> = geometry.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, vec!["1", "2", "3", "4", "5", "6", "7", "8"]);
    }

    #[test]
    fn every_lead_is_through_hole_with_a_drill() {
        let geometry = generate_sip(&sip8()).unwrap();
        for lead in &geometry.leads {
            assert_eq!(lead.mounting, Mounting::ThroughHole);
            assert_eq!(lead.drill, Some(crate::defaults::sip::DRILL_DIAMETER));
        }
    }

    #[test]
    fn leads_form_a_single_straight_row_evenly_spaced_by_pitch() {
        let geometry = generate_sip(&sip8()).unwrap();
        for lead in &geometry.leads {
            assert_eq!(lead.position.y, 0.0);
        }
        let pin1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        let pin2 = geometry.leads.iter().find(|l| l.number == "2").unwrap();
        assert!((pin2.position.x - pin1.position.x - 2.54).abs() < 1e-9);
    }

    #[test]
    fn missing_nominal_is_an_error_not_an_invented_value() {
        let mut package = sip8();
        package.dimensions.body_width.typ = None;
        let err = generate_sip(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }

    #[test]
    fn rejects_zero_lead_count() {
        let mut package = sip8();
        package.lead_count = 0;
        let err = generate_sip(&package).unwrap_err();
        assert!(matches!(err, McadError::InvalidLeadCount(0)));
    }
}
