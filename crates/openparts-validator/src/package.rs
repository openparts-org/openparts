//! Packae checks: dimensions, their consistency and provenance, and the
//! family-specific fields (tab, aux pads, headers, pad order).

use crate::Diagnostic;
use openparts_core::Package;
use std::collections::BTreeSet;

/// Paths of the package fields that must carry their own provenance.
pub(crate) const PROVENANCE_REQUIRED: [&str; 24] = [
    "/dimensions/overall_width",
    "/dimensions/overall_length",
    "/dimensions/overall_height",
    "/dimensions/standoff",
    "/dimensions/lead_width",
    "/dimensions/lead_length",
    "/dimensions/lead_thickness",
    "/dimensions/tab_width",
    "/dimensions/tab_length",
    "/dimensions/row_spacing",
    "/dimensions/hole_diameter",
    "/dimensions/aux_pad_width",
    "/dimensions/aux_pad_length",
    "/dimensions/aux_row_x",
    "/dimensions/aux_row_y",
    "/dimensions/mounting_hole_diameter",
    "/dimensions/mounting_hole_spacing",
    "/dimensions/mounting_hole_inset",
    "/dimensions/mating_length",
    "/dimensions/tail_length",
    "/dimensions/pad_diameter",
    "/dimensions/body_offset",
    "/dimensions/lead_inset",
    "/dimensions/actuator_diameter",
];

pub(crate) fn check_package_provenance(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let mut present: Vec<&str> = package
        .dimensions
        .entries()
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| PROVENANCE_REQUIRED.contains(path))
        .collect();
    if package.construction.is_some() {
        present.push("/construction");
    }
    if package.tab.is_some() {
        present.push("/tab");
    }
    if !package.pad_order.is_empty() {
        present.push("/pad_order");
    }
    if !package.pads.is_empty() {
        present.push("/pads");
    }
    if !package.aux_pads.is_empty() {
        present.push("/aux_pads");
    }
    if package.edge.is_some() {
        present.push("/edge");
    }
    if !package.keepouts.is_empty() {
        present.push("/keepouts");
    }
    present
        .into_iter()
        .filter(|path| {
            // An entry for the field itself or for one of its parents
            // (e.g. "/dimensions") covers it.
            !package.provenance.keys().any(|key| {
                path.strip_prefix(key.as_str())
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
            })
        })
        .map(|path| {
            Diagnostic::error(
                "provenance.missing",
                entity,
                Some(path),
                format!("\"{path}\" has no provenance entry"),
            )
        })
        .collect()
}

/// A tab's trimmed leads must be lead positions of the package, and a
/// tab needs its width (the generators can't draw a tab without it).
pub(crate) fn check_package_tab(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let Some(tab) = &package.tab else {
        return vec![];
    };
    let mut out = Vec::new();
    if tab.number.trim().is_empty() {
        out.push(Diagnostic::error(
            "tab.invalid",
            entity,
            Some("/tab/number"),
            "tab.number is empty".to_string(),
        ));
    }
    for lead in &tab.trimmed_leads {
        let valid = lead
            .parse::<u32>()
            .is_ok_and(|n| (1..=package.lead_count).contains(&n));
        if !valid {
            out.push(Diagnostic::error(
                "tab.invalid",
                entity,
                Some("/tab/trimmed_leads"),
                format!(
                    "trimmed lead \"{lead}\" is not a lead position (1..={})",
                    package.lead_count
                ),
            ));
        }
    }
    if package.dimensions.tab_width.is_none() {
        out.push(Diagnostic::error(
            "tab.invalid",
            entity,
            Some("/dimensions/tab_width"),
            "a package with a tab must state dimensions.tab_width".to_string(),
        ));
    }
    out
}

/// Auxiliary pads (`module`): distinct, non-empty numbers that don't
/// reuse an edge pin's number, and the row's size and place stated (the
/// generator can't draw them otherwise).
pub(crate) fn check_package_aux_pads(entity: &str, package: &Package) -> Vec<Diagnostic> {
    if package.aux_pads.is_empty() {
        return vec![];
    }
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for number in &package.aux_pads {
        let edge_pin = number
            .parse::<u32>()
            .is_ok_and(|n| (1..=package.lead_count).contains(&n));
        let problem = if number.trim().is_empty() {
            Some("an aux pad number is empty".to_string())
        } else if edge_pin {
            Some(format!(
                "aux pad \"{number}\" reuses an edge pin number (1..={})",
                package.lead_count
            ))
        } else if !seen.insert(number.as_str()) {
            Some(format!("aux pad \"{number}\" is listed twice"))
        } else {
            None
        };
        if let Some(message) = problem {
            out.push(Diagnostic::error(
                "aux_pads.invalid",
                entity,
                Some("/aux_pads"),
                message,
            ));
        }
    }
    let d = &package.dimensions;
    for (path, dim) in [
        ("/dimensions/aux_pad_width", &d.aux_pad_width),
        ("/dimensions/aux_pad_length", &d.aux_pad_length),
        ("/dimensions/aux_row_x", &d.aux_row_x),
        ("/dimensions/aux_row_y", &d.aux_row_y),
    ] {
        if dim.is_none() {
            out.push(Diagnostic::error(
                "aux_pads.invalid",
                entity,
                Some(path),
                format!("a package with aux_pads must state {path}"),
            ));
        }
    }
    out
}

/// Pin headers and sockets: the grid and the drill must be stated (the
/// generator never guesses either), and a right-angle header's housing
/// position.
pub(crate) fn check_package_header(entity: &str, package: &Package) -> Vec<Diagnostic> {
    if !package.family.is_header() {
        return vec![];
    }
    let right_angle = package.family == openparts_core::PackageFamily::RightAngleHeader;
    let max_rows = if right_angle { 1 } else { 2 };
    let mut out = Vec::new();
    let rows = match package.lead_layout.as_deref() {
        Some(&[rows, per_row])
            if (1..=max_rows).contains(&rows) && rows * per_row == package.lead_count =>
        {
            Some(rows)
        }
        _ => {
            out.push(Diagnostic::error(
                "header.invalid",
                entity,
                Some("/lead_layout"),
                format!(
                    "a {} needs lead_layout [rows, pins_per_row] with {} and \
                     rows x pins_per_row = lead_count ({})",
                    package.family,
                    if right_angle { "1 row" } else { "1 or 2 rows" },
                    package.lead_count
                ),
            ));
            None
        }
    };
    let d = &package.dimensions;
    if d.hole_diameter.is_none() {
        out.push(Diagnostic::error(
            "header.invalid",
            entity,
            Some("/dimensions/hole_diameter"),
            "a header must state dimensions.hole_diameter (the recommended PCB hole)".to_string(),
        ));
    }
    if rows == Some(2) && d.row_spacing.is_none() {
        out.push(Diagnostic::error(
            "header.invalid",
            entity,
            Some("/dimensions/row_spacing"),
            "a 2-row header must state dimensions.row_spacing".to_string(),
        ));
    }
    if right_angle && d.body_offset.is_none() {
        out.push(Diagnostic::error(
            "header.invalid",
            entity,
            Some("/dimensions/body_offset"),
            "a right-angle header must state dimensions.body_offset (where its housing lies \
             beside the holes)"
                .to_string(),
        ));
    }
    out
}

/// The largest / smallest value a dimension states (nominal, min or
/// max) -- comparisons use only stated numbers, never a synthesized one.
pub(crate) fn largest(dim: &openparts_core::Dimension) -> Option<f64> {
    [dim.typ, dim.min, dim.max]
        .into_iter()
        .flatten()
        .reduce(f64::max)
}

pub(crate) fn smallest(dim: &openparts_core::Dimension) -> Option<f64> {
    [dim.typ, dim.min, dim.max]
        .into_iter()
        .flatten()
        .reduce(f64::min)
}

pub(crate) fn check_package_consistency(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let d = &package.dimensions;
    let mut out = Vec::new();
    // (larger, smaller): `larger` must exceed `smaller` even comparing
    // the most pessimistic stated values.
    let pairs = [
        (
            ("/dimensions/overall_width", d.overall_width.as_ref()),
            ("/dimensions/body_width", Some(&d.body_width)),
        ),
        (
            ("/dimensions/overall_length", d.overall_length.as_ref()),
            ("/dimensions/body_length", Some(&d.body_length)),
        ),
        (
            ("/dimensions/overall_height", d.overall_height.as_ref()),
            ("/dimensions/standoff", d.standoff.as_ref()),
        ),
        (
            ("/dimensions/overall_height", d.overall_height.as_ref()),
            ("/dimensions/body_height", d.body_height.as_ref()),
        ),
    ];
    for ((big_path, big), (small_path, small)) in pairs {
        let (Some(big), Some(small)) = (big, small) else {
            continue;
        };
        let (Some(big_v), Some(small_v)) = (largest(big), smallest(small)) else {
            continue;
        };
        if big_v <= small_v {
            out.push(Diagnostic::error(
                "dimension.inconsistent",
                entity,
                Some(big_path),
                format!(
                    "{big_path} (at most {big_v}) must be larger than {small_path} (at least {small_v})"
                ),
            ));
        }
    }
    out
}

pub(crate) fn check_dimension(
    entity: &str,
    field_path: &str,
    dim: &openparts_core::Dimension,
) -> Option<Diagnostic> {
    let (Some(nominal), Some(min), Some(max)) = (dim.typ, dim.min, dim.max) else {
        // Only checkable when all three are present (Testing and Quality
        // Specification section 6: never synthesize a missing value in
        // order to validate).
        return None;
    };
    if !(min <= nominal && nominal <= max) {
        Some(Diagnostic::error(
            "dimension.out_of_range",
            entity,
            Some(field_path),
            format!("typ ({nominal}) is not within [{min}, {max}]"),
        ))
    } else {
        None
    }
}

/// `pad_order`: exactly one distinct number per lead position.
pub(crate) fn check_package_pad_order(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let order = &package.pad_order;
    if order.is_empty() {
        return vec![];
    }
    let distinct: std::collections::BTreeSet<&String> = order.iter().collect();
    if order.len() == package.lead_count as usize && distinct.len() == order.len() {
        return vec![];
    }
    vec![Diagnostic::error(
        "pad_order.invalid",
        entity,
        Some("/pad_order"),
        format!(
            "pad_order must list {} distinct pad numbers (got {}, {} distinct)",
            package.lead_count,
            order.len(),
            distinct.len()
        ),
    )]
}

/// `edge`: the PCB edge line, when drawn, must leave every land on the
/// board (in the package frame, along the edge side's normal).
pub(crate) fn check_package_edge(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let Some(edge) = &package.edge else {
        return vec![];
    };
    let Some(line) = edge.pcb_edge else {
        return vec![];
    };
    if !line.is_finite() {
        return vec![Diagnostic::error(
            "edge.invalid",
            entity,
            Some("/edge/pcb_edge"),
            "edge.pcb_edge is not a number".to_string(),
        )];
    }
    let Ok(g) = openparts_mcad::generate(package) else {
        return vec![];
    };
    let (nx, ny) = edge.side.normal();
    let lands = if g.lands.is_empty() {
        &g.leads
    } else {
        &g.lands
    };
    let limit = (nx + ny) * line;
    lands
        .iter()
        .filter(|l| {
            let reach = nx * l.position.x
                + ny * l.position.y
                + (nx.abs() * l.size.x + ny.abs() * l.size.y) / 2.0;
            reach > limit + 1e-6
        })
        .map(|l| {
            Diagnostic::error(
                "edge.invalid",
                entity,
                Some("/edge/pcb_edge"),
                format!(
                    "pad \"{}\" reaches past the PCB edge line ({line} along {})",
                    l.number,
                    serde_json::to_string(&edge.side).unwrap_or_default()
                ),
            )
        })
        .collect()
}

/// `keepouts`: each either located (`rect`) or only sized (`size`), with
/// positive lengths and at least one restriction.
pub(crate) fn check_package_keepouts(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (i, k) in package.keepouts.iter().enumerate() {
        let path = format!("/keepouts/{i}");
        let mut bad = |msg: &str| {
            out.push(Diagnostic::error(
                "keepout.invalid",
                entity,
                Some(&path),
                msg.to_string(),
            ))
        };
        match (&k.rect, &k.size) {
            (Some(_), Some(_)) | (None, None) => {
                bad("a keep-out has exactly one of rect (located) or size (only sized)")
            }
            (Some(r), None) if !(r.width > 0.0 && r.length > 0.0) => {
                bad("a keep-out's rect needs a positive width and length")
            }
            (None, Some(s)) if !(s[0] > 0.0 && s[1] > 0.0) => {
                bad("a keep-out's size needs two positive lengths")
            }
            _ => {}
        }
        if k.restricts.is_empty() {
            bad("a keep-out restricts something (components, copper, tracks, vias)");
        }
    }
    out
}

#[cfg(test)]
mod edge_keepout_tests {
    use super::*;

    fn usb(extra: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: usb_c\n\
             lead_count: 2\npitch: {{unit: mm}}\ndimensions:\n\
             \x20 body_width: {{nominal: 8.94, unit: mm}}\n\
             \x20 body_length: {{nominal: 7.35, unit: mm}}\n\
             \x20 body_height: {{nominal: 3.31, unit: mm}}\n\
             \x20 body_offset: {{nominal: -1.075, unit: mm}}\n\
             pads:\n\
             \x20 - {{number: A1, x: 3.2, y: -4.755, width: 0.6, length: 1.15}}\n\
             \x20 - {{number: SHELL, x: -4.32, y: 0.0, width: 1.0, length: 1.8, drill: 0.6, drill_length: 1.4}}\n\
             {extra}"
        ))
        .unwrap()
    }

    fn rules(d: &[Diagnostic]) -> Vec<&str> {
        d.iter().map(|d| d.rule_id).collect()
    }

    #[test]
    fn an_edge_line_through_a_land_is_an_error() {
        let ok = usb("edge: {side: +y, pcb_edge: 2.6}\n");
        assert!(check_package_edge("p", &ok).is_empty());
        // The front slot's land reaches y = 0.9.
        let bad = usb("edge: {side: +y, pcb_edge: 0.5}\n");
        assert_eq!(rules(&check_package_edge("p", &bad)), ["edge.invalid"]);
        // No line drawn: nothing to check.
        let recommended = usb("edge: {side: -y, requirement: recommended}\n");
        assert!(check_package_edge("p", &recommended).is_empty());
    }

    #[test]
    fn edge_and_keepouts_need_provenance() {
        let p = usb("edge: {side: +y, pcb_edge: 2.6}\n\
             keepouts:\n  - {purpose: antenna, restricts: [copper], size: [14, 9], side: -y}\n\
             provenance:\n  /pads: [{source: x/Y}]\n  /dimensions/body_offset: [{source: x/Y}]\n");
        let missing: Vec<_> = check_package_provenance("p", &p)
            .into_iter()
            .filter_map(|d| d.field_path)
            .collect();
        assert_eq!(missing, ["/edge", "/keepouts"]);
    }

    #[test]
    fn a_keepout_is_located_or_sized_not_both() {
        let both = usb(
            "keepouts:\n  - {purpose: other, restricts: [copper], size: [1, 2], \
             rect: {x: 0, y: 0, width: 1, length: 2}}\n",
        );
        assert_eq!(
            rules(&check_package_keepouts("p", &both)),
            ["keepout.invalid"]
        );
        let empty = usb("keepouts:\n  - {purpose: other, restricts: [], size: [1, 0]}\n");
        assert_eq!(check_package_keepouts("p", &empty).len(), 2);
        let ok = usb(
            "keepouts:\n  - {purpose: antenna, restricts: [components, copper], size: [14, 9]}\n",
        );
        assert!(check_package_keepouts("p", &ok).is_empty());
    }
}
