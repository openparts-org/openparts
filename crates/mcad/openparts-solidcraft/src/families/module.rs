//! Castellated modules (openparts-mcad's `module`, e.g. the Raspberry Pi
//! Pico): the bare PCB in solder-mask green with its edge pads -- plated
//! half-holes where the board edge cuts the pin holes, and the
//! through-holes at the row spacing -- the extra pad row and the mounting
//! holes, each only when the package states its dimensions.
//!
//! Components on the board (the MCU, USB connector, button, LED, a
//! wireless module) are not drawn: the module drawings this family was
//! built from don't dimension their size or place.

use crate::common::{self, Row, Side};
use crate::script::{looks, mm, Look};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::{module_features, Lead, ModuleFeatures};

/// Solder mask over FR-4.
const SOLDER_MASK: Look = Look {
    color: "#1e6b3a",
    metallic: 0.0,
    roughness: 0.45,
    opacity: 1.0,
};

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let package = input.package;
    let features = module_features(package)
        .map_err(|e| ModelError::UnsupportedFamily(format!("{} ({e})", package.family)))?;
    let is_aux = |number: &str| package.aux_pads.iter().any(|a| a == number);

    // The edge pins only (an extra pad row can sit anywhere on the
    // board), split by the long edge they are on: a long, narrow board
    // defeats `common::perimeter_rows`'s nearest-edge rule at its corners.
    let rows: Vec<Row<'_>> = [(Side::Left, -1.0), (Side::Right, 1.0)]
        .into_iter()
        .filter_map(|(side, sign)| {
            let mut leads: Vec<&Lead> = g
                .leads
                .iter()
                .filter(|l| !is_aux(&l.number) && l.position.x * sign > 0.0)
                .collect();
            leads.sort_by(|a, b| a.position.y.total_cmp(&b.position.y));
            (!leads.is_empty()).then_some(Row { side, leads })
        })
        .collect();
    if rows.len() != 2 {
        return Err(ModelError::NoLeads(package.family.clone()));
    }
    let half_w = g.body.size.x / 2.0;
    let (pad_len, pad_w) = rows[0].lead_size();
    let inner = rows[0].outer_extent() - pad_len;
    let edge_pad = (half_w - inner).clamp(0.3, half_w);

    common::body_params(s, g)?;
    s.fragment(
        fragment!("module_board"),
        &vars!(pad_w = mm(pad_w), edge_pad = mm(edge_pad)),
    )?;
    let layouts = common::row_layouts(s, &rows, package.pitch.nominal);

    let ModuleFeatures {
        holes,
        mounting_holes,
    } = features;
    let mut metal = Vec::new();
    for (row, layout) in rows.iter().zip(&layouts) {
        let at = row.side.row_vars(&layout.first);
        s.fragment(fragment!("module_pad_row"), &at)?;
        if let Some(h) = holes {
            s.fragment(
                fragment!("module_castellation"),
                &at.clone().set("hole_d", mm(h.diameter)),
            )?;
            if let Some(span) = h.row_spacing {
                s.fragment(
                    fragment!("module_via"),
                    &at.clone().set("row_span", mm(span)),
                )?;
            }
        }
        let seed = format!("Pad{}", row.side.tag());
        if layout.n > 1 {
            s.fragment(
                fragment!("pattern_row"),
                &vars!(
                    seed = seed.clone(),
                    dir = serde_json::json!([0, 1, 0]),
                    count = layout.count.clone(),
                    spacing = layout.spacing.clone()
                ),
            )?;
        }
        metal.extend(common::copies(&seed, layout.n));
    }

    // Drill the board: castellations, through-holes, mounting holes.
    if let Some(h) = holes {
        for (row, layout) in rows.iter().zip(&layouts) {
            let mut drills = vec![("Castellation", "body_w / 2")];
            if h.row_spacing.is_some() {
                drills.push(("PinHole", "row_span / 2"));
            }
            for (what, x) in drills {
                s.fragment(
                    fragment!("module_hole_row"),
                    &row.side.row_vars(&layout.first).with(&vars!(
                        x = x,
                        what = what,
                        count = layout.count.clone(),
                        spacing = layout.spacing.clone()
                    )),
                )?;
            }
        }
    }
    if let Some(m) = mounting_holes {
        s.fragment(
            fragment!("module_mounting_holes"),
            &vars!(d = mm(m.diameter), dx = mm(m.spacing), inset = mm(m.inset)),
        )?;
    }

    // The extra pad row: the part of each land on the board.
    let mut aux: Vec<_> = g.leads.iter().filter(|l| is_aux(&l.number)).collect();
    aux.sort_by(|a, b| a.position.x.total_cmp(&b.position.x));
    if let Some(first) = aux.first() {
        let half_l = g.body.size.y / 2.0;
        let lo = (first.position.y - first.size.y / 2.0).max(-half_l);
        let hi = (first.position.y + first.size.y / 2.0).min(half_l);
        if hi > lo {
            s.fragment(
                fragment!("module_aux_pad"),
                &vars!(
                    x0 = mm(first.position.x),
                    y = mm((lo + hi) / 2.0),
                    w = mm(first.size.x),
                    l = mm(hi - lo)
                ),
            )?;
            let n = aux.len() as u32;
            if n > 1 {
                let spacing = aux[1].position.x - aux[0].position.x;
                let spacing = match package.pitch.nominal {
                    Some(p) if (p - spacing).abs() < 1e-6 => "pitch".to_string(),
                    _ => mm(spacing),
                };
                s.fragment(
                    fragment!("pattern_row"),
                    &vars!(
                        seed = "Aux",
                        dir = serde_json::json!([1, 0, 0]),
                        count = n,
                        spacing = spacing
                    ),
                )?;
            }
            metal.extend(common::copies("Aux", n));
        }
    }

    s.paint(&["Board".into()], SOLDER_MASK);
    s.paint(&metal, looks::GOLD);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{build_script, run, Format, ModelInput};

    fn pico(aux: &str) -> openparts_core::Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: module\n\
             lead_count: 40\npitch: {{nominal: 2.54, unit: mm}}\n{aux}\
             dimensions:\n\
             \x20 body_width: {{nominal: 21, unit: mm}}\n\
             \x20 body_length: {{nominal: 51, unit: mm}}\n\
             \x20 body_height: {{nominal: 1, unit: mm}}\n\
             \x20 overall_width: {{nominal: 22.58, unit: mm}}\n\
             \x20 lead_width: {{nominal: 1.6, unit: mm}}\n\
             \x20 lead_length: {{nominal: 3.2, unit: mm}}\n\
             \x20 row_spacing: {{nominal: 17.78, unit: mm}}\n\
             \x20 hole_diameter: {{nominal: 1, unit: mm}}\n\
             \x20 aux_pad_width: {{nominal: 1.6, unit: mm}}\n\
             \x20 aux_pad_length: {{nominal: 3.2, unit: mm}}\n\
             \x20 aux_row_x: {{nominal: 10.5, unit: mm}}\n\
             \x20 aux_row_y: {{nominal: 0.8, unit: mm}}\n\
             \x20 mounting_hole_diameter: {{nominal: 2.1, unit: mm}}\n\
             \x20 mounting_hole_spacing: {{nominal: 11.4, unit: mm}}\n\
             \x20 mounting_hole_inset: {{nominal: 2, unit: mm}}\n"
        ))
        .unwrap()
    }

    #[test]
    fn a_pico_is_a_green_board_with_forty_castellated_pads_and_its_debug_row() {
        let package = pico("aux_pads: [D1, D2, D3]\n");
        let geometry = openparts_mcad::generate(&package).unwrap();
        let script = build_script(&ModelInput {
            package: &package,
            geometry: &geometry,
            category: Some(openparts_core::ComponentCategory::Microcontroller),
            marked_pin: None,
        })
        .unwrap();
        let painted: Vec<(String, usize)> = script
            .commands()
            .iter()
            .filter(|c| c["command"] == "appearance.assign")
            .map(|c| {
                (
                    c["params"]["color"].as_str().unwrap().to_string(),
                    c["params"]["bodies"].as_array().unwrap().len(),
                )
            })
            .collect();
        assert_eq!(painted.len(), 2);
        assert_eq!(painted[0].1, 1, "the board");
        assert_eq!(painted[1].1, 40 + 3, "edge pads and debug pads");
        // Parametric: the rows follow leads_per_side and pitch.
        let names: Vec<&str> = script
            .commands()
            .iter()
            .filter(|c| c["command"] == "parameters.add")
            .map(|c| c["params"]["name"].as_str().unwrap())
            .collect();
        for p in ["pitch", "leads_per_side", "hole_d", "row_span", "mount_d"] {
            assert!(names.contains(&p), "{p} in {names:?}");
        }
        if crate::available() {
            let step = run(&script, "TEST", &[Format::Step]).unwrap().remove(0);
            let step = String::from_utf8(step).unwrap();
            assert_eq!(step.matches("MANIFOLD_SOLID_BREP(").count(), 44);
        }
    }

    #[test]
    fn without_hole_dimensions_only_the_pads_are_drawn() {
        let mut package = pico("");
        package.dimensions.hole_diameter = None;
        package.dimensions.mounting_hole_inset = None;
        let geometry = openparts_mcad::generate(&package).unwrap();
        let script = build_script(&ModelInput {
            package: &package,
            geometry: &geometry,
            category: None,
            marked_pin: None,
        })
        .unwrap();
        let commands: Vec<&str> = script
            .commands()
            .iter()
            .filter_map(|c| c["params"]["name"].as_str())
            .collect();
        assert!(!commands.iter().any(|n| n.contains("Castellation")
            || n.contains("Via")
            || n.contains("Mounting")
            || *n == "Aux"));
    }
}
