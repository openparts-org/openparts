use crate::defaults::{self, Axis, GullWing};
use crate::{
    Body, BodyShape, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Mounting, Point3,
    Size3,
};
use openparts_core::Package;

/// Kept well inside even a small SOT-23-class body (~1.3 x 2.9mm) --
/// unlike `lqfp.rs`/`soic.rs`'s 0.5mm marker offset, which can exceed
/// half of a SOT body's own dimensions.
const MARKER_OFFSET_MM: f64 = 0.2;

/// Generates Mechanical Geometry for a SOT-style package: a rectangular
/// body with leads on 2 opposite (bottom/top) sides, split
/// asymmetrically per [`Package::lead_layout`] (e.g. SOT-23's 3 leads =
/// 2 on the bottom edge, 1 centered on the top edge) rather than a
/// `lead_count`-derived even split -- see `lead_layout`'s docs for why
/// this family can't safely default to a guessed convention.
///
/// Numbering convention (this generator's own choice, since no single
/// vendor convention is universal -- see `lead_layout`'s docs): pin 1 at
/// the bottom-left, `lead_layout[0]` pins along the bottom edge
/// left-to-right, then `lead_layout[1]` pins along the top edge
/// right-to-left (counter-clockwise, as SOT-23-5/6 datasheets number
/// them), on the bottom row's position grid
/// left-to-right, continuing the numbering.
pub fn generate_sot(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let layout = package
        .lead_layout
        .clone()
        .ok_or(McadError::MissingLeadLayout {
            family: package.family,
        })?;
    let invalid = || McadError::InvalidLeadLayout {
        layout: layout.clone(),
        lead_count: package.lead_count,
    };
    if layout.len() != 2 || layout.contains(&0) {
        return Err(invalid());
    }
    let (bottom_count, top_count) = (layout[0], layout[1]);
    if bottom_count + top_count != package.lead_count {
        return Err(invalid());
    }

    let pitch = defaults::dim(&package.pitch, "pitch.typ")?;
    let body_w = defaults::dim(&package.dimensions.body_width, "dimensions.body_width.typ")?;
    let body_l = defaults::dim(
        &package.dimensions.body_length,
        "dimensions.body_length.typ",
    )?;
    let shape = GullWing::resolve(package, &defaults::SOT);
    let body_h = shape.body_height;
    // Both rows leave across `body_length`, so their toes land at
    // `overall_length / 2` (HE / 2 on SOT-23 drawings).
    let reach = shape.protrusion(package, Axis::Y)?;
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

    let lead_size = Size3 {
        x: shape.lead_width,
        y: reach,
        z: lead_t,
    };
    let mut leads = Vec::with_capacity(package.lead_count as usize);
    let mut pin_num = 1u32;

    // Bottom edge: y = -body_l/2 - protrusion/2, x centered, left to right.
    let bottom_span = (bottom_count as f64 - 1.0) * pitch;
    let bottom_half_span = bottom_span / 2.0;
    for i in 0..bottom_count {
        let x = -bottom_half_span + i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x,
                y: -(body_l / 2.0 + reach / 2.0),
                z: lead_t / 2.0,
            },
            size: lead_size,
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
        });
        pin_num += 1;
    }
    // Top edge: y = +body_l/2 + protrusion/2. Numbering continues
    // counter-clockwise (right to left), and the top leads sit on the
    // bottom row's position grid, as on real SOT-23-5/6 outlines (pin 4
    // above pin 3, pin 5 above pin 1; e1 apart, not e):
    // - one top lead (SOT-23-3 [2, 1]): centred;
    // - as many as the bottom row (SOT-23-6 [3, 3]): every position;
    // - [3, 2] (SOT-23-5): the two outer positions.
    let bottom_xs: Vec<f64> = (0..bottom_count)
        .map(|i| -bottom_half_span + i as f64 * pitch)
        .collect();
    let top_xs: Vec<f64> = match (bottom_count, top_count) {
        (_, 1) => vec![0.0],
        (b, t) if b == t => bottom_xs.iter().rev().copied().collect(),
        (3, 2) => vec![bottom_xs[2], bottom_xs[0]],
        _ => {
            return Err(McadError::InvalidLeadLayout {
                layout: layout.clone(),
                lead_count: package.lead_count,
            })
        }
    };
    for x in top_xs {
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x,
                y: body_l / 2.0 + reach / 2.0,
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
            x: -body_w / 2.0 + MARKER_OFFSET_MM,
            y: -body_l / 2.0 + MARKER_OFFSET_MM,
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

    fn sot23_3() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/SOT23-TEST"),
            family: openparts_core::PackageFamily::Sot,
            lead_count: 3,
            pitch: Dimension {
                typ: Some(0.95),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    typ: Some(1.3),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    typ: Some(2.9),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                ..Default::default()
            },
            lead_layout: Some(vec![2, 1]),
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
        let geometry = generate_sot(&sot23_3()).unwrap();
        assert_eq!(geometry.leads.len(), 3);
        let numbers: Vec<&str> = geometry.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, vec!["1", "2", "3"]);
    }

    #[test]
    fn two_pins_on_bottom_one_centered_on_top() {
        let geometry = generate_sot(&sot23_3()).unwrap();
        let bottom: Vec<&Lead> = geometry
            .leads
            .iter()
            .filter(|l| l.position.y < 0.0)
            .collect();
        let top: Vec<&Lead> = geometry
            .leads
            .iter()
            .filter(|l| l.position.y > 0.0)
            .collect();
        assert_eq!(bottom.len(), 2);
        assert_eq!(top.len(), 1);
        assert!(
            (top[0].position.x).abs() < 1e-9,
            "lone top pin should be centered"
        );
        assert_eq!(bottom[0].number, "1");
        assert_eq!(bottom[1].number, "2");
        assert_eq!(top[0].number, "3");
    }

    fn sot_with(layout: Vec<u32>, pitch: f64) -> Package {
        let mut p = sot23_3();
        p.lead_count = layout.iter().sum();
        p.lead_layout = Some(layout);
        p.pitch.typ = Some(pitch);
        p
    }

    fn x_of(g: &MechanicalGeometry, n: &str) -> f64 {
        g.leads.iter().find(|l| l.number == n).unwrap().position.x
    }

    /// SOT-23-5 (e.g. Microchip C04-091-OT, Diodes AP2112K): pins 1-3
    /// along the bottom at e = 0.95, pin 4 above pin 3 and pin 5 above
    /// pin 1 -- the top pair is e1 = 1.90 apart, numbered right to left.
    #[test]
    fn sot23_5_numbers_counter_clockwise_with_the_top_pair_over_pins_3_and_1() {
        let g = generate_sot(&sot_with(vec![3, 2], 0.95)).unwrap();
        assert!((x_of(&g, "4") - x_of(&g, "3")).abs() < 1e-9);
        assert!((x_of(&g, "5") - x_of(&g, "1")).abs() < 1e-9);
        assert!((x_of(&g, "4") - x_of(&g, "5") - 1.90).abs() < 1e-9);
    }

    /// SOT-23-6: pin 4 above 3, 5 above 2, 6 above 1.
    #[test]
    fn sot23_6_top_row_mirrors_the_bottom_row() {
        let g = generate_sot(&sot_with(vec![3, 3], 0.95)).unwrap();
        for (top, bottom) in [("4", "3"), ("5", "2"), ("6", "1")] {
            assert!(
                (x_of(&g, top) - x_of(&g, bottom)).abs() < 1e-9,
                "pin {top} over {bottom}"
            );
        }
    }

    #[test]
    fn an_unknown_top_row_split_is_rejected_not_guessed() {
        assert!(matches!(
            generate_sot(&sot_with(vec![4, 2], 0.65)),
            Err(McadError::InvalidLeadLayout { .. })
        ));
    }

    #[test]
    fn missing_lead_layout_is_an_error_not_a_guessed_default() {
        let mut package = sot23_3();
        package.lead_layout = None;
        let err = generate_sot(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingLeadLayout { .. }));
    }

    #[test]
    fn lead_layout_not_summing_to_lead_count_is_rejected() {
        let mut package = sot23_3();
        package.lead_layout = Some(vec![2, 2]); // sums to 4, lead_count is 3
        let err = generate_sot(&package).unwrap_err();
        assert!(matches!(err, McadError::InvalidLeadLayout { .. }));
    }

    #[test]
    fn missing_nominal_is_an_error_not_an_invented_value() {
        let mut package = sot23_3();
        package.dimensions.body_width.typ = None;
        let err = generate_sot(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }
}
