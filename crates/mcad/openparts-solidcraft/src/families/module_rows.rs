//! Modules whose pads are `pad_rows` / `pad_grids` (ESP32-WROOM and
//! similar, openparts-mcad's `module` with rows): the bare PCB in solder
//! mask green, its pads on the underside -- a plated half-hole
//! (castellation) wherever a pad reaches the board edge and the package
//! states `hole_diameter`, LGA pads elsewhere -- and the shield can on
//! top. Pads sit where the geometry puts them (the footprint's source),
//! so every row keeps its own pitch and start; each pad is drawn on its
//! own (rows of different pitch and grids don't share one pattern).
//!
//! The antenna (a PCB trace or a ceramic part) and the parts under the
//! can are not drawn: module drawings don't dimension them.

use crate::script::{looks, mm};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::{Details, Lead};
use serde_json::json;

use super::module::SOLDER_MASK;

const EPS: f64 = 1e-6;

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let Details::Module(features) = g.details else {
        return Err(ModelError::MissingDetails(g.family));
    };
    if g.leads.is_empty() {
        return Err(ModelError::NoLeads(g.family));
    }
    crate::common::body_params(s, g)?;
    s.param("pad_t", "0.035 mm", "copper pad thickness");
    s.param(
        "plating_t",
        "0.05 mm",
        "plating thickness on the hole walls",
    );
    s.cmd(
        "solid.box",
        json!({"length": "body_w", "width": "body_l", "height": "body_h - 2 * pad_t",
               "center": [0, 0, "body_h / 2"], "operation": "new",
               "body_name": "Board", "name": "Board"}),
    );
    let hole = features.holes.map(|h| h.diameter);
    if let Some(d) = hole {
        s.param("hole_d", mm(d), "castellation diameter (datasheet)");
    }
    let (hw, hl) = (g.body.size.x / 2.0, g.body.size.y / 2.0);
    let mut metal = Vec::new();
    for (i, lead) in g.leads.iter().enumerate() {
        let name = format!("Pad{}", i + 1);
        let (x0, x1) = (
            (lead.position.x - lead.size.x / 2.0).max(-hw),
            (lead.position.x + lead.size.x / 2.0).min(hw),
        );
        let (y0, y1) = (
            (lead.position.y - lead.size.y / 2.0).max(-hl),
            (lead.position.y + lead.size.y / 2.0).min(hl),
        );
        if x1 - x0 < EPS || y1 - y0 < EPS {
            continue;
        }
        let pad_box = |s: &mut Script, z: &str, op: &str, part: &str| {
            let mut p = json!({"length": mm(x1 - x0), "width": mm(y1 - y0), "height": "pad_t",
                   "center": [mm((x0 + x1) / 2.0), mm((y0 + y1) / 2.0), z],
                   "operation": op, "name": format!("{name}{part}")});
            if op == "new" {
                p["body_name"] = json!(name);
            } else {
                p["targets"] = json!([name]);
            }
            s.cmd("solid.box", p);
        };
        pad_box(s, "pad_t / 2", "new", "Bottom");
        if let (Some(_), Some((ex, ey))) = (hole, edge_point(lead, hw, hl)) {
            // Castellation: the pad wraps up the half barrel to the top.
            pad_box(s, "body_h - pad_t / 2", "join", "Top");
            castellation(s, &name, ex, ey);
        }
        metal.push(name);
    }
    if let Some(sh) = features.shield {
        s.param(
            "shield_w",
            mm(sh.width),
            "shield can size along X (datasheet)",
        );
        s.param(
            "shield_l",
            mm(sh.length),
            "shield can size along Y (datasheet)",
        );
        s.param(
            "shield_top",
            mm(sh.top),
            "seating plane to the can top (datasheet overall height)",
        );
        s.cmd(
            "solid.box",
            json!({"length": "shield_w", "width": "shield_l", "height": "shield_top - body_h",
                   "center": [mm(sh.x), mm(sh.y), "(body_h + shield_top) / 2"],
                   "operation": "new", "body_name": "Shield", "name": "Shield"}),
        );
        s.paint(&["Shield".into()], looks::NICKEL);
    }
    s.paint(&["Board".into()], SOLDER_MASK);
    s.paint(&metal, looks::GOLD);
    Ok(())
}

/// Where a pad meets the board edge (the castellation's centre), if it
/// reaches it.
fn edge_point(lead: &Lead, hw: f64, hl: f64) -> Option<(f64, f64)> {
    let p = lead.position;
    if p.x - lead.size.x / 2.0 <= -hw + EPS {
        Some((-hw, p.y))
    } else if p.x + lead.size.x / 2.0 >= hw - EPS {
        Some((hw, p.y))
    } else if p.y - lead.size.y / 2.0 <= -hl + EPS {
        Some((p.x, -hl))
    } else if p.y + lead.size.y / 2.0 >= hl - EPS {
        Some((p.x, hl))
    } else {
        None
    }
}

/// The plated half barrel at (`x`, `y`) on the board edge, joined to the
/// pad `name`, and the board drilled for it.
fn castellation(s: &mut Script, name: &str, x: f64, y: f64) {
    let base = json!([mm(x), mm(y), 0]);
    s.cmd(
        "solid.cylinder",
        json!({"diameter": "hole_d + 2 * plating_t", "height": "body_h", "base": base,
               "operation": "join", "targets": [name], "name": format!("{name}Barrel")}),
    );
    s.cmd(
        "solid.cylinder",
        json!({"diameter": "hole_d", "height": "body_h + 0.2 mm",
               "base": [mm(x), mm(y), "-0.1 mm"],
               "operation": "cut", "targets": [name], "name": format!("{name}Hole")}),
    );
    s.cmd(
        "solid.cylinder",
        json!({"diameter": "hole_d + 2 * plating_t", "height": "body_h + 0.2 mm",
               "base": [mm(x), mm(y), "-0.1 mm"],
               "operation": "cut", "targets": ["Board"], "name": format!("{name}Drill")}),
    );
    // Keep only the half on the board.
    s.cmd(
        "solid.box",
        json!({"length": "body_w", "width": "body_l", "height": "body_h",
               "center": [0, 0, "body_h / 2"],
               "operation": "intersect", "targets": [name], "name": format!("{name}Trim")}),
    );
}

#[cfg(test)]
mod tests {
    use crate::{build_script, run, Format, ModelInput};

    /// An ESP32-WROOM-style module: 3 castellated rows, a 3 x 3 ground
    /// grid and a shield can.
    pub(crate) fn wroom() -> openparts_core::Package {
        serde_yaml::from_str(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: module\n\
             lead_count: 39\npitch: {nominal: 1.27, unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {nominal: 18, unit: mm}\n\
             \x20 body_length: {nominal: 25.5, unit: mm}\n\
             \x20 body_height: {nominal: 0.8, unit: mm}\n\
             \x20 overall_height: {nominal: 3.1, unit: mm}\n\
             \x20 hole_diameter: {nominal: 0.55, unit: mm}\n\
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
    fn a_wroom_module_has_castellations_lga_pads_and_a_can() {
        let package = wroom();
        let geometry = openparts_mcad::generate(&package).unwrap();
        let script = build_script(&ModelInput {
            package: &package,
            geometry: &geometry,
            category: None,
            marked_pin: None,
        })
        .unwrap();
        let named = |prefix: &str| {
            script
                .commands()
                .iter()
                .filter(|c| {
                    c["params"]["name"]
                        .as_str()
                        .is_some_and(|n| n.ends_with(prefix))
                })
                .count()
        };
        assert_eq!(named("Barrel"), 38, "one castellation per edge pad");
        assert_eq!(named("Bottom"), 38 + 9);
        assert_eq!(named("Shield"), 1);
        if crate::available_for_test(module_path!()) {
            let step = run(&script, "TEST", &[Format::Step]).unwrap().remove(0);
            let step = String::from_utf8(step).unwrap();
            assert_eq!(step.matches("MANIFOLD_SOLID_BREP(").count(), 1 + 47 + 1);
        }
    }
}
