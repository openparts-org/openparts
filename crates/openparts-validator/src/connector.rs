//! The connector families (Canonical Data Specification section 20.6):
//! a stated land pattern and body, a `connector` record whose fields fit
//! the family, an edge-mount part with its board edge, and provenance for
//! the connector facts. Also the land shapes of any stated pad
//! (section 20.7) and the land patterns of the `smd_led`,
//! `smd_electrolytic` and `melf` families.

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

/// `pad_side.invalid`: a bottom-side pad with a hole.
pub(crate) fn check_pad_sides(entity: &str, package: &Package) -> Vec<Diagnostic> {
    package
        .pads
        .iter()
        .enumerate()
        .filter(|(_, p)| p.side == openparts_core::PadSide::Bottom && p.drill.is_some())
        .map(|(i, p)| {
            Diagnostic::error(
                "pad_side.invalid",
                entity,
                Some(&format!("/pads/{i}")),
                format!(
                    "pad \"{}\": only an SMD land can be on the bottom side",
                    p.number
                ),
            )
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
        // A round buzzer states its can too (electromechanical.invalid).
        if d.can_diameter.is_some() && package.family != F::Buzzer {
            bad(
                "/dimensions/can_diameter",
                "can_diameter belongs to the smd_electrolytic and buzzer families",
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

/// `melf.invalid`: a `melf` package states its land pattern (`pads`,
/// two plated SMD pads with a size) and its body (length and diameter).
pub(crate) fn check_melf(entity: &str, package: &Package) -> Vec<Diagnostic> {
    if package.family != F::Melf {
        return vec![];
    }
    let mut out = Vec::new();
    let mut bad = |path: &str, message: &str| {
        out.push(Diagnostic::error(
            "melf.invalid",
            entity,
            Some(path),
            message.to_string(),
        ))
    };
    if package.lead_count != 2 {
        bad("/lead_count", "a melf package has two terminals");
    }
    if package.pads.is_empty() {
        bad("/pads", "a melf package lists its land pattern in pads");
    } else {
        let plated: Vec<_> = package.pads.iter().filter(|p| p.plated).collect();
        if plated.len() != 2 {
            bad("/pads", "a melf land pattern has two plated pads");
        }
        if plated
            .iter()
            .any(|p| p.land_unstated() || p.drill.is_some())
        {
            bad(
                "/pads",
                "every pad of a melf package is an SMD land with a size",
            );
        }
    }
    let d = &package.dimensions;
    let unstated = |dim: &openparts_core::Dimension| {
        dim.typ.is_none() && dim.min.is_none() && dim.max.is_none()
    };
    if unstated(&d.body_length) {
        bad(
            "/dimensions/body_length",
            "a melf package states body_length (end cap to end cap)",
        );
    }
    if unstated(&d.body_width) {
        bad(
            "/dimensions/body_width",
            "a melf package states body_width (the diameter)",
        );
    }
    out
}

/// The land-pattern body checks shared by the connector and the
/// electromechanical families, reported under `rule`: the pads, one
/// plated pad per lead, the body height and every SMD pad's size or a
/// bare slot's axis. (`body_length`, which an edge-mount RF connector
/// may leave out, is the caller's.)
fn check_pad_body(package: &Package, mut bad: impl FnMut(&str, String)) {
    let family = package.family;
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
}

/// `electromechanical.invalid`: a `relay`, `buzzer`, `trimmer` or
/// `rotary_encoder` package states its land pattern (`pads`, one plated
/// pad per lead), its body (`body_length`, `body_height`), no `connector`
/// facts, and a rotor/shaft top (`overall_height`) above the housing.
pub(crate) fn check_electromechanical(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let family = package.family;
    if !openparts_core::is_electromechanical_family(family) {
        return out;
    }
    let mut bad = |path: &str, message: String| {
        out.push(Diagnostic::error(
            "electromechanical.invalid",
            entity,
            Some(path),
            message,
        ))
    };
    check_pad_body(package, &mut bad);
    let d = &package.dimensions;
    if d.body_length
        .typ
        .or(d.body_length.min)
        .or(d.body_length.max)
        .is_none()
    {
        bad(
            "/dimensions/body_length",
            format!("a {family} package states dimensions.body_length"),
        );
    }
    if package.connector.is_some() {
        bad(
            "/connector",
            format!("a {family} package has no connector facts"),
        );
    }
    let value =
        |x: &Option<openparts_core::Dimension>| x.as_ref().and_then(|x| x.typ.or(x.max).or(x.min));
    if let (Some(top), Some(h)) = (value(&d.overall_height), value(&d.body_height)) {
        if top + 1e-9 < h + value(&d.standoff).unwrap_or(0.0) {
            bad(
                "/dimensions/overall_height",
                "overall_height (the rotor or shaft top) is below the housing top".into(),
            );
        }
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
        check_pad_body(package, &mut bad);
        // An edge-mount RF connector's flange depth may be unprinted: its
        // frame then starts at the PCB edge line (20.6).
        let depth = &package.dimensions.body_length;
        if depth.typ.or(depth.min).or(depth.max).is_none() {
            match openparts_mcad::edge_flange(package) {
                None => bad(
                    "/dimensions/body_length",
                    format!("a {family} package states dimensions.body_length"),
                ),
                Some(edge) if edge.abs() > 1e-9 => bad(
                    "/edge/pcb_edge",
                    "without body_length, the frame's Y origin is the PCB edge line: pcb_edge is 0"
                        .to_string(),
                ),
                Some(_) => {}
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

/// `chip_array.invalid` (section 20.15): a chip array states an even
/// `lead_count` of at least four, its land pattern (one plated SMD pad
/// per terminal, each with a size), its body and pitch; `terminal_shape`
/// belongs to `chip_array` alone.
pub(crate) fn check_chip_array(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut bad = |path: &str, message: &str| {
        out.push(Diagnostic::error(
            "chip_array.invalid",
            entity,
            Some(path),
            message.to_string(),
        ))
    };
    if package.family != F::ChipArray {
        if package.terminal_shape.is_some() {
            bad(
                "/terminal_shape",
                "terminal_shape belongs to the chip_array family",
            );
        }
        return out;
    }
    let n = package.lead_count;
    if n < 4 || !n.is_multiple_of(2) {
        bad(
            "/lead_count",
            "a chip array has two rows of terminals: an even lead_count of at least 4",
        );
    }
    let plated: Vec<_> = package.pads.iter().filter(|p| p.plated).collect();
    if plated.is_empty() {
        bad(
            "/pads",
            "a chip array's footprint is the source's land pattern: list it in pads",
        );
    } else {
        let numbers: std::collections::BTreeSet<&str> =
            plated.iter().map(|p| p.number.as_str()).collect();
        let expected: std::collections::BTreeSet<String> = (1..=n).map(|i| i.to_string()).collect();
        if numbers.len() != plated.len()
            || !numbers.iter().all(|x| expected.contains(*x))
            || numbers.len() != n as usize
        {
            bad(
                "/pads",
                "a chip array's pads are one SMD land per terminal, numbered 1..lead_count",
            );
        }
        if plated
            .iter()
            .any(|p| p.land_unstated() || p.drill.is_some())
        {
            bad("/pads", "every chip-array pad is an SMD land with a size");
        }
    }
    let unstated = |dim: &openparts_core::Dimension| {
        dim.typ.is_none() && dim.min.is_none() && dim.max.is_none()
    };
    if unstated(&package.pitch) {
        bad("/pitch", "a chip array states its terminal pitch");
    }
    if unstated(&package.dimensions.body_width) || unstated(&package.dimensions.body_length) {
        bad(
            "/dimensions",
            "a chip array states body_width (across the rows) and body_length (along them)",
        );
    }
    out
}

/// `exposed_pad.invalid`: a top-face (pad-up) exposed pad on a family
/// other than the two-row gull-wing ones (soic, tssop, ssop, msop), or
/// with a board land (`land_width` / `land_length`).
pub(crate) fn check_exposed_pad_face(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let Some(ep) = &package.dimensions.exposed_pad else {
        return vec![];
    };
    if ep.face != openparts_core::ExposedPadFace::Top {
        return vec![];
    }
    let mut out = Vec::new();
    if !matches!(package.family, F::Soic | F::Tssop | F::Ssop | F::Msop) {
        out.push(Diagnostic::error(
            "exposed_pad.invalid",
            entity,
            Some("/dimensions/exposed_pad/face"),
            format!(
                "a top-face (pad-up) exposed pad is drawn on soic/tssop/ssop/msop only, not {}",
                package.family
            ),
        ));
    }
    if ep.land_width.is_some() || ep.land_length.is_some() {
        out.push(Diagnostic::error(
            "exposed_pad.invalid",
            entity,
            Some("/dimensions/exposed_pad"),
            "a top-face (pad-up) exposed pad has no board land: drop land_width / land_length"
                .to_string(),
        ));
    }
    out
}

/// `lga.invalid`: an `lga` whose `lead_layout` counts fewer terminals
/// than `lead_count` must list the others in `pads` -- exactly
/// `lead_count` minus the layout's sum of plated SMD pad numbers beyond
/// the perimeter's (`1..=` the layout's sum, after `pad_order`).
pub(crate) fn check_lga(entity: &str, package: &Package) -> Vec<Diagnostic> {
    if package.family != F::Lga {
        return vec![];
    }
    let Some(layout) = &package.lead_layout else {
        return vec![];
    };
    let perimeter: u32 = layout.iter().sum();
    let n = package.lead_count;
    if perimeter >= n {
        return vec![];
    }
    let perimeter_numbers: std::collections::BTreeSet<String> = (1..=perimeter as usize)
        .map(|i| {
            package
                .pad_order
                .get(i - 1)
                .cloned()
                .unwrap_or_else(|| i.to_string())
        })
        .collect();
    let extra: std::collections::BTreeSet<&str> = package
        .pads
        .iter()
        .filter(|p| p.plated && p.drill.is_none())
        .map(|p| p.number.as_str())
        .filter(|x| !perimeter_numbers.contains(*x))
        .collect();
    if extra.len() as u32 == n - perimeter {
        return vec![];
    }
    vec![Diagnostic::error(
        "lga.invalid",
        entity,
        Some("/pads"),
        format!(
            "lead_count {n} is {} more than lead_layout's {perimeter} perimeter terminals, \
             so pads must give that many other SMD pad numbers (it gives {})",
            n - perimeter,
            extra.len()
        ),
    )]
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

    fn yaml(text: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\n{text}"
        ))
        .unwrap()
    }

    #[test]
    fn a_chip_array_states_its_land_pattern_and_terminal_shape_is_its_own() {
        let pads: String = (1..=8)
            .map(|i| {
                let (x, y) = if i <= 4 {
                    (-0.5, 0.75 - 0.5 * (i - 1) as f64)
                } else {
                    (0.5, -0.75 + 0.5 * (i - 5) as f64)
                };
                format!("  - {{number: '{i}', x: {x}, y: {y}, width: 0.5, length: 0.3}}\n")
            })
            .collect();
        let array = |lead_count: u32, pads: &str| {
            yaml(&format!(
                "family: chip_array\nlead_count: {lead_count}\nterminal_shape: concave\n\
                 pitch: {{nominal: 0.5, unit: mm}}\ndimensions:\n\
                 \x20 body_width: {{nominal: 1.0, unit: mm}}\n\
                 \x20 body_length: {{nominal: 2.0, unit: mm}}\npads:\n{pads}"
            ))
        };
        assert!(check_chip_array("p", &array(8, &pads)).is_empty());
        assert_eq!(
            paths(&check_chip_array("p", &array(7, &pads))),
            ["chip_array.invalid /lead_count", "chip_array.invalid /pads"]
        );
        let no_pads = array(8, "  []\n");
        assert_eq!(
            paths(&check_chip_array("p", &no_pads)),
            ["chip_array.invalid /pads"]
        );
        // terminal_shape on another family.
        let mut chip = pkg("melf", "");
        chip.terminal_shape = Some(openparts_core::TerminalShape::Convex);
        assert_eq!(
            paths(&check_chip_array("p", &chip)),
            ["chip_array.invalid /terminal_shape"]
        );
    }

    #[test]
    fn a_pad_up_exposed_pad_is_on_a_leaded_two_row_package_without_a_land() {
        let pad_up = |family: &str, land: &str| {
            yaml(&format!(
                "family: {family}\nlead_count: 32\npitch: {{nominal: 0.65, unit: mm}}\n\
                 dimensions:\n\
                 \x20 body_width: {{nominal: 6.1, unit: mm}}\n\
                 \x20 body_length: {{nominal: 11.0, unit: mm}}\n\
                 \x20 exposed_pad: {{width: {{nominal: 3.6, unit: mm}}, length: {{nominal: 7.5, unit: mm}}, face: top{land}}}\n"
            ))
        };
        assert!(check_exposed_pad_face("p", &pad_up("tssop", "")).is_empty());
        let g = openparts_mcad::generate(&pad_up("tssop", "")).unwrap();
        assert!(g.leads.iter().all(|l| l.number != "EP"), "no EP terminal");
        assert_eq!(
            paths(&check_exposed_pad_face("p", &pad_up("qfn", ""))),
            ["exposed_pad.invalid /dimensions/exposed_pad/face"]
        );
        assert_eq!(
            paths(&check_exposed_pad_face(
                "p",
                &pad_up("tssop", ", land_width: {nominal: 4, unit: mm}")
            )),
            ["exposed_pad.invalid /dimensions/exposed_pad"]
        );
    }

    #[test]
    fn an_lga_lists_its_pads_beyond_the_perimeter() {
        // ENS160-like: 8 perimeter pads, the centre pad 9.
        let grid: String = [
            ("1", -1.05, 1.05),
            ("2", -1.05, 0.0),
            ("3", -1.05, -1.05),
            ("4", 0.0, -1.05),
            ("5", 1.05, -1.05),
            ("6", 1.05, 0.0),
            ("7", 1.05, 1.05),
            ("8", 0.0, 1.05),
            ("9", 0.0, 0.0),
        ]
        .iter()
        .map(|(n, x, y)| {
            format!("  - {{number: '{n}', x: {x}, y: {y}, width: 0.8, length: 0.8}}\n")
        })
        .collect();
        let lga = |lead_count: u32, pads: &str| {
            yaml(&format!(
                "family: lga\nlead_count: {lead_count}\nlead_layout: [3, 1, 3, 1]\n\
                 pitch: {{nominal: 1.05, unit: mm}}\ndimensions:\n\
                 \x20 body_width: {{nominal: 3.0, unit: mm}}\n\
                 \x20 body_length: {{nominal: 3.0, unit: mm}}\n\
                 \x20 lead_width: {{nominal: 0.7, unit: mm}}\n\
                 \x20 lead_length: {{nominal: 0.7, unit: mm}}\n\
                 \x20 lead_inset: {{nominal: 0.1, unit: mm}}\npads:\n{pads}"
            ))
        };
        let p = lga(9, &grid);
        assert!(check_lga("p", &p).is_empty());
        let g = openparts_mcad::generate(&p).unwrap();
        let centre = g.leads.iter().find(|l| l.number == "9").unwrap();
        assert!(centre.position.x.abs() < 1e-9 && centre.position.y.abs() < 1e-9);
        assert_eq!(g.lands.len(), 9);
        // One more lead than the pads give.
        assert_eq!(
            paths(&check_lga("p", &lga(10, &grid))),
            ["lga.invalid /pads"]
        );
        assert!(openparts_mcad::generate(&lga(10, &grid)).is_err());
    }

    #[test]
    fn a_melf_states_two_lands_and_its_body() {
        // Two plated lands and a peg hole: fine.
        assert!(check_melf("p", &pkg("melf", "")).is_empty());
        let mut p = pkg("melf", "");
        p.pads[1].drill = Some(0.5);
        p.lead_count = 3;
        p.dimensions.body_width.typ = None;
        assert_eq!(
            paths(&check_melf("p", &p)),
            [
                "melf.invalid /lead_count",
                "melf.invalid /pads",
                "melf.invalid /dimensions/body_width"
            ]
        );
        p.pads.clear();
        assert!(paths(&check_melf("p", &p)).contains(&"melf.invalid /pads".to_string()));
        // Other families are not this rule's.
        assert!(check_melf("p", &pkg("smd_led", "")).is_empty());
    }

    #[test]
    fn electromechanical_packages_state_pads_and_body() {
        for family in ["relay", "buzzer", "trimmer", "rotary_encoder"] {
            assert!(
                check_electromechanical("p", &pkg(family, "")).is_empty(),
                "{family}"
            );
        }
        // A round buzzer's can is not the electrolytic rule's business.
        let mut round = pkg("buzzer", "");
        round.dimensions.can_diameter = round.dimensions.body_height.clone();
        assert!(check_smd_electrolytic("p", &round).is_empty());
        let mut p = pkg("rotary_encoder", "connector: {orientation: vertical}\n");
        p.lead_count = 3;
        p.pads.remove(0);
        p.dimensions.body_height = None;
        p.dimensions.overall_height = Some(openparts_core::Dimension {
            typ: Some(1.0),
            unit: "mm".into(),
            ..Default::default()
        });
        assert_eq!(
            paths(&check_electromechanical("p", &p)),
            [
                "electromechanical.invalid /lead_count",
                "electromechanical.invalid /dimensions/body_height",
                "electromechanical.invalid /connector",
            ]
        );
        p.dimensions.body_height = Some(p.dimensions.body_width.clone());
        assert!(paths(&check_electromechanical("p", &p))
            .contains(&"electromechanical.invalid /dimensions/overall_height".to_string()));
        // Other families are not this rule's.
        assert!(check_electromechanical("p", &pkg("melf", "")).is_empty());
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

    #[test]
    fn a_bottom_land_has_no_hole_and_an_edge_flange_may_leave_its_depth_out() {
        let mut p = pkg(
            "rf_coax",
            "connector: {interface: sma, orientation: right_angle, edge_mount: true}\n\
             edge: {side: +y, pcb_edge: 0}\n",
        );
        p.dimensions.body_length = openparts_core::Dimension::default();
        assert!(!check_package_connector("p", &p)
            .iter()
            .any(|d| d.field_path.as_deref() == Some("/dimensions/body_length")));
        p.edge.as_mut().unwrap().pcb_edge = Some(-0.8);
        assert!(check_package_connector("p", &p)
            .iter()
            .any(|d| d.field_path.as_deref() == Some("/edge/pcb_edge")));
        p.connector.as_mut().unwrap().edge_mount = false;
        assert!(check_package_connector("p", &p)
            .iter()
            .any(|d| d.field_path.as_deref() == Some("/dimensions/body_length")));
        p.pads[0].side = openparts_core::PadSide::Bottom;
        assert!(check_pad_sides("p", &p).is_empty());
        p.pads[0].drill = Some(0.5);
        assert_eq!(check_pad_sides("p", &p)[0].rule_id, "pad_side.invalid");
    }
}
