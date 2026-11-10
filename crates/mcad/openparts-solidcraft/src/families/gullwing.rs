//! Gull-wing leaded packages (QFP, SOIC/SOP/TSSOP/SSOP/MSOP, SOT): a drafted mold body
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

/// TSSOP (MO-153), MSOP/VSSOP (MO-187): thin bodies, leads mid-height.
pub const THIN_SOP: Family = Family {
    dimple: true,
    parting_ratio: 0.45,
    draft_deg: 8.0,
    lead_exit: 0.15,
    lead_embed: 0.15,
};

/// SSOP (MO-150).
pub const SSOP: Family = Family {
    dimple: true,
    parting_ratio: 0.4,
    draft_deg: 10.0,
    lead_exit: 0.2,
    lead_embed: 0.2,
};

/// SOT-223: leads and tab leave low on a thick body, no pin-1 dimple.
pub const SOT223: Family = Family {
    dimple: false,
    parting_ratio: 0.35,
    draft_deg: 8.0,
    lead_exit: 0.25,
    lead_embed: 0.3,
};

/// TO-252 / DPAK: leads leave just above the tab plate, no dimple.
pub const DPAK: Family = Family {
    dimple: false,
    parting_ratio: 0.3,
    draft_deg: 6.0,
    lead_exit: 0.4,
    lead_embed: 0.4,
};

pub fn build(s: &mut Script, input: &ModelInput<'_>, family: &Family) -> Result<(), ModelError> {
    let g = input.geometry;
    let rows = common::perimeter_rows(g);
    if rows.is_empty() {
        return Err(ModelError::NoLeads(input.package.family.clone()));
    }
    let form = lead_form(input)?;
    body(s, input, family, form)?;
    lead_params(s, input, &rows, family, form)?;
    let layouts = common::row_layouts(s, &rows, input.package.pitch.nominal);
    let leads = rows_of(s, &rows, &layouts, "lead_w")?;
    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&leads, looks::TIN);
    Ok(())
}

/// The geometry's gull-wing lead form: (thickness, foot length, standoff).
pub fn lead_form(input: &ModelInput<'_>) -> Result<(f64, f64, f64), ModelError> {
    match input.geometry.lead_form {
        Some(LeadForm::GullWing {
            thickness,
            foot_length,
            standoff,
        }) => Ok((thickness, foot_length, standoff)),
        _ => Err(ModelError::UnsupportedFamily(format!(
            "{} (geometry has no gull-wing lead form)",
            input.package.family
        ))),
    }
}

/// The drafted mold `Body` (and its pin-1 dimple if the family has one).
pub fn body(
    s: &mut Script,
    input: &ModelInput<'_>,
    family: &Family,
    (_, _, standoff): (f64, f64, f64),
) -> Result<(), ModelError> {
    let g = input.geometry;
    common::body_params(s, g)?;
    common::molded_body(s, standoff, family.parting_ratio, family.draft_deg)?;
    if family.dimple {
        common::pin1_dimple(s, g, "Body", "standoff + body_h")?;
    }
    Ok(())
}

/// Declares the lead parameters shared by every row, from `rows[0]`.
pub fn lead_params(
    s: &mut Script,
    input: &ModelInput<'_>,
    rows: &[common::Row<'_>],
    family: &Family,
    (thickness, foot_length, standoff): (f64, f64, f64),
) -> Result<(), ModelError> {
    let g = input.geometry;
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
    let parting = standoff + g.body.size.z * family.parting_ratio;
    let bend_r = bend_radius(thickness, reach, foot, exit, family.lead_embed, parting);
    s.fragment(
        fragment!("gullwing_lead"),
        &vars!(
            lead_w = mm(width),
            lead_t = mm(thickness),
            reach = mm(reach),
            foot = mm(foot),
            exit = mm(exit),
            embed = mm(family.lead_embed),
            bend_r = mm(bend_r),
        ),
    )?;
    Ok(())
}

/// The inner bend radius of the gull-wing profile (`gullwing_row.yaml`):
/// 0.75 x the lead thickness, reduced so every bend fits on its straight
/// segments. A fillet of radius r between a horizontal run and the slope
/// takes r * tan(theta / 2) of each, where theta is the slope's angle; the
/// outer bends use r + t. Thin, short-reach parts (TSOT-23) need the
/// smaller radius, or the sketch fillet is refused.
fn bend_radius(t: f64, reach: f64, foot: f64, exit: f64, embed: f64, parting: f64) -> f64 {
    let dx = (reach - foot - exit).max(1e-3);
    let dz = (parting - t).max(1e-3);
    let k = (dz.atan2(dx) / 2.0).tan(); // tangent length per unit radius
    let slope = dx.hypot(dz);
    // Room per segment (with a 10 % margin), as `radius sum * k <= length`.
    let limits = [
        slope / (2.0 * k) - t / 2.0,  // slope: (r + t) + r on each face
        (embed + exit) / k - t,       // top run into the upper bend: r + t
        (embed + exit - t * 0.5) / k, // bottom run: r
        foot / k,                     // foot top: r
        (foot + t * 0.5) / k - t,     // foot bottom: r + t
    ];
    let fit = limits.iter().copied().fold(f64::INFINITY, f64::min) * 0.9;
    (t * 0.75).min(fit).max(0.005)
}

/// Gull-wing lead rows extruded to the parameter `width`; returns the bodies.
pub fn rows_of(
    s: &mut Script,
    rows: &[common::Row<'_>],
    layouts: &[common::RowLayout],
    width: &str,
) -> Result<Vec<String>, ModelError> {
    Ok(common::lead_rows_with(
        s,
        rows,
        layouts,
        fragment!("gullwing_row"),
        "Lead",
        &vars!(w = width),
    )?)
}
