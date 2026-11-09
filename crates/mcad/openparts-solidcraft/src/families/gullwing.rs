//! Gull-wing leaded packages (QFP, SOIC/SOP, SOT): a drafted mold body
//! with bent leads on two or four edges, sharing every lead parameter.

use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::LeadForm;

/// Modelling choices per family -- how the molding and the bend are
/// drawn, not package dimensions: lead thickness, foot length and
/// standoff come from the geometry's `LeadForm` (datasheet value, else
/// openparts-mcad's family default), the same source as the footprint.
pub struct Family {
    /// Whether the top carries a pin-1 dimple (SOT-23 class parts don't).
    pub dimple: bool,
    pub parting_ratio: f64,
    pub draft_deg: f64,
    /// Straight run out of the body before the bend.
    pub lead_exit: f64,
    /// Lead length kept inside the body.
    pub lead_embed: f64,
}

/// LQFP/TQFP (JEDEC MS-026).
pub const QFP: Family = Family {
    dimple: true,
    parting_ratio: 0.45,
    draft_deg: 12.0,
    lead_exit: 0.25,
    lead_embed: 0.2,
};

/// SOIC/SOP (JEDEC MS-012).
pub const SOIC: Family = Family {
    dimple: true,
    parting_ratio: 0.4,
    draft_deg: 10.0,
    lead_exit: 0.2,
    lead_embed: 0.2,
};

/// SOT-23 class: small body, leads leave low on the body.
pub const SOT: Family = Family {
    dimple: false,
    parting_ratio: 0.35,
    draft_deg: 8.0,
    lead_exit: 0.1,
    lead_embed: 0.15,
};

pub fn build(s: &mut Script, input: &ModelInput<'_>, family: &Family) -> Result<(), ModelError> {
    let g = input.geometry;
    let rows = common::perimeter_rows(g);
    if rows.is_empty() {
        return Err(ModelError::NoLeads(input.package.family.clone()));
    }
    let Some(LeadForm::GullWing {
        thickness,
        foot_length,
        standoff,
    }) = g.lead_form
    else {
        return Err(ModelError::UnsupportedFamily(format!(
            "{} (geometry has no gull-wing lead form)",
            input.package.family
        )));
    };
    common::body_params(s, g)?;
    common::molded_body(s, standoff, family.parting_ratio, family.draft_deg)?;
    if family.dimple {
        common::pin1_dimple(s, g, "Body", "standoff + body_h")?;
    }
    let layouts = common::row_layouts(s, &rows, input.package.pitch.nominal);

    // The geometry fixes the lead width, where the toe lands and the
    // foot length; the bend takes what is left of the reach (the foot is
    // shortened only if the reach can't fit both).
    let (_, width) = rows[0].lead_size();
    let half = if rows[0].side.across_x() {
        g.body.size.x
    } else {
        g.body.size.y
    } / 2.0;
    let reach = rows[0].outer_extent() - half;
    let exit = family.lead_exit.min(reach * 0.3);
    let foot = foot_length.min(reach - exit - 0.1).max(0.1);
    s.fragment(
        fragment!("gullwing_lead"),
        &vars!(
            lead_w = mm(width),
            lead_t = mm(thickness),
            reach = mm(reach),
            foot = mm(foot),
            exit = mm(exit),
            embed = mm(family.lead_embed),
        ),
    )?;
    let leads = common::lead_rows(s, &rows, &layouts, fragment!("gullwing_row"), "Lead")?;
    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&leads, looks::TIN);
    Ok(())
}
