//! A footprint's drawn layers: courtyard (IPC-7351B excess around the
//! lands and body), fabrication outline (the body, pin 1 corner
//! chamfered) and silkscreen (the body outline kept clear of the pads,
//! plus a pin-1 dot).

use crate::{Courtyard, FpGraphic, FpLayer, FpShape, Pad, Point2};
use openparts_mcad::{BodyShape, MechanicalGeometry, Mounting};

/// Courtyard excess around the lands and body: IPC-7351B density level
/// N (0.25 mm SMD), through-hole parts 0.5 mm.
const COURTYARD_SMD: f64 = 0.25;
const COURTYARD_THT: f64 = 0.5;
/// Courtyard corners snap outward to this grid (KiCad's libraries' rule).
const COURTYARD_GRID: f64 = 0.05;
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
    fn union(self, o: Rect) -> Rect {
        Rect {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
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

fn body_rect(g: &MechanicalGeometry) -> Rect {
    // A lying cylinder's footprint is its length by its diameter --
    // `size` already is that; a standing one's bounding square too.
    let b = &g.body;
    let (w, l) = (b.size.x, b.size.y);
    Rect {
        x0: b.position.x - w / 2.0,
        y0: b.position.y - l / 2.0,
        x1: b.position.x + w / 2.0,
        y1: b.position.y + l / 2.0,
    }
}

fn pt(x: f64, y: f64) -> Point2 {
    Point2 { x, y }
}

/// The courtyard: lands, body and silkscreen, grown by the IPC excess,
/// snapped outward to the courtyard grid.
pub(crate) fn courtyard(g: &MechanicalGeometry, pads: &[Pad], drawn: &[FpGraphic]) -> Courtyard {
    let tht = pads.iter().any(|p| p.mounting != Mounting::Smd);
    let silk = drawn
        .iter()
        .filter(|d| d.layer == FpLayer::Silk)
        .flat_map(|d| match &d.shape {
            FpShape::Line(a, b) => vec![*a, *b],
            FpShape::Polygon(p) => p.clone(),
            FpShape::Circle { center, radius } => vec![
                pt(center.x - radius, center.y - radius),
                pt(center.x + radius, center.y + radius),
            ],
        })
        .map(|p| Rect {
            x0: p.x,
            y0: p.y,
            x1: p.x,
            y1: p.y,
        });
    let r = pads
        .iter()
        .map(pad_rect)
        .chain(silk)
        .fold(body_rect(g), Rect::union)
        .grow(if tht { COURTYARD_THT } else { COURTYARD_SMD });
    let snap_down = |v: f64| (v / COURTYARD_GRID).floor() * COURTYARD_GRID;
    let snap_up = |v: f64| (v / COURTYARD_GRID).ceil() * COURTYARD_GRID;
    let (x0, y0, x1, y1) = (
        snap_down(r.x0),
        snap_down(r.y0),
        snap_up(r.x1),
        snap_up(r.y1),
    );
    Courtyard {
        outline: vec![pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1)],
    }
}

/// Fabrication and silkscreen drawings.
pub(crate) fn drawings(g: &MechanicalGeometry, pads: &[Pad]) -> Vec<FpGraphic> {
    let mut out = Vec::new();
    let b = body_rect(g);
    let pin1 = pads
        .iter()
        .find(|p| p.number == "1" && p.mounting != Mounting::NonPlated);

    // Fabrication: the body, its pin-1 corner chamfered.
    if g.body.shape == BodyShape::Cylinder {
        out.push(FpGraphic {
            layer: FpLayer::Fab,
            width: FAB_WIDTH,
            shape: FpShape::Circle {
                center: pt(g.body.position.x, g.body.position.y),
                radius: g.body.size.x / 2.0,
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
    if g.body.shape != BodyShape::Cylinder {
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
        let r = g.body.size.x / 2.0 + SILK_WIDTH / 2.0 + 0.05;
        let center = pt(g.body.position.x, g.body.position.y);
        let clear = keep_out.iter().all(|k| {
            // The circle stays clear when every pad lies wholly inside it
            // (radial can footprints) or outside its ring.
            let far = [
                pt(k.x0, k.y0),
                pt(k.x1, k.y0),
                pt(k.x0, k.y1),
                pt(k.x1, k.y1),
            ]
            .iter()
            .map(|c| (c.x - center.x).hypot(c.y - center.y))
            .fold(0.0, f64::max);
            far < r
        });
        if clear {
            out.push(FpGraphic {
                layer: FpLayer::Silk,
                width: SILK_WIDTH,
                shape: FpShape::Circle { center, radius: r },
            });
        }
    }

    // A pin-1 dot just outside pin 1's land, away from the body.
    if let Some(p) = pin1.filter(|_| pads.len() > 2) {
        let (dx, dy) = (
            p.position.x - g.body.position.x,
            p.position.y - g.body.position.y,
        );
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
