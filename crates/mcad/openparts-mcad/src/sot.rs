use crate::{Body, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Point3, Size3};
use openparts_core::Package;

/// Engineering defaults used only when the datasheet doesn't supply a
/// value -- generator implementation details for a geometric
/// approximation, never written back into Canonical Data as a fact
/// (same policy as this crate's other generators' defaults). SOT bodies
/// and leads are much smaller than LQFP/SOIC's, so these are distinct,
/// smaller constants.
const DEFAULT_BODY_HEIGHT_MM: f64 = 1.0;
const DEFAULT_LEAD_WIDTH_MM: f64 = 0.4;
const DEFAULT_LEAD_PROTRUSION_MM: f64 = 0.45;
const DEFAULT_LEAD_HEIGHT_MM: f64 = 0.1;
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
/// left-to-right, continuing the numbering.
pub fn generate_sot(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if !package.family.eq_ignore_ascii_case("sot") {
        return Err(McadError::UnsupportedFamily(package.family.clone()));
    }
    if let Some(geometry) = &package.geometry {
        if let Some(generator) = &geometry.generator {
            if !generator.eq_ignore_ascii_case("sot") {
                return Err(McadError::UnsupportedGenerator(generator.clone()));
            }
        }
    }

    let layout = package
        .lead_layout
        .clone()
        .ok_or_else(|| McadError::MissingLeadLayout {
            family: package.family.clone(),
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

    let pitch = package
        .pitch
        .nominal
        .ok_or(McadError::MissingDimension("pitch.nominal"))?;
    let body_w = package
        .dimensions
        .body_width
        .nominal
        .ok_or(McadError::MissingDimension("dimensions.body_width.nominal"))?;
    let body_l = package
        .dimensions
        .body_length
        .nominal
        .ok_or(McadError::MissingDimension(
            "dimensions.body_length.nominal",
        ))?;
    let body_h = package
        .dimensions
        .body_height
        .as_ref()
        .and_then(|d| d.nominal)
        .unwrap_or(DEFAULT_BODY_HEIGHT_MM);

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
    };

    let lead_size = Size3 {
        x: DEFAULT_LEAD_WIDTH_MM,
        y: DEFAULT_LEAD_PROTRUSION_MM,
        z: DEFAULT_LEAD_HEIGHT_MM,
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
                y: -(body_l / 2.0 + DEFAULT_LEAD_PROTRUSION_MM / 2.0),
                z: DEFAULT_LEAD_HEIGHT_MM / 2.0,
            },
            size: lead_size,
        });
        pin_num += 1;
    }
    // Top edge: y = +body_l/2 + protrusion/2, x centered, left to right --
    // numbering continues directly from the bottom edge.
    let top_span = (top_count as f64 - 1.0) * pitch;
    let top_half_span = top_span / 2.0;
    for i in 0..top_count {
        let x = -top_half_span + i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x,
                y: body_l / 2.0 + DEFAULT_LEAD_PROTRUSION_MM / 2.0,
                z: DEFAULT_LEAD_HEIGHT_MM / 2.0,
            },
            size: lead_size,
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
        body,
        leads,
        markers,
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
            family: "sot".into(),
            lead_count: 3,
            pitch: Dimension {
                nominal: Some(0.95),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    nominal: Some(1.3),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    nominal: Some(2.9),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_height: None,
                exposed_pad: None,
            },
            lead_layout: Some(vec![2, 1]),
            geometry: None,
            provenance: BTreeMap::new(),
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
        package.dimensions.body_width.nominal = None;
        let err = generate_sot(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }

    #[test]
    fn rejects_non_sot_family() {
        let mut package = sot23_3();
        package.family = "soic".into();
        let err = generate_sot(&package).unwrap_err();
        assert!(matches!(err, McadError::UnsupportedFamily(_)));
    }
}
