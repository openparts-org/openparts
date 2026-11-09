//! Gull-wing leaded packages (QFP, SOIC/SOP, SOT): a drafted mold body
//! with bent leads on two or four edges, sharing every lead parameter.

use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};

/// Generator defaults per family (JEDEC-typical; not datasheet facts).
pub struct Family {
    /// Whether the top carries a pin-1 dimple (SOT-23 class parts don't).
    pub dimple: bool,
    pub standoff: f64,
    pub parting_ratio: f64,
    pub draft_deg: f64,
    pub lead_t: f64,
    /// Straight run out of the body before the bend.
    pub lead_exit: f64,
    /// Lead length kept inside the body.
    pub lead_embed: f64,
}

/// LQFP/TQFP (JEDEC MS-026): 0.15 mm lead frame, 0.1 mm standoff.
pub const QFP: Family = Family {
    dimple: true,
    standoff: 0.1,
    parting_ratio: 0.45,
    draft_deg: 12.0,
    lead_t: 0.15,
    lead_exit: 0.25,
    lead_embed: 0.2,
};

/// SOIC/SOP (JEDEC MS-012): thicker 0.2 mm leads.
pub const SOIC: Family = Family {
    dimple: true,
    standoff: 0.15,
    parting_ratio: 0.4,
    draft_deg: 10.0,
    lead_t: 0.2,
    lead_exit: 0.2,
    lead_embed: 0.2,
};

/// SOT-23 class: small body, leads leave low on the body.
pub const SOT: Family = Family {
    dimple: false,
    standoff: 0.05,
    parting_ratio: 0.35,
    draft_deg: 8.0,
    lead_t: 0.12,
    lead_exit: 0.1,
    lead_embed: 0.15,
};

pub fn build(s: &mut Script, input: &ModelInput<'_>, family: &Family) -> Result<(), ModelError> {
    let g = input.geometry;
    let rows = common::perimeter_rows(g);
    if rows.is_empty() {
        return Err(ModelError::NoLeads(input.package.family.clone()));
    }
    common::body_params(s, g)?;
    common::molded_body(s, family.standoff, family.parting_ratio, family.draft_deg)?;
    if family.dimple {
        common::pin1_dimple(s, g, "Body", "standoff + body_h")?;
    }
    let layouts = common::row_layouts(s, &rows, input.package.pitch.nominal);

    // The footprint fixes the lead width and where the toe lands; the
    // foot and the bend share what is left of the reach.
    let (out, width) = rows[0].lead_size();
    let half = if rows[0].side.across_x() {
        g.body.size.x
    } else {
        g.body.size.y
    } / 2.0;
    let reach = rows[0].outer_extent() - half;
    let exit = family.lead_exit.min(reach * 0.3);
    let foot = (out * 0.75).min(reach - exit - 0.1).max(0.1);
    s.fragment(
        fragment!("gullwing_lead"),
        &vars!(
            lead_w = mm(width),
            lead_t = mm(family.lead_t),
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
