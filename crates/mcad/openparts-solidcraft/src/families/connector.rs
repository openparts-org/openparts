//! The connector families (openparts-mcad's `connector.rs`): a body per
//! family -- card socket frame and shell, barrel-jack housing with its
//! plug opening, terminal-block housing with a wire entry per pole, FPC
//! housing with slot and actuator, RF base with its coupling barrel,
//! micro- and mini-USB shell, USB-B shell around its insulator and plug
//! mouth -- plus a foot on every SMD pad and a pin or tab down
//! every plated hole. The electromechanical families (relay housing,
//! buzzer can or box with its sound hole, trimmer housing with its
//! slotted rotor, encoder housing with bushing and shaft) share the
//! contacts.

use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::{Mounting, Orientation, PackageFamily as F};

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    if g.leads.is_empty() {
        return Err(ModelError::NoLeads(g.family));
    }
    let (tail_length, connector, electromech) = match &g.details {
        openparts_mcad::Details::Connector(f) => (f.tail_length, Some(*f), None),
        openparts_mcad::Details::Electromechanical(e) => (e.tail_length, None, Some(*e)),
        _ => return Err(ModelError::MissingDetails(g.family)),
    };
    let b = &g.body;
    common::body_params(s, g)?;
    s.fragment(
        fragment!("connector_params"),
        &vars!(z0 = mm(b.position.z - b.size.z / 2.0)),
    )?;
    // (body, look) pairs to paint after the contacts are placed.
    let mut looks_of: Vec<(&str, crate::script::Look)> = Vec::new();
    match (connector, electromech) {
        (_, Some(e)) => electromechanical(s, g.family, &e, &mut looks_of)?,
        (Some(f), None) => housing(s, g, &f, &mut looks_of)?,
        (None, None) => return Err(ModelError::MissingDetails(g.family)),
    }
    contacts(s, g, tail_length, looks_of)
}

fn electromechanical(
    s: &mut Script,
    family: F,
    e: &openparts_mcad::ElectromechanicalFeatures,
    looks_of: &mut Vec<(&str, crate::script::Look)>,
) -> Result<(), ModelError> {
    match family {
        F::Relay => {
            s.fragment(fragment!("relay"), &vars!())?;
            looks_of.push(("Housing", looks::MOLD));
        }
        F::Buzzer => {
            match e.can_diameter {
                Some(d) => s.fragment(fragment!("buzzer_round"), &vars!(can_d = mm(d)))?,
                None => s.fragment(fragment!("buzzer_square"), &vars!())?,
            }
            looks_of.push(("Housing", looks::MOLD));
        }
        F::Trimmer => {
            s.fragment(
                fragment!("trimmer"),
                &vars!(rotor_d = mm(e.top_diameter), rotor_h = mm(e.top_height)),
            )?;
            looks_of.extend([("Housing", looks::TRIMMER), ("Rotor", looks::NYLON_NATURAL)]);
        }
        F::RotaryEncoder => {
            s.fragment(
                fragment!("rotary_encoder"),
                &vars!(shaft_d = mm(e.top_diameter), rise = mm(e.top_height)),
            )?;
            looks_of.extend([
                ("Housing", looks::MOLD),
                ("Bushing", looks::NICKEL),
                ("Shaft", looks::NICKEL),
            ]);
        }
        _ => return Err(ModelError::MissingDetails(family)),
    }
    Ok(())
}

fn housing(
    s: &mut Script,
    g: &openparts_mcad::MechanicalGeometry,
    f: &openparts_mcad::ConnectorFeatures,
    looks_of: &mut Vec<(&str, crate::script::Look)>,
) -> Result<(), ModelError> {
    let b = &g.body;
    let shell_t = f.shell_thickness.min(b.size.z * 0.2);
    match g.family {
        F::CardSocket => {
            s.fragment(fragment!("card_socket"), &vars!(shell_t = mm(shell_t)))?;
            looks_of.extend([("Housing", looks::MOLD), ("Shell", looks::NICKEL)]);
        }
        F::BarrelJack => {
            let bore = f.mating_diameter.min(b.size.x * 0.9).min(b.size.z * 0.9);
            s.fragment(
                fragment!("barrel_jack"),
                &vars!(
                    bore_d = mm(bore),
                    pin_d = mm(f.pin_diameter.min(bore * 0.6))
                ),
            )?;
            looks_of.extend([("Housing", looks::MOLD), ("Pin", looks::TIN)]);
        }
        F::TerminalBlock => {
            s.fragment(fragment!("terminal_block"), &vars!())?;
            let poles = pole_positions(g);
            let w = pole_width(g, &poles);
            let side = f.orientation == Orientation::RightAngle;
            let mut cuts = Vec::new();
            for (i, x) in poles.iter().enumerate() {
                let name = format!("Entry{}", i + 1);
                let frag = if side {
                    fragment!("terminal_entry_side")
                } else {
                    fragment!("terminal_entry_top")
                };
                s.fragment(frag, &vars!(name = name.clone(), x = mm(*x), w = mm(w)))?;
                cuts.push(name);
            }
            s.fragment(
                fragment!("socket_cut"),
                &vars!(tools = serde_json::json!(cuts)),
            )?;
            looks_of.push(("Housing", looks::TERMINAL_BLOCK));
        }
        F::FpcConnector => {
            s.fragment(fragment!("fpc_connector"), &vars!())?;
            looks_of.extend([("Housing", looks::NYLON_NATURAL), ("Actuator", looks::MOLD)]);
        }
        F::RfCoax => {
            let frag = if f.orientation == Orientation::Vertical && !f.edge_mount {
                fragment!("rf_coax_vertical")
            } else {
                fragment!("rf_coax_right_angle")
            };
            s.fragment(
                frag,
                &vars!(
                    barrel_d = mm(f.mating_diameter),
                    barrel_l = mm(f.mating_length),
                    body_y = mm(b.position.y)
                ),
            )?;
            looks_of.extend([
                ("Base", looks::GOLD),
                ("Barrel", looks::GOLD),
                ("Insulator", looks::ALUMINA),
            ]);
        }
        // Mini-B draws as micro-B does: a flat shell around its tongue.
        F::MicroUsbB | F::MiniUsbB => {
            s.fragment(fragment!("micro_usb_b"), &vars!(shell_t = mm(shell_t)))?;
            looks_of.extend([("Shell", looks::NICKEL), ("Tongue", looks::MOLD)]);
        }
        F::UsbB => {
            s.fragment(fragment!("usb_b"), &vars!(shell_t = mm(shell_t)))?;
            looks_of.extend([
                ("Shell", looks::NICKEL),
                ("Housing", looks::MOLD),
                ("Tongue", looks::MOLD),
            ]);
        }
        _ => return Err(ModelError::MissingDetails(g.family)),
    }
    Ok(())
}

/// A foot on every SMD pad and a pin or tab down every plated hole,
/// then the bodies' looks.
fn contacts(
    s: &mut Script,
    g: &openparts_mcad::MechanicalGeometry,
    tail_length: f64,
    looks_of: Vec<(&str, crate::script::Look)>,
) -> Result<(), ModelError> {
    let mut contacts = Vec::new();
    for (i, l) in g.leads.iter().enumerate() {
        match l.mounting {
            Mounting::Smd => {
                let name = format!("Contact{}", i + 1);
                s.fragment(
                    fragment!("usb_c_pad"),
                    &vars!(
                        name = name.clone(),
                        x = mm(l.position.x),
                        y = mm(l.position.y),
                        // A bottom-side land's foot hangs under the board.
                        z = mm(match l.side {
                            openparts_core::PadSide::Bottom => l.position.z - 0.05,
                            openparts_core::PadSide::Top => 0.05,
                        }),
                        w = mm(l.size.x * 0.8),
                        l = mm(l.size.y * 0.8),
                    ),
                )?;
                contacts.push(name);
            }
            // Alignment-peg holes: the pegs are part of the housing.
            Mounting::NonPlated => {}
            Mounting::ThroughHole => {
                let name = format!("Pin{}", i + 1);
                let drill = l.drill.unwrap_or(1.0);
                let long = l.slot.unwrap_or(drill) * 0.85;
                let thin = drill * 0.6;
                let (tw, tl) = if l.size.x >= l.size.y {
                    (long, thin)
                } else {
                    (thin, long)
                };
                s.fragment(
                    fragment!("usb_c_tab"),
                    &vars!(
                        name = name.clone(),
                        x = mm(l.position.x),
                        y = mm(l.position.y),
                        w = mm(tw),
                        l = mm(tl),
                        tail = mm(tail_length),
                    ),
                )?;
                contacts.push(name);
            }
        }
    }
    for (body, look) in looks_of {
        s.paint(&[body.to_string()], look);
    }
    let contact_look = if g.family == F::RfCoax {
        looks::GOLD
    } else {
        looks::TIN
    };
    s.paint(&contacts, contact_look);
    Ok(())
}

/// A terminal block's poles: the distinct X positions of its plated
/// pads, left to right.
fn pole_positions(g: &openparts_mcad::MechanicalGeometry) -> Vec<f64> {
    let mut xs: Vec<f64> = g
        .leads
        .iter()
        .filter(|l| l.mounting != Mounting::NonPlated)
        .map(|l| l.position.x)
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    xs
}

/// A pole opening's width: 55 % of the pitch (else of the closest pole
/// spacing, else of the body width).
fn pole_width(g: &openparts_mcad::MechanicalGeometry, poles: &[f64]) -> f64 {
    let spacing = g.pitch.filter(|p| *p > 0.0).unwrap_or_else(|| {
        poles
            .windows(2)
            .map(|w| w[1] - w[0])
            .fold(g.body.size.x, f64::min)
    });
    spacing * 0.55
}

#[cfg(test)]
mod tests {
    use crate::{build_script, run, Format, ModelInput};
    use openparts_core::ComponentCategory;

    fn package(yaml: &str) -> openparts_core::Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\n{yaml}"
        ))
        .unwrap()
    }

    /// The script's coloured bodies, and the STEP solid count when
    /// SolidCraft is installed (`OPENPARTS_DUMP_PNG=<dir>` also renders).
    fn model(
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
            let c = package.connector.clone().unwrap_or_default();
            let name = format!(
                "{}-{:?}-{}.png",
                package.family, c.orientation, c.edge_mount
            );
            std::fs::write(std::path::Path::new(&dir).join(name), png).unwrap();
        }
        let solids = crate::available_for_test(module_path!()).then(|| {
            let step = run(&script, "TEST", &[Format::Step]).unwrap().remove(0);
            String::from_utf8(step)
                .unwrap()
                .matches("MANIFOLD_SOLID_BREP(")
                .count()
        });
        (bodies, solids)
    }

    const SMD_PADS: &str = "pads:\n\
        \x20 - {number: '1', x: -1.0, y: -3.0, width: 0.6, length: 1.2}\n\
        \x20 - {number: '2', x: 0.0, y: -3.0, width: 0.6, length: 1.2}\n\
        \x20 - {number: '3', x: 1.0, y: -3.0, width: 0.6, length: 1.2}\n\
        \x20 - {number: SH, x: -4.0, y: 1.0, width: 1.2, length: 1.6}\n\
        \x20 - {number: SH, x: 4.0, y: 1.0, width: 1.2, length: 1.6}\n";

    fn connector(family: &str, extra: &str, pads: &str, lead_count: u32) -> Vec<String> {
        let p = package(&format!(
            "family: {family}\nlead_count: {lead_count}\npitch: {{nominal: 1.0, unit: mm}}\n\
             dimensions:\n\
             \x20 body_width: {{nominal: 9.0, unit: mm}}\n\
             \x20 body_length: {{nominal: 6.0, unit: mm}}\n\
             \x20 body_height: {{nominal: 3.0, unit: mm}}\n\
             {extra}{pads}"
        ));
        let (bodies, solids) = model(&p, ComponentCategory::Connector);
        assert!(
            solids.is_none_or(|n| n == bodies.len()),
            "{family}: {solids:?} {bodies:?}"
        );
        bodies
    }

    #[test]
    fn every_connector_family_builds_its_body_and_contacts() {
        let b = connector("card_socket", "", SMD_PADS, 5);
        assert_eq!(&b[..2], ["Housing", "Shell"]);
        assert_eq!(b.len(), 7);
        let b = connector("fpc_connector", "", SMD_PADS, 5);
        assert_eq!(&b[..2], ["Housing", "Actuator"]);
        for family in ["micro_usb_b", "mini_usb_b"] {
            let b = connector(family, "", SMD_PADS, 5);
            assert_eq!(&b[..2], ["Shell", "Tongue"], "{family}");
        }
        let tht = "pads:\n\
            \x20 - {number: '1', x: 0, y: 2.0, width: 3.0, length: 1.6, drill: 1.0, drill_length: 2.4}\n\
            \x20 - {number: '2', x: 0, y: -2.0, width: 3.0, length: 1.6, drill: 1.0, drill_length: 2.4}\n\
            \x20 - {number: '3', x: 3.0, y: 0.0, width: 1.6, length: 3.0, drill: 1.0, drill_length: 2.4}\n";
        let b = connector("usb_b", "", tht, 3);
        assert_eq!(&b[..3], ["Shell", "Housing", "Tongue"]);
        assert_eq!(b.len(), 6);
        let b = connector("barrel_jack", "", tht, 3);
        assert_eq!(&b[..2], ["Housing", "Pin"]);
        assert_eq!(b.len(), 5);
    }

    #[test]
    fn electromechanical_families_build_their_bodies_and_pins() {
        let tht = "pads:\n\
            \x20 - {number: '1', x: -2.5, y: 0, width: 1.8, length: 1.8, drill: 1.0}\n\
            \x20 - {number: '2', x: 0, y: 0, width: 1.8, length: 1.8, drill: 1.0}\n\
            \x20 - {number: '3', x: 2.5, y: 0, width: 1.8, length: 1.8, drill: 1.0}\n";
        let b = connector("relay", "", tht, 3);
        assert_eq!(b, ["Housing", "Pin1", "Pin2", "Pin3"]);
        let b = connector("buzzer", "", tht, 3);
        assert_eq!(b[0], "Housing");
        let round = "\x20 can_diameter: {nominal: 9.0, unit: mm}\n";
        let b = connector("buzzer", round, tht, 3);
        assert_eq!(b[0], "Housing");
        let b = connector("trimmer", "", tht, 3);
        assert_eq!(&b[..2], ["Housing", "Rotor"]);
        let shaft = "\x20 overall_height: {nominal: 20.0, unit: mm}\n";
        let b = connector("rotary_encoder", shaft, tht, 3);
        assert_eq!(&b[..3], ["Housing", "Bushing", "Shaft"]);
        assert_eq!(b.len(), 6);
    }

    #[test]
    fn terminal_blocks_cut_one_entry_per_pole_side_or_top() {
        let poles = "pads:\n\
            \x20 - {number: '1', x: -2.5, y: 0, width: 2.0, length: 2.0, drill: 1.3}\n\
            \x20 - {number: '2', x: 2.5, y: 0, width: 2.0, length: 2.0, drill: 1.3}\n";
        for entry in ["right_angle", "vertical"] {
            let extra = format!("connector: {{orientation: {entry}}}\n");
            let b = connector("terminal_block", &extra, poles, 2);
            assert_eq!(b, ["Housing", "Pin1", "Pin2"], "{entry}");
        }
    }

    #[test]
    fn rf_connectors_vertical_right_angle_and_edge_mount() {
        let pads = "pads:\n\
            \x20 - {number: '1', x: 0, y: -4.0, width: 1.0, length: 2.0}\n\
            \x20 - {number: '2', x: -2.5, y: -4.0, width: 1.2, length: 2.0}\n";
        for c in [
            "connector: {orientation: vertical, interface: sma}\n",
            "connector: {orientation: right_angle, interface: sma}\n",
            "connector: {orientation: right_angle, interface: sma, edge_mount: true}\n",
        ] {
            let b = connector("rf_coax", c, pads, 2);
            assert_eq!(&b[..3], ["Base", "Barrel", "Insulator"], "{c}");
        }
    }
}
