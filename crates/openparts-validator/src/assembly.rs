//! Module assemblies (Canonical Data Specification section 20.12): each
//! component is a known part or a box, cited, not itself an assembly,
//! and (a warning) on the board.

use crate::diagnostic::Diagnostic;
use openparts_core::{Device, Package, Part};
use std::collections::{BTreeMap, BTreeSet};

/// What an assembly's components are resolved against: the dataset's
/// parts (by id), devices and packages (by id).
#[derive(Debug, Clone, Copy)]
pub struct AssemblyContext<'a> {
    pub parts: &'a BTreeMap<String, &'a Part>,
    pub devices: &'a BTreeMap<String, Device>,
    pub packages: &'a BTreeMap<String, Package>,
}

/// Checks `part`'s `assembly` on its (board) `package`.
pub fn validate_assembly(
    part: &Part,
    package: &Package,
    ctx: &AssemblyContext<'_>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if part.assembly.is_empty() {
        return out;
    }
    let entity = format!("part:{}", part.id);
    let board = openparts_mcad::generate(package).ok().map(|g| {
        let b = g.body;
        (
            b.position.x - b.size.x / 2.0,
            b.position.y - b.size.y / 2.0,
            b.position.x + b.size.x / 2.0,
            b.position.y + b.size.y / 2.0,
        )
    });
    let cited = |i: usize| {
        part.provenance.contains_key("/assembly")
            || part.provenance.contains_key(&format!("/assembly/{i}"))
    };
    let mut refs = BTreeSet::new();
    for (i, c) in part.assembly.iter().enumerate() {
        let path = format!("/assembly/{i}");
        let mut bad = |msg: String| {
            out.push(Diagnostic::error(
                "assembly.invalid",
                &entity,
                Some(&path),
                msg,
            ))
        };
        if c.reference.trim().is_empty() {
            bad("a component needs a ref (its reference designator)".into());
        } else if !refs.insert(c.reference.as_str()) {
            bad(format!("ref \"{}\" is used twice", c.reference));
        }
        if c.part.is_some() == c.body.is_some() {
            bad(format!(
                "{}: give exactly one of part (an OpenParts part) or body (a placeholder box)",
                c.reference
            ));
        }
        if !(c.x.is_finite() && c.y.is_finite() && c.rotation.is_finite()) {
            bad(format!(
                "{}: x, y and rotation must be numbers",
                c.reference
            ));
        }
        if let Some(b) = &c.body {
            if !(b.width > 0.0 && b.length > 0.0 && b.height > 0.0) {
                bad(format!(
                    "{}: a body needs a positive width, length and height",
                    c.reference
                ));
            }
            if let Some(color) = &b.color {
                let hex = color.strip_prefix('#').unwrap_or("");
                if hex.len() != 6 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
                    bad(format!("{}: color \"{color}\" is not #rrggbb", c.reference));
                }
            }
        }
        if !cited(i) {
            out.push(Diagnostic::error(
                "provenance.missing",
                &entity,
                Some(&path),
                format!(
                    "{}: an assembly component needs a src (the design file or BOM it was read from)",
                    c.reference
                ),
            ));
        }

        // The component's outline on the board: its pads and body in its
        // own Placement Model frame, placed; a box is its own outline.
        let mut corners: Vec<(f64, f64)> = Vec::new();
        if let Some(id) = &c.part {
            if *id == part.id {
                out.push(Diagnostic::error(
                    "assembly.nested",
                    &entity,
                    Some(&format!("{path}/part")),
                    format!("{}: a module can't be mounted on itself", c.reference),
                ));
                continue;
            }
            let Some(component) = ctx.parts.get(&id.0) else {
                out.push(Diagnostic::error(
                    "assembly.unknown_part",
                    &entity,
                    Some(&format!("{path}/part")),
                    format!("{}: part {id} is not in the dataset", c.reference),
                ));
                continue;
            };
            if !component.assembly.is_empty() {
                out.push(Diagnostic::error(
                    "assembly.nested",
                    &entity,
                    Some(&format!("{path}/part")),
                    format!(
                        "{}: part {id} has an assembly of its own; assemblies don't nest",
                        c.reference
                    ),
                ));
            }
            let (Some(device), Some(pkg)) = (
                ctx.devices.get(&component.device.0),
                ctx.packages.get(&component.package.0),
            ) else {
                continue;
            };
            let Ok(g) = openparts_mcad::generate(pkg) else {
                continue;
            };
            let m =
                openparts_placement::build(pkg, Some(device), &g, openparts_placement::Density::N);
            for p in &m.pads {
                let (lo, hi) = p.bounds();
                corners.extend([(lo.x, lo.y), (hi.x, hi.y), (lo.x, hi.y), (hi.x, lo.y)]);
            }
            for r in &m.body.regions {
                let (hx, hy) = (r.size.x / 2.0, r.size.y / 2.0);
                for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    corners.push((r.center.x + sx * hx, r.center.y + sy * hy));
                }
            }
        } else if let Some(b) = &c.body {
            let (hx, hy) = (b.width / 2.0, b.length / 2.0);
            corners.extend([(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)]);
        }
        let Some((x0, y0, x1, y1)) = board else {
            continue;
        };
        const TOL: f64 = 0.01;
        let off = corners
            .iter()
            .map(|&(px, py)| c.place(px, py))
            .map(|(x, y)| (x0 - x).max(x - x1).max(y0 - y).max(y - y1))
            .fold(f64::MIN, f64::max);
        if off > TOL {
            out.push(Diagnostic::warning(
                "assembly.outside",
                &entity,
                Some(&path),
                format!(
                    "{} reaches {off:.2} mm past the board outline ({:.2} x {:.2}): check its \
                     x/y/rotation (a connector overhanging the edge is expected)",
                    c.reference,
                    x1 - x0,
                    y1 - y0
                ),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml<T: serde::de::DeserializeOwned>(s: &str) -> T {
        serde_yaml::from_str(s).unwrap()
    }

    fn board() -> Package {
        yaml(
            "schema_version: '0.1'\nkind: package\nid: standards/BOARD\nfamily: module\n\
             lead_count: 4\npitch: {nominal: 2.54, unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {nominal: 18, unit: mm}\n\
             \x20 body_length: {nominal: 10, unit: mm}\n\
             pad_rows:\n\
             \x20 - {side: -x, first: 1, count: 2, pitch: 2.54, start: 3, inset: 1.27, pad: [1.7, 1.7], drill: 1.0}\n\
             \x20 - {side: +x, first: 3, count: 2, pitch: 2.54, start: 3, inset: 1.27, pad: [1.7, 1.7], drill: 1.0}\n",
        )
    }

    fn part(id: &str, assembly: &str) -> Part {
        yaml(&format!(
            "schema_version: '0.1'\nkind: part\nid: {id}\nmanufacturer: t\nmpn: X\n\
             device: t/D\npackage: standards/BOARD\nexistence: {{status: unverified}}\n\
             lifecycle: {{status: active}}\n{assembly}"
        ))
    }

    fn rules(d: &[Diagnostic]) -> Vec<String> {
        d.iter()
            .map(|d| format!("{} {}", d.rule_id, d.field_path.as_deref().unwrap_or("")))
            .collect()
    }

    #[test]
    fn components_are_known_cited_unnested_and_on_the_board() {
        let chip: Part = yaml(
            "schema_version: '0.1'\nkind: part\nid: t/R1\nmanufacturer: t\nmpn: R1\n\
             device: t/R\npackage: standards/R0603\nexistence: {status: unverified}\n\
             lifecycle: {status: active}\n",
        );
        let nested = part(
            "t/NESTED",
            "assembly:\n  - {ref: U9, body: {width: 1, length: 1, height: 1}, x: 0, y: 0}\n",
        );
        let devices: BTreeMap<String, Device> = [(
            "t/R".to_string(),
            yaml::<Device>(
                "schema_version: '0.1'\nkind: device\nid: t/R\nmanufacturer: t\ncategory: resistor\n\
                 pins:\n  \"1\": {name: \"1\", type: passive}\n  \"2\": {name: \"2\", type: passive}\n",
            ),
        )]
        .into();
        let packages: BTreeMap<String, Package> = [(
            "standards/R0603".to_string(),
            yaml::<Package>(
                "schema_version: '0.1'\nkind: package\nid: standards/R0603\nfamily: chip\n\
                 lead_count: 2\npitch: {nominal: 1.6, unit: mm}\n\
                 dimensions:\n\
                 \x20 body_width: {nominal: 1.6, unit: mm}\n\
                 \x20 body_length: {nominal: 0.8, unit: mm}\n\
                 \x20 lead_length: {nominal: 0.3, unit: mm}\n",
            ),
        )]
        .into();
        let parts: BTreeMap<String, &Part> = [
            ("t/R1".to_string(), &chip),
            ("t/NESTED".to_string(), &nested),
        ]
        .into();
        let ctx = AssemblyContext {
            parts: &parts,
            devices: &devices,
            packages: &packages,
        };
        let module = part(
            "t/M",
            "assembly:\n\
             \x20 - {ref: R1, part: t/R1, x: 0, y: 0, rotation: 90}\n\
             \x20 - {ref: R2, part: t/R1, x: 8.9, y: 0}\n\
             \x20 - {ref: U1, part: t/NOPE, x: 0, y: 0}\n\
             \x20 - {ref: U2, part: t/NESTED, x: 0, y: 0}\n\
             \x20 - {ref: J1, body: {width: 2, length: 2, height: 1, color: '#c0c0c0'}, x: 0, y: 4, side: bottom}\n\
             \x20 - {ref: J1, body: {width: 0, length: 2, height: 1, color: red}, part: t/R1, x: 0, y: 0}\n\
             provenance:\n\
             \x20 /assembly/0: [{source: s/DESIGN}]\n\
             \x20 /assembly/1: [{source: s/DESIGN}]\n\
             \x20 /assembly/2: [{source: s/DESIGN}]\n\
             \x20 /assembly/3: [{source: s/DESIGN}]\n\
             \x20 /assembly/4: [{source: s/DESIGN}]\n",
        );
        let d = validate_assembly(&module, &board(), &ctx);
        let r = rules(&d);
        assert!(
            r.contains(&"assembly.outside /assembly/1".to_string()),
            "{r:?}"
        );
        assert!(r.contains(&"assembly.unknown_part /assembly/2/part".to_string()));
        assert!(r.contains(&"assembly.nested /assembly/3/part".to_string()));
        assert!(
            !r.iter()
                .any(|x| x.ends_with("/assembly/0") || x.ends_with("/assembly/4")),
            "{r:?}"
        );
        let last: Vec<&Diagnostic> = d
            .iter()
            .filter(|d| d.field_path.as_deref() == Some("/assembly/5"))
            .collect();
        // Duplicate ref, part and body, bad size, bad colour, no src.
        assert_eq!(last.len(), 5, "{last:?}");
        assert!(last.iter().any(|d| d.rule_id == "provenance.missing"));
    }
}
