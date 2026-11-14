//! Power packages with a wide tab (openparts-mcad's `sot223` / `dpak`):
//! gull-wing leads on the left body edge, the tab on the right. The
//! geometry's last lead is always the tab.

use super::gullwing::{self, Family};
use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::MechanicalGeometry;

/// The geometry without its tab, and the tab.
fn split_tab(
    input: &ModelInput<'_>,
) -> Result<(MechanicalGeometry, openparts_mcad::Lead), ModelError> {
    let mut leads = input.geometry.clone();
    match leads.leads.pop() {
        Some(tab) if !leads.leads.is_empty() => Ok((leads, tab)),
        _ => Err(ModelError::NoLeads(input.geometry.family)),
    }
}

/// SOT-223: the tab is a wide gull-wing lead mirroring the leads.
pub fn sot223(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let (lead_geometry, tab) = split_tab(input)?;
    let family = &gullwing::SOT223;
    let form = gullwing::lead_form(input)?;
    gullwing::body(s, input, family, form)?;
    let rows = common::perimeter_rows(&lead_geometry);
    gullwing::lead_params(s, input, &rows, family, form)?;
    let layouts = common::row_layouts(s, &rows, input.geometry.pitch);
    let mut metal = gullwing::rows_of(s, &rows, &layouts, "lead_w")?;

    s.param("tab_w", mm(tab.size.y), "tab width (b2)");
    let tab_geometry = MechanicalGeometry {
        leads: vec![tab],
        ..lead_geometry
    };
    let tab_rows = common::perimeter_rows(&tab_geometry);
    let tab_layouts = common::row_layouts(s, &tab_rows, None);
    metal.extend(gullwing::rows_of(s, &tab_rows, &tab_layouts, "tab_w")?);

    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&metal, looks::TIN);
    Ok(())
}

/// TO-252 / DPAK: gull-wing leads, stubs where leads are trimmed at the
/// body, and the flat tab plate inlaid in the body's underside.
pub fn dpak(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let (lead_geometry, tab) = split_tab(input)?;
    let family: &Family = &gullwing::DPAK;
    let form = gullwing::lead_form(input)?;
    gullwing::body(s, input, family, form)?;
    let rows = common::perimeter_rows(&lead_geometry);
    gullwing::lead_params(s, input, &rows, family, form)?;
    let layouts = common::row_layouts(s, &rows, input.geometry.pitch);
    let mut metal = gullwing::rows_of(s, &rows, &layouts, "lead_w")?;

    // Trimmed leads: stubs at the lead positions the geometry left
    // without a pad.
    if let openparts_mcad::Details::TrimmedLeads(trimmed) = &input.geometry.details {
        for (n, y) in trimmed {
            s.fragment(
                fragment!("lead_stub"),
                &vars!(n = n.to_string(), y = mm(*y)),
            )?;
            metal.push(format!("Stub{n}"));
        }
    }

    let half_w = input.geometry.body.size.x / 2.0;
    let tab_end = tab.position.x + tab.size.x / 2.0;
    s.fragment(
        fragment!("tab_plate"),
        &vars!(
            tab_w = mm(tab.size.y),
            tab_reach = mm(tab_end - half_w),
            tab_len = mm(tab.size.x),
            tab_t = mm(tab.size.z.min(input.geometry.body.size.z * 0.4)),
        ),
    )?;
    s.fragment(
        fragment!("inlay"),
        &vars!(target = "Body", tools = vec!["Tab".to_string()]),
    )?;
    metal.push("Tab".into());

    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&metal, looks::TIN);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{build_script, run, Format, ModelInput};

    fn package(yaml_tail: &str) -> openparts_core::Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nlead_count: 3\n{yaml_tail}"
        ))
        .unwrap()
    }

    fn sot223() -> openparts_core::Package {
        package(
            "family: sot223\n\
             pitch: {nominal: 2.3, unit: mm}\n\
             tab: {number: '4'}\n\
             dimensions:\n\
             \x20 body_width: {nominal: 3.5, unit: mm}\n\
             \x20 body_length: {nominal: 6.5, unit: mm}\n\
             \x20 overall_width: {min: 6.7, max: 7.3, unit: mm}\n\
             \x20 tab_width: {min: 2.9, max: 3.1, unit: mm}\n",
        )
    }

    fn dpak() -> openparts_core::Package {
        package(
            "family: dpak\n\
             pitch: {nominal: 2.285, unit: mm}\n\
             tab: {number: '2', trimmed_leads: ['2']}\n\
             dimensions:\n\
             \x20 body_width: {nominal: 6.1, unit: mm}\n\
             \x20 body_length: {nominal: 6.54, unit: mm}\n\
             \x20 overall_width: {min: 9.40, max: 10.42, unit: mm}\n\
             \x20 tab_width: {min: 4.96, max: 5.46, unit: mm}\n\
             \x20 tab_length: {min: 0.88, max: 1.27, unit: mm}\n",
        )
    }

    /// The script's coloured bodies, and the STEP solid count when
    /// SolidCraft is installed.
    fn model(package: &openparts_core::Package) -> (Vec<String>, Option<usize>) {
        let geometry = openparts_mcad::generate(package).unwrap();
        let script = build_script(&ModelInput {
            package,
            geometry: &geometry,
            category: Some(openparts_core::ComponentCategory::Other),
            marked_pin: None,
        })
        .unwrap();
        let bodies = script
            .commands()
            .iter()
            .filter(|c| c["command"] == "appearance.assign")
            .flat_map(|c| c["params"]["bodies"].as_array().unwrap().clone())
            .map(|b| b.as_str().unwrap().to_string())
            .collect();
        let solids = crate::available_for_test(module_path!()).then(|| {
            let step = run(&script, "TEST", &[Format::Step]).unwrap().remove(0);
            String::from_utf8(step)
                .unwrap()
                .matches("MANIFOLD_SOLID_BREP(")
                .count()
        });
        (bodies, solids)
    }

    #[test]
    fn sot223_has_three_leads_and_a_wide_gull_wing_tab() {
        let (bodies, solids) = model(&sot223());
        assert_eq!(bodies, ["Body", "LeadL", "LeadL (2)", "LeadL (3)", "LeadR"]);
        assert!(solids.is_none_or(|n| n == 5), "{solids:?}");
    }

    #[test]
    fn dpak_has_two_leads_a_trimmed_stub_and_the_tab_plate() {
        let (bodies, solids) = model(&dpak());
        assert_eq!(bodies, ["Body", "LeadL", "LeadL (2)", "Stub2", "Tab"]);
        assert!(solids.is_none_or(|n| n == 5), "{solids:?}");
    }
}
