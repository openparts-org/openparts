use crate::defaults::{self, Axis, GullWing};
use crate::{
    Body, BodyShape, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Mounting, Point3,
    Size3,
};
use openparts_core::{Package, PackageFamily as F};

/// Generates Mechanical Geometry for a SOIC/SOP-style package (and
/// TSSOP/SSOP/MSOP, see below): a
/// rectangular body with `lead_count / 2` gull-wing leads evenly spaced
/// at `pitch` on each of the 2 long sides (left/right) -- structurally
/// [`crate::generate_lqfp`]'s left-edge/right-edge pattern with the
/// top/bottom edges dropped, since SOIC's physical layout (unlike
/// SOT's) is unambiguous and fully determined by `lead_count`. Lead
/// toes land at `overall_width / 2`; unstated dimensions fall back to
/// `defaults::SOIC` (JEDEC MS-012).
///
/// Numbering convention (matches common SOIC/JEDEC MS-012 datasheets):
/// pin 1 at the top of the LEFT edge, proceeding down the left edge,
/// then up the right edge from its bottom to its top.
///
/// The same two-row layout serves TSSOP (JEDEC MO-153), SSOP (MO-150) and
/// MSOP/VSSOP (MO-187): `family` picks only the defaults used for
/// dimensions the package doesn't state.
pub fn generate_soic(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let family_defaults = match package.family {
        F::Soic => &defaults::SOIC,
        F::Tssop => &defaults::TSSOP,
        F::Ssop => &defaults::SSOP,
        F::Msop => &defaults::MSOP,
        other => return Err(McadError::UnsupportedFamily(other)),
    };

    let lead_count = package.lead_count;
    if lead_count == 0 || !lead_count.is_multiple_of(2) {
        return Err(McadError::InvalidLeadCount(lead_count));
    }
    let pins_per_side = lead_count / 2;

    let pitch = defaults::dim(&package.pitch, "pitch.typ")?;
    // Body size: nominal, else the stated min..max midpoint (TI and JEDEC
    // tables often give only limits).
    let body_w = defaults::required(&package.dimensions.body_width)
        .ok_or(McadError::MissingDimension("dimensions.body_width"))?;
    let body_l = defaults::required(&package.dimensions.body_length)
        .ok_or(McadError::MissingDimension("dimensions.body_length"))?;
    let shape = GullWing::resolve(package, family_defaults);
    let body_h = shape.body_height;
    let reach = shape.protrusion(package, Axis::X)?;
    let lead_t = shape.thickness;

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

    let lead_size = Size3 {
        x: reach,
        y: shape.lead_width,
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
                x: -(body_w / 2.0 + reach / 2.0),
                y,
                z: lead_t / 2.0,
            },
            size: lead_size,
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
        });
        pin_num += 1;
    }
    // Right edge: x = +body_w/2 + protrusion/2, y from -half_span up to +half_span
    // -- numbering continues directly from the left edge (no bottom/top
    // edges exist on a 2-sided package).
    for i in 0..pins_per_side {
        let y = -half_span + i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x: body_w / 2.0 + reach / 2.0,
                y,
                z: lead_t / 2.0,
            },
            size: lead_size,
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
    use openparts_core::{Dimension, Kind, Package, PackageDimensions};
    use std::collections::BTreeMap;

    fn soic8() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/SOIC8-TEST"),
            family: openparts_core::PackageFamily::Soic,
            lead_count: 8,
            pitch: Dimension {
                typ: Some(1.27),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    typ: Some(3.9),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    typ: Some(4.9),
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
        let geometry = generate_soic(&soic8()).unwrap();
        assert_eq!(geometry.leads.len(), 8);
        let numbers: Vec<&str> = geometry.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, vec!["1", "2", "3", "4", "5", "6", "7", "8"]);
    }

    #[test]
    fn pin_1_is_on_the_left_edge_top() {
        let geometry = generate_soic(&soic8()).unwrap();
        let pin1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        // 4 pins/side, pitch 1.27 -> half_span = 1.905. No overall_width
        // stated: the toe is at the JEDEC MS-012 default reach of 1.05 mm
        // ((E 6.0 - E1 3.9) / 2; was 0.5 mm, which left the toe ~0.5 mm
        // short of a real SOIC's), lead outline centred half way out.
        assert!((pin1.position.x - (-(3.9 / 2.0 + 0.525))).abs() < 1e-9);
        assert!((pin1.position.y - 1.905).abs() < 1e-9);
    }

    #[test]
    fn only_left_and_right_edges_are_populated() {
        // A 2-sided package: every lead's x is at one of exactly two
        // values (no bottom/top-edge leads at intermediate x positions).
        let geometry = generate_soic(&soic8()).unwrap();
        let xs: std::collections::BTreeSet<i64> = geometry
            .leads
            .iter()
            .map(|l| (l.position.x * 1000.0).round() as i64)
            .collect();
        assert_eq!(
            xs.len(),
            2,
            "expected leads on exactly 2 distinct x positions"
        );
    }

    #[test]
    fn missing_nominal_is_an_error_not_an_invented_value() {
        let mut package = soic8();
        package.dimensions.body_width.typ = None;
        let err = generate_soic(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }

    #[test]
    fn rejects_odd_lead_count() {
        let mut package = soic8();
        package.lead_count = 7;
        let err = generate_soic(&package).unwrap_err();
        assert!(matches!(err, McadError::InvalidLeadCount(7)));
    }

    fn mm(v: f64) -> Option<Dimension> {
        Some(Dimension {
            typ: Some(v),
            min: None,
            max: None,
            unit: "mm".into(),
        })
    }

    fn tssop20() -> Package {
        let mut package = soic8();
        package.family = openparts_core::PackageFamily::Tssop;
        package.lead_count = 20;
        package.pitch.typ = Some(0.65);
        package.dimensions.body_width.typ = Some(4.4);
        package.dimensions.body_length.typ = Some(6.5);
        package
    }

    #[test]
    fn tssop_uses_mo153_defaults_when_the_package_states_nothing() {
        let geometry = generate_soic(&tssop20()).unwrap();
        assert_eq!(geometry.leads.len(), 20);
        let pin1 = &geometry.leads[0];
        // 10 per side at 0.65 -> pin 1 at y = 4.5 * 0.65; MO-153 toe at
        // E/2 = 3.2 (protrusion 1.0 past the 4.4 body), b 0.25.
        assert!((pin1.position.y - 2.925).abs() < 1e-9);
        assert!((pin1.position.x - pin1.size.x / 2.0 + 3.2).abs() < 1e-9);
        assert!((pin1.size.y - 0.25).abs() < 1e-9);
        // Pin 11 starts the right row at its bottom.
        let pin11 = &geometry.leads[10];
        assert_eq!(pin11.number, "11");
        assert!(pin11.position.x > 0.0 && (pin11.position.y + 2.925).abs() < 1e-9);
        let Some(crate::LeadForm::GullWing {
            thickness,
            foot_length,
            standoff,
        }) = geometry.lead_form
        else {
            panic!("expected a gull-wing lead form");
        };
        assert_eq!((thickness, foot_length, standoff), (0.15, 0.6, 0.1));
        assert!((geometry.body.size.z - 1.0).abs() < 1e-9);
    }

    #[test]
    fn datasheet_lead_dimensions_override_the_family_defaults() {
        let mut package = tssop20();
        package.dimensions.overall_width = Some(Dimension {
            typ: None,
            min: Some(6.2),
            max: Some(6.6),
            unit: "mm".into(),
        });
        package.dimensions.lead_width = mm(0.22);
        package.dimensions.lead_length = mm(0.5);
        package.dimensions.lead_thickness = mm(0.12);
        package.dimensions.standoff = mm(0.08);
        package.dimensions.body_height = mm(0.95);
        let geometry = generate_soic(&package).unwrap();
        let right_toe = geometry
            .leads
            .iter()
            .map(|l| l.position.x + l.size.x / 2.0)
            .fold(f64::MIN, f64::max);
        assert!(
            (right_toe - 3.2).abs() < 1e-9,
            "toe at the 6.2..6.6 midpoint / 2"
        );
        assert!(geometry
            .leads
            .iter()
            .all(|l| (l.size.y - 0.22).abs() < 1e-9));
        assert_eq!(
            geometry.lead_form,
            Some(crate::LeadForm::GullWing {
                thickness: 0.12,
                foot_length: 0.5,
                standoff: 0.08
            })
        );
        assert!((geometry.body.size.z - 0.95).abs() < 1e-9);
    }

    #[test]
    fn ssop_and_msop_have_their_own_defaults() {
        let mut ssop = tssop20();
        ssop.family = openparts_core::PackageFamily::Ssop;
        ssop.dimensions.body_width.typ = Some(5.3);
        let toe = |p: &Package| {
            let g = generate_soic(p).unwrap();
            g.leads[0].position.x - g.leads[0].size.x / 2.0
        };
        assert!((toe(&ssop) + 7.8 / 2.0).abs() < 1e-9);
        let mut msop = soic8();
        msop.family = openparts_core::PackageFamily::Msop;
        msop.pitch.typ = Some(0.65);
        msop.dimensions.body_width.typ = Some(3.0);
        msop.dimensions.body_length.typ = Some(3.0);
        assert!((toe(&msop) + 4.9 / 2.0).abs() < 1e-9);
    }

    #[test]
    fn generator_must_name_the_package_family() {
        let mut package = tssop20();
        for (generator, ok) in [("tssop", true), ("TSSOP", true), ("soic", false)] {
            package.geometry = Some(openparts_core::PackageGeometrySpec {
                geometry_type: openparts_core::GeometryType::Parametric,
                generator: Some(generator.into()),
            });
            assert_eq!(crate::generate(&package).is_ok(), ok, "{generator}");
        }
        package.geometry = None;
        package.pitch.typ = None;
        assert!(matches!(
            generate_soic(&package),
            Err(McadError::MissingDimension("pitch.typ"))
        ));
    }
}
