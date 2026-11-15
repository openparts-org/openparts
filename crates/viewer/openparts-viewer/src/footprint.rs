//! The footprint view: openparts-pcbcad's own SVG (the same drawing the
//! `footprint-svg` artifact is) inside a larger canvas, with pad size,
//! pitch, pad span and courtyard dimensions drawn around it, and the
//! points the measure tool snaps to.
//!
//! Coordinates here are SVG user units: millimetres, Y down (the
//! footprint's Y negated, as `render_svg` draws it).

use crate::format::number;
use crate::labels::Labels;
use crate::snap::SnapKind;
use openparts_pcbcad::{FpShape, PcbFootprint};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct P {
    pub x: f64,
    pub y: f64,
}

impl P {
    fn new(x: f64, y: f64) -> P {
        P { x, y }
    }
    fn add(self, o: P, k: f64) -> P {
        P::new(self.x + o.x * k, self.y + o.y * k)
    }
    pub fn minus(self, o: P) -> P {
        P::new(self.x - o.x, self.y - o.y)
    }
    fn dot(self, o: P) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FpSnap {
    pub at: P,
    pub kind: SnapKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Drawing {
    /// The whole `<svg>` element.
    pub svg: String,
    /// The viewBox: x, y, width, height.
    pub view: [f64; 4],
    /// The label font size (mm): the measure overlay draws at it too.
    pub font: f64,
    pub snaps: Vec<FpSnap>,
}

struct Bounds {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Bounds {
    fn empty() -> Bounds {
        Bounds {
            x0: f64::MAX,
            y0: f64::MAX,
            x1: f64::MIN,
            y1: f64::MIN,
        }
    }
    fn add(&mut self, p: P) {
        self.x0 = self.x0.min(p.x);
        self.y0 = self.y0.min(p.y);
        self.x1 = self.x1.max(p.x);
        self.y1 = self.y1.max(p.y);
    }
    fn is_empty(&self) -> bool {
        self.x0 > self.x1
    }
}

/// A pad in SVG coordinates: centre and full size.
#[derive(Debug, Clone)]
struct Pad<'a> {
    number: &'a str,
    c: P,
    w: f64,
    h: f64,
}

impl Pad<'_> {
    /// Half its extent along the axis-aligned unit `u`.
    fn half(&self, u: P) -> f64 {
        (u.x.abs() * self.w + u.y.abs() * self.h) * 0.5
    }
}

const DIM: &str = "#7fd1ff";
const MARGIN: f64 = 1.5;

/// Escapes text for HTML/SVG.
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// SVG for the drawn dimensions, tracking what they cover.
struct Ink {
    s: String,
    bounds: Bounds,
    font: f64,
}

impl Ink {
    fn line(&mut self, a: P, b: P, width: f64) {
        self.bounds.add(a);
        self.bounds.add(b);
        self.s.push_str(&format!(
            "<line x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\" stroke-width=\"{width:.3}\"/>\n",
            a.x, a.y, b.x, b.y
        ));
    }

    /// Text centred on `at`, along `dir` (horizontal or vertical).
    fn text(&mut self, at: P, vertical: bool, text: &str, title: &str) {
        let w = self.font * 0.58 * text.chars().count() as f64;
        let (hw, hh) = if vertical {
            (self.font * 0.6, w * 0.5)
        } else {
            (w * 0.5, self.font * 0.6)
        };
        self.bounds.add(P::new(at.x - hw, at.y - hh));
        self.bounds.add(P::new(at.x + hw, at.y + hh));
        let rotate = if vertical {
            format!(" transform=\"rotate(-90 {:.3} {:.3})\"", at.x, at.y)
        } else {
            String::new()
        };
        self.s.push_str(&format!(
            "<text x=\"{:.3}\" y=\"{:.3}\" font-size=\"{:.3}\" text-anchor=\"middle\" \
             dominant-baseline=\"central\" stroke=\"#0b1a12\" stroke-width=\"{:.3}\"{rotate}>\
             <title>{}</title>{}</text>\n",
            at.x,
            at.y,
            self.font,
            self.font * 0.22,
            esc(title),
            esc(text)
        ));
    }

    /// A dimension between features `f0`/`f1`, its line through `l0`/`l1`,
    /// labelled on the side `out` (unit, away from the features).
    fn dimension(&mut self, f: [P; 2], l: [P; 2], out: P, text: &str, title: &str) {
        let sw = self.font * 0.08;
        let arrow = self.font * 0.7;
        for (f, p) in [(f[0], l[0]), (f[1], l[1])] {
            let e = p.minus(f);
            let len = e.len();
            if len > 1e-6 {
                let u = P::new(e.x / len, e.y / len);
                self.line(
                    f.add(u, (arrow * 0.3).min(len * 0.3)),
                    p.add(u, arrow * 0.4),
                    sw,
                );
            }
        }
        let d = l[1].minus(l[0]);
        let len = d.len();
        if len < 1e-9 {
            return;
        }
        let d = P::new(d.x / len, d.y / len);
        let w = P::new(-d.y, d.x);
        let inside = len > arrow * 2.4;
        let (a, b) = if inside {
            (l[0], l[1])
        } else {
            (l[0].add(d, -arrow * 2.0), l[1].add(d, arrow * 2.0))
        };
        self.line(a, b, sw);
        let back = if inside { 1.0 } else { -1.0 };
        for (tip, s) in [(l[0], back), (l[1], -back)] {
            let base = tip.add(d, arrow * s);
            self.s.push_str(&format!(
                "<polygon points=\"{:.3},{:.3} {:.3},{:.3} {:.3},{:.3}\" stroke=\"none\"/>\n",
                tip.x,
                tip.y,
                base.add(w, arrow * 0.3).x,
                base.add(w, arrow * 0.3).y,
                base.add(w, -arrow * 0.3).x,
                base.add(w, -arrow * 0.3).y
            ));
        }
        let mid = P::new((l[0].x + l[1].x) * 0.5, (l[0].y + l[1].y) * 0.5);
        let vertical = d.y.abs() > d.x.abs();
        let along = self.font * 0.58 * text.chars().count() as f64 * 0.5;
        let reach = if vertical == (out.y.abs() > out.x.abs()) {
            along
        } else {
            self.font * 0.6
        };
        self.text(mid.add(out, reach + self.font * 0.3), vertical, text, title);
    }
}

/// The footprint, dimensioned.
pub fn drawing(fp: &PcbFootprint, labels: &Labels) -> Drawing {
    let pads: Vec<Pad> = fp
        .pads
        .iter()
        .map(|p| Pad {
            number: &p.number,
            c: P::new(p.position.x, -p.position.y),
            w: p.size.0,
            h: p.size.1,
        })
        .collect();
    // The same extents render_svg uses.
    let mut base = Bounds::empty();
    for p in &pads {
        base.add(P::new(p.c.x - p.w / 2.0, p.c.y - p.h / 2.0));
        base.add(P::new(p.c.x + p.w / 2.0, p.c.y + p.h / 2.0));
    }
    let court: Vec<P> = fp
        .courtyard
        .as_ref()
        .map(|c| c.outline.iter().map(|p| P::new(p.x, -p.y)).collect())
        .unwrap_or_default();
    court.iter().for_each(|p| base.add(*p));
    for g in &fp.graphics {
        match &g.shape {
            FpShape::Line(a, b) => {
                base.add(P::new(a.x, -a.y));
                base.add(P::new(b.x, -b.y));
            }
            FpShape::Polygon(ps) => ps.iter().for_each(|p| base.add(P::new(p.x, -p.y))),
            FpShape::Circle { center, radius } => {
                base.add(P::new(center.x - radius, -center.y - radius));
                base.add(P::new(center.x + radius, -center.y + radius));
            }
        }
    }
    if base.is_empty() {
        base = Bounds {
            x0: -1.0,
            y0: -1.0,
            x1: 1.0,
            y1: 1.0,
        };
    }
    let size = (base.x1 - base.x0).max(base.y1 - base.y0);
    let font = (size * 0.032).clamp(0.2, 1.2);
    let gap = font * 2.6;
    let mut ink = Ink {
        s: String::new(),
        bounds: Bounds::empty(),
        font,
    };
    let computed = &labels.computed;
    let mm = |v: f64| format!("{} mm", number(v));

    if !pads.is_empty() {
        let left = pads
            .iter()
            .min_by(|a, b| (a.c.x - a.w / 2.0).total_cmp(&(b.c.x - b.w / 2.0)))
            .unwrap();
        let right = pads
            .iter()
            .max_by(|a, b| (a.c.x + a.w / 2.0).total_cmp(&(b.c.x + b.w / 2.0)))
            .unwrap();
        let top = pads
            .iter()
            .min_by(|a, b| (a.c.y - a.h / 2.0).total_cmp(&(b.c.y - b.h / 2.0)))
            .unwrap();
        let bottom = pads
            .iter()
            .max_by(|a, b| (a.c.y + a.h / 2.0).total_cmp(&(b.c.y + b.h / 2.0)))
            .unwrap();
        let max_w = pads.iter().map(|p| p.w).fold(0.0, f64::max);
        let max_h = pads.iter().map(|p| p.h).fold(0.0, f64::max);
        // Pad span X above, courtyard X a tier further.
        let span_x = (right.c.x + right.w / 2.0) - (left.c.x - left.w / 2.0);
        let y = base.y0 - gap;
        if span_x > max_w + 1e-6 {
            let f = [
                P::new(left.c.x - left.w / 2.0, left.c.y - left.h / 2.0),
                P::new(right.c.x + right.w / 2.0, right.c.y - right.h / 2.0),
            ];
            let text = format!("{} {}", labels.pad_span_x, mm(span_x));
            ink.dimension(
                f,
                [P::new(f[0].x, y), P::new(f[1].x, y)],
                P::new(0.0, -1.0),
                &text,
                computed,
            );
        }
        let span_y = (bottom.c.y + bottom.h / 2.0) - (top.c.y - top.h / 2.0);
        let x = base.x1 + gap;
        if span_y > max_h + 1e-6 {
            let f = [
                P::new(top.c.x + top.w / 2.0, top.c.y - top.h / 2.0),
                P::new(bottom.c.x + bottom.w / 2.0, bottom.c.y + bottom.h / 2.0),
            ];
            let text = format!("{} {}", labels.pad_span_y, mm(span_y));
            ink.dimension(
                f,
                [P::new(x, f[0].y), P::new(x, f[1].y)],
                P::new(1.0, 0.0),
                &text,
                computed,
            );
        }
        if !court.is_empty() {
            let mut c = Bounds::empty();
            court.iter().for_each(|p| c.add(*p));
            let y = y - gap * 1.3;
            ink.dimension(
                [P::new(c.x0, c.y0), P::new(c.x1, c.y0)],
                [P::new(c.x0, y), P::new(c.x1, y)],
                P::new(0.0, -1.0),
                &format!("{} {}", labels.courtyard_x, mm(c.x1 - c.x0)),
                computed,
            );
            let x = x + gap * 1.3;
            ink.dimension(
                [P::new(c.x1, c.y0), P::new(c.x1, c.y1)],
                [P::new(x, c.y0), P::new(x, c.y1)],
                P::new(1.0, 0.0),
                &format!("{} {}", labels.courtyard_y, mm(c.y1 - c.y0)),
                computed,
            );
        }
        // Pitch: pad 1 and its nearest like-sized neighbour, outside them.
        let p1 = pads.iter().find(|p| p.number == "1").unwrap_or(&pads[0]);
        let same = |a: &Pad, b: &Pad| (a.w - b.w).abs() < 1e-6 && (a.h - b.h).abs() < 1e-6;
        let neighbour = pads
            .iter()
            .filter(|p| same(p, p1) && p.c.minus(p1.c).len() > 1e-6)
            .min_by(|a, b| a.c.minus(p1.c).len().total_cmp(&b.c.minus(p1.c).len()));
        if let Some(p2) = neighbour {
            let v = p2.c.minus(p1.c);
            let pitch = v.len();
            let d = P::new(v.x / pitch, v.y / pitch);
            let mut n = P::new(-d.y, d.x);
            let centre = P::new((base.x0 + base.x1) * 0.5, (base.y0 + base.y1) * 0.5);
            let r = n.dot(p1.c.minus(centre));
            if r < -1e-6 || (r.abs() <= 1e-6 && n.y > 0.0) {
                n = P::new(-n.x, -n.y);
            }
            let t = [p1, p2]
                .iter()
                .map(|p| p.c.dot(n) + p.half(n))
                .fold(f64::MIN, f64::max)
                + gap * 0.5;
            let f = [p1.c.add(n, p1.half(n)), p2.c.add(n, p2.half(n))];
            let l = [f[0].add(n, t - f[0].dot(n)), f[1].add(n, t - f[1].dot(n))];
            ink.dimension(
                f,
                l,
                n,
                &format!("{} {}", labels.pitch, mm(pitch)),
                computed,
            );
        }
        // Pad sizes: one callout per distinct size, below the drawing.
        let mut sizes: Vec<&Pad> = Vec::new();
        for p in &pads {
            if !sizes.iter().any(|q| same(p, q)) {
                sizes.push(p);
            }
        }
        sizes.sort_by(|a, b| {
            (b.number == "1")
                .cmp(&(a.number == "1"))
                .then((a.w * a.h).total_cmp(&(b.w * b.h)))
        });
        let mut y = base.y1 + MARGIN + font * 1.2;
        for p in sizes.iter().take(4) {
            let text = format!(
                "{} {}: {} × {} mm",
                labels.pad_size,
                p.number,
                number(p.w),
                number(p.h)
            );
            let w = font * 0.58 * text.chars().count() as f64;
            let at = P::new(base.x0 + w * 0.5, y);
            ink.line(p.c, P::new(base.x0 - font * 0.3, y), font * 0.05);
            ink.text(at.add(P::new(font * 0.2, 0.0), 1.0), false, &text, computed);
            y += font * 1.6;
        }
    }

    // The canvas: the drawing, its dimensions and a margin.
    let mut all = Bounds::empty();
    all.add(P::new(base.x0 - MARGIN, base.y0 - MARGIN));
    all.add(P::new(base.x1 + MARGIN, base.y1 + MARGIN + 1.0));
    if !ink.bounds.is_empty() {
        all.add(P::new(ink.bounds.x0 - font, ink.bounds.y0 - font));
        all.add(P::new(ink.bounds.x1 + font, ink.bounds.y1 + font));
    }
    let view = [all.x0, all.y0, all.x1 - all.x0, all.y1 - all.y0];
    let inner = openparts_pcbcad::render_svg(fp);
    let inner_body: String = inner
        .lines()
        .filter(|l| !l.starts_with("<svg") && !l.starts_with("</svg"))
        .map(|l| format!("{l}\n"))
        .collect();
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"opv-fp-svg\" viewBox=\"{:.3} {:.3} {:.3} {:.3}\" \
         font-family=\"sans-serif\">\n<rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" fill=\"#0b1a12\"/>\n\
         {inner_body}<g class=\"opv-fp-dims\" stroke=\"{DIM}\" fill=\"{DIM}\" \
         style=\"paint-order: stroke\">\n{}</g>\n<g class=\"opv-fp-measure\"></g>\n</svg>\n",
        view[0], view[1], view[2], view[3], view[0], view[1], view[2], view[3], ink.s
    );

    let mut snaps = Vec::new();
    for p in &pads {
        snaps.push(FpSnap {
            at: p.c,
            kind: SnapKind::PadCenter,
        });
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            snaps.push(FpSnap {
                at: P::new(p.c.x + sx * p.w / 2.0, p.c.y + sy * p.h / 2.0),
                kind: SnapKind::Vertex,
            });
        }
        for (sx, sy) in [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)] {
            snaps.push(FpSnap {
                at: P::new(p.c.x + sx * p.w / 2.0, p.c.y + sy * p.h / 2.0),
                kind: SnapKind::EdgeMidpoint,
            });
        }
    }
    for (i, c) in court.iter().enumerate() {
        snaps.push(FpSnap {
            at: *c,
            kind: SnapKind::Vertex,
        });
        let n = court[(i + 1) % court.len()];
        snaps.push(FpSnap {
            at: P::new((c.x + n.x) * 0.5, (c.y + n.y) * 0.5),
            kind: SnapKind::EdgeMidpoint,
        });
    }
    Drawing {
        svg,
        view,
        font,
        snaps,
    }
}

/// The snap target within `radius` (mm) of `at`, pad centres favoured.
pub fn pick(snaps: &[FpSnap], at: P, radius: f64) -> Option<FpSnap> {
    snaps
        .iter()
        .filter_map(|s| {
            let d = s.at.minus(at).len();
            let bonus = if s.kind == SnapKind::PadCenter {
                radius * 0.3
            } else {
                0.0
            };
            (d <= radius).then_some((d - bonus, *s))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, s)| s)
}

/// What the footprint's measure overlay shows.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasureDraw {
    /// The first point.
    pub a: Option<P>,
    /// The second, held to `axis` when it has one...
    pub end: Option<P>,
    /// ...projected from this (the snapped point).
    pub from: Option<P>,
    /// 0 X, 1 Y.
    pub axis: Option<usize>,
    pub hover: Option<P>,
    pub view: [f64; 4],
    pub font: f64,
    pub text: String,
}

/// The measure overlay: the snap ring, the end points, the distance in
/// the axis colour with a dashed guide along the axis, a dashed tie to
/// the snapped point, and the distance label.
pub fn measure_svg(m: &MeasureDraw) -> String {
    const FREE: &str = "#ff8a3d";
    let font = m.font;
    let color = m.axis.map(|i| crate::measure::AXIS_CSS[i]).unwrap_or(FREE);
    let mut s = String::new();
    if let Some(h) = m.hover {
        s.push_str(&format!(
            "<circle cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\" fill=\"none\" stroke=\"{FREE}\" stroke-width=\"{:.3}\"/>\n",
            h.x,
            h.y,
            font * 0.45,
            font * 0.08
        ));
    }
    let Some(a) = m.a else { return s };
    let [vx, vy, vw, vh] = m.view;
    if let (Some(i), Some(_)) = (m.axis, m.end) {
        let (x1, y1, x2, y2) = if i == 0 {
            (vx, a.y, vx + vw, a.y)
        } else {
            (a.x, vy, a.x, vy + vh)
        };
        s.push_str(&format!(
            "<line x1=\"{x1:.3}\" y1=\"{y1:.3}\" x2=\"{x2:.3}\" y2=\"{y2:.3}\" stroke=\"{color}\" \
             stroke-width=\"{:.3}\" stroke-dasharray=\"{:.3} {:.3}\" opacity=\"0.7\"/>\n",
            font * 0.05,
            font * 0.4,
            font * 0.3
        ));
    }
    let dot = |p: P, s: &mut String| {
        s.push_str(&format!(
            "<circle cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\" fill=\"{color}\"/>\n",
            p.x,
            p.y,
            font * 0.25
        ))
    };
    dot(a, &mut s);
    if let Some(b) = m.end {
        if let Some(f) = m.from.filter(|f| f.minus(b).len() > font * 0.05) {
            s.push_str(&format!(
                "<line x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\" stroke=\"#cfd4da\" \
                 stroke-width=\"{:.3}\" stroke-dasharray=\"{:.3} {:.3}\"/>\n",
                b.x,
                b.y,
                f.x,
                f.y,
                font * 0.05,
                font * 0.25,
                font * 0.2
            ));
        }
        dot(b, &mut s);
        s.push_str(&format!(
            "<line x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\" stroke=\"{color}\" stroke-width=\"{:.3}\"/>\n",
            a.x,
            a.y,
            b.x,
            b.y,
            font * 0.12
        ));
        s.push_str(&format!(
            "<text x=\"{:.3}\" y=\"{:.3}\" font-size=\"{:.3}\" text-anchor=\"middle\" fill=\"{color}\" \
             stroke=\"#0b1a12\" stroke-width=\"{:.3}\" style=\"paint-order: stroke\" font-weight=\"700\">{}</text>\n",
            (a.x + b.x) * 0.5,
            (a.y + b.y) * 0.5 - font * 0.6,
            font,
            font * 0.25,
            esc(&m.text)
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn footprint(json: &str) -> PcbFootprint {
        let p: openparts_core::Package = serde_json::from_str(json).unwrap();
        let g = openparts_mcad::generate(&p).unwrap();
        openparts_pcbcad::build_footprint("x", &g)
    }

    #[test]
    fn dimensions_the_qfn_footprint() {
        let d = drawing(
            &footprint(include_str!("../tests/qfn56.json")),
            &Labels::default(),
        );
        assert!(d.svg.contains("Pitch 0.4 mm"), "{}", d.svg);
        assert!(d.svg.contains("Pad 1: 0.87 × 0.2 mm"));
        assert!(d.svg.contains("Courtyard X 8.9 mm"));
        assert!(d.svg.contains("Pad span X"));
        assert!(d.svg.contains("class=\"opv-fp-measure\""));
        // The pad drawing itself is render_svg's.
        assert!(d.svg.contains("#c9a227"));
        assert!(d.view[2] > 8.9 && d.view[3] > 8.9);
        let s = pick(&d.snaps, P::new(-3.42, -2.6), 0.1).unwrap();
        assert_eq!(s.kind, SnapKind::PadCenter);
    }

    #[test]
    fn dimensions_a_two_pad_radial() {
        let d = drawing(
            &footprint(include_str!("../tests/hc49u.json")),
            &Labels::default(),
        );
        assert!(d.svg.contains("Pitch 4.88 mm"));
        // One row: no Y span.
        assert!(!d.svg.contains("Pad span Y"));
        assert!(d.svg.contains("Pad span X 6.48 mm"));
    }
}
