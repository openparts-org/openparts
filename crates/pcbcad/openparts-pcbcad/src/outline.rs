//! A footprint's drawn layers: fabrication outline (the body, pin 1
//! corner chamfered) and silkscreen (the body outline kept clear of the
//! pads, plus a pin-1 dot). The courtyard is the Placement Model's.

use crate::{FpGraphic, FpLayer, FpShape, Pad, Point2};
use openparts_mcad::Mounting;
use openparts_placement::{OutlineShape, PlacementModel};

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
    if round.is_none() {
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
    } else {
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
