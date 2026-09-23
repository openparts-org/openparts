//! openparts-step: Mechanical Geometry -> STEP text, via `truck`
//! (`openparts-brep` builds the real B-rep solids; `truck-stepio`
//! serializes them). Geometry itself -- point/edge/face authoring, and
//! for a `BodyShape::Cylinder` body a true curved surface, not a
//! tessellated approximation -- is entirely `truck`'s responsibility
//! now; this crate's own hand-written STEP text is limited to
//! re-attaching per-body/per-lead color, which `truck-stepio` has no
//! support for at all (confirmed by reading its source: no
//! `STYLED_ITEM`/`COLOUR_RGB` anywhere in `truck-stepio` or
//! `truck-modeling`).
//!
//! NOTE (Testing and Quality Specification section 9): this crate has
//! no independent STEP reader of its own, but every generated file is
//! round-tripped through `ruststep::parser::parse` before being
//! returned, which is a real (if EXPRESS-syntax-level, not
//! B-rep-semantic-level) read-back check -- stronger than the old
//! hand-rolled writer had.

use openparts_brep::BrepGeometry;
use openparts_mcad::MechanicalGeometry;
use truck_modeling::{Curve, Point3, Solid, Surface};
use truck_stepio::out;
use truck_topology::compress::CompressedSolid;

#[derive(Debug, thiserror::Error)]
pub enum StepError {
    #[error("geometry has no solids to export (no body?)")]
    Empty,
}

/// Dark charcoal, matching real epoxy mold compound IC body resin.
const BODY_COLOR: [f64; 3] = [0.15, 0.15, 0.15];
/// Metallic silver, matching exposed leads/pads.
const LEAD_COLOR: [f64; 3] = [0.75, 0.75, 0.75];

/// Renders `geometry` (one solid per body + lead) as a complete STEP
/// file (`ISO-10303-042`/AP242, `truck-stepio`'s own default schema --
/// more modern than this crate's old hand-written AP214) named after
/// `product_name`.
pub fn generate_step(
    geometry: &MechanicalGeometry,
    product_name: &str,
) -> Result<String, StepError> {
    let BrepGeometry { body, leads } = openparts_brep::build(geometry);

    let mut solids: Vec<Solid> = Vec::with_capacity(1 + leads.len());
    solids.push(body);
    solids.extend(leads);
    if solids.is_empty() {
        return Err(StepError::Empty);
    }

    let compressed: Vec<CompressedSolid<Point3, Curve, Surface>> =
        solids.iter().map(|s| s.compress()).collect();
    let mut models = out::StepModels::default();
    for solid in &compressed {
        models.push_solid(solid);
    }

    let step_text = out::CompleteStepDisplay::new(
        models,
        out::StepHeaderDescriptor {
            file_name: format!("{product_name}.step"),
            organization_system: "OpenParts STEP Exporter".to_owned(),
            ..Default::default()
        },
    )
    .to_string();

    // solids[0] is the body (BODY_COLOR); solids[1..] are leads
    // (LEAD_COLOR) -- same push order used to build `models` above.
    let colors =
        std::iter::once(BODY_COLOR).chain(std::iter::repeat_n(LEAD_COLOR, solids.len() - 1));
    let step_text = restyle(&step_text, colors);

    ruststep::parser::parse(&step_text)
        .expect("truck-stepio output plus this crate's own appended color entities must be valid EXPRESS -- a parse failure here is a bug in `restyle`, not a caller error");

    Ok(step_text)
}

/// Appends a `STYLED_ITEM`/`COLOUR_RGB` chain for each
/// `MANIFOLD_SOLID_BREP` entity `truck-stepio` wrote, in the order they
/// appear in `step_text` (which matches solid push order, since we
/// control it). `truck-stepio` has no color support at all, so this is
/// the one piece of hand-written STEP text this crate still produces --
/// the same entity chain (`COLOUR_RGB` -> `FILL_AREA_STYLE_COLOUR` ->
/// `FILL_AREA_STYLE` -> `SURFACE_STYLE_FILL_AREA` -> `SURFACE_SIDE_STYLE`
/// -> `SURFACE_STYLE_USAGE` -> `PRESENTATION_STYLE_ASSIGNMENT` ->
/// `STYLED_ITEM`) this crate's hand-rolled writer used before this
/// migration, just retargeted at truck-authored entity ids instead of
/// self-authored ones.
fn restyle(step_text: &str, colors: impl Iterator<Item = [f64; 3]>) -> String {
    let mut brep_ids = Vec::new();
    let mut max_id: u32 = 0;
    for line in step_text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix('#') else {
            continue;
        };
        let Some(eq) = rest.find(" = ") else {
            continue;
        };
        let Ok(id) = rest[..eq].parse::<u32>() else {
            continue;
        };
        max_id = max_id.max(id);
        if rest[eq + 3..].starts_with("MANIFOLD_SOLID_BREP(") {
            brep_ids.push(id);
        }
    }

    let mut next_id = max_id + 1;
    let mut alloc = || {
        let id = next_id;
        next_id += 1;
        id
    };

    let mut new_lines = String::new();
    for (target_id, rgb) in brep_ids.into_iter().zip(colors) {
        let colour_id = alloc();
        new_lines.push_str(&format!(
            "#{colour_id} = COLOUR_RGB('', {:.6}, {:.6}, {:.6});\n",
            rgb[0], rgb[1], rgb[2]
        ));
        let fill_colour_id = alloc();
        new_lines.push_str(&format!(
            "#{fill_colour_id} = FILL_AREA_STYLE_COLOUR('', #{colour_id});\n"
        ));
        let fill_area_style_id = alloc();
        new_lines.push_str(&format!(
            "#{fill_area_style_id} = FILL_AREA_STYLE('', (#{fill_colour_id}));\n"
        ));
        let surface_fill_id = alloc();
        new_lines.push_str(&format!(
            "#{surface_fill_id} = SURFACE_STYLE_FILL_AREA(#{fill_area_style_id});\n"
        ));
        let side_style_id = alloc();
        new_lines.push_str(&format!(
            "#{side_style_id} = SURFACE_SIDE_STYLE('', (#{surface_fill_id}));\n"
        ));
        let usage_id = alloc();
        new_lines.push_str(&format!(
            "#{usage_id} = SURFACE_STYLE_USAGE(.BOTH., #{side_style_id});\n"
        ));
        let assignment_id = alloc();
        new_lines.push_str(&format!(
            "#{assignment_id} = PRESENTATION_STYLE_ASSIGNMENT((#{usage_id}));\n"
        ));
        let styled_item_id = alloc();
        new_lines.push_str(&format!(
            "#{styled_item_id} = STYLED_ITEM('', (#{assignment_id}), #{target_id});\n"
        ));
    }

    let marker = "ENDSEC;\nEND-ISO-10303-21;";
    let insert_at = step_text
        .rfind(marker)
        .expect("truck-stepio's CompleteStepDisplay always ends with ENDSEC;/END-ISO-10303-21;");
    let mut result = String::with_capacity(step_text.len() + new_lines.len());
    result.push_str(&step_text[..insert_at]);
    result.push_str(&new_lines);
    result.push_str(&step_text[insert_at..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_mcad::{
        Body, BodyShape, Lead, Marker, MarkerKind, Mounting, Point3 as McadPoint3, Size3,
    };

    fn tiny_geometry() -> MechanicalGeometry {
        MechanicalGeometry {
            body: Body {
                position: McadPoint3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.5,
                },
                size: Size3 {
                    x: 3.0,
                    y: 3.0,
                    z: 1.0,
                },
                shape: BodyShape::Box,
            },
            leads: vec![
                Lead {
                    number: "1".into(),
                    position: McadPoint3 {
                        x: -2.0,
                        y: 0.0,
                        z: 0.05,
                    },
                    size: Size3 {
                        x: 0.6,
                        y: 0.2,
                        z: 0.1,
                    },
                    mounting: Mounting::Smd,
                    drill: None,
                },
                Lead {
                    number: "2".into(),
                    position: McadPoint3 {
                        x: 2.0,
                        y: 0.0,
                        z: 0.05,
                    },
                    size: Size3 {
                        x: 0.6,
                        y: 0.2,
                        z: 0.1,
                    },
                    mounting: Mounting::Smd,
                    drill: None,
                },
            ],
            markers: vec![Marker {
                kind: MarkerKind::Pin1Dot,
                position: McadPoint3 {
                    x: -1.0,
                    y: 1.0,
                    z: 1.0,
                },
            }],
        }
    }

    fn tiny_cylinder_geometry() -> MechanicalGeometry {
        MechanicalGeometry {
            body: Body {
                position: McadPoint3 {
                    x: 0.0,
                    y: 0.0,
                    z: 5.5,
                },
                size: Size3 {
                    x: 5.0,
                    y: 5.0,
                    z: 11.0,
                },
                shape: BodyShape::Cylinder,
            },
            leads: vec![
                Lead {
                    number: "1".into(),
                    position: McadPoint3 {
                        x: -1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    size: Size3 {
                        x: 1.6,
                        y: 1.6,
                        z: 3.0,
                    },
                    mounting: Mounting::ThroughHole,
                    drill: Some(0.8),
                },
                Lead {
                    number: "2".into(),
                    position: McadPoint3 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    size: Size3 {
                        x: 1.6,
                        y: 1.6,
                        z: 3.0,
                    },
                    mounting: Mounting::ThroughHole,
                    drill: Some(0.8),
                },
            ],
            markers: vec![Marker {
                kind: MarkerKind::NegativeStripe,
                position: McadPoint3 {
                    x: 1.0,
                    y: 0.0,
                    z: 11.0,
                },
            }],
        }
    }

    #[test]
    fn produces_a_well_formed_step_file() {
        let step = generate_step(&tiny_geometry(), "TEST").unwrap();
        assert!(step.starts_with("ISO-10303-21;\n"));
        assert!(step.trim_end().ends_with("END-ISO-10303-21;"));
        assert!(step.contains("HEADER;"));
        assert!(step.contains("DATA;"));
    }

    #[test]
    fn parses_as_valid_express() {
        let step = generate_step(&tiny_geometry(), "TEST").unwrap();
        ruststep::parser::parse(&step).expect("must be valid EXPRESS");
    }

    #[test]
    fn has_one_manifold_solid_brep_per_solid() {
        let step = generate_step(&tiny_geometry(), "TEST").unwrap();
        let brep_count = step.matches("MANIFOLD_SOLID_BREP(").count();
        // 1 body + 2 leads = 3 solids.
        assert_eq!(brep_count, 3);
    }

    #[test]
    fn cylinder_body_produces_one_brep_per_solid_too() {
        let step = generate_step(&tiny_cylinder_geometry(), "TEST").unwrap();
        let brep_count = step.matches("MANIFOLD_SOLID_BREP(").count();
        assert_eq!(brep_count, 3);
    }

    #[test]
    fn every_solid_has_a_styled_color() {
        let step = generate_step(&tiny_geometry(), "TEST").unwrap();
        let styled_item_count = step.matches("STYLED_ITEM(").count();
        let colour_count = step.matches("COLOUR_RGB(").count();
        // 1 body + 2 leads = 3 solids, one STYLED_ITEM/COLOUR_RGB pair each.
        assert_eq!(styled_item_count, 3);
        assert_eq!(colour_count, 3);
        assert!(step.contains(&format!(
            "COLOUR_RGB('', {:.6}, {:.6}, {:.6})",
            BODY_COLOR[0], BODY_COLOR[1], BODY_COLOR[2]
        )));
        assert!(step.contains(&format!(
            "COLOUR_RGB('', {:.6}, {:.6}, {:.6})",
            LEAD_COLOR[0], LEAD_COLOR[1], LEAD_COLOR[2]
        )));
    }

    #[test]
    fn cylinder_body_also_gets_colored() {
        let step = generate_step(&tiny_cylinder_geometry(), "TEST").unwrap();
        let styled_item_count = step.matches("STYLED_ITEM(").count();
        let colour_count = step.matches("COLOUR_RGB(").count();
        assert_eq!(styled_item_count, 3);
        assert_eq!(colour_count, 3);
    }

    #[test]
    fn appended_color_entities_still_parse_as_valid_express() {
        // Specifically exercises `restyle`'s id-collision avoidance and
        // splice-before-ENDSEC logic, since this is now the one piece
        // of hand-written STEP text in this crate.
        let step = generate_step(&tiny_cylinder_geometry(), "TEST").unwrap();
        ruststep::parser::parse(&step).expect("spliced-in color entities must parse");
    }
}
