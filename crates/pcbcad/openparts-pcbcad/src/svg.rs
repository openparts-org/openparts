//! A footprint as an SVG image (top view): copper pads with their
//! numbers and drills, silkscreen, fabrication outline and courtyard,
//! and a 1 mm scale bar -- drawn from the same `PcbFootprint` the KiCad
//! and LibrePCB files are written from, for previewing in a browser.

use crate::{FpLayer, FpShape, Mounting, PadShape, PcbFootprint, Point2};

const MARGIN: f64 = 1.5;

/// Renders `footprint`; SVG user units are millimetres (Y down, so IR Y
/// is negated).
pub fn render_svg(footprint: &PcbFootprint) -> String {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut extend = |p: Point2| {
        x0 = x0.min(p.x);
        x1 = x1.max(p.x);
        y0 = y0.min(-p.y);
        y1 = y1.max(-p.y);
    };
    for p in &footprint.pads {
        extend(Point2 {
            x: p.position.x - p.size.0 / 2.0,
            y: p.position.y - p.size.1 / 2.0,
        });
        extend(Point2 {
            x: p.position.x + p.size.0 / 2.0,
            y: p.position.y + p.size.1 / 2.0,
        });
    }
    if let Some(c) = &footprint.courtyard {
        c.outline.iter().copied().for_each(&mut extend);
    }
    for g in &footprint.graphics {
        match &g.shape {
            FpShape::Line(a, b) => {
                extend(*a);
                extend(*b);
            }
            FpShape::Polygon(p) => p.iter().copied().for_each(&mut extend),
            FpShape::Circle { center, radius } => {
                extend(Point2 {
                    x: center.x - radius,
                    y: center.y - radius,
                });
                extend(Point2 {
                    x: center.x + radius,
                    y: center.y + radius,
                });
            }
        }
    }
    if x0 > x1 {
        (x0, y0, x1, y1) = (-1.0, -1.0, 1.0, 1.0);
    }
    let (vx, vy) = (x0 - MARGIN, y0 - MARGIN);
    let (vw, vh) = (x1 - x0 + 2.0 * MARGIN, y1 - y0 + 2.0 * MARGIN + 1.0);
    let mut s = String::new();
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{vx:.3} {vy:.3} {vw:.3} {vh:.3}\" \
         width=\"{:.0}\" height=\"{:.0}\" font-family=\"sans-serif\">\n",
        vw * 40.0,
        vh * 40.0
    ));
    s.push_str(&format!(
        "<title>{}</title>\n<rect x=\"{vx:.3}\" y=\"{vy:.3}\" width=\"{vw:.3}\" height=\"{vh:.3}\" fill=\"#0b1a12\"/>\n",
        escape(&footprint.name)
    ));
    // Courtyard, fabrication, then copper, then silkscreen on top.
    if let Some(c) = &footprint.courtyard {
        s.push_str(&shape(
            &FpShape::Polygon(c.outline.clone()),
            0.05,
            "#ff3fa0",
            Some("0.3 0.2"),
        ));
    }
    for g in footprint
        .graphics
        .iter()
        .filter(|g| g.layer == FpLayer::Fab)
    {
        s.push_str(&shape(&g.shape, g.width, "#9aa3ad", None));
    }
    // Overlaid pads (one contact, several numbers) are drawn once, with
    // their numbers joined. Bottom-side lands are drawn first, under the
    // top copper, in blue and outlined.
    let same = |a: &crate::Pad, b: &crate::Pad| {
        (a.position.x - b.position.x).abs() < 1e-9
            && (a.position.y - b.position.y).abs() < 1e-9
            && (a.size.0 - b.size.0).abs() < 1e-9
            && (a.size.1 - b.size.1).abs() < 1e-9
            && a.side == b.side
    };
    let mut pads: Vec<&crate::Pad> = footprint.pads.iter().collect();
    pads.sort_by_key(|p| p.side.is_top());
    for (i, p) in pads.iter().copied().enumerate() {
        if pads[..i].iter().any(|q| same(p, q)) {
            continue;
        }
        let label: Vec<&str> = pads[i..]
            .iter()
            .filter(|q| same(p, q) && !q.number.is_empty())
            .map(|q| q.number.as_str())
            .collect();
        let label = label.join("/");
        let (cx, cy, w, h) = (p.position.x, -p.position.y, p.size.0, p.size.1);
        let r = match p.shape {
            PadShape::Circle | PadShape::Oval => w.min(h) / 2.0,
            _ => 0.0,
        };
        let bottom = !p.side.is_top();
        let fill = if p.mounting == Mounting::NonPlated {
            "#0b1a12"
        } else if bottom {
            "#3a62a8"
        } else {
            "#c9a227"
        };
        let stroke = if p.mounting == Mounting::NonPlated {
            " stroke=\"#9aa3ad\" stroke-width=\"0.05\""
        } else if bottom {
            " stroke=\"#9fbef0\" stroke-width=\"0.05\" stroke-dasharray=\"0.2 0.12\" fill-opacity=\"0.7\""
        } else {
            ""
        };
        if let Some(outline) = p.shape.outline(p.size) {
            let points: Vec<String> = outline
                .iter()
                .map(|q| format!("{:.3},{:.3}", cx + q.x, cy - q.y))
                .collect();
            s.push_str(&format!(
                "<polygon points=\"{}\" fill=\"{fill}\"{stroke}/>\n",
                points.join(" ")
            ));
        } else {
            s.push_str(&format!(
                "<rect x=\"{:.3}\" y=\"{:.3}\" width=\"{w:.3}\" height=\"{h:.3}\" rx=\"{r:.3}\" fill=\"{fill}\"{stroke}/>\n",
                cx - w / 2.0,
                cy - h / 2.0
            ));
        }
        if let (Some(d), true) = (p.drill, p.mounting != Mounting::NonPlated) {
            let (dw, dh) = match p.slot {
                Some(l) if w >= h => (l, d),
                Some(l) => (d, l),
                None => (d, d),
            };
            s.push_str(&format!(
                "<rect x=\"{:.3}\" y=\"{:.3}\" width=\"{dw:.3}\" height=\"{dh:.3}\" rx=\"{:.3}\" fill=\"#0b1a12\"/>\n",
                cx - dw / 2.0,
                cy - dh / 2.0,
                dw.min(dh) / 2.0
            ));
        }
        if !label.is_empty() {
            // Along the pad's long side, as large as fits it.
            let (long, short) = (w.max(h), w.min(h));
            let chars = label.chars().count() as f64;
            let size = (short * 0.75)
                .min(long * 0.85 / (0.6 * chars))
                .clamp(0.05, 1.2);
            let rotate = if h > w * 1.2 {
                format!(" transform=\"rotate(-90 {cx:.3} {cy:.3})\"")
            } else {
                String::new()
            };
            s.push_str(&format!(
                "<text x=\"{cx:.3}\" y=\"{cy:.3}\" dy=\"0.35em\" font-size=\"{size:.3}\" fill=\"#ffffff\" \
                 text-anchor=\"middle\"{rotate}>{}</text>\n",
                escape(&label)
            ));
        }
    }
    for g in footprint
        .graphics
        .iter()
        .filter(|g| g.layer == FpLayer::Silk)
    {
        s.push_str(&shape(&g.shape, g.width, "#f2f2f2", None));
    }
    // 1 mm scale bar under the drawing.
    let (bx, by) = (x0, y1 + MARGIN * 0.6);
    s.push_str(&format!(
        "<line x1=\"{bx:.3}\" y1=\"{by:.3}\" x2=\"{:.3}\" y2=\"{by:.3}\" stroke=\"#9aa3ad\" stroke-width=\"0.08\"/>\n\
         <text x=\"{:.3}\" y=\"{:.3}\" font-size=\"0.4\" fill=\"#9aa3ad\" dominant-baseline=\"central\">1 mm</text>\n",
        bx + 1.0,
        bx + 1.2,
        by
    ));
    s.push_str("</svg>\n");
    s
}

fn shape(shape: &FpShape, width: f64, color: &str, dash: Option<&str>) -> String {
    let style = format!(
        "fill=\"none\" stroke=\"{color}\" stroke-width=\"{width:.3}\"{}",
        dash.map(|d| format!(" stroke-dasharray=\"{d}\""))
            .unwrap_or_default()
    );
    match shape {
        FpShape::Line(a, b) => format!(
            "<line x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\" {style}/>\n",
            a.x, -a.y, b.x, -b.y
        ),
        FpShape::Polygon(points) => {
            let pts: Vec<String> = points
                .iter()
                .map(|p| format!("{:.3},{:.3}", p.x, -p.y))
                .collect();
            format!("<polygon points=\"{}\" {style}/>\n", pts.join(" "))
        }
        FpShape::Circle { center, radius } if *radius <= 0.15 => format!(
            "<circle cx=\"{:.3}\" cy=\"{:.3}\" r=\"{radius:.3}\" fill=\"{color}\"/>\n",
            center.x, -center.y
        ),
        FpShape::Circle { center, radius } => format!(
            "<circle cx=\"{:.3}\" cy=\"{:.3}\" r=\"{radius:.3}\" {style}/>\n",
            center.x, -center.y
        ),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
