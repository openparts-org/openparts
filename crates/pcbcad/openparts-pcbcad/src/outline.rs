//! A footprint's drawn layers: fabrication outline (the body, pin 1
//! corner chamfered) and silkscreen (the body outline kept clear of the
//! pads, plus a pin-1 dot). The courtyard is the Placement Model's.

use crate::{FpGraphic, FpLayer, FpShape, Pad, Point2};
use openparts_mcad::Mounting;
use openparts_placement::{OutlineShape, PlacementModel, PolarityMark};

/// Stroke width adapters draw the courtyard with.
pub const COURTYARD_WIDTH: f64 = 0.05;
pub(crate) const FAB_WIDTH: f64 = 0.1;
pub(crate) const SILK_WIDTH: f64 = 0.12;
/// Silkscreen kept this far from copper (KiCad's library convention).
const SILK_PAD_CLEARANCE: f64 = 0.2;

#[derive(Clone, Copy)]
struct Rect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Rect {
    fn grow(self, d: f64) -> Rect {
        Rect {
            x0: self.x0 - d,
            y0: self.y0 - d,
            x1: self.x1 + d,
            y1: self.y1 + d,
        }
    }
}

fn pad_rect(p: &Pad) -> Rect {
    Rect {
        x0: p.position.x - p.size.0 / 2.0,
        y0: p.position.y - p.size.1 / 2.0,
        x1: p.position.x + p.size.0 / 2.0,
        y1: p.position.y + p.size.1 / 2.0,
    }
}

fn pt(x: f64, y: f64) -> Point2 {
    Point2 { x, y }
}

/// The main body region: (centre, size, round or stadium).
fn body(m: &PlacementModel) -> (Point2, (f64, f64), Option<OutlineShape>) {
    match m.body.regions.first() {
        Some(r) => (
            pt(r.center.x, r.center.y),
            (r.size.x, r.size.y),
            (r.shape != OutlineShape::Rect).then_some(r.shape),
        ),
        None => (pt(0.0, 0.0), (0.0, 0.0), None),
    }
}

/// Fabrication and silkscreen drawings.
pub(crate) fn drawings(m: &PlacementModel, pads: &[Pad]) -> Vec<FpGraphic> {
    let mut out = Vec::new();
    let (center, (sx, sy), round) = body(m);
    let b = Rect {
        x0: center.x - sx / 2.0,
        y0: center.y - sy / 2.0,
        x1: center.x + sx / 2.0,
        y1: center.y + sy / 2.0,
    };
    let pin1_number = m.pin1.as_ref().map(|p| p.pad.as_str());
    let pin1 = pads
        .iter()
        .find(|p| Some(p.number.as_str()) == pin1_number && p.mounting != Mounting::NonPlated);

    // Fabrication: the body, its pin-1 corner chamfered.
    if round.is_some() {
        out.push(FpGraphic {
            layer: FpLayer::Fab,
            width: FAB_WIDTH,
            shape: if is_round(sx, sy) {
                FpShape::Circle {
                    center,
                    radius: sx / 2.0,
                }
            } else {
                FpShape::Polygon(stadium(center, sx, sy))
            },
        });
    } else {
        let c = (b.x1 - b.x0).min(b.y1 - b.y0) * 0.25;
        let c = c.min(1.0);
        let corners = [
            pt(b.x0, b.y1),
            pt(b.x1, b.y1),
            pt(b.x1, b.y0),
            pt(b.x0, b.y0),
        ];
        // The corner nearest pin 1 (when there are more than two pads).
        let chamfer = pin1.filter(|_| pads.len() > 2).map(|p| {
            (0..4)
                .min_by(|&i, &j| {
                    let d =
                        |k: usize| (corners[k].x - p.position.x).hypot(corners[k].y - p.position.y);
                    d(i).total_cmp(&d(j))
                })
                .unwrap()
        });
        let mut poly = Vec::new();
        for (i, k) in corners.iter().enumerate() {
            if Some(i) == chamfer {
                let prev = corners[(i + 3) % 4];
                let next = corners[(i + 1) % 4];
                let toward = |a: Point2, d: f64| {
                    let len = (a.x - k.x).hypot(a.y - k.y);
                    pt(k.x + (a.x - k.x) / len * d, k.y + (a.y - k.y) / len * d)
                };
                poly.push(toward(prev, c));
                poly.push(toward(next, c));
            } else {
                poly.push(*k);
            }
        }
        out.push(FpGraphic {
            layer: FpLayer::Fab,
            width: FAB_WIDTH,
            shape: FpShape::Polygon(poly),
        });
    }

    // Silkscreen: the body outline just outside the body, broken where
    // it would come near copper.
    let keep_out: Vec<Rect> = pads
        .iter()
        .map(|p| pad_rect(p).grow(SILK_PAD_CLEARANCE + SILK_WIDTH / 2.0))
        .collect();
    let polar = polarity_marks(m, pads, b, round.is_some(), &keep_out);
    let outline = !polar.replaces_outline;
    out.extend(polar.graphics);
    if outline && round.is_none() {
        let s = b.grow(SILK_WIDTH / 2.0 + 0.05);
        let edges = [
            (pt(s.x0, s.y1), pt(s.x1, s.y1)),
            (pt(s.x1, s.y1), pt(s.x1, s.y0)),
            (pt(s.x1, s.y0), pt(s.x0, s.y0)),
            (pt(s.x0, s.y0), pt(s.x0, s.y1)),
        ];
        for (a, z) in edges {
            for (p, q) in clip(a, z, &keep_out) {
                if (q.x - p.x).hypot(q.y - p.y) > 0.2 {
                    out.push(FpGraphic {
                        layer: FpLayer::Silk,
                        width: SILK_WIDTH,
                        shape: FpShape::Line(p, q),
                    });
                }
            }
        }
    } else if outline {
        let grow = SILK_WIDTH / 2.0 + 0.05;
        let (sx, sy) = (sx + 2.0 * grow, sy + 2.0 * grow);
        // The outline stays clear when every pad lies wholly inside it
        // (radial can footprints).
        let clear = keep_out.iter().all(|k| {
            [
                pt(k.x0, k.y0),
                pt(k.x1, k.y0),
                pt(k.x0, k.y1),
                pt(k.x1, k.y1),
            ]
            .iter()
            .all(|c| inside_stadium(center, sx, sy, *c))
        });
        if clear && is_round(sx, sy) {
            out.push(FpGraphic {
                layer: FpLayer::Silk,
                width: SILK_WIDTH,
                shape: FpShape::Circle {
                    center,
                    radius: sx / 2.0,
                },
            });
        } else if clear {
            let pts = stadium(center, sx, sy);
            for (i, a) in pts.iter().enumerate() {
                out.push(FpGraphic {
                    layer: FpLayer::Silk,
                    width: SILK_WIDTH,
                    shape: FpShape::Line(*a, pts[(i + 1) % pts.len()]),
                });
            }
        }
    }

    // A pin-1 dot just outside pin 1's land, away from the body.
    if let Some(p) = pin1.filter(|_| pads.len() > 2) {
        let (dx, dy) = (p.position.x - center.x, p.position.y - center.y);
        let r = pad_rect(p);
        let gap = SILK_PAD_CLEARANCE + 0.3;
        let at = if dx.abs() / (b.x1 - b.x0).max(1e-9) >= dy.abs() / (b.y1 - b.y0).max(1e-9) {
            pt(if dx < 0.0 { r.x0 - gap } else { r.x1 + gap }, p.position.y)
        } else {
            pt(p.position.x, if dy < 0.0 { r.y0 - gap } else { r.y1 + gap })
        };
        out.push(FpGraphic {
            layer: FpLayer::Silk,
            width: SILK_WIDTH,
            shape: FpShape::Circle {
                center: at,
                radius: 0.1,
            },
        });
    }
    out
}

/// A polarised part's marks (Canonical Data Specification 20.10).
struct PolarityMarks {
    graphics: Vec<FpGraphic>,
    /// The silkscreen bracket stands in for the body outline.
    replaces_outline: bool,
}

/// The frame of a two-terminal part's axis: `u` along it, towards the
/// marked terminal; `v` across it.
#[derive(Clone, Copy)]
struct Axis {
    along_x: bool,
    sign: f64,
}

impl Axis {
    fn point(self, u: f64, v: f64) -> Point2 {
        if self.along_x {
            pt(self.sign * u, v)
        } else {
            pt(v, self.sign * u)
        }
    }
    /// A rectangle as (u0, u1, v0, v1).
    fn rect(self, r: Rect) -> (f64, f64, f64, f64) {
        let (a0, a1, c0, c1) = if self.along_x {
            (r.x0, r.x1, r.y0, r.y1)
        } else {
            (r.y0, r.y1, r.x0, r.x1)
        };
        let (u0, u1) = if self.sign > 0.0 {
            (a0, a1)
        } else {
            (-a1, -a0)
        };
        (u0, u1, c0, c1)
    }
}

/// Arm of the silkscreen `+` sign (KiCad's CP_Elec: about 0.8 across).
const PLUS_ARM: f64 = 0.4;
/// Arm of the fabrication `+` sign.
const FAB_PLUS_ARM: f64 = 0.3;

fn line(layer: FpLayer, width: f64, a: Point2, z: Point2) -> FpGraphic {
    FpGraphic {
        layer,
        width,
        shape: FpShape::Line(a, z),
    }
}

/// The marks of the Placement Model's `polarity` (KiCad library
/// conventions, IPC-7351B): a band end (a diode's or LED's cathode, a
/// tantalum's `+`) is bracketed on the silkscreen -- the outline's two
/// long sides carried past the marked pad and closed by a bar beyond it,
/// all clear of the copper -- and crossed by a bar on the fabrication
/// layer; an electrolytic can gets a `+` beside its `+` terminal, outside
/// the body, and one inside the body on the fabrication layer. A round
/// body keeps its outline and gets a bar outside it at the band end.
fn polarity_marks(
    m: &PlacementModel,
    pads: &[Pad],
    body: Rect,
    round: bool,
    keep_out: &[Rect],
) -> PolarityMarks {
    let none = PolarityMarks {
        graphics: vec![],
        replaces_outline: false,
    };
    let Some(pol) = &m.polarity else { return none };
    let plated = |p: &&Pad| p.mounting != Mounting::NonPlated;
    let Some(marked) = pads.iter().filter(plated).find(|p| p.number == pol.pad) else {
        return none;
    };
    let Some(other) = pads.iter().filter(plated).find(|p| p.number != pol.pad) else {
        return none;
    };
    let (dx, dy) = (
        marked.position.x - other.position.x,
        marked.position.y - other.position.y,
    );
    let along_x = dx.abs() >= dy.abs();
    let axis = Axis {
        along_x,
        sign: if (if along_x { dx } else { dy }) < 0.0 {
            -1.0
        } else {
            1.0
        },
    };
    let grow = SILK_WIDTH / 2.0 + 0.05;
    let (bu0, bu1, bv0, bv1) = axis.rect(body);
    let mk = axis.rect(pad_rect(marked).grow(SILK_PAD_CLEARANCE + SILK_WIDTH / 2.0));
    let clear = |a: Point2, z: Point2, out: &mut Vec<FpGraphic>| {
        for (p, q) in clip(a, z, keep_out) {
            if (q.x - p.x).hypot(q.y - p.y) > 0.2 {
                out.push(line(FpLayer::Silk, SILK_WIDTH, p, q));
            }
        }
    };
    let plus = |c: Point2, arm: f64, layer: FpLayer, width: f64| {
        [
            line(layer, width, pt(c.x - arm, c.y), pt(c.x + arm, c.y)),
            line(layer, width, pt(c.x, c.y - arm), pt(c.x, c.y + arm)),
        ]
    };
    let mut out = Vec::new();
    let mut replaces_outline = false;
    match (pol.mark, round) {
        (PolarityMark::Band, false) => {
            // The band on the fabrication layer, where the 3D model puts
            // it (a fifth of the body in from the marked end).
            let band_u = bu1 - (bu1 - bu0) * 0.2;
            out.push(line(
                FpLayer::Fab,
                FAB_WIDTH,
                axis.point(band_u, bv0),
                axis.point(band_u, bv1),
            ));
            if marked.mounting == Mounting::ThroughHole || mk.0 >= bu1 + grow + SILK_WIDTH {
                // Leads clear of the body (an axial part): the outline
                // stays, the band is printed across the body.
                for k in [0.0, 1.0] {
                    let u = band_u - k * SILK_WIDTH;
                    clear(
                        axis.point(u, bv0 - grow),
                        axis.point(u, bv1 + grow),
                        &mut out,
                    );
                }
            } else {
                let (mut v0, mut v1) = (bv0 - grow, bv1 + grow);
                for k in keep_out {
                    let (_, _, k0, k1) = axis.rect(*k);
                    v0 = v0.min(k0);
                    v1 = v1.max(k1);
                }
                let bar = (bu1 + grow).max(mk.1);
                for (a, z) in [
                    (axis.point(bu0, v1), axis.point(bar, v1)),
                    (axis.point(bar, v1), axis.point(bar, v0)),
                    (axis.point(bar, v0), axis.point(bu0, v0)),
                ] {
                    clear(a, z, &mut out);
                }
                replaces_outline = true;
            }
        }
        (PolarityMark::Band, true) => {
            let bar = (bu1 + grow + 0.15).max(mk.1);
            let half = (bv1 - bv0) * 0.3;
            let mid = (bv0 + bv1) / 2.0;
            clear(
                axis.point(bar, mid - half),
                axis.point(bar, mid + half),
                &mut out,
            );
        }
        (PolarityMark::Plus, _) => {
            let (cu, cv) = if round {
                // Inside the can above the + pad (KiCad's CP_Radial), else
                // outside it, towards the marked terminal at 45°.
                let (mu, mv) = ((bu0 + bu1) / 2.0, (bv0 + bv1) / 2.0);
                let radius = (bu1 - bu0) / 2.0;
                let inside = ((mk.0 + mk.1) / 2.0, mk.3 + 0.1 + PLUS_ARM);
                let fits = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
                    .iter()
                    .all(|(du, dv)| {
                        (inside.0 + du * PLUS_ARM - mu).hypot(inside.1 + dv * PLUS_ARM - mv)
                            < radius - SILK_WIDTH
                    });
                if fits {
                    inside
                } else {
                    let d = (radius + grow + 0.15 + PLUS_ARM * std::f64::consts::SQRT_2)
                        / std::f64::consts::SQRT_2;
                    (mu + d, mv + d)
                }
            } else {
                // Beyond the body's marked end, above the marked pad.
                (bu1 + grow + 0.15 + PLUS_ARM, mk.3 + 0.1 + PLUS_ARM)
            };
            out.extend(plus(
                axis.point(cu, cv),
                PLUS_ARM,
                FpLayer::Silk,
                SILK_WIDTH,
            ));
            let (fu, fv) = if round {
                let r = (bu1 - bu0) / 2.0;
                // Below the + pad, clear of the silkscreen sign above it.
                ((bu0 + bu1) / 2.0 + r * 0.45, (bv0 + bv1) / 2.0 - r * 0.45)
            } else {
                (
                    bu1 - (bu1 - bu0) * 0.25,
                    (bv0 + bv1) / 2.0 + (bv1 - bv0) * 0.25,
                )
            };
            out.extend(plus(
                axis.point(fu, fv),
                FAB_PLUS_ARM.min((bu1 - bu0) * 0.1),
                FpLayer::Fab,
                FAB_WIDTH,
            ));
        }
    }
    PolarityMarks {
        graphics: out,
        replaces_outline,
    }
}

/// A standing cylinder's outline is a circle when its two sizes agree.
fn is_round(sx: f64, sy: f64) -> bool {
    (sx - sy).abs() < 1e-9
}

/// Segments per half circle of a [`stadium`] outline.
const STADIUM_ARC_SEGMENTS: usize = 16;

/// The closed outline of a stadium (a slot: two half circles joined by
/// straight sides) `sx` by `sy` around `center`, its long axis along the
/// larger size.
fn stadium(center: Point2, sx: f64, sy: f64) -> Vec<Point2> {
    let along_x = sx >= sy;
    let r = sx.min(sy) / 2.0;
    let half = (sx.max(sy) / 2.0 - r).max(0.0);
    let mut out = Vec::new();
    for (end, start_angle) in [(1.0, -0.5), (-1.0, 0.5)] {
        for i in 0..=STADIUM_ARC_SEGMENTS {
            let t = (start_angle + i as f64 / STADIUM_ARC_SEGMENTS as f64) * std::f64::consts::PI;
            let (u, v) = (end * half + r * t.cos(), r * t.sin());
            out.push(if along_x {
                pt(center.x + u, center.y + v)
            } else {
                pt(center.x + v, center.y + u)
            });
        }
    }
    out
}

fn inside_stadium(center: Point2, sx: f64, sy: f64, p: Point2) -> bool {
    let (u, v) = if sx >= sy {
        (p.x - center.x, p.y - center.y)
    } else {
        (p.y - center.y, p.x - center.x)
    };
    let r = sx.min(sy) / 2.0;
    let half = (sx.max(sy) / 2.0 - r).max(0.0);
    (u.abs() - half).max(0.0).hypot(v) < r
}

/// The parts of the axis-aligned segment a..z outside every rectangle.
fn clip(a: Point2, z: Point2, keep_out: &[Rect]) -> Vec<(Point2, Point2)> {
    let horizontal = (a.y - z.y).abs() < 1e-12;
    let (lo, hi) = if horizontal {
        (a.x.min(z.x), a.x.max(z.x))
    } else {
        (a.y.min(z.y), a.y.max(z.y))
    };
    let fixed = if horizontal { a.y } else { a.x };
    let mut cuts: Vec<(f64, f64)> = keep_out
        .iter()
        .filter_map(|r| {
            let (f0, f1, s0, s1) = if horizontal {
                (r.y0, r.y1, r.x0, r.x1)
            } else {
                (r.x0, r.x1, r.y0, r.y1)
            };
            (fixed > f0 && fixed < f1).then_some((s0.max(lo), s1.min(hi)))
        })
        .filter(|(s, e)| s < e)
        .collect();
    cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out = Vec::new();
    let mut at = lo;
    for (s, e) in cuts {
        if s > at {
            out.push((at, s));
        }
        at = at.max(e);
    }
    if at < hi {
        out.push((at, hi));
    }
    out.into_iter()
        .map(|(s, e)| {
            if horizontal {
                (pt(s, fixed), pt(e, fixed))
            } else {
                (pt(fixed, s), pt(fixed, e))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_placement::Density;

    fn device(category: &str, first: &str, second: &str) -> openparts_core::Device {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: device\nid: t/T\nmanufacturer: t\ncategory: {category}\n\
             pins:\n  \"1\": {{ name: \"{first}\", type: passive }}\n  \"2\": {{ name: \"{second}\", type: passive }}\n"
        ))
        .unwrap()
    }

    fn footprint(package: &str, device: &openparts_core::Device) -> crate::PcbFootprint {
        let p: openparts_core::Package = serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\n{package}"
        ))
        .unwrap();
        let g = openparts_mcad::generate(&p).unwrap();
        let m = openparts_placement::build(&p, Some(device), &g, Density::N);
        crate::footprint_from_placement("T", &m)
    }

    const CHIP: &str = "family: chip\nlead_count: 2\npitch: {unit: mm}\ndimensions:\n\
        \x20 body_width: {nominal: 1.6, unit: mm}\n  body_length: {nominal: 0.8, unit: mm}\n\
        \x20 body_height: {nominal: 0.45, unit: mm}\n  lead_length: {nominal: 0.3, unit: mm}\n";

    fn silk_lines(fp: &crate::PcbFootprint) -> Vec<(Point2, Point2)> {
        fp.graphics
            .iter()
            .filter(|g| g.layer == FpLayer::Silk)
            .filter_map(|g| match g.shape {
                FpShape::Line(a, b) => Some((a, b)),
                _ => None,
            })
            .collect()
    }

    /// Every silkscreen line keeps the clearance to every pad.
    fn assert_clear(fp: &crate::PcbFootprint) {
        for (a, b) in silk_lines(fp) {
            for p in &fp.pads {
                let r = pad_rect(p).grow(SILK_PAD_CLEARANCE + SILK_WIDTH / 2.0 - 1e-9);
                let (lo, hi) = ((a.x.min(b.x), a.y.min(b.y)), (a.x.max(b.x), a.y.max(b.y)));
                let overlaps = lo.0 < r.x1 && hi.0 > r.x0 && lo.1 < r.y1 && hi.1 > r.y0;
                assert!(!overlaps, "silk {a:?}-{b:?} too near pad {}", p.number);
            }
        }
    }

    #[test]
    fn a_cathode_is_bracketed_on_its_side() {
        // Everlight numbers the anode 1: the bracket closes beyond pad 2.
        let fp = footprint(CHIP, &device("led", "Anode", "Cathode"));
        let pad2 = fp.pads.iter().find(|p| p.number == "2").unwrap();
        assert!(pad2.position.x > 0.0);
        let lines = silk_lines(&fp);
        let bar = lines
            .iter()
            .find(|(a, b)| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() > 0.5)
            .expect("a bar across the axis");
        assert!(bar.0.x >= pad2.position.x + pad2.size.0 / 2.0 + SILK_PAD_CLEARANCE);
        assert_eq!(lines.len(), 3, "the bracket replaces the outline");
        assert_clear(&fp);
        // The fabrication band sits at the cathode end.
        assert!(fp.graphics.iter().any(|g| g.layer == FpLayer::Fab
            && matches!(g.shape, FpShape::Line(a, b) if a.x == b.x && a.x > 0.0)));
    }

    #[test]
    fn an_unpolarised_part_keeps_its_plain_outline() {
        let fp = footprint(CHIP, &device("capacitor", "1", "2"));
        assert!(silk_lines(&fp)
            .iter()
            .all(|(a, b)| (a.y - b.y).abs() < 1e-9));
        assert!(!fp
            .graphics
            .iter()
            .any(|g| g.layer == FpLayer::Fab && matches!(g.shape, FpShape::Line(..))));
    }

    #[test]
    fn an_electrolytic_gets_a_plus_beside_its_plus_terminal() {
        let fp = footprint(
            "family: smd_electrolytic\nlead_count: 2\npitch: {unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 6.6, unit: mm}\n  body_length: {nominal: 6.6, unit: mm}\n\
             \x20 can_diameter: {nominal: 6.3, unit: mm}\n  overall_height: {nominal: 5.8, unit: mm}\n\
             pads:\n\
             \x20 - { number: \"1\", x: -2.7, y: 0, width: 3.5, length: 1.6 }\n\
             \x20 - { number: \"2\", x: 2.7, y: 0, width: 3.5, length: 1.6 }\n",
            &device("capacitor", "+", "-"),
        );
        assert_clear(&fp);
        let lines = silk_lines(&fp);
        // The + sign: two short crossing lines left of the plate.
        let short: Vec<_> = lines
            .iter()
            .filter(|(a, b)| ((a.x - b.x).hypot(a.y - b.y) - 2.0 * PLUS_ARM).abs() < 1e-9)
            .collect();
        assert_eq!(short.len(), 2);
        assert!(short.iter().all(|(a, b)| a.x.max(b.x) < -3.3));
    }
}
