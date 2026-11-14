//! Footprint pads from an explicit land pattern (`Package::pads`).

use crate::{Lead, McadError, Mounting, Point3, Size3};
use openparts_core::Package;

/// Every `Package::pads` entry as a `Lead`: SMD, plated (round or
/// slotted) or non-plated hole. `standoff` is the lead height a plated
/// pad's 3D tab reaches.
pub(crate) fn from_pads_with(package: &Package, standoff: f64) -> Result<Vec<Lead>, McadError> {
    package
        .pads
        .iter()
        .map(|p| {
            let bad = |why: &str| {
                McadError::InconsistentDimensions(format!("pad \"{}\": {why}", p.number))
            };
            if p.width <= 0.0 || p.length <= 0.0 {
                return Err(bad("non-positive size"));
            }
            if !p.plated && p.drill.is_none() {
                return Err(bad("a non-plated hole needs a drill"));
            }
            if p.plated && p.number.trim().is_empty() {
                return Err(bad("a plated pad needs a number"));
            }
            match (p.drill, p.drill_length) {
                (None, Some(_)) => return Err(bad("drill_length without a drill")),
                (Some(d), None) if p.plated && d >= p.width.min(p.length) => {
                    return Err(bad("the drill leaves no copper"))
                }
                (Some(d), Some(l)) if l < d || l >= p.width.max(p.length) => {
                    return Err(bad(
                        "slot length must lie between the drill and the pad length",
                    ))
                }
                _ => {}
            }
            let mounting = match (p.drill, p.plated) {
                (None, _) => Mounting::Smd,
                (Some(_), true) => Mounting::ThroughHole,
                (Some(_), false) => Mounting::NonPlated,
            };
            let smd = mounting == Mounting::Smd;
            Ok(Lead {
                number: p.number.clone(),
                position: Point3 {
                    x: p.x,
                    y: p.y,
                    z: if smd {
                        crate::defaults::land::PAD_T / 2.0
                    } else {
                        0.0
                    },
                },
                size: Size3 {
                    x: p.width,
                    y: p.length,
                    z: if smd {
                        crate::defaults::land::PAD_T
                    } else {
                        standoff
                    },
                },
                mounting,
                drill: p.drill,
                slot: p.drill_length,
            })
        })
        .collect()
}

pub(crate) fn from_pads(package: &Package) -> Result<Vec<Lead>, McadError> {
    from_pads_with(package, 0.0)
}

#[cfg(test)]
mod tests {
    use crate::{generate, Mounting};
    use openparts_core::Package;

    fn pkg(yaml: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\n{yaml}"
        ))
        .unwrap()
    }

    const SMD_SWITCH: &str = "family: tact_switch_smd\nlead_count: 4\n\
        pitch: {nominal: 4.5, unit: mm}\ndimensions:\n\
        \x20 body_width: {nominal: 6.0, unit: mm}\n\
        \x20 body_length: {nominal: 6.0, unit: mm}\n\
        \x20 body_height: {nominal: 3.5, unit: mm}\n\
        \x20 overall_width: {nominal: 9.0, unit: mm}\n\
        \x20 lead_width: {nominal: 0.7, unit: mm}\n";

    #[test]
    fn a_stated_land_pattern_becomes_the_footprint_and_may_have_npth_holes() {
        let p = pkg(&format!(
            "{SMD_SWITCH}pads:\n\
             \x20 - {{number: '1', x: -3.975, y: 2.25, width: 1.55, length: 1.0}}\n\
             \x20 - {{number: '', x: 0.0, y: 0.0, width: 1.0, length: 1.0, drill: 1.0, plated: false}}\n"
        ));
        let g = generate(&p).unwrap();
        assert_eq!(g.leads.len(), 4, "the 3D terminals stay the leads");
        assert_eq!(g.lands.len(), 2);
        assert_eq!(g.lands[1].mounting, Mounting::NonPlated);
        assert!((g.lands[0].position.x + 3.975).abs() < 1e-9);
    }

    #[test]
    fn pad_order_renumbers_any_family() {
        let mut p = pkg("family: lga\nlead_count: 8\nlead_layout: [4, 0, 4, 0]\n\
             pitch: {nominal: 0.65, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 2.5, unit: mm}\n\
             \x20 body_length: {nominal: 2.5, unit: mm}\n");
        // Clockwise in the top view: the left edge numbered upward.
        p.pad_order = ["4", "3", "2", "1", "8", "7", "6", "5"]
            .map(String::from)
            .to_vec();
        let g = generate(&p).unwrap();
        let top_left = &g.leads[0];
        assert_eq!(top_left.number, "4");
        p.pad_order.pop();
        assert!(generate(&p).is_err());
    }
}
