//! Castellated modules: a small PCB soldered onto a carrier board by the
//! plated half-holes (castellations) along its two long edges -- the
//! Raspberry Pi Pico and boards like it.
//!
//! Frame (top view, as the Pico datasheet draws it): the board is
//! `body_width` along X and `body_length` along Y, pin 1 at the top of
//! the LEFT edge. Pins 1..`lead_count / 2` run down the left edge at
//! `pitch`, the rest up the right edge (DIP numbering), both rows centred
//! on the board along Y. The footprint pads are the source's recommended
//! land pattern: `lead_width` wide (along Y), `lead_length` long (along
//! X), their outer ends at `overall_width / 2` -- so a land may reach
//! past the board edge, as the Pico's do.
//!
//! An optional extra row of pads ([`Package::aux_pads`], e.g. SWD debug
//! pads) sits `dimensions.aux_row_y` above the board's bottom edge,
//! centred `dimensions.aux_row_x` from its left edge, one pad per number
//! from left to right at `pitch`, each `aux_pad_width` x `aux_pad_length`.
//!
//! The body is the bare PCB, `body_height` thick. What only the 3D model
//! needs (plated holes, mounting holes) is resolved by [`module_features`]
//! so it follows the same value rules as the footprint.

use crate::defaults::{self, stated};
use crate::{
    Body, BodyShape, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Mounting, Point3,
    Size3,
};
use openparts_core::Package;

/// Generates Mechanical Geometry for a castellated module (see the module
/// docs). Every pad dimension is required: modules have no JEDEC outline
/// a default could come from.
pub fn generate_module(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let lead_count = package.lead_count;
    if lead_count == 0 || !lead_count.is_multiple_of(2) {
        return Err(McadError::InvalidLeadCount(lead_count));
    }

    let d = &package.dimensions;
    let pitch = defaults::dim(&package.pitch, "pitch.nominal")?;
    let body_w = defaults::dim(&d.body_width, "dimensions.body_width")?;
    let body_l = defaults::dim(&d.body_length, "dimensions.body_length")?;
    let board_t = stated(&d.body_height).unwrap_or(defaults::MODULE.board_thickness);
    let span =
        stated(&d.overall_width).ok_or(McadError::MissingDimension("dimensions.overall_width"))?;
    let pad_w =
        stated(&d.lead_width).ok_or(McadError::MissingDimension("dimensions.lead_width"))?;
    let pad_l =
        stated(&d.lead_length).ok_or(McadError::MissingDimension("dimensions.lead_length"))?;
    if pad_l <= 0.0 || span / 2.0 - pad_l <= 0.0 {
        return Err(McadError::InconsistentDimensions(format!(
            "dimensions.lead_length ({pad_l}) must be positive and leave the two pad rows of \
             overall_width ({span}) apart"
        )));
    }
    let per_side = lead_count / 2;
    let half_span = (per_side as f64 - 1.0) * pitch / 2.0;
    if half_span + pad_w / 2.0 > body_l / 2.0 {
        return Err(McadError::InconsistentDimensions(format!(
            "{per_side} pins at pitch {pitch} don't fit along body_length ({body_l})"
        )));
    }

    let pad_t = defaults::MODULE.pad_thickness;
    let pad_x = span / 2.0 - pad_l / 2.0;
    let pad = |number: u32, x: f64, y: f64| Lead {
        number: number.to_string(),
        position: Point3 {
            x,
            y,
            z: pad_t / 2.0,
        },
        size: Size3 {
            x: pad_l,
            y: pad_w,
            z: pad_t,
        },
        mounting: Mounting::Smd,
        drill: None,
        slot: None,
    };
    let mut leads: Vec<Lead> = (1..=per_side)
        .map(|n| pad(n, -pad_x, half_span - (n - 1) as f64 * pitch))
        .chain(
            (1..=per_side).map(|i| pad(per_side + i, pad_x, -half_span + (i - 1) as f64 * pitch)),
        )
        .collect();

    if !package.aux_pads.is_empty() {
        let aux_w = stated(&d.aux_pad_width)
            .ok_or(McadError::MissingDimension("dimensions.aux_pad_width"))?;
        let aux_l = stated(&d.aux_pad_length)
            .ok_or(McadError::MissingDimension("dimensions.aux_pad_length"))?;
        let row_x =
            stated(&d.aux_row_x).ok_or(McadError::MissingDimension("dimensions.aux_row_x"))?;
        let row_y =
            stated(&d.aux_row_y).ok_or(McadError::MissingDimension("dimensions.aux_row_y"))?;
        let k = package.aux_pads.len() as f64;
        for (i, number) in package.aux_pads.iter().enumerate() {
            leads.push(Lead {
                number: number.clone(),
                position: Point3 {
                    x: -body_w / 2.0 + row_x + (i as f64 - (k - 1.0) / 2.0) * pitch,
                    y: -body_l / 2.0 + row_y,
                    z: pad_t / 2.0,
                },
                size: Size3 {
                    x: aux_w,
                    y: aux_l,
                    z: pad_t,
                },
                mounting: Mounting::Smd,
                drill: None,
                slot: None,
            });
        }
    }

    let body = Body {
        position: Point3 {
            x: 0.0,
            y: 0.0,
            z: board_t / 2.0,
        },
        size: Size3 {
            x: body_w,
            y: body_l,
            z: board_t,
        },
        shape: BodyShape::Box,
    };
    // Pin 1's corner of the board, on its top face.
    let markers = vec![Marker {
        kind: MarkerKind::Pin1Dot,
        position: Point3 {
            x: -body_w / 2.0 + pitch / 2.0,
            y: half_span,
            z: board_t,
        },
    }];
    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],
        pitch: crate::defaults::required(&package.pitch),
        details: crate::Details::Module(module_features(package)?),
        body,
        leads,
        markers,
        lead_form: None,
    })
}

/// Plated holes of a module's pin rows, for the 3D model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModuleHoles {
    /// `dimensions.hole_diameter`.
    pub diameter: f64,
    /// `dimensions.row_spacing`, when stated: the through-holes inside
    /// the castellations. `None` draws the castellations only.
    pub row_spacing: Option<f64>,
}

/// Four mounting holes, a pair at each end of the board, symmetric about
/// its centre (this generator's convention; the Pico datasheets draw
/// them so).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MountingHoles {
    pub diameter: f64,
    /// Centre to centre along X.
    pub spacing: f64,
    /// Short board edge to the hole centres, along Y.
    pub inset: f64,
}

/// What a module's 3D model draws beyond its `MechanicalGeometry`: each
/// part only when the package states every dimension it needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModuleFeatures {
    pub holes: Option<ModuleHoles>,
    pub mounting_holes: Option<MountingHoles>,
}

/// Resolves the module's [`ModuleFeatures`] (the same value rules as the
/// footprint: nominal, else the min..max midpoint, else the one limit).
pub fn module_features(package: &Package) -> Result<ModuleFeatures, McadError> {
    let d = &package.dimensions;
    let body_w = defaults::dim(&d.body_width, "dimensions.body_width")?;
    let body_l = defaults::dim(&d.body_length, "dimensions.body_length")?;
    let holes = stated(&d.hole_diameter).map(|diameter| ModuleHoles {
        diameter,
        row_spacing: stated(&d.row_spacing),
    });
    if let Some(ModuleHoles {
        diameter,
        row_spacing: Some(rows),
    }) = holes
    {
        if diameter <= 0.0 || rows <= diameter || rows >= body_w {
            return Err(McadError::InconsistentDimensions(format!(
                "dimensions.row_spacing ({rows}) must lie inside the board ({body_w}) and hold \
                 holes of hole_diameter ({diameter})"
            )));
        }
    }
    let mounting_holes = match (
        stated(&d.mounting_hole_diameter),
        stated(&d.mounting_hole_spacing),
        stated(&d.mounting_hole_inset),
    ) {
        (Some(diameter), Some(spacing), Some(inset)) => {
            if spacing + diameter >= body_w || 2.0 * inset >= body_l || inset < diameter / 2.0 {
                return Err(McadError::InconsistentDimensions(format!(
                    "mounting holes (diameter {diameter}, spacing {spacing}, inset {inset}) \
                     don't fit the board ({body_w} x {body_l})"
                )));
            }
            Some(MountingHoles {
                diameter,
                spacing,
                inset,
            })
        }
        _ => None,
    };
    Ok(ModuleFeatures {
        holes,
        mounting_holes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Dimension, Kind, PackageDimensions};
    use std::collections::BTreeMap;

    fn mm(v: f64) -> Dimension {
        Dimension {
            nominal: Some(v),
            min: None,
            max: None,
            unit: "mm".into(),
        }
    }

    /// The Raspberry Pi Pico's recommended SMT footprint (Pico datasheet
    /// Figures 3 and 5).
    fn pico() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/MODULE-TEST"),
            family: openparts_core::PackageFamily::Module,
            lead_count: 40,
            pitch: mm(2.54),
            dimensions: PackageDimensions {
                body_width: mm(21.0),
                body_length: mm(51.0),
                body_height: Some(mm(1.0)),
                overall_width: Some(mm(22.58)),
                lead_width: Some(mm(1.6)),
                lead_length: Some(mm(3.2)),
                row_spacing: Some(mm(17.78)),
                hole_diameter: Some(mm(1.0)),
                aux_pad_width: Some(mm(1.6)),
                aux_pad_length: Some(mm(3.2)),
                aux_row_x: Some(mm(10.5)),
                aux_row_y: Some(mm(0.8)),
                mounting_hole_diameter: Some(Dimension {
                    nominal: Some(2.1),
                    min: Some(2.05),
                    max: Some(2.15),
                    unit: "mm".into(),
                }),
                mounting_hole_spacing: Some(mm(11.4)),
                mounting_hole_inset: Some(mm(2.0)),
                ..Default::default()
            },
            lead_layout: None,
            tab: None,
            aux_pads: vec!["D1".into(), "D2".into(), "D3".into()],
            pad_order: vec![],
            pads: vec![],
            construction: None,
            geometry: None,
            provenance: BTreeMap::new(),
            review: None,
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn lead<'a>(g: &'a MechanicalGeometry, number: &str) -> &'a Lead {
        g.leads.iter().find(|l| l.number == number).unwrap()
    }

    #[test]
    fn pins_run_down_the_left_edge_and_up_the_right_then_the_aux_row() {
        let g = generate_module(&pico()).unwrap();
        let numbers: Vec<&str> = g.leads.iter().map(|l| l.number.as_str()).collect();
        let mut expected: Vec<String> = (1..=40).map(|n| n.to_string()).collect();
        expected.extend(["D1", "D2", "D3"].map(String::from));
        assert_eq!(numbers, expected);
        // Pin 1 top-left, pin 20 bottom-left, pin 21 bottom-right, pin 40
        // top-right; 48.26 (19 x 2.54) between the end pins, centred.
        assert!(close(lead(&g, "1").position.y, 24.13));
        assert!(close(lead(&g, "20").position.y, -24.13));
        assert!(close(lead(&g, "21").position.y, -24.13));
        assert!(close(lead(&g, "40").position.y, 24.13));
        assert!(lead(&g, "1").position.x < 0.0 && lead(&g, "40").position.x > 0.0);
        assert!(close(lead(&g, "2").position.y, 24.13 - 2.54));
    }

    #[test]
    fn pads_are_the_land_pattern_reaching_past_the_board_edge() {
        let g = generate_module(&pico()).unwrap();
        let p1 = lead(&g, "1");
        // Figure 5: outer edges 22.58 apart, inner edges 16.18 apart.
        assert!(close(p1.position.x - p1.size.x / 2.0, -11.29));
        assert!(close(p1.position.x + p1.size.x / 2.0, -8.09));
        assert!(close(p1.size.y, 1.6));
        let p40 = lead(&g, "40");
        assert!(close(p40.position.x + p40.size.x / 2.0, 11.29));
        // 0.79 past the 21 mm board edge.
        assert!(close(-p1.position.x + p1.size.x / 2.0 - 10.5, 0.79));
        assert!(g.leads.iter().all(|l| l.mounting == Mounting::Smd));
        assert!(close(g.body.size.z, 1.0));
        assert!(close(g.body.size.x, 21.0) && close(g.body.size.y, 51.0));
    }

    #[test]
    fn aux_pads_sit_where_the_row_is_stated() {
        let g = generate_module(&pico()).unwrap();
        // Centred on the board (10.5 from the left edge), 2.54 apart,
        // 0.8 above the bottom edge: from 0.8 past it to 2.4 inside.
        let (d1, d2, d3) = (lead(&g, "D1"), lead(&g, "D2"), lead(&g, "D3"));
        assert!(close(d1.position.x, -2.54) && close(d2.position.x, 0.0));
        assert!(close(d3.position.x, 2.54));
        assert!(close(d2.position.y - d2.size.y / 2.0, -25.5 - 0.8));
        assert!(close(d2.position.y + d2.size.y / 2.0, -25.5 + 2.4));
        assert!(close(d1.size.x, 1.6));
        // The Pico W row: off-centre, mid-board.
        let mut p = pico();
        p.aux_pads = vec!["SWCLK".into(), "GND".into(), "SWDIO".into()];
        p.dimensions.aux_pad_width = Some(mm(1.5748));
        p.dimensions.aux_pad_length = Some(mm(1.5748));
        p.dimensions.aux_row_x = Some(mm(13.6241));
        p.dimensions.aux_row_y = Some(mm(19.8));
        let g = generate_module(&p).unwrap();
        assert!(close(lead(&g, "SWCLK").position.x, 11.0841 - 10.5));
        assert!(close(lead(&g, "SWDIO").position.x, 16.1641 - 10.5));
        assert!(close(lead(&g, "GND").position.y, 19.8 - 25.5));
    }

    #[test]
    fn missing_pad_dimensions_are_errors_not_guesses() {
        for (clear, name) in [
            (
                (|p: &mut Package| p.dimensions.overall_width = None) as fn(&mut Package),
                "dimensions.overall_width",
            ),
            (|p| p.dimensions.lead_width = None, "dimensions.lead_width"),
            (
                |p| p.dimensions.lead_length = None,
                "dimensions.lead_length",
            ),
            (|p| p.dimensions.aux_row_y = None, "dimensions.aux_row_y"),
            (
                |p| p.dimensions.aux_pad_width = None,
                "dimensions.aux_pad_width",
            ),
        ] {
            let mut p = pico();
            clear(&mut p);
            match generate_module(&p) {
                Err(McadError::MissingDimension(what)) => assert_eq!(what, name),
                other => panic!("{name}: {other:?}"),
            }
        }
        // Without aux pads their dimensions aren't needed.
        let mut p = pico();
        p.aux_pads.clear();
        p.dimensions.aux_row_y = None;
        assert_eq!(generate_module(&p).unwrap().leads.len(), 40);
        // No board thickness: the generator default, never an error.
        let mut p = pico();
        p.dimensions.body_height = None;
        let g = generate_module(&p).unwrap();
        assert!(close(g.body.size.z, defaults::MODULE.board_thickness));
    }

    #[test]
    fn rejects_odd_pin_counts_overlapping_rows_and_other_families() {
        let mut p = pico();
        p.lead_count = 39;
        assert!(matches!(
            generate_module(&p),
            Err(McadError::InvalidLeadCount(39))
        ));
        let mut p = pico();
        p.dimensions.lead_length = Some(mm(11.3));
        assert!(matches!(
            generate_module(&p),
            Err(McadError::InconsistentDimensions(_))
        ));
        assert!(crate::generate(&pico()).is_ok(), "dispatched by family");
    }

    #[test]
    fn features_come_from_the_stated_holes() {
        let f = module_features(&pico()).unwrap();
        assert_eq!(
            f.holes,
            Some(ModuleHoles {
                diameter: 1.0,
                row_spacing: Some(17.78)
            })
        );
        assert_eq!(
            f.mounting_holes,
            Some(MountingHoles {
                diameter: 2.1,
                spacing: 11.4,
                inset: 2.0
            })
        );
        // Any mounting-hole dimension missing: no mounting holes drawn.
        let mut p = pico();
        p.dimensions.mounting_hole_inset = None;
        p.dimensions.hole_diameter = None;
        let f = module_features(&p).unwrap();
        assert_eq!(f.mounting_holes, None);
        assert_eq!(f.holes, None);
        let mut p = pico();
        p.dimensions.row_spacing = Some(mm(22.0));
        assert!(matches!(
            module_features(&p),
            Err(McadError::InconsistentDimensions(_))
        ));
    }
}
