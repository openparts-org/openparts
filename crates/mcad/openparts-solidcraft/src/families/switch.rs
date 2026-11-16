//! Tactile switches (openparts-mcad's `tact_switch_smd` / `_tht`): the
//! housing, a round actuator on top or an actuator leaving the +Y face
//! (side-operated), and SMD feet or straight THT pins (a ground terminal's
//! foot runs out along -Y).

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
    match f.side {
        None => s.fragment(
            fragment!("tact_switch"),
            &vars!(
                standoff = mm(standoff),
                act_d = mm(f.actuator_diameter),
                act_top = mm(f.actuator_top),
            ),
        )?,
        Some(a) if a.round => s.fragment(
            fragment!("tact_switch_side"),
            &vars!(
                standoff = mm(standoff),
                act_d = mm(a.width),
                act_len = mm(a.length),
                act_z = mm(a.elevation),
            ),
        )?,
        Some(a) => s.fragment(
            fragment!("tact_switch_side_rect"),
            &vars!(
                standoff = mm(standoff),
                act_w = mm(a.width),
                act_t = mm(a.thickness),
                act_len = mm(a.length),
                act_z = mm(a.elevation),
            ),
        )?,
    }
    let mut metal = Vec::new();
    for (i, l) in g.leads.iter().enumerate() {
        let name = format!("Terminal{}", i + 1);
        match l.mounting {
            Mounting::Smd if f.ground_lead == Some(i) => {
                let tip = l.position.y - l.size.y / 2.0;
                let inner = -(b.size.y / 2.0 - 0.3).max(0.0);
                s.fragment(
                    fragment!("tact_smd_ground_lead"),
                    &vars!(
                        name = name.clone(),
                        y0 = mm(inner),
                        y1 = mm(tip),
                        x = mm(l.position.x),
                        w = mm(l.size.x),
                        t = mm(l.size.z),
                    ),
                )?;
            }
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
            Mounting::NonPlated => continue,
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

#[cfg(test)]
mod tests {
    use crate::families::diode::tests::{model, package};
    use openparts_core::ComponentCategory;

    #[test]
    fn side_operated_smd_switch_with_a_ground_foot() {
        let p = package(
            "family: tact_switch_smd\nlead_count: 3\npitch: {nominal: 3.4, unit: mm}\n\
             actuation: side\ndimensions:\n\
             \x20 body_width: {nominal: 3.0, unit: mm}\n\
             \x20 body_length: {nominal: 2.5, unit: mm}\n\
             \x20 body_height: {nominal: 1.2, unit: mm}\n\
             \x20 overall_width: {nominal: 4.0, unit: mm}\n\
             \x20 lead_width: {nominal: 1.4, unit: mm}\n\
             \x20 ground_terminal_offset: {nominal: 1.9, unit: mm}\n\
             \x20 ground_terminal_width: {nominal: 0.5, unit: mm}\n\
             \x20 actuator_length: {nominal: 0.7, unit: mm}\n\
             \x20 actuator_width: {nominal: 1.7, unit: mm}\n\
             \x20 actuator_thickness: {nominal: 1.0, unit: mm}\n",
        );
        let (bodies, solids) = model(&p, ComponentCategory::Other);
        assert_eq!(
            bodies,
            ["Body", "Actuator", "Terminal1", "Terminal2", "Terminal3"]
        );
        assert!(solids.is_none_or(|n| n == 5), "{solids:?}");
    }

    #[test]
    fn side_operated_tht_switch_on_its_pcb_holes() {
        let p = package(
            "family: tact_switch_tht\nlead_count: 4\npitch: {nominal: 2.5, unit: mm}\n\
             actuation: side\ndimensions:\n\
             \x20 body_width: {nominal: 7.3, unit: mm}\n\
             \x20 body_length: {nominal: 6.25, unit: mm}\n\
             \x20 body_height: {nominal: 7.4, unit: mm}\n\
             \x20 actuator_diameter: {nominal: 3.5, unit: mm}\n\
             \x20 actuator_length: {nominal: 0.9, unit: mm}\n\
             \x20 actuator_elevation: {nominal: 4.0, unit: mm}\n\
             pads:\n\
             \x20 - {number: '2', x: -2.25, y: 0.875, drill: 1.0}\n\
             \x20 - {number: '1', x: 2.25, y: 0.875, drill: 1.0}\n\
             \x20 - {number: '4', x: -3.5, y: -1.625, drill: 1.5}\n\
             \x20 - {number: '3', x: 3.5, y: -1.625, drill: 1.5}\n",
        );
        let (bodies, solids) = model(&p, ComponentCategory::Other);
        assert_eq!(bodies.len(), 6, "{bodies:?}");
        assert!(solids.is_none_or(|n| n == 6), "{solids:?}");
    }
}
