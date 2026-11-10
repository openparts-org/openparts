//! Pin headers and socket strips (openparts-mcad's `header`, `socket`):
//! a black insulator housing on a grid of square gold pins. A pin
//! header's pins stand `mating_length` above the housing, with breakaway
//! notches across the housing top between neighbouring pins; a socket's
//! contacts sit in funnelled entries in the housing top. Both have their
//! solder tails `tail_length` below the housing. The grid is parametric
//! (`pitch`, `pin_rows`, `pins_per_row`, `row_pitch`).
//!
//! No pin-1 mark: the housings this family was built from carry none
//! (the footprint's square pad marks pin 1).

use crate::common::{self};
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::{header_features, HeaderKind};

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let package = input.package;
    let f = header_features(package)
        .map_err(|e| ModelError::UnsupportedFamily(format!("{} ({e})", package.family)))?;
    let Some(first) = g.leads.first() else {
        return Err(ModelError::NoLeads(package.family.clone()));
    };
    let pitch = if f.pins_per_row > 1 {
        first.position.y - g.leads[f.rows as usize].position.y
    } else {
        package.pitch.nominal.unwrap_or(2.54)
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
            "standoff + body_h + mating_l"
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
            "standoff + body_h - entry_d - cavity_d"
        }
    };
    s.fragment(
        fragment!("header_pin"),
        &vars!(bottom = "-tail_l", top = top),
    )?;
    if grid {
        s.fragment(fragment!("header_grid"), &vars!(seed = "Pin"))?;
    }
    s.paint(&["Housing".into()], looks::MOLD);
    s.paint(&common::copies("Pin", g.leads.len() as u32), looks::GOLD);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{build_script, run, Format, ModelInput};

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
             \x20 hole_diameter: {{nominal: 1.1, unit: mm}}\n",
            layout[0],
            layout[1],
            w = 2.54 * layout[0] as f64,
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
        if crate::available() {
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
        ] {
            let script = script(&header(family, layout));
            assert_eq!(painted(&script).iter().sum::<usize>(), bodies);
            if crate::available() {
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
