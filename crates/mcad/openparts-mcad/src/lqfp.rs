use crate::defaults::{self, Axis, GullWing};
use crate::{
    Body, BodyShape, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Mounting, Point3,
    Size3,
};
use openparts_core::Package;

/// Generates Mechanical Geometry for an LQFP/TQFP-style package: a
/// rectangular body with `lead_count / 4` gull-wing leads evenly spaced
/// at `pitch` on each of the 4 sides. Each lead's outline runs from the
/// body edge to the toe at `overall_width / 2` (left/right) or
/// `overall_length / 2` (bottom/top), `lead_width` wide; unstated
/// dimensions fall back to `defaults::QFP` (JEDEC MS-026).
///
/// Numbering convention (matches common LQFP/TQFP datasheets): pin 1 at
/// the top of the LEFT edge, proceeding down the left edge, then
/// left-to-right along the bottom, then up the right edge, then
/// right-to-left along the top, ending just short of pin 1.
pub fn generate_lqfp(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let lead_count = package.lead_count;
    if lead_count == 0 || !lead_count.is_multiple_of(4) {
        return Err(McadError::InvalidLeadCount(lead_count));
    }
    let pins_per_side = lead_count / 4;

    let pitch = defaults::dim(&package.pitch, "pitch.typ")?;
    let body_w = defaults::dim(&package.dimensions.body_width, "dimensions.body_width.typ")?;
    let body_l = defaults::dim(
        &package.dimensions.body_length,
        "dimensions.body_length.typ",
    )?;
    let shape = GullWing::resolve(package, &defaults::QFP);
    let body_h = shape.body_height;
    let reach_x = shape.protrusion(package, Axis::X)?;
    let reach_y = shape.protrusion(package, Axis::Y)?;
    let (lead_w, lead_t) = (shape.lead_width, shape.thickness);

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

    let span = (pins_per_side as f64 - 1.0) * pitch;
    let half_span = span / 2.0;

    let lead_size_along = Size3 {
        x: reach_x,
        y: lead_w,
        z: lead_t,
    };
    let lead_size_perp = Size3 {
        x: lead_w,
        y: reach_y,
        z: lead_t,
    };

    let mut leads = Vec::with_capacity(lead_count as usize);
    let mut pin_num = 1u32;

    // Left edge: x = -body_w/2 - protrusion/2, y from +half_span down to -half_span.
    for i in 0..pins_per_side {
        let y = half_span - i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x: -(body_w / 2.0 + reach_x / 2.0),
                y,
                z: lead_t / 2.0,
            },
            size: lead_size_along,
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
        });
        pin_num += 1;
    }
    // Bottom edge: y = -body_l/2 - protrusion/2, x from -half_span to +half_span.
    for i in 0..pins_per_side {
        let x = -half_span + i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x,
                y: -(body_l / 2.0 + reach_y / 2.0),
                z: lead_t / 2.0,
            },
            size: lead_size_perp,
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
        });
        pin_num += 1;
    }
    // Right edge: x = +body_w/2 + protrusion/2, y from -half_span to +half_span.
    for i in 0..pins_per_side {
        let y = -half_span + i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x: body_w / 2.0 + reach_x / 2.0,
                y,
                z: lead_t / 2.0,
            },
            size: lead_size_along,
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
        });
        pin_num += 1;
    }
    // Top edge: y = +body_l/2 + protrusion/2, x from +half_span down to -half_span.
    for i in 0..pins_per_side {
        let x = half_span - i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x,
                y: body_l / 2.0 + reach_y / 2.0,
                z: lead_t / 2.0,
            },
            size: lead_size_perp,
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
        });
        pin_num += 1;
    }

    let markers = vec![Marker {
        kind: MarkerKind::Pin1Dot,
        position: Point3 {
            x: -body_w / 2.0 + 0.5,
            y: body_l / 2.0 - 0.5,
            z: body_h,
        },
    }];

    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],

        pitch: crate::defaults::required(&package.pitch),

        details: crate::Details::None,
        body,
        leads,
        markers,
        lead_form: Some(shape.lead_form()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LeadForm;
    use openparts_core::{Dimension, Kind, Package, PackageDimensions};
    use std::collections::BTreeMap;

    fn lqfp8() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/LQFP8-TEST"),
            family: openparts_core::PackageFamily::Lqfp,
            lead_count: 8,
            pitch: Dimension {
                typ: Some(0.5),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    typ: Some(3.0),
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
            edge: None,
            keepouts: vec![],
            pads: vec![],
            construction: None,
            geometry: None,
            provenance: BTreeMap::new(),
            review: None,
        }
    }

    #[test]
    fn generates_expected_lead_count_and_numbering() {
        let geometry = generate_lqfp(&lqfp8()).unwrap();
        assert_eq!(geometry.leads.len(), 8);
        let numbers: Vec<&str> = geometry.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, vec!["1", "2", "3", "4", "5", "6", "7", "8"]);
    }

    #[test]
    fn pin_1_is_on_the_left_edge_top() {
        let geometry = generate_lqfp(&lqfp8()).unwrap();
        let pin1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        // 2 pins/side, pitch 0.5 -> half_span = 0.25. Pin 1 at top of left edge.
        // No overall_width stated, so the toe is at the JEDEC MS-026
        // default reach of 1.0 mm (E = E1 + 2.0); the lead outline is
        // centred half way out. (Was 0.6 mm before the defaults moved to
        // defaults.rs -- that put a 7x7 LQFP's toe at 4.1 mm instead of
        // the real E/2 = 4.5 mm.)
        assert!((pin1.position.x - (-(3.0 / 2.0 + 0.5))).abs() < 1e-9);
        assert!((pin1.position.y - 0.25).abs() < 1e-9);
    }

    fn mm(v: f64) -> Option<Dimension> {
        Some(Dimension {
            typ: Some(v),
            min: None,
            max: None,
            unit: "mm".into(),
        })
    }

    #[test]
    fn datasheet_lead_dimensions_win_over_defaults() {
        let mut package = lqfp8();
        package.dimensions.overall_width = mm(4.6);
        package.dimensions.overall_length = mm(5.0);
        package.dimensions.lead_width = mm(0.2);
        package.dimensions.lead_thickness = mm(0.127);
        package.dimensions.lead_length = mm(0.45);
        package.dimensions.standoff = Some(Dimension {
            typ: None,
            min: Some(0.05),
            max: Some(0.15),
            unit: "mm".into(),
        });
        let geometry = generate_lqfp(&package).unwrap();
        let pin1 = &geometry.leads[0];
        // Toe at overall_width / 2 = 2.3; reach from the 1.5 body edge 0.8.
        assert!((pin1.position.x - pin1.size.x / 2.0 + 2.3).abs() < 1e-9);
        assert!((pin1.size.x - 0.8).abs() < 1e-9);
        assert!((pin1.size.y - 0.2).abs() < 1e-9);
        let bottom = &geometry.leads[2];
        assert!((bottom.position.y - bottom.size.y / 2.0 + 2.5).abs() < 1e-9);
        let Some(LeadForm::GullWing {
            thickness,
            foot_length,
            standoff,
        }) = geometry.lead_form
        else {
            panic!("expected a gull-wing lead form");
        };
        assert_eq!((thickness, foot_length), (0.127, 0.45));
        // A min/max-only standoff is drawn at its midpoint.
        assert!((standoff - 0.1).abs() < 1e-9);
    }

    #[test]
    fn lead_span_inside_the_body_is_an_error() {
        let mut package = lqfp8();
        package.dimensions.overall_width = mm(2.5);
        let err = generate_lqfp(&package).unwrap_err();
        assert!(matches!(err, McadError::InconsistentDimensions(_)));
    }

    #[test]
    fn missing_nominal_is_an_error_not_an_invented_value() {
        let mut package = lqfp8();
        package.dimensions.body_width.typ = None;
        let err = generate_lqfp(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }
}
