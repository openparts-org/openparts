//! A schematic symbol as an SVG image, for previewing in a browser.
//!
//! Two layers: a small drawing model ([`SymbolSheet`]: bodies, leads and
//! texts per unit, millimetres, Y down) laid out and written by
//! [`SymbolSheet::render`] -- shared with the PCBCraft adapter, which
//! draws its own units with PCBCraft's label rules -- and
//! [`render_symbol_svg`], which draws a [`PcbSymbol`] (the KiCad-layout
//! IR, Y up) as KiCad does.
//!
//! Colours are CSS classes on the `sym` root, defaulting to the web
//! site's tokens (`--color-text` and friends) and `currentColor`, so an
//! inlined drawing takes the page's colours; `--sym-*` variables
//! override the symbol palette, and a dark-scheme block covers images
//! shown on their own. Every text is escaped.

use crate::{GraphicKind, PcbSymbol, PinOrientation, SymbolPin};
use std::fmt::Write as _;

/// How a text is aligned along its run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

/// Where a text sits across its run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Baseline {
    /// `at` is the text's vertical centre.
    Centre,
    /// `at` is on the baseline: the text stands above it.
    Bottom,
}

/// What a text is (its CSS class).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    PinName,
    PinNumber,
    Reference,
    Value,
    /// Text drawn as part of the symbol's own graphics.
    Note,
}

impl TextRole {
    fn class(self) -> &'static str {
        match self {
            TextRole::PinName => "name",
            TextRole::PinNumber => "num",
            TextRole::Reference => "ref",
            TextRole::Value => "val",
            TextRole::Note => "note",
        }
    }
}

/// One drawn thing, millimetres, Y down.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A body rectangle.
    Body { x0: f64, y0: f64, x1: f64, y1: f64 },
    /// A straight body line.
    Line { from: (f64, f64), to: (f64, f64) },
    /// A pin's lead from its connection end `from` to the body `to`.
    Lead { from: (f64, f64), to: (f64, f64) },
    Text {
        at: (f64, f64),
        text: String,
        /// The capital height, mm.
        size: f64,
        anchor: Anchor,
        baseline: Baseline,
        /// Turned a quarter counter-clockwise (reads bottom to top).
        vertical: bool,
        role: TextRole,
    },
}

/// One unit of a symbol.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UnitDrawing {
    /// Shown above the unit (`None` for a symbol of one unit).
    pub label: Option<String>,
    pub items: Vec<Item>,
}

/// Every unit of a symbol, side by side.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SymbolSheet {
    /// The `<title>` (the symbol's name).
    pub title: String,
    /// `kicad` / `pcbcraft`: a class on the root, so a page can style
    /// the two apart.
    pub flavour: &'static str,
    pub units: Vec<UnitDrawing>,
}

/// SVG pixels per millimetre at the image's natural size: at least
/// this, more for small symbols (up to `MAX_PX_PER_MM`) so they are at
/// least `MIN_WIDTH_PX` wide.
const PX_PER_MM: f64 = 9.0;
const MAX_PX_PER_MM: f64 = 16.0;
const MIN_WIDTH_PX: f64 = 320.0;
/// Around the drawing and between units, mm.
const MARGIN: f64 = 2.0;
const GAP: f64 = 6.0;
/// A row of units wraps past this width (mm) once it holds one unit.
const ROW_WIDTH: f64 = 170.0;
/// The unit labels' height, mm.
const LABEL_SIZE: f64 = 1.5;
/// Capital height over font size, for the fonts browsers fall back on.
const CAP_PER_EM: f64 = 0.72;
/// An estimate of the average advance per character, in capital
/// heights (sans-serif, mostly capitals and digits).
const ADVANCE_PER_CAP: f64 = 0.86;

const STYLE: &str = "\
svg.sym{color:var(--color-text,#1a1a1e);font-family:system-ui,-apple-system,'Segoe UI',Roboto,sans-serif}\
.sym .body{fill:var(--sym-body-fill,#fffbea);stroke:var(--sym-body,#8b1d0f);stroke-width:.254;stroke-linejoin:round}\
.sym .line{fill:none;stroke:var(--sym-body,#8b1d0f);stroke-width:.254;stroke-linecap:round}\
.sym .lead{stroke:var(--sym-pin,#8b1d0f);stroke-width:.2;stroke-linecap:round}\
.sym text{stroke:none;fill:currentColor}\
.sym .name{fill:var(--sym-name,#006464)}\
.sym .num{fill:var(--sym-num,#a33a1f)}\
.sym .ref,.sym .val{fill:var(--sym-field,#5c5c66)}\
.sym .unit{fill:var(--color-text-muted,#5c5c66);font-weight:600}\
@media (prefers-color-scheme:dark){svg.sym:not(.sym-light){color:#e4e4e8;\
--sym-body-fill:#24221c;--sym-body:#e0866b;--sym-pin:#e0866b;--sym-name:#6cc9cf;--sym-num:#f0a58c;\
--sym-field:#a0a0aa;--color-text-muted:#a0a0aa;--color-border:#3a3a42}}";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The text's run length, estimated, mm.
fn text_width(text: &str, size: f64) -> f64 {
    text.chars().count() as f64 * size * ADVANCE_PER_CAP
}

#[derive(Clone, Copy)]
struct Bounds {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Bounds {
    const EMPTY: Bounds = Bounds {
        x0: f64::MAX,
        y0: f64::MAX,
        x1: f64::MIN,
        y1: f64::MIN,
    };
    fn add(&mut self, x: f64, y: f64) {
        self.x0 = self.x0.min(x);
        self.y0 = self.y0.min(y);
        self.x1 = self.x1.max(x);
        self.y1 = self.y1.max(y);
    }
    fn is_empty(&self) -> bool {
        self.x0 > self.x1
    }
    fn w(&self) -> f64 {
        self.x1 - self.x0
    }
    fn h(&self) -> f64 {
        self.y1 - self.y0
    }
}

fn item_bounds(items: &[Item]) -> Bounds {
    let mut b = Bounds::EMPTY;
    for item in items {
        match item {
            Item::Body { x0, y0, x1, y1 } => {
                b.add(*x0, *y0);
                b.add(*x1, *y1);
            }
            Item::Line { from, to } | Item::Lead { from, to } => {
                b.add(from.0, from.1);
                b.add(to.0, to.1);
            }
            Item::Text {
                at,
                text,
                size,
                anchor,
                baseline,
                vertical,
                ..
            } => {
                let w = text_width(text, *size);
                let (a0, a1) = match anchor {
                    Anchor::Start => (0.0, w),
                    Anchor::Middle => (-w / 2.0, w / 2.0),
                    Anchor::End => (-w, 0.0),
                };
                // Across the run: up from the baseline, or both ways.
                let (c0, c1) = match baseline {
                    Baseline::Centre => (-size * 0.75, size * 0.75),
                    Baseline::Bottom => (-size * 1.25, size * 0.3),
                };
                if *vertical {
                    // The run goes up the page, its top to the left.
                    b.add(at.0 + c0, at.1 - a1);
                    b.add(at.0 + c1, at.1 - a0);
                } else {
                    b.add(at.0 + a0, at.1 + c0);
                    b.add(at.0 + a1, at.1 + c1);
                }
            }
        }
    }
    b
}

fn n(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

fn write_item(s: &mut String, item: &Item, dx: f64, dy: f64) {
    match item {
        Item::Body { x0, y0, x1, y1 } => {
            let _ = writeln!(
                s,
                "<rect class=\"body\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                n(x0.min(*x1) + dx),
                n(y0.min(*y1) + dy),
                n((x1 - x0).abs()),
                n((y1 - y0).abs())
            );
        }
        Item::Line { from, to } => {
            let _ = writeln!(
                s,
                "<line class=\"line\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
                n(from.0 + dx),
                n(from.1 + dy),
                n(to.0 + dx),
                n(to.1 + dy)
            );
        }
        Item::Lead { from, to } => {
            let _ = writeln!(
                s,
                "<line class=\"lead\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
                n(from.0 + dx),
                n(from.1 + dy),
                n(to.0 + dx),
                n(to.1 + dy)
            );
        }
        Item::Text {
            at,
            text,
            size,
            anchor,
            baseline,
            vertical,
            role,
        } => {
            let (x, y) = (at.0 + dx, at.1 + dy);
            let anchor = match anchor {
                Anchor::Start => "",
                Anchor::Middle => " text-anchor=\"middle\"",
                Anchor::End => " text-anchor=\"end\"",
            };
            // Centred: capitals' middle on `at` (0.36 em below the
            // baseline's default, for the fonts browsers fall back on).
            let shift = match baseline {
                Baseline::Centre => " dy=\"0.36em\"",
                Baseline::Bottom => "",
            };
            let place = if *vertical {
                format!("transform=\"translate({} {}) rotate(-90)\"", n(x), n(y))
            } else {
                format!("x=\"{}\" y=\"{}\"", n(x), n(y))
            };
            let _ = writeln!(
                s,
                "<text class=\"{}\" {place}{shift} font-size=\"{}\"{anchor}>{}</text>",
                role.class(),
                n(size / CAP_PER_EM),
                esc(text)
            );
        }
    }
}

impl SymbolSheet {
    /// The SVG document: each unit labelled when there are
    /// several, laid out left to right in rows; user units are mm.
    pub fn render(&self) -> String {
        // Each unit's box (its own coordinates) and its place.
        struct Placed<'a> {
            unit: &'a UnitDrawing,
            b: Bounds,
            x: f64,
            y: f64,
        }
        let labelled = self.units.iter().any(|u| u.label.is_some());
        let head = if labelled { LABEL_SIZE * 2.2 } else { 0.0 };
        let mut placed: Vec<Placed> = Vec::new();
        let (mut x, mut y, mut row_h, mut width) = (MARGIN, MARGIN, 0.0_f64, 0.0_f64);
        for unit in &self.units {
            let mut b = item_bounds(&unit.items);
            if b.is_empty() {
                b = Bounds {
                    x0: 0.0,
                    y0: 0.0,
                    x1: 1.0,
                    y1: 1.0,
                };
            }
            let label_w = unit
                .label
                .as_deref()
                .map_or(0.0, |l| text_width(l, LABEL_SIZE));
            let w = b.w().max(label_w);
            if x > MARGIN && x + w > ROW_WIDTH {
                x = MARGIN;
                y += row_h + GAP;
                row_h = 0.0;
            }
            placed.push(Placed { unit, b, x, y });
            row_h = row_h.max(b.h() + head);
            width = width.max(x + w);
            x += w + GAP;
        }
        let (vw, vh) = (width + MARGIN, y + row_h + MARGIN);
        let px = (MIN_WIDTH_PX / vw).clamp(PX_PER_MM, MAX_PX_PER_MM);
        let mut s = String::new();
        let _ = writeln!(
            s,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"sym sym-{}\" viewBox=\"0 0 {} {}\" \
             width=\"{}\" height=\"{}\" role=\"img\">",
            self.flavour,
            n(vw),
            n(vh),
            (vw * px).round(),
            (vh * px).round()
        );
        let _ = writeln!(
            s,
            "<title>{}</title>\n<style>{STYLE}</style>",
            esc(&self.title)
        );
        for p in &placed {
            s.push_str("<g class=\"unit-drawing\">\n");
            if let Some(label) = &p.unit.label {
                let _ = writeln!(
                    s,
                    "<text class=\"unit\" x=\"{}\" y=\"{}\" font-size=\"{}\">{}</text>",
                    n(p.x),
                    n(p.y + LABEL_SIZE),
                    n(LABEL_SIZE / CAP_PER_EM),
                    esc(label)
                );
            }
            let (dx, dy) = (p.x - p.b.x0, p.y + head - p.b.y0);
            for item in &p.unit.items {
                write_item(&mut s, item, dx, dy);
            }
            s.push_str("</g>\n");
        }
        s.push_str("</svg>\n");
        s
    }
}

// ---- KiCad ------------------------------------------------------------

/// KiCad's default text size and pin name offset (mm).
const KICAD_TEXT: f64 = 1.27;
const KICAD_NAME_OFFSET: f64 = 0.508;
/// A pin number's baseline over its lead.
const KICAD_NUMBER_LIFT: f64 = 0.38;

/// KiCad's unit letters: A..Z, then AA, AB...
pub fn unit_letter(index: u32) -> String {
    let mut i = index.max(1) - 1;
    let mut out = Vec::new();
    loop {
        out.push(b'A' + (i % 26) as u8);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// A pin's lead (connection end, body end), SVG frame (Y down).
fn kicad_lead(pin: &SymbolPin) -> ((f64, f64), (f64, f64)) {
    let (x, y, l) = (pin.position.x, -pin.position.y, pin.length);
    let end = match pin.orientation {
        PinOrientation::Left => (x + l, y),
        PinOrientation::Right => (x - l, y),
        // Y up in the IR: a bottom-edge pin's stub points up the page.
        PinOrientation::Bottom => (x, y - l),
        PinOrientation::Top => (x, y + l),
    };
    ((x, y), end)
}

/// KiCad's `~{...}` overbar markup, shown as plain text.
fn plain_name(name: &str) -> String {
    name.replace("~{", "").replace('}', "")
}

fn kicad_pin(items: &mut Vec<Item>, pin: &SymbolPin) {
    let (from, to) = kicad_lead(pin);
    items.push(Item::Lead { from, to });
    let vertical = matches!(
        pin.orientation,
        PinOrientation::Top | PinOrientation::Bottom
    );
    let mid = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
    if !pin.number.is_empty() {
        let at = if vertical {
            (mid.0 - KICAD_NUMBER_LIFT, mid.1)
        } else {
            (mid.0, mid.1 - KICAD_NUMBER_LIFT)
        };
        items.push(Item::Text {
            at,
            text: pin.number.clone(),
            size: KICAD_TEXT,
            anchor: Anchor::Middle,
            baseline: Baseline::Bottom,
            vertical,
            role: TextRole::PinNumber,
        });
    }
    let name = plain_name(&pin.name);
    if name.is_empty() || name == "~" {
        return;
    }
    let (at, anchor) = match pin.orientation {
        PinOrientation::Left => ((to.0 + KICAD_NAME_OFFSET, to.1), Anchor::Start),
        PinOrientation::Right => ((to.0 - KICAD_NAME_OFFSET, to.1), Anchor::End),
        // Vertical names read bottom to top: a bottom pin's runs up
        // into the body from its end, a top pin's ends above its end.
        PinOrientation::Bottom => ((to.0, to.1 - KICAD_NAME_OFFSET), Anchor::Start),
        PinOrientation::Top => ((to.0, to.1 + KICAD_NAME_OFFSET), Anchor::End),
    };
    items.push(Item::Text {
        at,
        text: name,
        size: KICAD_TEXT,
        anchor,
        baseline: Baseline::Centre,
        vertical,
        role: TextRole::PinName,
    });
}

/// `symbol` as KiCad draws it: per unit the body, every visible pin
/// with its number over the lead and its name inside the body
/// (vertical for top and bottom pins), the reference above and the
/// value below. Pins of unit 0 belong to every unit; hidden (stacked)
/// pins are not drawn, as in KiCad.
pub fn render_symbol_svg(symbol: &PcbSymbol) -> String {
    let mut indices: Vec<u32> = symbol.units.iter().map(|u| u.index).collect();
    indices.sort_unstable();
    indices.dedup();
    if indices.is_empty() {
        indices.push(1);
    }
    let several = indices.len() > 1;
    let units = indices
        .iter()
        .map(|&unit| {
            let mut items = Vec::new();
            let mut top = f64::MAX;
            let mut bottom = f64::MIN;
            for g in &symbol.graphics {
                let GraphicKind::Rectangle { start, end } = g.kind;
                let (y0, y1) = (-start.y, -end.y);
                top = top.min(y0.min(y1));
                bottom = bottom.max(y0.max(y1));
                items.push(Item::Body {
                    x0: start.x,
                    y0,
                    x1: end.x,
                    y1,
                });
            }
            for pin in symbol
                .pins
                .iter()
                .filter(|p| (p.unit == 0 || p.unit == unit) && !p.hidden)
            {
                let (from, _) = kicad_lead(pin);
                top = top.min(from.1);
                bottom = bottom.max(from.1);
                kicad_pin(&mut items, pin);
            }
            if top > bottom {
                (top, bottom) = (0.0, 0.0);
            }
            let reference = if several {
                format!("{}?{}", symbol.reference, unit_letter(unit))
            } else {
                format!("{}?", symbol.reference)
            };
            for (y, text, role) in [
                (top - 2.54, reference, TextRole::Reference),
                (bottom + 2.54, symbol.value.clone(), TextRole::Value),
            ] {
                items.push(Item::Text {
                    at: (0.0, y),
                    text,
                    size: KICAD_TEXT,
                    anchor: Anchor::Middle,
                    baseline: Baseline::Centre,
                    vertical: false,
                    role,
                });
            }
            UnitDrawing {
                label: several.then(|| format!("Unit {}", unit_letter(unit))),
                items,
            }
        })
        .collect();
    SymbolSheet {
        title: symbol.name.clone(),
        flavour: "kicad",
        units,
    }
    .render()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Graphic, PinElectricalType, Point2, SymbolUnit};

    fn pin(number: &str, name: &str, x: f64, y: f64, o: PinOrientation, unit: u32) -> SymbolPin {
        SymbolPin {
            number: number.into(),
            name: name.into(),
            electrical_type: PinElectricalType::Passive,
            position: Point2 { x, y },
            length: 2.54,
            orientation: o,
            unit,
            hidden: false,
        }
    }

    fn symbol(units: &[u32], pins: Vec<SymbolPin>) -> PcbSymbol {
        PcbSymbol {
            name: "X<1>".into(),
            reference: "U".into(),
            value: "X<1>".into(),
            footprint: String::new(),
            datasheet: String::new(),
            units: units.iter().map(|&index| SymbolUnit { index }).collect(),
            pins,
            graphics: vec![Graphic {
                kind: GraphicKind::Rectangle {
                    start: Point2 { x: -5.08, y: 5.08 },
                    end: Point2 { x: 5.08, y: -5.08 },
                },
            }],
        }
    }

    #[test]
    fn draws_pins_names_and_numbers() {
        let mut hidden = pin("9", "GND", -7.62, 0.0, PinOrientation::Left, 1);
        hidden.hidden = true;
        let s = symbol(
            &[1],
            vec![
                pin("1", "~{RST}", -7.62, 2.54, PinOrientation::Left, 1),
                pin("2", "OUT", 7.62, 0.0, PinOrientation::Right, 1),
                pin("3", "VDD", 0.0, 7.62, PinOrientation::Top, 1),
                pin("4", "~", 0.0, -7.62, PinOrientation::Bottom, 1),
                hidden,
            ],
        );
        let svg = render_symbol_svg(&s);
        assert!(
            svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"sym sym-kicad\"")
        );
        assert_eq!(svg.matches("class=\"lead\"").count(), 4);
        assert_eq!(svg.matches("class=\"num\"").count(), 4);
        // `~` is no name; the overbar markup is dropped.
        assert_eq!(svg.matches("class=\"name\"").count(), 3);
        assert!(svg.contains(">RST</text>") && !svg.contains(">GND<"));
        // The top pin's name and number are turned.
        assert!(
            svg.contains("rotate(-90)\" dy=\"0.36em\" font-size=\"1.764\" text-anchor=\"end\">VDD")
        );
        // Escaped, and one unit: no unit label.
        assert!(svg.contains("<title>X&lt;1&gt;</title>"));
        assert!(!svg.contains("class=\"unit\""));
        assert!(svg.contains(">U?</text>"));
    }

    #[test]
    fn units_are_drawn_side_by_side_with_their_shared_pins() {
        let s = symbol(
            &[1, 2],
            vec![
                pin("1", "PWR", -7.62, 2.54, PinOrientation::Left, 0),
                pin("2", "A", 7.62, 0.0, PinOrientation::Right, 1),
                pin("3", "B", 7.62, 0.0, PinOrientation::Right, 2),
            ],
        );
        let svg = render_symbol_svg(&s);
        assert_eq!(svg.matches("class=\"unit\"").count(), 2);
        assert!(svg.contains(">Unit A</text>") && svg.contains(">Unit B</text>"));
        assert!(svg.contains(">U?A</text>") && svg.contains(">U?B</text>"));
        // The shared pin in both, each unit's own once.
        assert_eq!(svg.matches(">PWR</text>").count(), 2);
        assert_eq!(svg.matches(">A</text>").count(), 1);
        assert_eq!(svg.matches("class=\"body\"").count(), 2);
    }

    #[test]
    fn unit_letters_follow_kicad() {
        assert_eq!(unit_letter(1), "A");
        assert_eq!(unit_letter(26), "Z");
        assert_eq!(unit_letter(27), "AA");
    }
}
