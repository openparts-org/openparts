//! `melf` (openparts-mcad's `melf.rs`): a cylinder lying on the board,
//! a tin end cap at each end and a band at the marked end (a diode's
//! cathode, from the device's pin names, else pin 1). A body the source
//! states as glass (`construction: glass`, MiniMELF diodes) is drawn as
//! tinted glass with a dark band; anything else as a dark molding with a
//! light band.

use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_core::PackageConstruction;

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let cap = g.leads.first().ok_or(ModelError::NoLeads(g.family))?;
    let b = &g.body;
    let marked_left = common::marked_lead(input).is_none_or(|l| l.position.x <= 0.0);
    let (band_x, body_x) = if marked_left {
        (
            "-(melf_len / 2) + cap_len",
            "-(melf_len / 2) + cap_len + band_w",
        )
    } else {
        (
            "melf_len / 2 - cap_len - band_w",
            "-(melf_len / 2) + cap_len",
        )
    };
    s.fragment(
        fragment!("melf"),
        &vars!(
            melf_len = mm(b.size.x),
            melf_d = mm(b.size.y),
            cap_len = mm(cap.size.x),
            band_x = band_x,
            body_x = body_x,
        ),
    )?;
    if input.package.construction == Some(PackageConstruction::Glass) {
        s.paint(&["Body".into()], looks::GLASS);
        s.paint(&["Band".into()], looks::MOLD);
    } else {
        s.paint(&["Body".into()], looks::MOLD);
        s.paint(&["Band".into()], looks::MARK_LIGHT);
    }
    s.paint(&["Cap1".into(), "Cap2".into()], looks::TIN);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{build_script, run, Format, ModelInput};
    use openparts_core::ComponentCategory;

    #[test]
    fn a_melf_has_two_caps_a_band_and_a_body() {
        let package: openparts_core::Package = serde_yaml::from_str(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: melf\n\
             lead_count: 2\npitch: {unit: mm}\nconstruction: glass\ndimensions:\n\
             \x20 body_length: {min: 3.3, max: 3.7, unit: mm}\n\
             \x20 body_width: {min: 1.4, max: 1.6, unit: mm}\n\
             \x20 lead_length: {max: 0.47, unit: mm}\n\
             pads:\n\
             \x20 - {number: '1', x: -1.875, y: 0, width: 1.25, length: 2.0}\n\
             \x20 - {number: '2', x: 1.875, y: 0, width: 1.25, length: 2.0}\n",
        )
        .unwrap();
        let geometry = openparts_mcad::generate(&package).unwrap();
        for marked in ["1", "2"] {
            let script = build_script(&ModelInput {
                package: &package,
                geometry: &geometry,
                category: Some(ComponentCategory::Diode),
                marked_pin: Some(marked),
            })
            .unwrap();
            let painted: Vec<String> = script
                .commands()
                .iter()
                .filter(|c| c["command"] == "appearance.assign")
                .flat_map(|c| c["params"]["bodies"].as_array().unwrap().clone())
                .map(|b| b.as_str().unwrap().to_string())
                .collect();
            assert_eq!(painted, ["Body", "Band", "Cap1", "Cap2"]);
            if crate::available_for_test(module_path!()) {
                let step = run(&script, "TEST", &[Format::Step]).unwrap().remove(0);
                let solids = String::from_utf8(step)
                    .unwrap()
                    .matches("MANIFOLD_SOLID_BREP(")
                    .count();
                assert_eq!(solids, 4);
            }
        }
    }
}
