//! Footprint pads from an explicit land pattern (`Package::pads`).

use crate::{LandShape, Lead, McadError, Mounting, Point3, Size3};
use openparts_core::{LandShape as PadShapeIn, Package, PackagePad};

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
            let shape = land_shape(p).map_err(|why| bad(&why))?;
            let (width, length) = p.extent();
            if width <= 0.0 || length <= 0.0 {
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
                (Some(d), None) if p.plated && d >= width.min(length) => {
                    return Err(bad("the drill leaves no copper"))
                }
                (Some(d), Some(l)) if l < d || l >= width.max(length) => {
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
                    x: width,
                    y: length,
                    z: if smd {
                        crate::defaults::land::PAD_T
                    } else {
                        standoff
                    },
                },
                mounting,
                drill: p.drill,
                slot: p.drill_length,
                shape,
            })
        })
        .collect()
}

/// Why a stated pad's `shape`/`chamfer`/`corners`/`points`/`cutout`
/// can't be drawn (Canonical Data Specification 20.7), if they can't.
pub fn pad_shape_error(p: &PackagePad) -> Option<String> {
    land_shape(p).err()
}

/// A stated pad's land outline (Canonical Data Specification 20.7).
fn land_shape(p: &PackagePad) -> Result<LandShape, String> {
    let (w, l) = (p.width, p.length);
    let only = |what: &str, ok: bool| {
        if ok {
            Ok(())
        } else {
            Err(format!("{what} belongs to another pad shape"))
        }
    };
    only(
        "chamfer",
        p.chamfer.is_none() || p.shape == PadShapeIn::Chamfered,
    )?;
    only(
        "corners",
        p.corners.is_empty() || p.shape == PadShapeIn::Chamfered,
    )?;
    only(
        "points",
        p.points.is_empty() || p.shape == PadShapeIn::Polygon,
    )?;
    only("cutout", p.cutout.is_none() || p.shape == PadShapeIn::Rect)?;
    let shape = match p.shape {
        PadShapeIn::Rect => match p.cutout {
            None => LandShape::Rect,
            Some(c) => LandShape::Polygon(notched(w, l, c.x - p.x, c.y - p.y, c.diameter / 2.0)?),
        },
        PadShapeIn::Circle => {
            if (w - l).abs() > 1e-9 {
                return Err("a circle needs width = length (the diameter)".into());
            }
            LandShape::Circle
        }
        PadShapeIn::Chamfered => {
            let c = p.chamfer.ok_or("a chamfered pad needs chamfer")?;
            if c <= 0.0 || 2.0 * c > w.min(l) + 1e-9 {
                return Err("chamfer must be positive and at most half the shorter side".into());
            }
            LandShape::Chamfered {
                size: c,
                corners: p.chamfered_corners(),
            }
        }
        PadShapeIn::Polygon => {
            if p.points.len() < 3 {
                return Err("a polygon needs at least three points".into());
            }
            if p.width != 0.0 || p.length != 0.0 {
                return Err("a polygon's size is its points: leave width/length out".into());
            }
            let mut pts = p.points.clone();
            // Counter-clockwise, as every outline here.
            if signed_area(&pts) < 0.0 {
                pts.reverse();
            }
            if signed_area(&pts).abs() < 1e-9 {
                return Err("the polygon has no area".into());
            }
            if !contains(&pts, 0.0, 0.0) {
                return Err("the pad's x/y must lie inside its polygon".into());
            }
            LandShape::Polygon(pts)
        }
    };
    Ok(shape)
}

fn signed_area(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0
}

/// Whether (x, y) lies inside the polygon (even-odd rule).
pub(crate) fn contains(pts: &[[f64; 2]], x: f64, y: f64) -> bool {
    let n = pts.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        if (a[1] > y) != (b[1] > y) {
            let xi = a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if x < xi {
                inside = !inside;
            }
        }
    }
    inside
}

/// A `w` x `l` rectangle centred on the origin with the circle at
/// (`cx`, `cy`), radius `r`, cleared from it: one notch, the circle
/// crossing the outline exactly twice and leaving the centre. The arc is
/// drawn in segments of at most 5°. Counter-clockwise.
fn notched(w: f64, l: f64, cx: f64, cy: f64, r: f64) -> Result<Vec<[f64; 2]>, String> {
    const EPS: f64 = 1e-9;
    if r <= 0.0 {
        return Err("a cutout needs a positive diameter".into());
    }
    if cx.hypot(cy) <= r {
        return Err("the cutout covers the pad's centre".into());
    }
    let (hw, hl) = (w / 2.0, l / 2.0);
    let corners = [[-hw, -hl], [hw, -hl], [hw, hl], [-hw, hl]];
    let outside = |p: [f64; 2]| (p[0] - cx).hypot(p[1] - cy) > r + EPS;
    // The outline walked counter-clockwise: kept corners, and where it
    // enters (true) or leaves (false) the circle.
    let mut walk: Vec<([f64; 2], Option<bool>)> = Vec::new();
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        if outside(a) {
            walk.push((a, None));
        }
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let (fx, fy) = (a[0] - cx, a[1] - cy);
        let qa = dx * dx + dy * dy;
        let qb = 2.0 * (fx * dx + fy * dy);
        let qc = fx * fx + fy * fy - r * r;
        let disc = qb * qb - 4.0 * qa * qc;
        if disc <= EPS {
            continue;
        }
        let sq = disc.sqrt();
        for t in [(-qb - sq) / (2.0 * qa), (-qb + sq) / (2.0 * qa)] {
            if t > EPS && t < 1.0 - EPS {
                let p = [a[0] + t * dx, a[1] + t * dy];
                // Entering: the point just after lies inside the circle.
                let after = [p[0] + dx * 1e-6, p[1] + dy * 1e-6];
                walk.push((p, Some(!outside(after))));
            }
        }
    }
    let crossings: Vec<usize> = (0..walk.len()).filter(|&i| walk[i].1.is_some()).collect();
    if crossings.is_empty() {
        let inside = corners.iter().all(|&c| !outside(c));
        return if inside {
            Err("the cutout covers the whole pad".into())
        } else {
            Err("the cutout does not reach the pad".into())
        };
    }
    if crossings.len() != 2 {
        return Err("the cutout must cut a single notch".into());
    }
    let leave = *crossings
        .iter()
        .find(|&&i| walk[i].1 == Some(false))
        .ok_or("the cutout must cut a single notch")?;
    // From where the outline leaves the circle, along the outline to
    // where it enters it again, then back along the arc (clockwise
    // about the circle's centre: the land lies outside it).
    let n = walk.len();
    let mut out = Vec::new();
    let mut i = leave;
    loop {
        out.push(walk[i].0);
        i = (i + 1) % n;
        if walk[i].1 == Some(true) {
            out.push(walk[i].0);
            break;
        }
    }
    let enter = *out.last().unwrap();
    let a0 = (enter[1] - cy).atan2(enter[0] - cx);
    let a1 = (out[0][1] - cy).atan2(out[0][0] - cx);
    let mut sweep = a1 - a0;
    while sweep > 0.0 {
        sweep -= std::f64::consts::TAU;
    }
    while sweep <= -std::f64::consts::TAU {
        sweep += std::f64::consts::TAU;
    }
    let steps = ((sweep.abs() / 5f64.to_radians()).ceil() as usize).max(2);
    for k in 1..steps {
        let a = a0 + sweep * k as f64 / steps as f64;
        out.push([cx + r * a.cos(), cy + r * a.sin()]);
    }
    Ok(out
        .into_iter()
        .map(|p| [(p[0] * 1e4).round() / 1e4, (p[1] * 1e4).round() / 1e4])
        .collect())
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

    /// An MMCX jack's land pattern (Samtec MMCX-J-P-X-ST-SM1): a round
    /// centre pad, four square ground pads notched by a circle around it.
    const MMCX: &str = "family: rf_coax\nlead_count: 5\npitch: {unit: mm}\n\
        connector: {gender: female, orientation: vertical, interface: mmcx}\n\
        dimensions:\n\
        \x20 body_width: {nominal: 3.45, unit: mm}\n\
        \x20 body_length: {nominal: 3.45, unit: mm}\n\
        pads:\n\
        \x20 - {number: '1', x: 0, y: 0, width: 1.2, length: 1.2, shape: circle}\n\
        \x20 - {number: '2', x: 1.3, y: 1.3, width: 1.35, length: 1.35, cutout: {x: 0, y: 0, diameter: 2.5}}\n\
        \x20 - {number: '2', x: -1.3, y: 1.3, width: 1.35, length: 1.35, cutout: {x: 0, y: 0, diameter: 2.5}}\n\
        \x20 - {number: '2', x: -1.3, y: -1.3, width: 1.35, length: 1.35, cutout: {x: 0, y: 0, diameter: 2.5}}\n\
        \x20 - {number: '2', x: 1.3, y: -1.3, width: 1.35, length: 1.35, cutout: {x: 0, y: 0, diameter: 2.5}}\n";

    #[test]
    fn a_cutout_notches_the_land_with_an_arc_around_its_centre() {
        use crate::LandShape;
        let g = generate(&pkg(MMCX)).unwrap();
        assert_eq!(g.leads[0].shape, LandShape::Circle);
        let LandShape::Polygon(pts) = &g.leads[1].shape else {
            panic!("a notched pad is a polygon: {:?}", g.leads[1].shape)
        };
        // The pad at (1.3, 1.3): its lower-left corner is cut away; every
        // vertex lies inside the square and on or outside the circle.
        for p in pts {
            let (x, y) = (p[0] + 1.3, p[1] + 1.3);
            assert!(p[0].abs() <= 0.675 + 1e-9 && p[1].abs() <= 0.675 + 1e-9);
            assert!(x.hypot(y) >= 1.25 - 1e-3, "{p:?} inside the cutout");
        }
        // The notch meets the bottom and left edges where the circle
        // crosses them: x = sqrt(1.25² - 0.625²) - 1.3 = -0.2175.
        let meets = |q: [f64; 2]| {
            pts.iter()
                .any(|p| (p[0] - q[0]).abs() < 1e-3 && (p[1] - q[1]).abs() < 1e-3)
        };
        assert!(meets([-0.2175, -0.675]) && meets([-0.675, -0.2175]));
        assert!(!meets([-0.675, -0.675]), "the cut corner is gone");
        // Counter-clockwise, and the size stays the square.
        let area: f64 = (0..pts.len())
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum();
        assert!(area > 0.0);
        assert!((g.leads[1].size.x - 1.35).abs() < 1e-9);
    }

    #[test]
    fn chamfered_and_polygon_lands_and_their_errors() {
        use crate::LandShape;
        use openparts_core::PadCorner;
        let base = "family: tact_switch_smd\nlead_count: 4\n\
            pitch: {nominal: 4.5, unit: mm}\ndimensions:\n\
            \x20 body_width: {nominal: 6.0, unit: mm}\n\
            \x20 body_length: {nominal: 6.0, unit: mm}\n\
            \x20 body_height: {nominal: 3.5, unit: mm}\n\
            \x20 overall_width: {nominal: 9.0, unit: mm}\n\
            \x20 lead_width: {nominal: 0.7, unit: mm}\npads:\n";
        let with = |pad: &str| pkg(&format!("{base}\x20 - {pad}\n"));
        let g = generate(&with(
            "{number: '1', x: 0, y: 0, width: 1.0, length: 0.8, shape: chamfered, chamfer: 0.2, corners: [top_left]}",
        ))
        .unwrap();
        assert_eq!(
            g.lands[0].shape,
            LandShape::Chamfered {
                size: 0.2,
                corners: vec![PadCorner::TopLeft]
            }
        );
        // A clockwise polygon is turned counter-clockwise; its size is
        // its reach from the pad centre.
        let g = generate(&with(
            "{number: '1', x: 1, y: 1, shape: polygon, points: [[-0.5, 0.5], [0.5, 0.5], [0.5, -0.3], [-0.5, -0.3]]}",
        ))
        .unwrap();
        let LandShape::Polygon(pts) = &g.lands[0].shape else {
            panic!()
        };
        assert_eq!(pts[0], [-0.5, -0.3]);
        assert!((g.lands[0].size.x - 1.0).abs() < 1e-9 && (g.lands[0].size.y - 1.0).abs() < 1e-9);
        for bad in [
            "{number: '1', x: 0, y: 0, width: 1.0, length: 0.8, chamfer: 0.2}",
            "{number: '1', x: 0, y: 0, width: 1.0, length: 0.8, shape: chamfered, chamfer: 0.5}",
            "{number: '1', x: 0, y: 0, width: 1.0, length: 0.8, shape: circle}",
            "{number: '1', x: 0, y: 0, shape: polygon, points: [[0.1, 0.1], [0.5, 0.1], [0.5, 0.5]]}",
            "{number: '1', x: 0, y: 0, width: 1, length: 1, cutout: {x: 0, y: 0, diameter: 0.5}}",
            "{number: '1', x: 0, y: 0, width: 1, length: 1, cutout: {x: 5, y: 0, diameter: 0.5}}",
            "{number: '1', x: 0, y: 0, width: 2, length: 0.4, cutout: {x: 0.6, y: 0, diameter: 0.6}}",
        ] {
            assert!(generate(&with(bad)).is_err(), "{bad}");
        }
    }
}
