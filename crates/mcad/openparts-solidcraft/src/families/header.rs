//! The header families (openparts-mcad's `header`, `socket`,
//! `shrouded_header`, `right_angle_header`): an insulator housing on a
//! grid of square pins. A pin header's pins stand `mating_length` above
//! the housing, with breakaway notches across the housing top between
//! neighbouring pins; a socket's contacts sit in funnelled entries in the
//! housing top; a shrouded header's posts stand inside an open-top box
//! (walls and floor are generator defaults: wire-to-board drawings
//! dimension only the outline); a right-angle header's pins bend 90°
//! from the tails into posts running along X through a housing lying on
//! the board. All have their solder tails `tail_length` below the
//! housing (the board, for a right-angle header). The grid is
//! parametric (`pitch`, `pin_rows`, `pins_per_row`, `row_pitch`), the
//! housing sits `body_x` off it.
//!
//! Finishes: black housing and gold pins (the Würth WR-PHD parts the
//! straight and right-angle families were built from); a shrouded
//! header is natural (white) polyamide with tin-plated posts, as JST
//! XH/PH state -- other colours aren't modelled.
//!
//! No pin-1 mark: the housings this family was built from carry none, or
//! an undimensioned one (the footprint's square pad marks pin 1).

use crate::common::{self};
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::{Details, HeaderKind};

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let _package = input.package;
    let Details::Header(f) = g.details else {
        return Err(ModelError::MissingDetails(g.family));
    };
    let Some(first) = g.leads.first() else {
        return Err(ModelError::NoLeads(g.family));
    };
    if let Some(smt) = f.smt {
        return build_smt(s, input, &f, &smt);
    }
    let pitch = if f.pins_per_row > 1 {
        first.position.y - g.leads[f.rows as usize].position.y
    } else {
        g.pitch.unwrap_or(2.54)
    };

    common::body_params(s, g)?;
    s.fragment(
        fragment!("header_params"),
        &vars!(
            pitch = mm(pitch),
            rows = f.rows,
            per_row = f.pins_per_row,
            row_pitch = mm(f.row_spacing),
            pin_w = mm(f.pin_width),
            tail_l = mm(f.tail_length),
            standoff = mm(f.standoff),
            body_x = mm(g.body.position.x),
        ),
    )?;
    let grid = g.leads.len() > 1;
    let top = match f.kind {
        HeaderKind::Pin => {
            s.param(
                "mating_l",
                mm(f.mating_length),
                "pin length above the housing (datasheet, else generator default)",
            );
            if f.pins_per_row > 1 {
                s.fragment(fragment!("header_notches"), &vars!())?;
                if f.pins_per_row > 2 {
                    s.fragment(fragment!("header_notch_row"), &vars!())?;
                }
            }
            Some("standoff + body_h + mating_l")
        }
        HeaderKind::Socket => {
            s.fragment(fragment!("socket_entry"), &vars!())?;
            if grid {
                s.fragment(fragment!("header_grid"), &vars!(seed = "Entry"))?;
            }
            s.fragment(
                fragment!("socket_cut"),
                &vars!(tools = serde_json::json!(common::copies("Entry", g.leads.len() as u32))),
            )?;
            // The contact shows at the bottom of its cavity.
            Some("standoff + body_h - entry_d - cavity_d")
        }
        HeaderKind::Shrouded => {
            let shroud = f.shroud.ok_or(ModelError::MissingDetails(g.family))?;
            s.fragment(
                fragment!("shroud_cavity"),
                &vars!(
                    wall_t = mm(shroud.wall),
                    floor_h = mm(shroud.floor),
                    post_recess = mm(shroud.post_recess),
                ),
            )?;
            Some("standoff + body_h - post_recess")
        }
        HeaderKind::RightAngle => {
            s.param(
                "mating_l",
                mm(f.mating_length),
                "post length beyond the housing's front face (datasheet, else generator default)",
            );
            let (dir, inv) = if f.body_offset >= 0.0 {
                ("+", "-")
            } else {
                ("-", "+")
            };
            s.fragment(
                fragment!("header_ra_pin"),
                &vars!(bend_h = mm(f.bend_height), dir = dir, inv = inv),
            )?;
            None
        }
    };
    if let Some(top) = top {
        s.fragment(
            fragment!("header_pin"),
            &vars!(bottom = "-tail_l", top = top),
        )?;
    }
    if grid {
        s.fragment(fragment!("header_grid"), &vars!(seed = "Pin"))?;
    }
    let (housing, pins) = match f.kind {
        HeaderKind::Shrouded => (looks::NYLON_NATURAL, looks::TIN),
        _ => (looks::MOLD, looks::GOLD),
    };
    s.paint(&["Housing".into()], housing);
    s.paint(&common::copies("Pin", g.leads.len() as u32), pins);
    Ok(())
}

/// A surface-mount wire-to-board header (`Package::smt`): the housing
/// (top entry: the open-top shroud; side entry: a box opening away from
/// the contact row), one contact per land -- a post standing in the
/// shroud, or a post bent from the land into the housing -- with a flat
/// solder foot on its land, and a retention tab on each mechanical pad.
/// Contact and tab sizes beyond the lands are generator choices: the
/// drawings dimension only the outline.
fn build_smt(
    s: &mut Script,
    input: &ModelInput<'_>,
    f: &openparts_mcad::HeaderFeatures,
    smt: &openparts_mcad::HeaderSmt,
) -> Result<(), ModelError> {
    use serde_json::json;
    let g = input.geometry;
    let n = f.pins_per_row;
    let pitch = if n > 1 {
        g.leads[0].position.y - g.leads[1].position.y
    } else {
        g.pitch.unwrap_or(1.0)
    };
    common::body_params(s, g)?;
    s.fragment(
        fragment!("header_params"),
        &vars!(
            pitch = mm(pitch),
            rows = 1,
            per_row = n,
            row_pitch = mm(pitch),
            pin_w = mm(f.pin_width),
            tail_l = "0 mm",
            standoff = mm(f.standoff),
            body_x = mm(g.body.position.x),
        ),
    )?;
    // The side away from the housing, where each foot runs out to.
    let away = if f.body_offset >= 0.0 { -1.0 } else { 1.0 };
    let foot_t = (f.pin_width * 0.6).max(0.1);
    match f.kind {
        HeaderKind::Shrouded => {
            let shroud = f.shroud.ok_or(ModelError::MissingDetails(g.family))?;
            s.fragment(
                fragment!("shroud_cavity"),
                &vars!(
                    wall_t = mm(shroud.wall),
                    floor_h = mm(shroud.floor),
                    post_recess = mm(shroud.post_recess),
                ),
            )?;
            // The post stands mid-cavity (its place isn't dimensioned).
            s.cmd(
                "solid.box",
                json!({"length": "pin_w", "width": "pin_w",
                       "height": "standoff + body_h - post_recess",
                       "center": ["body_x", "pin_y0", "(standoff + body_h - post_recess) / 2"],
                       "operation": "new", "body_name": "Pin", "name": "Pin"}),
            );
        }
        _ => {
            // Side entry: the opening on the housing face away from the
            // contacts, the posts reaching into it.
            let wall = (f.pin_width * 1.2).max(0.3);
            let depth = g.body.size.x * 0.7;
            let front = g.body.position.x - away * g.body.size.x / 2.0;
            s.param("wall_t", mm(wall), "housing wall (generator default)");
            s.cmd(
                "solid.box",
                json!({"length": mm(depth + 0.1), "width": "body_l - 2 * wall_t",
                       "height": "body_h - 2 * wall_t",
                       "center": [mm(front + away * (depth - 0.1) / 2.0), 0, "standoff + body_h / 2"],
                       "operation": "cut", "targets": ["Housing"], "name": "Cavity"}),
            );
            s.param(
                "mating_l",
                mm(-depth * 0.6),
                "post tip inside the opening (generator default)",
            );
            let (dir, inv) = if f.body_offset >= 0.0 {
                ("+", "-")
            } else {
                ("-", "+")
            };
            s.fragment(
                fragment!("header_ra_pin"),
                &vars!(bend_h = mm(f.bend_height), dir = dir, inv = inv),
            )?;
        }
    }
    // The solder foot on pin 1's land, from the post out to the land's
    // outer end.
    let post_x = if f.kind == HeaderKind::Shrouded {
        g.body.position.x
    } else {
        0.0
    };
    let toe = away * smt.pad[0] / 2.0 * 0.9;
    s.cmd(
        "solid.box",
        json!({"length": mm((toe - post_x).abs() + f.pin_width / 2.0), "width": "pin_w",
               "height": mm(foot_t),
               "center": [mm((toe + post_x) / 2.0 - away * f.pin_width / 4.0), "pin_y0", mm(foot_t / 2.0)],
               "operation": "join", "targets": ["Pin"], "name": "PinFoot"}),
    );
    if n > 1 {
        s.fragment(fragment!("header_grid"), &vars!(seed = "Pin"))?;
    }
    // Retention tabs: a plate on each mechanical pad, folded up against
    // the housing's end.
    let [mw, ml] = smt.mech_pad;
    let [mx, my] = smt.mech_at;
    let plate_t = 0.2_f64.min(ml * 0.3);
    let up = f.standoff + f.body_height * 0.7;
    let mut tabs = Vec::new();
    for (k, sign) in [(1, 1.0), (2, -1.0)] {
        let name = format!("Tab{k}");
        let y = sign * my;
        s.cmd(
            "solid.box",
            json!({"length": mm(mw * 0.9), "width": mm(ml * 0.9), "height": mm(plate_t),
                   "center": [mm(mx), mm(y), mm(plate_t / 2.0)],
                   "operation": "new", "body_name": name, "name": format!("{name}Foot")}),
        );
        let end = sign * (g.body.size.y / 2.0 + plate_t / 2.0);
        if (end - y).abs() < ml / 2.0 {
            s.cmd(
                "solid.box",
                json!({"length": mm(mw * 0.9), "width": mm(plate_t), "height": mm(up),
                       "center": [mm(mx), mm(end), mm(up / 2.0)],
                       "operation": "join", "targets": [name], "name": format!("{name}Wall")}),
            );
        }
        tabs.push(name);
    }
    s.paint(&["Housing".into()], looks::NYLON_NATURAL);
    let mut metal = common::copies("Pin", n);
    metal.extend(tabs);
    s.paint(&metal, looks::TIN);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{build_script, run, Format, ModelInput};

    /// JST SH-style 4-circuit SMT headers: top entry (shrouded) and side
    /// entry (right-angle).
    fn sh(family: &str, offset: f64, mech_x: f64) -> openparts_core::Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: {family}\n\
             lead_count: 4\nlead_layout: [1, 4]\npitch: {{nominal: 1.0, unit: mm}}\n\
             dimensions:\n\
             \x20 body_width: {{nominal: 2.9, unit: mm}}\n\
             \x20 body_length: {{nominal: 6.0, unit: mm}}\n\
             \x20 body_height: {{nominal: 4.25, unit: mm}}\n\
             \x20 body_offset: {{nominal: {offset}, unit: mm}}\n\
             smt: {{pad: [1.55, 0.6], mech_pad: [1.8, 1.2], mech_pad_at: [{mech_x}, 2.8]}}\n"
        ))
        .unwrap()
    }

    #[test]
    fn smt_headers_get_feet_and_retention_tabs() {
        for (family, offset, mech_x) in [
            ("shrouded_header", 1.775, 2.525),
            ("right_angle_header", -2.45, -3.875),
        ] {
            let package = sh(family, offset, mech_x);
            let geometry = openparts_mcad::generate(&package).unwrap();
            assert_eq!(geometry.leads.len(), 6, "4 contacts and 2 MP");
            let script = build_script(&ModelInput {
                package: &package,
                geometry: &geometry,
                category: None,
                marked_pin: None,
            })
            .unwrap();
            let painted: usize = script
                .commands()
                .iter()
                .filter(|c| c["command"] == "appearance.assign")
                .map(|c| c["params"]["bodies"].as_array().unwrap().len())
                .sum();
            assert_eq!(painted, 1 + 4 + 2, "{family}");
            if crate::available_for_test(module_path!()) {
                let step = run(&script, "TEST", &[Format::Step]).unwrap().remove(0);
                let step = String::from_utf8(step).unwrap();
                assert_eq!(step.matches("MANIFOLD_SOLID_BREP(").count(), 7, "{family}");
            }
        }
    }

    fn header(family: &str, layout: [u32; 2]) -> openparts_core::Package {
        let n = layout[0] * layout[1];
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: {family}\n\
             lead_count: {n}\nlead_layout: [{}, {}]\npitch: {{nominal: 2.54, unit: mm}}\n\
             dimensions:\n\
             \x20 body_width: {{nominal: {w}, unit: mm}}\n\
             \x20 body_length: {{nominal: {l}, unit: mm}}\n\
             \x20 lead_width: {{nominal: 0.64, unit: mm}}\n\
             \x20 row_spacing: {{nominal: 2.54, unit: mm}}\n\
             \x20 hole_diameter: {{nominal: 1.1, unit: mm}}\n\
             \x20 body_offset: {{nominal: {x}, unit: mm}}\n",
            layout[0],
            layout[1],
            x = if family == "right_angle_header" {
                2.77
            } else {
                0.0
            },
            w = 2.54 * layout[0] as f64
                + if family == "shrouded_header" {
                    2.0
                } else {
                    0.0
                },
            l = 2.54 * layout[1] as f64,
        ))
        .unwrap()
    }

    fn script(package: &openparts_core::Package) -> crate::Script {
        let geometry = openparts_mcad::generate(package).unwrap();
        build_script(&ModelInput {
            package,
            geometry: &geometry,
            category: Some(openparts_core::ComponentCategory::Connector),
            marked_pin: None,
        })
        .unwrap()
    }

    fn painted(script: &crate::Script) -> Vec<usize> {
        script
            .commands()
            .iter()
            .filter(|c| c["command"] == "appearance.assign")
            .map(|c| c["params"]["bodies"].as_array().unwrap().len())
            .collect()
    }

    #[test]
    fn a_2x5_header_is_a_housing_and_ten_gold_pins_on_a_parametric_grid() {
        let script = script(&header("header", [2, 5]));
        assert_eq!(painted(&script), vec![1, 10]);
        let names: Vec<&str> = script
            .commands()
            .iter()
            .filter(|c| c["command"] == "parameters.add")
            .map(|c| c["params"]["name"].as_str().unwrap())
            .collect();
        for p in [
            "pitch",
            "pin_rows",
            "pins_per_row",
            "row_pitch",
            "mating_l",
            "tail_l",
        ] {
            assert!(names.contains(&p), "{p} in {names:?}");
        }
        if crate::available_for_test(module_path!()) {
            let step = run(&script, "TEST", &[Format::Step]).unwrap().remove(0);
            let step = String::from_utf8(step).unwrap();
            assert_eq!(step.matches("MANIFOLD_SOLID_BREP(").count(), 11);
        }
    }

    #[test]
    fn sockets_and_single_pins_build_too() {
        for (family, layout, bodies) in [
            ("socket", [1, 4], 5),
            ("socket", [2, 3], 7),
            ("header", [1, 1], 2),
            ("header", [1, 2], 3),
            ("shrouded_header", [1, 4], 5),
            ("shrouded_header", [2, 3], 7),
            ("right_angle_header", [1, 4], 5),
            ("right_angle_header", [1, 1], 2),
        ] {
            let script = script(&header(family, layout));
            assert_eq!(painted(&script).iter().sum::<usize>(), bodies);
            if crate::available_for_test(module_path!()) {
                let step = run(&script, "TEST", &[Format::Step]).unwrap().remove(0);
                let step = String::from_utf8(step).unwrap();
                assert_eq!(
                    step.matches("MANIFOLD_SOLID_BREP(").count(),
                    bodies,
                    "{family} {layout:?}"
                );
            }
        }
    }
}
