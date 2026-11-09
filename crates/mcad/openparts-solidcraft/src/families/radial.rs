//! Radial through-hole parts: aluminium electrolytics, T-1 LEDs and
//! crystals (HC-49/U or a small round can). The board top is z = 0.

use super::wire_d;
use crate::script::{looks, mm, Look};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_core::ComponentCategory;
use openparts_mcad::{Lead, MarkerKind};

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let leads: Vec<&Lead> = input.geometry.leads.iter().collect();
    if leads.is_empty() {
        return Err(ModelError::NoLeads(input.package.family.clone()));
    }
    let b = input.geometry.body.size;
    match input.category {
        Some(ComponentCategory::Led) => led(s, input, &leads),
        Some(ComponentCategory::Crystal) if b.x >= 8.0 => {
            // A slot outline takes plain numbers, so the can is fixed here;
            // its thickness is the HC-49/U proportion (a generator default).
            let (hw, t) = (b.x / 2.0, b.x * 0.42);
            s.fragment(
                fragment!("hc49"),
                &vars!(
                    can_w = mm(b.x),
                    can_h = mm(b.z),
                    half_w = hw,
                    neg_half_w = -hw,
                    thickness = t,
                    rim_half_w = hw + 0.25,
                    neg_rim_half_w = -(hw + 0.25),
                    rim_thickness = t + 0.5,
                ),
            )?;
            s.fragment(
                fragment!("tht_lead_params"),
                &vars!(pin_d = mm(wire_d(leads[0], 0.43))),
            )?;
            let wires = straight_wires(s, &leads)?;
            s.paint(&["Can".into()], looks::ALUMINIUM);
            s.paint(&["Rim".into()], looks::NICKEL);
            s.paint(&wires, looks::TIN);
            Ok(())
        }
        Some(ComponentCategory::Crystal) => {
            s.fragment(
                fragment!("small_can"),
                &vars!(can_d = mm(b.x), can_h = mm(b.z)),
            )?;
            s.fragment(
                fragment!("tht_lead_params"),
                &vars!(pin_d = mm(wire_d(leads[0], 0.3))),
            )?;
            let wires = straight_wires(s, &leads)?;
            s.paint(&["Can".into()], looks::ALUMINIUM);
            s.paint(&wires, looks::TIN);
            Ok(())
        }
        _ => electrolytic(s, input, &leads),
    }
}

/// Straight leads from the board up to the part (z = 0) and down through it.
fn straight_wires(s: &mut Script, leads: &[&Lead]) -> Result<Vec<String>, ModelError> {
    leads
        .iter()
        .map(|l| {
            let name = format!("Lead{}", l.number);
            s.fragment(
                fragment!("tht_wire"),
                &vars!(
                    name = name.clone(),
                    x = mm(l.position.x),
                    y = mm(l.position.y),
                    top = "0 mm"
                ),
            )?;
            Ok(name)
        })
        .collect()
}

fn electrolytic(s: &mut Script, input: &ModelInput<'_>, leads: &[&Lead]) -> Result<(), ModelError> {
    let g = input.geometry;
    // The stripe faces the negative lead (the geometry's marker side).
    let negative_on_plus_x = g
        .markers
        .iter()
        .find(|m| m.kind == MarkerKind::NegativeStripe)
        .is_none_or(|m| m.position.x >= 0.0);
    let (sleeve_neg, stripe_neg) = if negative_on_plus_x {
        ("-", "")
    } else {
        ("", "-")
    };
    s.fragment(
        fragment!("electrolytic"),
        &vars!(
            can_d = mm(g.body.size.x),
            can_h = mm(g.body.size.z),
            pin_d = mm(wire_d(leads[0], 0.5)),
            sleeve_neg = sleeve_neg,
            stripe_neg = stripe_neg,
        ),
    )?;
    let wires = straight_wires(s, leads)?;
    s.paint(&["Sleeve".into()], looks::SLEEVE);
    s.paint(&["Stripe".into()], looks::MARK_LIGHT);
    s.paint(&["Top".into()], looks::ALUMINIUM);
    s.paint(&["Bung".into()], looks::RUBBER);
    s.paint(&wires, looks::TIN);
    Ok(())
}

/// Pin 1 is the cathode (KiCad LED_D*.kicad_mod): it carries the
/// reflector anvil, and the flange is flattened on its side.
fn led(s: &mut Script, input: &ModelInput<'_>, leads: &[&Lead]) -> Result<(), ModelError> {
    let b = input.geometry.body.size;
    s.fragment(
        fragment!("radial_led"),
        &vars!(
            led_d = mm(b.x),
            led_h = mm(b.z),
            dome_r = mm(b.x / 2.0 * 0.97)
        ),
    )?;
    let mut frame = Vec::new();
    for (i, l) in leads.iter().enumerate() {
        let name = format!("Lead{}", l.number);
        let top = if i == 0 {
            "led_h * 0.45"
        } else {
            "led_h * 0.4"
        };
        s.fragment(
            fragment!("radial_led_lead"),
            &vars!(
                name = name.clone(),
                x = mm(l.position.x),
                y = mm(l.position.y),
                top = top
            ),
        )?;
        frame.push(name);
    }
    let cathode = leads[0];
    let flat_x = "led_d / 2 + 0.5 mm";
    s.fragment(
        fragment!("radial_led_finish"),
        &vars!(
            anvil_len = mm(cathode.position.x.abs() + 0.5),
            anvil_x = mm(cathode.position.x / 2.0),
            y = mm(cathode.position.y),
            cathode = frame[0].clone(),
            leads = frame.clone(),
            flat_x = if cathode.position.x <= 0.0 {
                format!("-({flat_x})")
            } else {
                flat_x.to_string()
            },
        ),
    )?;
    s.paint(
        &["Lens".into()],
        Look {
            color: "#f2d4d0",
            ..looks::LENS
        },
    );
    s.paint(&frame, looks::TIN);
    Ok(())
}
