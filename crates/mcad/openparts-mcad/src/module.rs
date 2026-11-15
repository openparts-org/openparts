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
    if !package.pad_rows.is_empty() {
        return generate_pad_rows(package);
    }
    let lead_count = package.lead_count;
    if lead_count == 0 || !lead_count.is_multiple_of(2) {
        return Err(McadError::InvalidLeadCount(lead_count));
    }

    let d = &package.dimensions;
    let pitch = defaults::dim(&package.pitch, "pitch.typ")?;
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
    /// The shield can (`Package::shield`), when stated.
    pub shield: Option<ModuleShield>,
    /// The pads come from `Package::pad_rows` / `pad_grids` (an
    /// ESP32-style module): castellations where a pad reaches the board
    /// edge, LGA pads elsewhere, all on the board's underside.
    pub pad_rows: bool,
}

/// A module's shield can: a box on the board's top face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModuleShield {
    /// Centre in the package frame.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub length: f64,
    /// Seating plane to the can's top (`dimensions.overall_height`).
    pub top: f64,
}

/// Mechanical Geometry for a module whose pads are `pad_rows` and
/// `pad_grids` (section 20.4): each row's pads along its board edge at
/// its own pitch and start, set in (or out) by `inset`; each grid's pads
/// about its centre. Every pad is SMD, the body is the bare board.
fn generate_pad_rows(package: &Package) -> Result<MechanicalGeometry, McadError> {
    use openparts_core::PackageSide as S;
    let d = &package.dimensions;
    let body_w = defaults::dim(&d.body_width, "dimensions.body_width")?;
    let body_l = defaults::dim(&d.body_length, "dimensions.body_length")?;
    let board_t = stated(&d.body_height).unwrap_or(defaults::MODULE.board_thickness);
    let pad_t = defaults::MODULE.pad_thickness;
    let (hw, hl) = (body_w / 2.0, body_l / 2.0);
    let pad = |number: String, x: f64, y: f64, sx: f64, sy: f64| Lead {
        number,
        position: Point3 {
            x,
            y,
            z: pad_t / 2.0,
        },
        size: Size3 {
            x: sx,
            y: sy,
            z: pad_t,
        },
        mounting: Mounting::Smd,
        drill: None,
        slot: None,
    };
    let mut leads = Vec::new();
    for row in &package.pad_rows {
        let [along, across] = row.pad;
        for i in 0..row.count {
            let s = row.start + f64::from(i) * row.pitch;
            let number = (row.first + i).to_string();
            leads.push(match row.side {
                S::MinusX => pad(number, -hw + row.inset, hl - s, across, along),
                S::MinusY => pad(number, -hw + s, -hl + row.inset, along, across),
                S::PlusX => pad(number, hw - row.inset, -hl + s, across, along),
                S::PlusY => pad(number, hw - s, hl - row.inset, along, across),
            });
        }
    }
    for grid in &package.pad_grids {
        let n = (grid.columns * grid.rows) as usize;
        if !(grid.numbers.len() == 1 || grid.numbers.len() == n) {
            return Err(McadError::InconsistentDimensions(format!(
                "a pad grid of {n} pads has {} numbers (one, or one per pad)",
                grid.numbers.len()
            )));
        }
        for r in 0..grid.rows {
            for c in 0..grid.columns {
                let k = (r * grid.columns + c) as usize;
                let number = grid.numbers[if grid.numbers.len() == 1 { 0 } else { k }].clone();
                let x = grid.x + (f64::from(c) - f64::from(grid.columns - 1) / 2.0) * grid.pitch[0];
                let y = grid.y + (f64::from(grid.rows - 1) / 2.0 - f64::from(r)) * grid.pitch[1];
                leads.push(pad(number, x, y, grid.pad[0], grid.pad[1]));
            }
        }
    }
    let distinct: std::collections::BTreeSet<&str> =
        leads.iter().map(|l| l.number.as_str()).collect();
    if distinct.len() != package.lead_count as usize {
        return Err(McadError::InconsistentDimensions(format!(
            "pad_rows and pad_grids give {} pad numbers for lead_count {}",
            distinct.len(),
            package.lead_count
        )));
    }
    for l in &leads {
        let inside = |c: f64, s: f64, h: f64| c - s / 2.0 < h && c + s / 2.0 > -h;
        if !inside(l.position.x, l.size.x, hw) || !inside(l.position.y, l.size.y, hl) {
            return Err(McadError::InconsistentDimensions(format!(
                "pad \"{}\" lies off the {body_w} x {body_l} board",
                l.number
            )));
        }
    }
    let pin1 = leads
        .iter()
        .find(|l| l.number == "1")
        .unwrap_or(&leads[0])
        .position;
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
    let markers = vec![Marker {
        kind: MarkerKind::Pin1Dot,
        position: Point3 {
            x: pin1.x,
            y: pin1.y,
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
    let shield = match &package.shield {
        None => None,
        Some(r) => {
            let top = stated(&d.overall_height)
                .ok_or(McadError::MissingDimension("dimensions.overall_height"))?;
            let board_t = stated(&d.body_height).unwrap_or(defaults::MODULE.board_thickness);
            if r.width <= 0.0
                || r.length <= 0.0
                || top <= board_t
                || (r.x.abs() + r.width / 2.0) > body_w / 2.0 + 1e-9
                || (r.y.abs() + r.length / 2.0) > body_l / 2.0 + 1e-9
            {
                return Err(McadError::InconsistentDimensions(format!(
                    "the shield ({} x {} at {}, {}, top {top}) must stand on the {body_w} x \
                     {body_l} x {board_t} board",
                    r.width, r.length, r.x, r.y
                )));
            }
            Some(ModuleShield {
                x: r.x,
                y: r.y,
                width: r.width,
                length: r.length,
                top,
            })
        }
    };
    Ok(ModuleFeatures {
        holes,
        mounting_holes,
        shield,
        pad_rows: !package.pad_rows.is_empty(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Dimension, Kind, PackageDimensions};
    use std::collections::BTreeMap;

    fn mm(v: f64) -> Dimension {
        Dimension {
            typ: Some(v),
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
                    typ: Some(2.1),
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

    /// ESP32-WROOM-32E's recommended land pattern (rows on three edges,
    /// a 3 x 3 ground grid numbered 39).
    fn wroom() -> Package {
        serde_yaml::from_str(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: module\n\
             lead_count: 39\npitch: {nominal: 1.27, unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {nominal: 18, unit: mm}\n\
             \x20 body_length: {nominal: 25.5, unit: mm}\n\
             \x20 body_height: {nominal: 0.8, unit: mm}\n\
             \x20 overall_height: {nominal: 3.1, unit: mm}\n\
             pad_rows:\n\
             \x20 - {side: -x, first: 1, count: 14, pitch: 1.27, start: 7.49, inset: 0.25, pad: [0.9, 1.5]}\n\
             \x20 - {side: -y, first: 15, count: 10, pitch: 1.27, start: 3.285, inset: 0.25, pad: [0.9, 1.5]}\n\
             \x20 - {side: +x, first: 25, count: 14, pitch: 1.27, start: 1.5, inset: 0.25, pad: [0.9, 1.5]}\n\
             pad_grids:\n\
             \x20 - {numbers: ['39'], columns: 3, rows: 3, pitch: [1.4, 1.4], pad: [0.9, 0.9], x: -1.5, y: -2.46}\n\
             shield: {x: 0, y: -2.9, width: 15.8, length: 17.6}\n",
        )
        .unwrap()
    }

    #[test]
    fn pad_rows_run_counter_clockwise_with_their_own_start_and_pitch() {
        let g = generate_module(&wroom()).unwrap();
        assert_eq!(g.leads.len(), 38 + 9);
        // Pin 1: 7.49 below the top edge, its centre 0.25 inside the left
        // edge, the 1.5 pad reaching 0.5 past it.
        let p1 = lead(&g, "1");
        assert!(close(p1.position.x, -8.75) && close(p1.position.y, 12.75 - 7.49));
        assert!(close(p1.size.x, 1.5) && close(p1.size.y, 0.9));
        assert!(close(p1.position.x - p1.size.x / 2.0, -9.5));
        let p14 = lead(&g, "14");
        assert!(close(p14.position.y, -12.75 + 1.5));
        // The bottom row, left to right, 3.285 from the left edge.
        let (p15, p24) = (lead(&g, "15"), lead(&g, "24"));
        assert!(close(p15.position.x, -9.0 + 3.285) && close(p24.position.x, 9.0 - 3.285));
        assert!(close(p15.position.y, -12.5) && close(p15.size.y, 1.5));
        // The right row, bottom to top.
        let (p25, p38) = (lead(&g, "25"), lead(&g, "38"));
        assert!(close(p25.position.y, -11.25) && close(p38.position.y, 12.75 - 7.49));
        assert!(close(p25.position.x, 8.75));
        // The ground grid: nine pads, one number, 1.4 apart about its centre.
        let ground: Vec<&Lead> = g.leads.iter().filter(|l| l.number == "39").collect();
        assert_eq!(ground.len(), 9);
        assert!(close(ground[0].position.x, -2.9) && close(ground[0].position.y, -1.06));
        let crate::Details::Module(f) = g.details else {
            panic!()
        };
        assert!(f.pad_rows);
        assert_eq!(
            f.shield,
            Some(ModuleShield {
                x: 0.0,
                y: -2.9,
                width: 15.8,
                length: 17.6,
                top: 3.1
            })
        );
    }

    #[test]
    fn pad_rows_must_match_lead_count_and_stay_on_the_board() {
        let mut p = wroom();
        p.lead_count = 38;
        assert!(matches!(
            generate_module(&p),
            Err(McadError::InconsistentDimensions(_))
        ));
        let mut p = wroom();
        p.pad_rows[0].start = 30.0;
        assert!(generate_module(&p).is_err());
        let mut p = wroom();
        p.dimensions.overall_height = None;
        assert!(matches!(
            generate_module(&p),
            Err(McadError::MissingDimension("dimensions.overall_height"))
        ));
    }
}
