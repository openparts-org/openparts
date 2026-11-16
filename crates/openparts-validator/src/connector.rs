//! The connector families (Canonical Data Specification section 20.6):
//! a stated land pattern and body, a `connector` record whose fields fit
//! the family, an edge-mount part with its board edge, and provenance for
//! the connector facts. Also the land shapes of any stated pad
//! (section 20.7) and the land patterns of the `smd_led` and
//! `smd_electrolytic` families.

use crate::Diagnostic;
use openparts_core::{is_connector_family, Package, PackageFamily as F};

/// `pad_shape.invalid`: a stated pad whose shape fields can't be drawn.
pub(crate) fn check_pad_shapes(entity: &str, package: &Package) -> Vec<Diagnostic> {
    package
        .pads
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let why = openparts_mcad::pad_shape_error(p)?;
            Some(Diagnostic::error(
                "pad_shape.invalid",
                entity,
                Some(&format!("/pads/{i}")),
                format!("pad \"{}\": {why}", p.number),
            ))
        })
        .collect()
}

/// `smd_led.invalid`: an `smd_led` package states its land pattern
/// (`pads`, one plated pad per lead) and a height.
pub(crate) fn check_smd_led(entity: &str, package: &Package) -> Vec<Diagnostic> {
    if package.family != F::SmdLed {
        return vec![];
    }
    let mut out = Vec::new();
    let mut bad = |path: &str, message: &str| {
        out.push(Diagnostic::error(
            "smd_led.invalid",
            entity,
            Some(path),
            message.to_string(),
        ))
    };
    if package.pads.is_empty() {
        bad("/pads", "an smd_led package lists its land pattern in pads");
    } else {
        let plated = package.pads.iter().filter(|p| p.plated).count();
        if plated != package.lead_count as usize {
            bad(
                "/lead_count",
                "lead_count must equal the number of plated pads",
            );
        }
        if package.pads.iter().any(|p| p.land_unstated()) {
            bad("/pads", "every pad of an smd_led package states its size");
        }
    }
    let d = &package.dimensions;
    if d.body_height.is_none() && d.overall_height.is_none() {
        bad(
            "/dimensions/body_height",
            "an smd_led package states body_height or overall_height",
        );
    }
    out
}

/// `smd_electrolytic.invalid`: an `smd_electrolytic` package states its
/// land pattern (`pads`, one plated pad per lead, at least two), its
/// height and its can; `can_diameter` belongs to this family only.
pub(crate) fn check_smd_electrolytic(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let d = &package.dimensions;
    let mut out = Vec::new();
    let mut bad = |path: &str, message: &str| {
        out.push(Diagnostic::error(
            "smd_electrolytic.invalid",
            entity,
            Some(path),
            message.to_string(),
        ))
    };
    if package.family != F::SmdElectrolytic {
        if d.can_diameter.is_some() {
            bad(
                "/dimensions/can_diameter",
                "can_diameter belongs to the smd_electrolytic family",
            );
        }
        return out;
    }
    if package.pads.is_empty() {
        bad(
            "/pads",
            "an smd_electrolytic package lists its land pattern in pads",
        );
    } else {
        let plated = package.pads.iter().filter(|p| p.plated).count();
        if plated != package.lead_count as usize || plated < 2 {
            bad(
                "/lead_count",
                "lead_count must equal the number of plated pads (at least two)",
            );
        }
        if package.pads.iter().any(|p| p.land_unstated()) {
            bad(
                "/pads",
                "every pad of an smd_electrolytic package states its size",
            );
        }
    }
    if d.overall_height.is_none() {
        bad(
            "/dimensions/overall_height",
            "an smd_electrolytic package states overall_height (the can top above the board)",
        );
    }
    if d.can_diameter.is_none() {
        bad(
            "/dimensions/can_diameter",
            "an smd_electrolytic package states can_diameter",
        );
    }
    out
}

/// `connector.invalid` and the connector facts' `provenance.missing`.
pub(crate) fn check_package_connector(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut bad = |path: &str, message: String| {
        out.push(Diagnostic::error(
            "connector.invalid",
            entity,
            Some(path),
            message,
        ))
    };
    let family = package.family;
    if let Some(c) = &package.connector {
        for field in c.misplaced(family) {
            bad(
                &format!("/connector/{field}"),
                format!("connector.{field} does not apply to family \"{family}\""),
            );
        }
        if c.edge_mount && !package.edge.as_ref().is_some_and(|e| e.pcb_edge.is_some()) {
            bad(
                "/edge",
                "an edge-mount connector must state edge (its side and the PCB edge line)"
                    .to_string(),
            );
        }
    }
    if is_connector_family(family) {
        if package.pads.is_empty() {
            bad(
                "/pads",
                format!("a {family} package lists its land pattern in pads"),
            );
        }
        let plated = package.pads.iter().filter(|p| p.plated).count();
        if !package.pads.is_empty() && plated != package.lead_count as usize {
            bad(
                "/lead_count",
                format!(
                    "lead_count ({}) must equal the number of plated pads ({plated})",
                    package.lead_count
                ),
            );
        }
        // An RF connector's base height is often not printed (3D default).
        if package.dimensions.body_height.is_none() && family != F::RfCoax {
            bad(
                "/dimensions/body_height",
                format!("a {family} package states dimensions.body_height"),
            );
        }
        for (i, p) in package.pads.iter().enumerate() {
            let problem = if !p.land_unstated() {
                None
            } else if p.drill.is_none() {
                Some("an SMD pad states its width and length")
            } else if p.drill_length.is_some() && p.slot_axis.is_none() {
                Some("a slot without a land states slot_axis")
            } else {
                None
            };
            if let Some(problem) = problem {
                bad(&format!("/pads/{i}"), problem.to_string());
            }
        }
        let c = package.connector.clone().unwrap_or_default();
        let required: &[(&str, bool)] = match family {
            F::CardSocket => &[
                ("mechanism", c.mechanism.is_some()),
                ("interface", c.interface.is_some()),
            ],
            F::FpcConnector => &[("contact_side", c.contact_side.is_some())],
            F::RfCoax => &[
                ("interface", c.interface.is_some()),
                ("orientation", c.orientation.is_some()),
            ],
            _ => &[],
        };
        for (field, present) in required {
            if !present {
                bad(
                    &format!("/connector/{field}"),
                    format!("a {family} package states connector.{field}"),
                );
            }
        }
    }

    let d = &package.dimensions;
    let mut need = Vec::new();
    if package.connector.is_some() {
        need.push("/connector");
    }
    if d.mating_diameter.is_some() {
        need.push("/dimensions/mating_diameter");
    }
    if d.pin_diameter.is_some() {
        need.push("/dimensions/pin_diameter");
    }
    for path in need {
        let covered = package.provenance.keys().any(|key| {
            path.strip_prefix(key.as_str())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        });
        if !covered {
            out.push(Diagnostic::error(
                "provenance.missing",
                entity,
                Some(path),
                format!("\"{path}\" has no provenance entry"),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(family: &str, extra: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: {family}\n\
             lead_count: 2\npitch: {{unit: mm}}\ndimensions:\n\
             \x20 body_width: {{nominal: 6.35, unit: mm}}\n\
             \x20 body_length: {{nominal: 1.7, unit: mm}}\n\
             \x20 body_height: {{nominal: 6.35, unit: mm}}\n\
             pads:\n\
             \x20 - {{number: '1', x: 0, y: -3.0, width: 1.27, length: 4.2}}\n\
             \x20 - {{number: '2', x: 2.865, y: -3.0, width: 1.27, length: 4.2}}\n\
             \x20 - {{number: '', x: 0, y: -6, width: 1, length: 1, drill: 1, plated: false}}\n\
             {extra}provenance:\n  /pads: [{{source: x/Y}}]\n"
        ))
        .unwrap()
    }

    fn paths(d: &[Diagnostic]) -> Vec<String> {
        d.iter()
            .map(|d| format!("{} {}", d.rule_id, d.field_path.clone().unwrap_or_default()))
            .collect()
    }

    #[test]
    fn a_complete_rf_connector_passes() {
        let p = pkg(
            "rf_coax",
            "connector: {gender: female, orientation: right_angle, interface: sma, edge_mount: true}\n\
             edge: {side: +y, pcb_edge: -0.8}\n",
        );
        let mut p = p;
        p.provenance
            .insert("/connector".into(), p.provenance["/pads"].clone());
        assert_eq!(
            paths(&check_package_connector("p", &p)),
            Vec::<String>::new()
        );
    }

    #[test]
    fn edge_mount_needs_its_edge_and_records_need_provenance() {
        let p = pkg(
            "rf_coax",
            "connector: {orientation: right_angle, interface: sma, edge_mount: true}\n",
        );
        assert_eq!(
            paths(&check_package_connector("p", &p)),
            ["connector.invalid /edge", "provenance.missing /connector"]
        );
    }

    #[test]
    fn family_fields_are_required_and_misplaced_ones_rejected() {
        let p = pkg(
            "card_socket",
            "connector: {interface: sma, contact_side: top}\n",
        );
        assert_eq!(
            paths(&check_package_connector("p", &p)),
            [
                "connector.invalid /connector/contact_side",
                "connector.invalid /connector/interface",
                "connector.invalid /connector/mechanism",
                "provenance.missing /connector"
            ]
        );
        let p = pkg("fpc_connector", "");
        assert_eq!(
            paths(&check_package_connector("p", &p)),
            ["connector.invalid /connector/contact_side"]
        );
    }

    #[test]
    fn lead_count_counts_plated_pads_and_body_height_is_required() {
        let mut p = pkg("barrel_jack", "");
        p.lead_count = 3;
        p.dimensions.body_height = None;
        assert_eq!(
            paths(&check_package_connector("p", &p)),
            [
                "connector.invalid /lead_count",
                "connector.invalid /dimensions/body_height"
            ]
        );
        // Other families aren't concerned.
        let mut p = pkg("usb_c", "");
        p.lead_count = 9;
        assert!(check_package_connector("p", &p).is_empty());
    }

    #[test]
    fn a_pad_without_a_land_must_be_a_hole_and_a_slot_needs_its_axis() {
        let mut p = pkg("barrel_jack", "");
        p.pads[0].width = 0.0;
        p.pads[0].length = 0.0;
        p.pads[1].width = 0.0;
        p.pads[1].length = 0.0;
        p.pads[1].drill = Some(1.0);
        p.pads[1].drill_length = Some(2.0);
        assert_eq!(
            paths(&check_package_connector("p", &p)),
            ["connector.invalid /pads/0", "connector.invalid /pads/1"]
        );
        p.pads[0].drill = Some(1.0);
        p.pads[1].slot_axis = Some(openparts_core::SlotAxis::Y);
        assert!(check_package_connector("p", &p).is_empty());
    }

    #[test]
    fn an_smd_electrolytic_states_its_can_and_height() {
        let p = pkg("smd_electrolytic", "");
        assert_eq!(
            paths(&check_smd_electrolytic("p", &p)),
            [
                "smd_electrolytic.invalid /dimensions/overall_height",
                "smd_electrolytic.invalid /dimensions/can_diameter"
            ]
        );
        let mut ok = p.clone();
        let d = ok.dimensions.body_height.clone();
        ok.dimensions.overall_height = d.clone();
        ok.dimensions.can_diameter = d;
        assert!(check_smd_electrolytic("p", &ok).is_empty());
        // The can belongs to this family only.
        let mut led = pkg("smd_led", "");
        led.dimensions.can_diameter = ok.dimensions.can_diameter.clone();
        assert_eq!(
            paths(&check_smd_electrolytic("p", &led)),
            ["smd_electrolytic.invalid /dimensions/can_diameter"]
        );
    }
}
