//! Two-terminal molded diodes with leads (openparts-mcad's `sod`,
//! `sod_flat`, `do214`): a drafted molding with a cathode band, and
//! gull-wing, flat or J-bend leads at each end.

use super::gullwing::{self, Family};
use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_core::ComponentCategory;
use openparts_mcad::LeadForm;

/// SOD-123 class: small body, leads leave low, no pin-1 dimple.
const SOD: Family = Family {
    dimple: false,
    parting_ratio: 0.3,
    draft_deg: 8.0,
    lead_exit: 0.08,
    lead_embed: 0.15,
};

/// Flat-lead and J-bend moldings: parting ratio and draft.
const FLAT_PARTING: f64 = 0.15;
const DO214_PARTING: f64 = 0.3;
const DRAFT_DEG: f64 = 6.0;

pub fn sod(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    gullwing::build(s, input, &SOD)?;
    let (_, _, standoff) = gullwing::lead_form(input)?;
    band(s, input, standoff, SOD.parting_ratio, SOD.draft_deg)
}

pub fn sod_flat(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let Some(LeadForm::Flat { thickness }) = g.lead_form else {
        return Err(no_form(input, "flat"));
    };
    let rows = common::perimeter_rows(g);
    if rows.len() != 2 {
        return Err(ModelError::NoLeads(input.geometry.family));
    }
    common::body_params(s, g)?;
    common::molded_body(s, 0.0, FLAT_PARTING, DRAFT_DEG)?;
    let (_, width) = rows[0].lead_size();
    let reach = rows[0].outer_extent() - g.body.size.x / 2.0;
    s.fragment(
        fragment!("flat_lead"),
        &vars!(
            lead_w = mm(width),
            lead_t = mm(thickness),
            reach = mm(reach),
            embed = mm((g.body.size.x * 0.15).min(0.4)),
        ),
    )?;
    let layouts = common::row_layouts(s, &rows, None);
    let leads = common::lead_rows(s, &rows, &layouts, fragment!("flat_lead_row"), "Lead")?;
    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&leads, looks::TIN);
    band(s, input, 0.0, FLAT_PARTING, DRAFT_DEG)
}

pub fn do214(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let Some(LeadForm::JBend {
        thickness,
        foot_length,
    }) = g.lead_form
    else {
        return Err(no_form(input, "J-bend"));
    };
    let rows = common::perimeter_rows(g);
    if rows.len() != 2 {
        return Err(ModelError::NoLeads(input.geometry.family));
    }
    common::body_params(s, g)?;
    // The body sits on the folded feet.
    common::molded_body(s, thickness, DO214_PARTING, DRAFT_DEG)?;
    let (_, width) = rows[0].lead_size();
    let reach = rows[0].outer_extent() - g.body.size.x / 2.0;
    let bend_r = (thickness * 0.5)
        .min((reach - thickness) * 0.4)
        .min((foot_length - thickness) * 0.3)
        .max(0.005);
    s.fragment(
        fragment!("jbend_lead"),
        &vars!(
            lead_w = mm(width),
            lead_t = mm(thickness),
            reach = mm(reach),
            foot = mm(foot_length),
            embed = mm((g.body.size.x * 0.08).max(0.2)),
            bend_r = mm(bend_r),
        ),
    )?;
    let layouts = common::row_layouts(s, &rows, None);
    let leads = common::lead_rows(s, &rows, &layouts, fragment!("jbend_row"), "Lead")?;
    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&leads, looks::TIN);
    band(s, input, thickness, DO214_PARTING, DRAFT_DEG)
}

fn no_form(input: &ModelInput<'_>, expected: &'static str) -> ModelError {
    ModelError::LeadForm {
        family: input.geometry.family,
        expected,
    }
}

/// The cathode band at the marked terminal's end, for diodes (and any
/// part whose device names a polarity pin).
fn band(
    s: &mut Script,
    input: &ModelInput<'_>,
    standoff: f64,
    parting_ratio: f64,
    draft_deg: f64,
) -> Result<(), ModelError> {
    let polar = input.marked_pin.is_some()
        || matches!(
            input.category,
            Some(ComponentCategory::Diode | ComponentCategory::Led)
        );
    if !polar {
        return Ok(());
    }
    let h = input.geometry.body.size.z;
    let inset = h * (1.0 - parting_ratio) * draft_deg.to_radians().tan() + 0.05;
    let at_minus_x = common::marked_lead(input).is_none_or(|l| l.position.x <= 0.0);
    s.fragment(
        fragment!("diode_band"),
        &vars!(
            neg = if at_minus_x { "-" } else { "" },
            inset = mm(inset),
            top = format!("{} + body_h", mm(standoff)),
        ),
    )?;
    s.paint(&["PolarityBand".into()], looks::MARK_LIGHT);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{build_script, run, Format, ModelInput};
    use openparts_core::ComponentCategory;

    pub(crate) fn package(yaml: &str) -> openparts_core::Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\n{yaml}"
        ))
        .unwrap()
    }

    /// The script's coloured bodies, and the STEP solid count when
    /// SolidCraft is installed.
    pub(crate) fn model(
        package: &openparts_core::Package,
        category: ComponentCategory,
    ) -> (Vec<String>, Option<usize>) {
        let geometry = openparts_mcad::generate(package).unwrap();
        let script = build_script(&ModelInput {
            package,
            geometry: &geometry,
            category: Some(category),
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
        if let Ok(dir) = std::env::var("OPENPARTS_DUMP_PNG") {
            let png = run(&script, "TEST", &[Format::Png]).unwrap().remove(0);
            let name = format!("{}-{}.png", package.family, package.lead_count);
            std::fs::write(std::path::Path::new(&dir).join(name), png).unwrap();
        }
        let solids = crate::available().then(|| {
            let step = run(&script, "TEST", &[Format::Step]).unwrap().remove(0);
            String::from_utf8(step)
                .unwrap()
                .matches("MANIFOLD_SOLID_BREP(")
                .count()
        });
        (bodies, solids)
    }

    const SOD123: &str = "family: sod\nlead_count: 2\npitch: {unit: mm}\ndimensions:\n\
        \x20 body_length: {min: 2.55, max: 2.85, unit: mm}\n\
        \x20 body_width: {min: 1.40, max: 1.70, unit: mm}\n\
        \x20 body_height: {min: 1.00, max: 1.35, unit: mm}\n\
        \x20 overall_length: {min: 3.55, max: 3.85, unit: mm}\n";

    #[test]
    fn sod_has_bent_leads_and_a_cathode_band() {
        let (bodies, solids) = model(&package(SOD123), ComponentCategory::Diode);
        assert_eq!(bodies, ["Body", "LeadL", "LeadR", "PolarityBand"]);
        assert!(solids.is_none_or(|n| n == 4), "{solids:?}");
    }

    #[test]
    fn sod_flat_has_flat_leads() {
        let (bodies, solids) = model(
            &package(&SOD123.replace("family: sod", "family: sod_flat")),
            ComponentCategory::Diode,
        );
        assert_eq!(bodies, ["Body", "LeadL", "LeadR", "PolarityBand"]);
        assert!(solids.is_none_or(|n| n == 4), "{solids:?}");
    }

    #[test]
    fn do214_has_j_bend_leads() {
        let p = package(
            "family: do214\nlead_count: 2\npitch: {unit: mm}\ndimensions:\n\
             \x20 body_length: {min: 3.99, max: 4.50, unit: mm}\n\
             \x20 body_width: {min: 2.54, max: 2.79, unit: mm}\n\
             \x20 body_height: {min: 1.98, max: 2.29, unit: mm}\n\
             \x20 overall_length: {min: 4.93, max: 5.28, unit: mm}\n\
             \x20 lead_width: {min: 1.25, max: 1.65, unit: mm}\n\
             \x20 lead_length: {min: 0.76, max: 1.52, unit: mm}\n\
             \x20 lead_thickness: {min: 0.15, max: 0.31, unit: mm}\n",
        );
        let (bodies, solids) = model(&p, ComponentCategory::Diode);
        assert_eq!(bodies, ["Body", "LeadL", "LeadR", "PolarityBand"]);
        assert!(solids.is_none_or(|n| n == 4), "{solids:?}");
    }

    #[test]
    fn dfn_d2pak_and_to220_build() {
        let dfn = package(
            "family: dfn\nlead_count: 8\npitch: {nominal: 0.5, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 2.0, unit: mm}\n\
             \x20 body_length: {nominal: 2.0, unit: mm}\n\
             \x20 exposed_pad: {width: {nominal: 0.9, unit: mm}, length: {nominal: 1.6, unit: mm}}\n",
        );
        let (bodies, solids) = model(&dfn, ComponentCategory::Other);
        assert_eq!(bodies.len(), 10, "{bodies:?}");
        assert!(solids.is_none_or(|n| n == 10), "{solids:?}");

        let d2pak = package(
            "family: d2pak\nlead_count: 3\npitch: {nominal: 2.54, unit: mm}\n\
             tab: {number: '2', trimmed_leads: ['2']}\ndimensions:\n\
             \x20 body_width: {min: 8.38, max: 9.65, unit: mm}\n\
             \x20 body_length: {min: 9.65, max: 10.67, unit: mm}\n\
             \x20 overall_width: {min: 14.61, max: 15.88, unit: mm}\n\
             \x20 tab_width: {min: 6.5, max: 8.0, unit: mm}\n",
        );
        let (bodies, solids) = model(&d2pak, ComponentCategory::Other);
        assert_eq!(bodies, ["Body", "LeadL", "LeadL (2)", "Stub2", "Tab"]);
        assert!(solids.is_none_or(|n| n == 5), "{solids:?}");

        let to220 = package(
            "family: to220\nlead_count: 3\npitch: {nominal: 2.54, unit: mm}\ndimensions:\n\
             \x20 body_width: {min: 9.66, max: 10.66, unit: mm}\n\
             \x20 body_length: {min: 4.4, max: 4.6, unit: mm}\n\
             \x20 body_height: {min: 8.6, max: 9.4, unit: mm}\n",
        );
        let (bodies, solids) = model(&to220, ComponentCategory::Transistor);
        assert_eq!(bodies, ["Body", "Tab", "Lead1", "Lead2", "Lead3"]);
        assert!(solids.is_none_or(|n| n == 5), "{solids:?}");
    }

    #[test]
    fn lga_tact_switches_and_usb_c_build() {
        let lga = package(
            "family: lga\nlead_count: 8\nlead_layout: [4, 0, 4, 0]\n\
             pitch: {nominal: 0.65, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 2.5, unit: mm}\n\
             \x20 body_length: {nominal: 2.5, unit: mm}\n\
             \x20 body_height: {nominal: 0.93, unit: mm}\n\
             \x20 lead_length: {nominal: 0.5, unit: mm}\n\
             \x20 lead_width: {nominal: 0.35, unit: mm}\n\
             \x20 lead_inset: {nominal: 0.1, unit: mm}\n",
        );
        let (bodies, solids) = model(&lga, ComponentCategory::Other);
        assert_eq!(bodies.len(), 9, "{bodies:?}");
        assert!(solids.is_none_or(|n| n == 9), "{solids:?}");

        let tht = package(
            "family: tact_switch_tht\nlead_count: 4\npitch: {nominal: 4.5, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 6.0, unit: mm}\n\
             \x20 body_length: {nominal: 6.0, unit: mm}\n\
             \x20 body_height: {nominal: 3.5, unit: mm}\n\
             \x20 overall_height: {nominal: 5.0, unit: mm}\n\
             \x20 actuator_diameter: {nominal: 3.5, unit: mm}\n\
             \x20 row_spacing: {nominal: 6.5, unit: mm}\n\
             \x20 hole_diameter: {nominal: 1.0, unit: mm}\n",
        );
        let (bodies, solids) = model(&tht, ComponentCategory::Other);
        assert_eq!(
            bodies,
            [
                "Body",
                "Actuator",
                "Terminal1",
                "Terminal2",
                "Terminal3",
                "Terminal4"
            ]
        );
        assert!(solids.is_none_or(|n| n == 6), "{solids:?}");

        let smd = package(
            "family: tact_switch_smd\nlead_count: 4\npitch: {nominal: 4.5, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 6.0, unit: mm}\n\
             \x20 body_length: {nominal: 6.0, unit: mm}\n\
             \x20 body_height: {nominal: 3.5, unit: mm}\n\
             \x20 overall_width: {nominal: 9.0, unit: mm}\n\
             \x20 lead_width: {nominal: 0.7, unit: mm}\n\
             \x20 lead_length: {nominal: 1.0, unit: mm}\n",
        );
        let (_, solids) = model(&smd, ComponentCategory::Other);
        assert!(solids.is_none_or(|n| n == 6), "{solids:?}");

        let usb = package(
            "family: usb_c\nlead_count: 6\npitch: {unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 8.94, unit: mm}\n\
             \x20 body_length: {nominal: 7.35, unit: mm}\n\
             \x20 body_height: {nominal: 3.16, unit: mm}\n\
             \x20 body_offset: {nominal: -0.5, unit: mm}\n\
             pads:\n\
             \x20 - {number: A5, x: -0.25, y: -4.0, width: 0.3, length: 1.0}\n\
             \x20 - {number: B5, x: 0.25, y: -4.0, width: 0.3, length: 1.0}\n\
             \x20 - {number: S1, x: -4.32, y: -3.3, width: 1.0, length: 2.1, drill: 0.6, drill_length: 1.7}\n\
             \x20 - {number: S1, x: 4.32, y: -3.3, width: 1.0, length: 2.1, drill: 0.6, drill_length: 1.7}\n\
             \x20 - {number: S1, x: -4.32, y: 0.9, width: 1.0, length: 1.6, drill: 0.6, drill_length: 1.2}\n\
             \x20 - {number: S1, x: 4.32, y: 0.9, width: 1.0, length: 1.6, drill: 0.6, drill_length: 1.2}\n",
        );
        let (bodies, solids) = model(&usb, ComponentCategory::Connector);
        assert_eq!(bodies.len(), 8, "{bodies:?}");
        assert!(solids.is_none_or(|n| n == 8), "{solids:?}");
    }
}
