//! Packae checks: dimensions, their consistency and provenance, and the
//! family-specific fields (tab, aux pads, headers, pad order).

use crate::Diagnostic;
use openparts_core::Package;
use std::collections::BTreeSet;

/// Paths of the package fields that must carry their own provenance.
pub(crate) const PROVENANCE_REQUIRED: [&str; 36] = [
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
    "/dimensions/exposed_pad/land_width",
    "/dimensions/exposed_pad/land_length",
    "/dimensions/body_offset",
    "/dimensions/lead_inset",
    "/dimensions/actuator_diameter",
    "/dimensions/actuator_length",
    "/dimensions/actuator_elevation",
    "/dimensions/actuator_width",
    "/dimensions/actuator_thickness",
    "/dimensions/ground_terminal_offset",
    "/dimensions/ground_terminal_width",
    "/dimensions/can_diameter",
    "/dimensions/bushing_diameter",
    "/dimensions/bushing_length",
    "/dimensions/actuator_travel",
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
    if package.actuation.is_some() {
        present.push("/actuation");
    }
    if package.terminal_shape.is_some() {
        present.push("/terminal_shape");
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
    if !package.pad_rows.is_empty() {
        present.push("/pad_rows");
    }
    if !package.pad_grids.is_empty() {
        present.push("/pad_grids");
    }
    if package.shield.is_some() {
        present.push("/shield");
    }
    if package.smt.is_some() {
        present.push("/smt");
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
    if package.smt.is_some()
        && !matches!(
            package.family,
            openparts_core::PackageFamily::ShroudedHeader
                | openparts_core::PackageFamily::RightAngleHeader
        )
    {
        out.push(Diagnostic::error(
            "header.invalid",
            entity,
            Some("/smt"),
            format!(
                "smt (surface-mount contacts) is for shrouded_header and right_angle_header, \
                 not {}",
                package.family
            ),
        ));
    }
    if package.smt.is_none() && d.hole_diameter.is_none() {
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

/// `tact_switch.invalid`: a tact switch has 2 to 5 terminals (an odd
/// count: a ground terminal, placed by `ground_terminal_offset` unless a
/// THT package's `pads` place every hole); `actuation` and the side
/// actuator and ground-terminal dimensions belong to the tact families,
/// the side actuator's to `actuation: side`, which needs
/// `actuator_length`; `actuator_width` and `actuator_thickness` go
/// together. Outside them: `actuation: side` with `actuator_length` /
/// `actuator_elevation` on a rotary encoder or shaft potentiometer, a
/// slide potentiometer's lever `actuator_width` / `actuator_thickness`. A THT package with `pads` gives every plated hole a drill.
pub(crate) fn check_package_tact_switch(entity: &str, package: &Package) -> Vec<Diagnostic> {
    use openparts_core::{Actuation, PackageFamily as F};
    let d = &package.dimensions;
    let mut out = Vec::new();
    let mut bad = |path: &str, message: &str| {
        out.push(Diagnostic::error(
            "tact_switch.invalid",
            entity,
            Some(path),
            message.to_string(),
        ))
    };
    let tact = matches!(package.family, F::TactSwitchSmd | F::TactSwitchTht);
    let side = [
        ("/dimensions/actuator_length", d.actuator_length.is_some()),
        (
            "/dimensions/actuator_elevation",
            d.actuator_elevation.is_some(),
        ),
        ("/dimensions/actuator_width", d.actuator_width.is_some()),
        (
            "/dimensions/actuator_thickness",
            d.actuator_thickness.is_some(),
        ),
    ];
    let ground = [
        (
            "/dimensions/ground_terminal_offset",
            d.ground_terminal_offset.is_some(),
        ),
        (
            "/dimensions/ground_terminal_width",
            d.ground_terminal_width.is_some(),
        ),
    ];
    if !tact {
        // Encoders and shaft potentiometers take a side shaft
        // (`actuation: side`, its length and elevation); a slide
        // potentiometer's lever its width and thickness.
        let shaft = openparts_core::is_shaft_family(package.family);
        let slide = package.family == F::SlidePotentiometer;
        let is_side = package.actuation == Some(Actuation::Side);
        if package.actuation.is_some() && !shaft {
            bad(
                "/actuation",
                "actuation belongs to the tact switch, rotary_encoder and potentiometer families",
            );
        }
        for (i, (path, present)) in side.iter().enumerate() {
            if !*present {
                continue;
            }
            let allowed = if i < 2 { shaft } else { slide };
            if !allowed {
                bad(
                    path,
                    "this dimension belongs to the tact switch families (actuator_length and \
                     actuator_elevation also to a side-shaft rotary_encoder or potentiometer, \
                     actuator_width and actuator_thickness to a slide_potentiometer)",
                );
            } else if shaft && !is_side {
                bad(
                    path,
                    "a shaft's actuator_length and actuator_elevation need actuation: side \
                     (a vertical shaft's top is overall_height)",
                );
            }
        }
        for (path, present) in &ground {
            if *present {
                bad(path, "this dimension belongs to the tact switch families");
            }
        }
        return out;
    }
    let n = package.lead_count;
    let tht_pads = package.family == F::TactSwitchTht && !package.pads.is_empty();
    if !(2..=5).contains(&n) {
        bad(
            "/lead_count",
            "a tact switch has 2 or 4 terminals, plus an optional ground terminal (3 or 5)",
        );
    } else if n % 2 == 1 && d.ground_terminal_offset.is_none() && !tht_pads {
        bad(
            "/dimensions/ground_terminal_offset",
            "a tact switch with a ground terminal (odd lead_count) states ground_terminal_offset",
        );
    } else if n.is_multiple_of(2) {
        for (path, present) in &ground {
            if *present {
                bad(
                    path,
                    "only a tact switch with a ground terminal (odd lead_count) states this",
                );
            }
        }
    }
    if package.actuation == Some(Actuation::Side) {
        if d.actuator_length.is_none() {
            bad(
                "/dimensions/actuator_length",
                "a side-operated switch states actuator_length (its reach beyond the housing face)",
            );
        }
        if d.actuator_width.is_some() != d.actuator_thickness.is_some() {
            bad(
                "/dimensions/actuator_width",
                "actuator_width and actuator_thickness go together",
            );
        }
    } else {
        for (path, present) in &side {
            if *present {
                bad(
                    path,
                    "only a side-operated switch (actuation: side) states this",
                );
            }
        }
    }
    if tht_pads && package.pads.iter().any(|p| p.plated && p.drill.is_none()) {
        bad(
            "/pads",
            "a tact_switch_tht pad is a PCB hole: every plated pad states its drill",
        );
    }
    out
}

/// `pad_order`: exactly one distinct number per lead position.
pub(crate) fn check_package_pad_order(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let order = &package.pad_order;
    if order.is_empty() {
        return vec![];
    }
    // An empty entry is a lead position without a lead.
    let numbers: Vec<&String> = order.iter().filter(|n| !n.trim().is_empty()).collect();
    let distinct: std::collections::BTreeSet<&String> = numbers.iter().copied().collect();
    if order.len() == package.lead_count as usize
        && distinct.len() == numbers.len()
        && !numbers.is_empty()
    {
        return vec![];
    }
    vec![Diagnostic::error(
        "pad_order.invalid",
        entity,
        Some("/pad_order"),
        format!(
            "pad_order must have {} entries (lead positions), the pad numbers distinct and at least one (got {}, {} numbers, {} distinct)",
            package.lead_count,
            order.len(),
            numbers.len(),
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

/// `pad_rows` / `pad_grids` / `shield` (`module` only) and `smt`
/// (headers, see [`check_package_header`]): positive sizes and counts,
/// distinct pad numbers, and the fields only on the family that draws
/// them.
pub(crate) fn check_package_pad_layout(entity: &str, package: &Package) -> Vec<Diagnostic> {
    use openparts_core::PackageFamily as F;
    let mut out = Vec::new();
    let mut bad = |path: &str, msg: String| {
        out.push(Diagnostic::error(
            "pad_layout.invalid",
            entity,
            Some(path),
            msg,
        ))
    };
    let module = package.family == F::Module;
    for (field, used) in [
        ("/pad_rows", !package.pad_rows.is_empty()),
        ("/pad_grids", !package.pad_grids.is_empty()),
        ("/shield", package.shield.is_some()),
    ] {
        if used && !module {
            bad(
                field,
                format!("{field} is for the module family, not {}", package.family),
            );
        }
    }
    if !package.pad_grids.is_empty() && package.pad_rows.is_empty() && package.pads.is_empty() {
        bad(
            "/pad_grids",
            "pad_grids needs pad_rows or pads (the module's edge, LGA or through-hole pads)"
                .to_string(),
        );
    }
    let mut numbers = BTreeSet::new();
    for (i, r) in package.pad_rows.iter().enumerate() {
        let path = format!("/pad_rows/{i}");
        if r.count == 0 || r.first == 0 || !(r.pad[0] > 0.0 && r.pad[1] > 0.0) {
            bad(
                &path,
                "a pad row needs a count, a first number from 1 and a positive pad size".into(),
            );
        }
        if r.count > 1 && r.pitch <= r.pad[0] {
            bad(
                &path,
                format!(
                    "pitch ({}) must exceed the pad size along the edge ({})",
                    r.pitch, r.pad[0]
                ),
            );
        }
        if let Some(d) = r.drill {
            if !(d > 0.0 && d < r.pad[0].min(r.pad[1])) {
                bad(
                    &path,
                    format!(
                        "drill ({d}) must be positive and leave copper around it (pad {:?})",
                        r.pad
                    ),
                );
            }
        }
        for n in r.first..r.first + r.count {
            if !numbers.insert(n.to_string()) {
                bad(&path, format!("pad \"{n}\" is in two rows"));
            }
        }
    }
    for (i, g) in package.pad_grids.iter().enumerate() {
        let path = format!("/pad_grids/{i}");
        let n = (g.columns * g.rows) as usize;
        if n == 0 || !(g.pad[0] > 0.0 && g.pad[1] > 0.0) {
            bad(
                &path,
                "a pad grid needs columns, rows and a positive pad size".into(),
            );
        }
        if (g.columns > 1 && g.pitch[0] <= g.pad[0]) || (g.rows > 1 && g.pitch[1] <= g.pad[1]) {
            bad(&path, "a pad grid's pitch must exceed its pad size".into());
        }
        if !(g.numbers.len() == 1 || g.numbers.len() == n) {
            bad(
                &path,
                format!(
                    "a pad grid has one number or one per pad ({n}), not {}",
                    g.numbers.len()
                ),
            );
        }
        if g.numbers.iter().any(|n| n.trim().is_empty()) {
            bad(&path, "a pad grid number is empty".into());
        }
        let own: BTreeSet<&String> = g.numbers.iter().collect();
        for n in own {
            if numbers.contains(n) {
                bad(&path, format!("pad \"{n}\" is also in a pad row"));
            }
        }
    }
    // A module's explicit pads join its rows and grids: one number each.
    if module && !package.pad_rows.is_empty() {
        for (i, p) in package.pads.iter().enumerate() {
            if p.plated && numbers.contains(&p.number) {
                bad(
                    &format!("/pads/{i}"),
                    format!("pad \"{}\" is also in a pad row", p.number),
                );
            }
        }
    }
    if let Some(r) = &package.shield {
        if !(r.width > 0.0 && r.length > 0.0) {
            bad(
                "/shield",
                "the shield needs a positive width and length".into(),
            );
        }
        if package.dimensions.overall_height.is_none() {
            bad(
                "/dimensions/overall_height",
                "a module with a shield must state dimensions.overall_height (its top)".into(),
            );
        }
    }
    if let Some(smt) = &package.smt {
        if !(smt.pad.iter().chain(&smt.mech_pad).all(|v| *v > 0.0)) {
            bad("/smt", "smt pad sizes must be positive".into());
        }
        if smt.mech_number.trim().is_empty() {
            bad("/smt", "smt.mech_number is empty".into());
        }
    }
    out
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

#[cfg(test)]
mod tact_switch_tests {
    use super::*;

    fn switch(family: &str, n: u32, extra: &str, dims: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: {family}\n\
             lead_count: {n}\npitch: {{nominal: 4.5, unit: mm}}\n{extra}dimensions:\n\
             \x20 body_width: {{nominal: 6, unit: mm}}\n\
             \x20 body_length: {{nominal: 6, unit: mm}}\n{dims}"
        ))
        .unwrap()
    }

    fn rules(p: &Package) -> Vec<String> {
        check_package_tact_switch("e", p)
            .into_iter()
            .map(|d| format!("{} {}", d.rule_id, d.field_path.unwrap_or_default()))
            .collect()
    }

    #[test]
    fn pad_order_may_leave_lead_positions_empty() {
        let order = |o: &str| {
            let p = switch("soic", 6, &format!("pad_order: {o}\n"), "");
            check_package_pad_order("p", &p).len()
        };
        assert_eq!(order("['1', '', '3', '4', '', '6']"), 0);
        assert_eq!(
            order("['1', '', '3', '4', '', '4']"),
            1,
            "a repeated number"
        );
        assert_eq!(order("['', '', '', '', '', '']"), 1, "no lead at all");
        assert_eq!(order("['1', '3', '4', '6']"), 1, "one entry per position");
    }

    #[test]
    fn ground_terminals_and_side_actuators() {
        let gnd = "  ground_terminal_offset: {nominal: 4.1, unit: mm}\n";
        assert!(rules(&switch("tact_switch_tht", 5, "", gnd)).is_empty());
        assert_eq!(
            rules(&switch("tact_switch_tht", 5, "", "")),
            ["tact_switch.invalid /dimensions/ground_terminal_offset"]
        );
        assert_eq!(
            rules(&switch("tact_switch_tht", 4, "", gnd)),
            ["tact_switch.invalid /dimensions/ground_terminal_offset"]
        );
        assert_eq!(
            rules(&switch("tact_switch_smd", 6, "", "")),
            ["tact_switch.invalid /lead_count"]
        );
        let side = "  actuator_length: {nominal: 0.7, unit: mm}\n";
        assert!(rules(&switch("tact_switch_smd", 2, "actuation: side\n", side)).is_empty());
        assert_eq!(
            rules(&switch("tact_switch_smd", 2, "", side)),
            ["tact_switch.invalid /dimensions/actuator_length"]
        );
        assert_eq!(
            rules(&switch("tact_switch_smd", 2, "actuation: side\n", "")),
            ["tact_switch.invalid /dimensions/actuator_length"]
        );
        let half = format!("{side}  actuator_width: {{nominal: 1.7, unit: mm}}\n");
        assert_eq!(
            rules(&switch("tact_switch_smd", 2, "actuation: side\n", &half)),
            ["tact_switch.invalid /dimensions/actuator_width"]
        );
        assert_eq!(
            rules(&switch("soic", 8, "actuation: side\n", side)),
            [
                "tact_switch.invalid /actuation",
                "tact_switch.invalid /dimensions/actuator_length"
            ]
        );
        // THT holes from pads: no ground offset needed, drills required.
        let pads = "pads:\n  - {number: '1', x: -3, y: 0, drill: 1}\n  \
                    - {number: '2', x: 3, y: 0, drill: 1}\n  - {number: '3', x: 0, y: -4}\n";
        let p = switch("tact_switch_tht", 3, pads, "");
        assert_eq!(rules(&p), ["tact_switch.invalid /pads"]);
    }
}
