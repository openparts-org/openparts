//! Tactile switches (openparts-mcad's `tact_switch_smd` / `_tht`): the
//! housing, a round actuator, and SMD feet or straight THT pins.

use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::Mounting;

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    if g.leads.is_empty() {
        return Err(ModelError::NoLeads(input.geometry.family));
    }
    let openparts_mcad::Details::Switch(f) = g.details else {
        return Err(ModelError::MissingDetails(g.family));
    };
    let b = &g.body;
    let standoff = b.position.z - b.size.z / 2.0;
    crate::common::body_params(s, g)?;
    s.fragment(
        fragment!("tact_switch"),
        &vars!(
            standoff = mm(standoff),
            act_d = mm(f.actuator_diameter),
            act_top = mm(f.actuator_top),
        ),
    )?;
    let mut metal = Vec::new();
    for (i, l) in g.leads.iter().enumerate() {
        let name = format!("Terminal{}", i + 1);
        match l.mounting {
            Mounting::Smd => {
                let sign = l.position.x.signum();
                let toe = l.position.x.abs() + l.size.x / 2.0;
                let inner = (b.size.x / 2.0 - 0.3).min(toe - l.size.x / 2.0);
                s.fragment(
                    fragment!("tact_smd_lead"),
                    &vars!(
                        name = name.clone(),
                        x0 = mm(sign * inner),
                        x1 = mm(sign * toe),
                        y = mm(l.position.y),
                        w = mm(l.size.y),
                        t = mm(l.size.z),
                    ),
                )?;
            }
            Mounting::ThroughHole => {
                let drill = l.drill.unwrap_or(1.0);
                s.fragment(
                    fragment!("tact_tht_pin"),
                    &vars!(
                        name = name.clone(),
                        x = mm(l.position.x),
                        y = mm(l.position.y),
                        w = mm(drill * 0.7),
                        t = mm((drill * 0.35).max(0.2)),
                        tail = mm(f.tail_length),
                    ),
                )?;
            }
        }
        metal.push(name);
    }
    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&["Actuator".into()], looks::FERRITE);
    s.paint(&metal, looks::TIN);
    Ok(())
}
