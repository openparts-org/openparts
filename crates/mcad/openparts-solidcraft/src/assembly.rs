//! Module assemblies (Canonical Data Specification section 20.12): a
//! board's model with the components mounted on it.
//!
//! Each distinct component is built from its own script and exported to
//! STEP first (in parallel, each its own SolidCraft run); the board's
//! script then inserts every placement of it with `file.insert_step` and
//! a rigid matrix, so components keep their own colours and their
//! parameter and body names never clash with the board's. A placeholder
//! box is drawn in the board's script and moved into place. The exports
//! (STEP, STL, 3MF, the `.sc` design) are written from that last run:
//! the design holds the imported bodies, not links to the temporary
//! STEP files.

use crate::runner::{self, Format, RunError};
use crate::script::{looks, mm, Script};
use crate::{Error, ModelInput};
use serde_json::json;

/// What is mounted at one place.
#[derive(Debug, Clone)]
pub enum Mounted<'a> {
    /// A component's own detailed model. Placements sharing a `key` (the
    /// part id) are built once.
    Model { key: String, input: ModelInput<'a> },
    /// A placeholder box, `size` = [X, Y, Z] in mm, standing on z = 0
    /// centred on the origin; `color` `#rrggbb`, else a neutral grey.
    Box {
        size: [f64; 3],
        color: Option<String>,
    },
}

/// One component on the board.
#[derive(Debug, Clone)]
pub struct Placed<'a> {
    /// The reference designator (the component's name in the model).
    pub name: String,
    pub mounted: Mounted<'a>,
    /// Rigid row-major 3x4 transform from the component's model frame
    /// to the board's.
    pub matrix: [f64; 12],
}

fn safe(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if s.is_empty() {
        "C".into()
    } else {
        s
    }
}

fn matrix_value(m: &[f64; 12]) -> serde_json::Value {
    // Full precision: rounding a 45° rotation breaks SolidCraft's
    // rigidity check (|RᵀR − I| <= 1e-9).
    json!(m
        .iter()
        .map(|v| if *v == 0.0 { 0.0 } else { *v })
        .collect::<Vec<_>>())
}

/// The board's script with every component added, and the component
/// scripts it inserts (file stem, script), in order.
pub fn build_assembly_script(
    board: &ModelInput<'_>,
    components: &[Placed<'_>],
) -> Result<(Script, Vec<(String, Script)>), Error> {
    let mut main = crate::build_script(board)?;
    let mut parts: Vec<(String, String, Script)> = Vec::new(); // key, stem, script
    let mut boxes: Vec<(String, String)> = Vec::new(); // body, colour
    for (i, c) in components.iter().enumerate() {
        let name = safe(&c.name);
        match &c.mounted {
            Mounted::Model { key, input } => {
                let stem = match parts.iter().find(|(k, _, _)| k == key) {
                    Some((_, stem, _)) => stem.clone(),
                    None => {
                        let stem = format!("component{}", parts.len() + 1);
                        parts.push((key.clone(), stem.clone(), crate::build_script(input)?));
                        stem
                    }
                };
                main.cmd(
                    "file.insert_step",
                    json!({"path": format!("{stem}.step"), "name": name,
                           "matrix": matrix_value(&c.matrix)}),
                );
            }
            Mounted::Box { size, color } => {
                let body = format!("{name}_{}", i + 1);
                main.cmd(
                    "solid.box",
                    json!({"length": mm(size[0]), "width": mm(size[1]), "height": mm(size[2]),
                           "center": [0, 0, mm(size[2] / 2.0)], "operation": "new",
                           "body_name": body, "name": body}),
                );
                main.cmd(
                    "solid.move",
                    json!({"bodies": [body], "matrix": matrix_value(&c.matrix)}),
                );
                boxes.push((
                    body,
                    color.clone().unwrap_or_else(|| looks::NEUTRAL.color.into()),
                ));
            }
        }
    }
    for (body, color) in boxes {
        main.cmd(
            "appearance.assign",
            json!({"bodies": [body], "color": color,
                   "pbr": {"metallic": 0.0, "roughness": 0.6, "dielectric_ior": 1.5}}),
        );
    }
    Ok((
        main,
        parts.into_iter().map(|(_, stem, s)| (stem, s)).collect(),
    ))
}

/// Builds the assembled model and exports it in each of `formats`.
pub fn generate_assembly(
    board: &ModelInput<'_>,
    components: &[Placed<'_>],
    product_name: &str,
    formats: &[Format],
) -> Result<Vec<Vec<u8>>, Error> {
    let (main, parts) = build_assembly_script(board, components)?;
    Ok(run_assembly(&main, &parts, product_name, formats)?)
}

/// Runs each component script to `<stem>.step`, then `main` (which
/// inserts them by those names) in the same working directory.
pub fn run_assembly(
    main: &Script,
    parts: &[(String, Script)],
    product_name: &str,
    formats: &[Format],
) -> Result<Vec<Vec<u8>>, RunError> {
    runner::run_with_inputs(main, parts, product_name, formats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small pin-header board: two rows of 4 plated holes.
    fn board() -> openparts_core::Package {
        serde_yaml::from_str(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: module\n\
             lead_count: 8\npitch: {nominal: 2.54, unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {nominal: 18, unit: mm}\n\
             \x20 body_length: {nominal: 14, unit: mm}\n\
             \x20 body_height: {nominal: 0.8, unit: mm}\n\
             pad_rows:\n\
             \x20 - {side: -x, first: 1, count: 4, pitch: 2.54, start: 3, inset: 1.27, pad: [1.7, 1.7], drill: 1.0}\n\
             \x20 - {side: +x, first: 5, count: 4, pitch: 2.54, start: 3, inset: 1.27, pad: [1.7, 1.7], drill: 1.0}\n",
        )
        .unwrap()
    }

    fn chip() -> openparts_core::Package {
        serde_yaml::from_str(
            "schema_version: '0.1'\nkind: package\nid: standards/SOIC8-TEST\nfamily: soic\n\
             lead_count: 8\npitch: {nominal: 1.27, unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {nominal: 3.9, unit: mm}\n\
             \x20 body_length: {nominal: 4.9, unit: mm}\n\
             \x20 body_height: {nominal: 1.5, unit: mm}\n",
        )
        .unwrap()
    }

    #[test]
    fn components_are_inserted_once_per_part_and_boxes_are_moved_into_place() {
        let board = board();
        let board_g = openparts_mcad::generate(&board).unwrap();
        let soic = chip();
        let soic_g = openparts_mcad::generate(&soic).unwrap();
        let input = |p, g| ModelInput {
            package: p,
            geometry: g,
            category: None,
            marked_pin: None,
        };
        let top = |x: f64| [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.8];
        let components = vec![
            Placed {
                name: "U1".into(),
                mounted: Mounted::Model {
                    key: "t/SOIC".into(),
                    input: input(&soic, &soic_g),
                },
                matrix: top(-4.0),
            },
            Placed {
                name: "U2".into(),
                mounted: Mounted::Model {
                    key: "t/SOIC".into(),
                    input: input(&soic, &soic_g),
                },
                matrix: top(4.0),
            },
            // 45°: the matrix keeps full precision (a rounded one is not
            // rigid to SolidCraft's 1e-9).
            Placed {
                name: "U3".into(),
                mounted: Mounted::Model {
                    key: "t/SOIC".into(),
                    input: input(&soic, &soic_g),
                },
                matrix: {
                    let (s, c) = 45f64.to_radians().sin_cos();
                    [c, -s, 0.0, 0.0, s, c, 0.0, 4.0, 0.0, 0.0, 1.0, 0.8]
                },
            },
            Placed {
                name: "J1".into(),
                mounted: Mounted::Box {
                    size: [3.0, 2.0, 1.0],
                    color: Some("#c8c8c8".into()),
                },
                // Under the board, turned over.
                matrix: [-1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 6.0, 0.0, 0.0, -1.0, 0.0],
            },
        ];
        let (main, parts) = build_assembly_script(&input(&board, &board_g), &components).unwrap();
        assert_eq!(parts.len(), 1, "one script per distinct part");
        let inserts = main
            .commands()
            .iter()
            .filter(|c| c["command"] == "file.insert_step")
            .count();
        assert_eq!(inserts, 3);
        if crate::available_for_test(module_path!()) {
            let out = run_assembly(&main, &parts, "ASM", &[Format::Step, Format::ThreeMf]).unwrap();
            let step = String::from_utf8(out[0].clone()).unwrap();
            // Board + its 8 pads, 3 x (body + 8 leads), the box.
            let solids = step.matches("MANIFOLD_SOLID_BREP(").count();
            assert!(solids > 1 + 8 + 3 * 9, "{solids} solids");
            assert!(out[1].starts_with(b"PK\x03\x04"));
        }
    }
}
