//! A reference circuit as a schematic (SVG), drawn from its facts
//! (Canonical Data Specification section 41) -- never traced from the
//! source's figure.
//!
//! The host is a box with only the pins the circuit uses (name inside,
//! numbers outside). Each pin is a row leading away from the box:
//!
//! - a part from the pin to ground hangs below the row (decoupling);
//! - a part to a power net stands above it, to a power symbol (pull-up);
//! - a part to another net sits in the row (USB 27 Ω, a supply's
//!   inductor) and the row ends at that net's label, or continues on a
//!   local net with its parts;
//! - a part between two host pins bridges their rows (`CAP2`--`CAP1`);
//! - a host pin tied to a net ends in that net's label, ground symbol,
//!   or a no-connect cross; pins tied only to ground sit at the bottom.
//!
//! A part between a net and ground hangs from the host pin it is placed
//! near (`place: near`), or from a pin tied to that net. A crystal
//! between two host pins (directly or through a series resistor) is
//! drawn as the Pierce network below those two rows, a bias resistor
//! beside it; other multi-pin parts are boxes joined by net labels;
//! parts between nets alone stand beside the host. Interface nets are
//! flags, local nets (one per instance) plain italic names, so no wire
//! crosses another.
//!
//! A composite (`includes`) is drawn as [`Section`]s: its own parts,
//! then each included circuit under its title.
//!
//! How strongly the source asks for a part is its colour and line style
//! (`lvl-required`, `lvl-recommended`, `lvl-example` dashed,
//! `lvl-optional` dotted); the page draws the legend. Every part and
//! label has a tooltip. Colours are `currentColor` and the site's CSS
//! variables, so the drawing follows the page's theme when inlined.

use openparts_core::LevelValue;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// The symbol a part is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sym {
    Resistor,
    Capacitor,
    PolarCapacitor,
    Inductor,
    /// Terminals: anode first.
    Diode,
    Crystal,
    Switch,
    /// A box with its pins (an IC, a connector, a regulator).
    Block,
}

/// Where a part's pin connects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// A host pin, by name.
    Pin(String),
    /// A net, by its display name.
    Net(String),
}

/// What a part is placed near (`place: { near }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Near {
    /// A host pin, by name.
    Pin(String),
    /// Another part, by role.
    Role(String),
}

/// One part (a role; `count` instances when it is per pin).
#[derive(Debug, Clone)]
pub struct Element {
    pub role: String,
    pub sym: Sym,
    /// Text beside the symbol: `100 nF`; a block's `Flash`, `≤ 128 Mbit`.
    pub value: Vec<String>,
    pub count: usize,
    pub level: LevelValue,
    pub tip: String,
    /// The part's pins and what they connect to.
    pub terms: Vec<(String, End)>,
    pub near: Option<Near>,
}

/// A host pin tied to a net (`None`: left unconnected).
#[derive(Debug, Clone)]
pub struct Connection {
    pub pin: String,
    pub net: Option<String>,
    pub level: LevelValue,
    pub tip: String,
}

/// Everything the drawing needs, resolved from the data.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    /// The host box's title (`RP2040`).
    pub host: String,
    /// The host's pins: (number, name), in pin-number order.
    pub pins: Vec<(String, String)>,
    pub ground: BTreeSet<String>,
    pub power: BTreeSet<String>,
    /// Nets internal to the circuit (a crystal's output node): drawn
    /// as wires or plain names, never as interface flags.
    pub local: BTreeSet<String>,
    pub connects: Vec<Connection>,
    pub elements: Vec<Element>,
}

/// One drawing of a composite: an included circuit (or the composite's
/// own parts) under its title.
#[derive(Debug, Clone)]
pub struct Section {
    pub title: Option<String>,
    pub scene: Scene,
}

// ---------------------------------------------------------------- drawing

/// Approximate advance per character at each text size (system sans).
fn text_width(s: &str, size: f64) -> f64 {
    s.chars()
        .map(|c| match c {
            'i' | 'l' | 'I' | '.' | ',' | ':' | '\'' | '|' | '!' => 0.32,
            'f' | 'j' | 't' | 'r' | ' ' | '(' | ')' | '/' | '-' => 0.42,
            'm' | 'w' | 'M' | 'W' => 0.9,
            'A'..='Z' | 'Ω' | '×' | '≤' | '≥' | '_' => 0.68,
            '0'..='9' => 0.58,
            _ => 0.56,
        })
        .sum::<f64>()
        * size
}

const T_NAME: f64 = 10.0; // pin names
const T_NUM: f64 = 9.0; // pin numbers
const T_VAL: f64 = 10.5; // values, labels
const T_HEAD: f64 = 12.0; // block titles

#[derive(Debug, Clone, Copy)]
struct BBox {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl BBox {
    const EMPTY: BBox = BBox {
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
    fn merge(&mut self, o: BBox, dx: f64, dy: f64) {
        if o.x0 <= o.x1 {
            self.add(o.x0 + dx, o.y0 + dy);
            self.add(o.x1 + dx, o.y1 + dy);
        }
    }
    fn moved(&self, dx: f64, dy: f64) -> BBox {
        BBox {
            x0: self.x0 + dx,
            y0: self.y0 + dy,
            x1: self.x1 + dx,
            y1: self.y1 + dy,
        }
    }
    fn shrunk(&self, d: f64) -> BBox {
        BBox {
            x0: self.x0 + d,
            y0: self.y0 + d,
            x1: self.x1 - d,
            y1: self.y1 - d,
        }
    }
    fn overlaps(&self, o: &BBox) -> bool {
        self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }
    fn contains(&self, o: &BBox) -> bool {
        o.x0 >= self.x0 && o.x1 <= self.x1 && o.y0 >= self.y0 && o.y1 <= self.y1
    }
    /// Whether the segment (a, b) passes through the box (Liang-Barsky).
    fn crossed_by(&self, a: (f64, f64), b: (f64, f64)) -> bool {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        for (p, q) in [
            (-dx, a.0 - self.x0),
            (dx, self.x1 - a.0),
            (-dy, a.1 - self.y0),
            (dy, self.y1 - a.1),
        ] {
            if p == 0.0 {
                if q < 0.0 {
                    return false;
                }
            } else {
                let r = q / p;
                if p < 0.0 {
                    t0 = t0.max(r);
                } else {
                    t1 = t1.min(r);
                }
            }
        }
        t0 < t1
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Anchor {
    Start,
    Middle,
    End,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn n(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        format!("{r:.1}")
    }
}

type Seg = ((f64, f64), (f64, f64));

/// SVG text with its bounding box (shapes, wires and texts tracked
/// separately, so tests can check that no text overlaps another or a
/// wire).
#[derive(Default)]
struct Pen {
    s: String,
    bb: Option<BBox>,
    texts: Vec<(BBox, String)>,
    segs: Vec<Seg>,
}

impl Pen {
    fn bb(&mut self) -> &mut BBox {
        self.bb.get_or_insert(BBox::EMPTY)
    }
    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        self.bb().add(x1, y1);
        self.bb().add(x2, y2);
        self.segs.push(((x1, y1), (x2, y2)));
        let _ = write!(
            self.s,
            "<path d=\"M{} {}L{} {}\"/>",
            n(x1),
            n(y1),
            n(x2),
            n(y2)
        );
    }
    /// A path in absolute coordinates with its own class (`b`: a part's
    /// body); `pts` is its outline as a polyline.
    fn path(&mut self, d: &str, class: &str, pts: &[(f64, f64)]) {
        for &(x, y) in pts {
            self.bb().add(x, y);
        }
        for w in pts.windows(2) {
            self.segs.push((w[0], w[1]));
        }
        let _ = write!(self.s, "<path class=\"{class}\" d=\"{d}\"/>");
    }
    /// A rectangle (a box's outline).
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, class: &str) {
        self.path(
            &format!("M{} {}H{}V{}H{}Z", n(x), n(y), n(x + w), n(y + h), n(x)),
            class,
            &[(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x, y)],
        );
    }
    fn dot(&mut self, x: f64, y: f64) {
        self.bb().add(x - 2.5, y - 2.5);
        self.bb().add(x + 2.5, y + 2.5);
        let _ = write!(
            self.s,
            "<circle class=\"dot\" cx=\"{}\" cy=\"{}\" r=\"2.4\"/>",
            n(x),
            n(y)
        );
    }
    fn text(&mut self, x: f64, y: f64, a: Anchor, class: &str, size: f64, t: &str) {
        let w = text_width(t, size);
        let x0 = match a {
            Anchor::Start => x,
            Anchor::Middle => x - w / 2.0,
            Anchor::End => x - w,
        };
        let bb = BBox {
            x0,
            y0: y - size * 0.8,
            x1: x0 + w,
            y1: y + size * 0.25,
        };
        self.bb().merge(bb, 0.0, 0.0);
        self.texts.push((bb, t.to_string()));
        let anchor = match a {
            Anchor::Start => "",
            Anchor::Middle => " text-anchor=\"middle\"",
            Anchor::End => " text-anchor=\"end\"",
        };
        let _ = write!(
            self.s,
            "<text class=\"{class}\" x=\"{}\" y=\"{}\"{anchor}>{}</text>",
            n(x),
            n(y),
            esc(t)
        );
    }
    /// Vertical text reading upwards, its end at (x, y).
    fn text_up(&mut self, x: f64, y: f64, class: &str, size: f64, t: &str) {
        let w = text_width(t, size);
        let bb = BBox {
            x0: x - size * 0.8,
            y0: y - w,
            x1: x + size * 0.25,
            y1: y,
        };
        self.bb().merge(bb, 0.0, 0.0);
        self.texts.push((bb, t.to_string()));
        let _ = write!(
            self.s,
            "<text class=\"{class}\" transform=\"translate({} {}) rotate(-90)\">{}</text>",
            n(x),
            n(y),
            esc(t)
        );
    }
    fn open(&mut self, class: &str, tip: &str) {
        let _ = write!(self.s, "<g class=\"{class}\"><title>{}</title>", esc(tip));
    }
    fn close(&mut self) {
        self.s.push_str("</g>");
    }
    /// Appends `o` moved by (dx, dy).
    fn place(&mut self, o: Pen, dx: f64, dy: f64) {
        if let Some(b) = o.bb {
            self.bb().merge(b, dx, dy);
        }
        for (b, t) in o.texts {
            self.texts.push((b.moved(dx, dy), t));
        }
        for (a, b) in o.segs {
            self.segs.push(((a.0 + dx, a.1 + dy), (b.0 + dx, b.1 + dy)));
        }
        let _ = write!(
            self.s,
            "<g transform=\"translate({} {})\">{}</g>",
            n(dx),
            n(dy),
            o.s
        );
    }
    fn bbox(&self) -> BBox {
        self.bb.unwrap_or(BBox {
            x0: 0.0,
            y0: 0.0,
            x1: 0.0,
            y1: 0.0,
        })
    }
}

fn level_class(l: LevelValue) -> &'static str {
    match l {
        LevelValue::Required => "lvl-required",
        LevelValue::Recommended => "lvl-recommended",
        LevelValue::Example => "lvl-example",
        LevelValue::Optional => "lvl-optional",
    }
}

/// Half the distance between a symbol's terminals.
const HALF: f64 = 20.0;

/// Draws `sym` with its terminals at `a` and `b` (axis-aligned, `HALF`
/// from the centre), leads included.
fn symbol(pen: &mut Pen, sym: Sym, a: (f64, f64), b: (f64, f64)) {
    let (cx, cy) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(1.0);
    let (ux, uy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    // u along the axis (a -> b), v across it.
    let p = |u: f64, v: f64| (cx + ux * u - uy * v, cy + uy * u + ux * v);
    let lead = |pen: &mut Pen, from: f64| {
        let (x1, y1) = p(-HALF, 0.0);
        let (x2, y2) = p(-from, 0.0);
        pen.line(x1, y1, x2, y2);
        let (x1, y1) = p(from, 0.0);
        let (x2, y2) = p(HALF, 0.0);
        pen.line(x1, y1, x2, y2);
    };
    let seg = |pen: &mut Pen, u1: f64, v1: f64, u2: f64, v2: f64| {
        let (x1, y1) = p(u1, v1);
        let (x2, y2) = p(u2, v2);
        pen.path(
            &format!("M{} {}L{} {}", n(x1), n(y1), n(x2), n(y2)),
            "b",
            &[(x1, y1), (x2, y2)],
        );
    };
    let poly = |pen: &mut Pen, pts: &[(f64, f64)], close: bool| {
        let mut pts: Vec<(f64, f64)> = pts.iter().map(|&(u, v)| p(u, v)).collect();
        let mut d = String::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            let _ = write!(d, "{}{} {}", if i == 0 { "M" } else { "L" }, n(*x), n(*y));
        }
        if close {
            d.push('Z');
            pts.push(pts[0]);
        }
        pen.path(&d, "b", &pts);
    };
    match sym {
        Sym::Resistor => {
            lead(pen, 11.0);
            poly(
                pen,
                &[(-11.0, -4.5), (11.0, -4.5), (11.0, 4.5), (-11.0, 4.5)],
                true,
            );
        }
        Sym::Capacitor => {
            lead(pen, 2.5);
            seg(pen, -2.5, -8.0, -2.5, 8.0);
            seg(pen, 2.5, -8.0, 2.5, 8.0);
        }
        Sym::PolarCapacitor => {
            lead(pen, 2.5);
            seg(pen, -2.5, -8.0, -2.5, 8.0);
            // The negative plate, curved.
            let (x1, y1) = p(5.0, -8.0);
            let (xc, yc) = p(1.5, 0.0);
            let (x2, y2) = p(5.0, 8.0);
            pen.path(
                &format!(
                    "M{} {}Q{} {} {} {}",
                    n(x1),
                    n(y1),
                    n(xc),
                    n(yc),
                    n(x2),
                    n(y2)
                ),
                "b",
                &[(x1, y1), (xc, yc), (x2, y2)],
            );
            seg(pen, -8.0, -9.0, -8.0, -5.0);
            seg(pen, -10.0, -7.0, -6.0, -7.0);
        }
        Sym::Inductor => {
            lead(pen, 12.0);
            let mut d = String::new();
            let (x0, y0) = p(-12.0, 0.0);
            let _ = write!(d, "M{} {}", n(x0), n(y0));
            let mut pts = vec![(x0, y0)];
            for i in 0..4 {
                let u = -12.0 + 6.0 * (i as f64 + 1.0);
                let (x, y) = p(u, 0.0);
                let (qx, qy) = p(u - 3.0, -7.0);
                let _ = write!(d, "Q{} {} {} {}", n(qx), n(qy), n(x), n(y));
                pts.push((qx, qy));
                pts.push((x, y));
            }
            pen.path(&d, "b", &pts);
        }
        Sym::Diode => {
            lead(pen, 6.0);
            poly(pen, &[(-6.0, -6.5), (-6.0, 6.5), (6.0, 0.0)], true);
            seg(pen, 6.0, -6.5, 6.0, 6.5);
        }
        Sym::Crystal => {
            lead(pen, 7.0);
            seg(pen, -7.0, -8.0, -7.0, 8.0);
            seg(pen, 7.0, -8.0, 7.0, 8.0);
            poly(
                pen,
                &[(-3.5, -6.0), (3.5, -6.0), (3.5, 6.0), (-3.5, 6.0)],
                true,
            );
        }
        Sym::Switch | Sym::Block => {
            // A push button: two contacts and the plunger beside them.
            lead(pen, 12.0);
            for u in [-12.0, 12.0] {
                let (x, y) = p(u, 0.0);
                pen.path(
                    &format!("M{} {}m-2 0a2 2 0 1 0 4 0a2 2 0 1 0 -4 0", n(x), n(y)),
                    "b",
                    &[(x - 2.0, y - 2.0), (x + 2.0, y + 2.0)],
                );
            }
            seg(pen, -13.0, -6.0, 13.0, -6.0);
            seg(pen, 0.0, -6.0, 0.0, -11.0);
            seg(pen, -4.0, -11.0, 4.0, -11.0);
        }
    }
}

/// How far a symbol reaches beside its axis on the side its value is
/// written (a switch's plunger).
fn side_reach(sym: Sym) -> f64 {
    match sym {
        Sym::Switch | Sym::Block => 11.0,
        _ => 0.0,
    }
}

/// A ground symbol hanging from (x, y).
fn ground(pen: &mut Pen, x: f64, y: f64) {
    pen.line(x - 7.0, y, x + 7.0, y);
    pen.line(x - 4.5, y + 3.0, x + 4.5, y + 3.0);
    pen.line(x - 2.0, y + 6.0, x + 2.0, y + 6.0);
}

/// A power symbol standing on (x, y): a bar and the net's name.
fn power(pen: &mut Pen, x: f64, y: f64, net: &str) {
    pen.line(x - 7.0, y, x + 7.0, y);
    pen.text(x, y - 4.0, Anchor::Middle, "lbl", T_VAL, net);
}

/// The space a power symbol's name takes beside its wire.
fn power_half(net: &str) -> f64 {
    text_width(net, T_VAL) / 2.0
}

/// A net label (a flag) whose wire end is (x, y), pointing `dir`;
/// returns its length.
fn flag(pen: &mut Pen, x: f64, y: f64, dir: f64, net: &str, class: &str, tip: &str) -> f64 {
    let w = text_width(net, T_VAL) + 10.0;
    let h = 7.5;
    pen.open(&format!("lbl {class}"), tip);
    let pts = [
        (x, y),
        (x + dir * h, y - h),
        (x + dir * (w + h), y - h),
        (x + dir * (w + h), y + h),
        (x + dir * h, y + h),
        (x, y),
    ];
    let mut d = String::new();
    for (i, (px, py)) in pts[..5].iter().enumerate() {
        let _ = write!(d, "{}{} {}", if i == 0 { "M" } else { "L" }, n(*px), n(*py));
    }
    d.push('Z');
    pen.path(&d, "b", &pts);
    let tx = x + dir * (h + w / 2.0);
    pen.text(tx, y + 3.6, Anchor::Middle, "", T_VAL, net);
    pen.close();
    w + h
}

/// A local net's name at a wire's end (x, y), going `dir`: plain
/// italic text, unlike an interface flag; returns its length.
fn local_name(pen: &mut Pen, x: f64, y: f64, dir: f64, net: &str, tip: &str) -> f64 {
    let w = text_width(net, T_VAL);
    pen.open("lbl loc", tip);
    pen.line(x, y - 4.0, x, y + 4.0);
    pen.text(
        x + dir * 4.0,
        y + 3.6,
        if dir > 0.0 {
            Anchor::Start
        } else {
            Anchor::End
        },
        "",
        T_VAL,
        net,
    );
    pen.close();
    w + 4.0
}

/// A no-connect cross at (x, y).
fn no_connect(pen: &mut Pen, x: f64, y: f64) {
    pen.line(x - 4.0, y - 4.0, x + 4.0, y + 4.0);
    pen.line(x - 4.0, y + 4.0, x + 4.0, y - 4.0);
}

// ---------------------------------------------------------------- planning

#[derive(Debug, Clone, PartialEq)]
enum At {
    Row(String),
    Gnd,
    Net(String),
}

#[derive(Debug, Clone, Default)]
struct Row {
    pin: String,
    numbers: Vec<String>,
    /// A name on the wire next to the pin (a block's pin joins here,
    /// or the pin's own local net).
    link: Option<String>,
    up: Vec<(usize, String)>,
    down: Vec<usize>,
    /// A part in the row to a net, and whether it is drawn reversed.
    series: Option<(usize, String, bool)>,
    /// Parts from the series part's far net to ground.
    far_down: Vec<usize>,
    end: RowEnd,
}

impl Row {
    /// Whether the row has anything but its wire.
    fn bare(&self) -> bool {
        self.up.is_empty()
            && self.down.is_empty()
            && self.series.is_none()
            && self.link.is_none()
            && matches!(self.end, RowEnd::Open)
    }
}

type Tip = Option<(LevelValue, String)>;

#[derive(Debug, Clone, Default)]
enum RowEnd {
    #[default]
    Open,
    Label(String, Tip),
    /// A local net's name.
    Local(String, Tip),
    Gnd(Tip),
    Nc(LevelValue, String),
    /// A name on the wire's end (a net label joining a block's pin).
    Name(String),
}

/// A crystal between two host pins.
#[derive(Debug, Clone)]
struct Pierce {
    crystal: usize,
    /// The crystal's ground pin, if any.
    grounded: bool,
    /// (pin, numbers, series part, caps to ground): the upper row is
    /// the one without a series part.
    upper: (String, Vec<String>, Option<usize>, Vec<usize>),
    lower: (String, Vec<String>, Option<usize>, Vec<usize>),
    /// Whether the crystal's first terminal is on the lower side.
    first_lower: bool,
    /// Parts across the crystal (a bias resistor).
    bias: Vec<usize>,
}

/// Two host pins with parts between them.
#[derive(Debug, Clone)]
struct Bridge {
    upper: Row,
    lower: Row,
    parts: Vec<usize>,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
enum Unit {
    Row(Row),
    Pierce(Pierce),
    Bridge(Bridge),
}

impl Unit {
    fn pins(&self) -> Vec<&str> {
        match self {
            Unit::Row(r) => vec![&r.pin],
            Unit::Pierce(p) => vec![&p.upper.0, &p.lower.0],
            Unit::Bridge(b) => vec![&b.upper.pin, &b.lower.pin],
        }
    }
}

/// Something drawn beside the host.
#[derive(Debug, Clone)]
enum Free {
    /// A two-pin part between two nets (or a net and ground), with
    /// the names its ends are labelled with.
    Part(usize, At, At, String, String),
    /// A block with its pins (left, right) and power pins on top.
    Block(usize, [Vec<Row>; 2], Vec<(String, String)>),
}

/// A pin tied only to ground: name, numbers, (level, tooltip).
type GroundPin = (String, Vec<String>, Tip);

struct Plan {
    sides: [Vec<Unit>; 2], // left, right
    bottom: Vec<GroundPin>,
    free: Vec<Free>,
}

/// Where a part between a net and ground hangs.
enum Hang {
    Down(String),
    Far(String),
}

impl Scene {
    fn numbers(&self, pin: &str) -> Vec<String> {
        self.pins
            .iter()
            .filter(|(_, name)| name == pin)
            .map(|(num, _)| num.clone())
            .collect()
    }

    fn pin_index(&self, pin: &str) -> usize {
        self.pins
            .iter()
            .position(|(_, name)| name == pin)
            .unwrap_or(usize::MAX)
    }

    /// The host pins tied to `net`, in pin order.
    fn tied(&self, net: &str) -> Vec<&str> {
        let mut pins: Vec<&str> = self
            .connects
            .iter()
            .filter(|c| c.net.as_deref() == Some(net))
            .map(|c| c.pin.as_str())
            .collect();
        pins.sort_by_key(|p| self.pin_index(p));
        pins.dedup();
        pins
    }

    fn at(&self, e: &End) -> At {
        match e {
            End::Pin(p) => At::Row(p.clone()),
            End::Net(net) if self.ground.contains(net) => At::Gnd,
            // A power net is a symbol (or a label), never a row.
            End::Net(net) if self.power.contains(net) => At::Net(net.clone()),
            End::Net(net) => {
                // A net that exactly one host pin is tied to is that pin's row.
                match self.tied(net).as_slice() {
                    [one] => At::Row(one.to_string()),
                    _ => At::Net(net.clone()),
                }
            }
        }
    }

    fn end_label(e: &End) -> String {
        match e {
            End::Pin(p) | End::Net(p) => p.clone(),
        }
    }

    fn is_local(&self, net: &str) -> bool {
        self.local.contains(net)
    }

    /// The text beside a part: its value (and count), else its role.
    fn lines(&self, i: usize) -> Vec<String> {
        let e = &self.elements[i];
        let mut v = e.value.clone();
        if v.is_empty() {
            v.push(e.role.clone());
        }
        if e.count > 1 {
            v[0].push_str(&format!(" ×{}", e.count));
        }
        v
    }

    fn lines_width(&self, i: usize) -> f64 {
        self.lines(i)
            .iter()
            .map(|l| text_width(l, T_VAL))
            .fold(0.0, f64::max)
    }

    /// Room a vertical part takes beside its wire on its text side.
    fn beside(&self, i: usize) -> f64 {
        13.0 + side_reach(self.elements[i].sym) + self.lines_width(i)
    }

    /// The Pierce network's depths below the lower row: (bias line,
    /// crystal line).
    fn pierce_depths(&self, p: &Pierce) -> (f64, f64) {
        let cl = self.lines(p.crystal).len() as f64;
        if p.bias.is_empty() {
            return (0.0, snap((12.0 * cl + 20.0).max(40.0)));
        }
        let bl = p
            .bias
            .iter()
            .map(|i| self.lines(*i).len())
            .max()
            .unwrap_or(1) as f64;
        let bias = snap(14.0 + 12.0 * bl);
        let between = 30.0 * (p.bias.len() as f64 - 1.0);
        (bias, snap(bias + between + (18.0 + 12.0 * cl).max(30.0)))
    }

    /// Space a row needs above and below its wire.
    fn row_extent(&self, r: &Row) -> (f64, f64) {
        let mut above: f64 = 20.0;
        if !r.up.is_empty() {
            above = above.max(70.0);
        }
        if let Some((i, _, _)) = &r.series {
            above = above.max(24.0 + 12.0 * (self.lines(*i).len() as f64 - 1.0));
        }
        let parts = r.down.iter().chain(&r.far_down);
        let below = match parts.map(|i| self.lines(*i).len()).max() {
            // The part's text, centred on it, may reach past it.
            Some(l) => 70.0f64.max(30.0 + 6.5 * l as f64 + 14.0),
            None if matches!(r.end, RowEnd::Gnd(_)) => 20.0,
            None => 10.0,
        };
        (above, below)
    }

    /// Rows of a bridge apart.
    fn bridge_gap(&self, b: &Bridge) -> f64 {
        let lines = b
            .parts
            .iter()
            .map(|i| self.lines(*i).len())
            .max()
            .unwrap_or(1) as f64;
        snap(
            (self.row_extent(&b.upper).1 + self.row_extent(&b.lower).0)
                .max(70.0)
                .max(13.0 * lines + 30.0),
        )
    }

    fn extent(&self, u: &Unit) -> (f64, f64) {
        match u {
            Unit::Row(r) => self.row_extent(r),
            Unit::Pierce(p) => (20.0, PIERCE_LOW + self.pierce_depths(p).1 + 70.0),
            Unit::Bridge(b) => (
                self.row_extent(&b.upper).0,
                self.bridge_gap(b) + self.row_extent(&b.lower).1,
            ),
        }
    }

    fn plan(&self) -> Plan {
        let mut used = vec![false; self.elements.len()];
        let mut rows: BTreeMap<String, Row> = BTreeMap::new();
        fn get<'a>(s: &Scene, rows: &'a mut BTreeMap<String, Row>, pin: &str) -> &'a mut Row {
            rows.entry(pin.to_string()).or_insert_with(|| Row {
                pin: pin.to_string(),
                numbers: s.numbers(pin),
                ..Row::default()
            })
        }
        let two = |e: &Element| -> Option<(At, At)> {
            match e.terms.as_slice() {
                [(_, a), (_, b)] if e.sym != Sym::Block => Some((self.at(a), self.at(b))),
                _ => None,
            }
        };
        let connected: BTreeSet<&str> = self.connects.iter().map(|c| c.pin.as_str()).collect();

        // Crystals between two host pins.
        let mut pierces = Vec::new();
        for (i, e) in self.elements.iter().enumerate() {
            if e.sym != Sym::Crystal {
                continue;
            }
            let ats: Vec<At> = e.terms.iter().map(|(_, t)| self.at(t)).collect();
            let signal: Vec<(usize, &At)> = ats
                .iter()
                .enumerate()
                .filter(|(_, a)| **a != At::Gnd)
                .collect();
            if signal.len() != 2 {
                continue;
            }
            // Each side: the host pin, the series part reaching it, and
            // the node its caps hang from.
            let mut sides = Vec::new();
            for (_, a) in &signal {
                let side = match a {
                    At::Row(p) if !connected.contains(p.as_str()) => {
                        Some((p.clone(), None, (*a).clone()))
                    }
                    At::Net(net) if self.is_local(net) => self
                        .elements
                        .iter()
                        .enumerate()
                        .find_map(|(j, s)| match two(s) {
                            Some((At::Row(p), At::Net(m))) | Some((At::Net(m), At::Row(p)))
                                if &m == net && j != i && !connected.contains(p.as_str()) =>
                            {
                                Some((p, Some(j), (*a).clone()))
                            }
                            _ => None,
                        }),
                    _ => None,
                };
                sides.push(side);
            }
            let (Some(s0), Some(s1)) = (sides[0].clone(), sides[1].clone()) else {
                continue;
            };
            if s0.0 == s1.0
                || pierces
                    .iter()
                    .any(|p: &Pierce| [&p.upper.0, &p.lower.0].contains(&&s0.0))
            {
                continue;
            }
            let caps = |node: &At| -> Vec<usize> {
                self.elements
                    .iter()
                    .enumerate()
                    .filter(|(j, s)| {
                        *j != i
                            && matches!(two(s), Some((ref a, At::Gnd)) | Some((At::Gnd, ref a)) if a == node)
                    })
                    .map(|(j, _)| j)
                    .collect()
            };
            let mk = |s: &(String, Option<usize>, At)| {
                (s.0.clone(), self.numbers(&s.0), s.1, caps(&s.2))
            };
            let (a, b) = (mk(&s0), mk(&s1));
            // The side with a series part is the lower row.
            let first_lower = a.2.is_some() && b.2.is_none();
            let (upper, lower) = if first_lower { (b, a) } else { (a, b) };
            used[i] = true;
            for j in [upper.2, lower.2].into_iter().flatten() {
                used[j] = true;
            }
            for &j in upper.3.iter().chain(&lower.3) {
                used[j] = true;
            }
            pierces.push(Pierce {
                crystal: i,
                grounded: signal.len() < ats.len(),
                upper,
                lower,
                first_lower,
                bias: Vec::new(),
            });
        }
        // Parts across a crystal's two pins.
        for p in &mut pierces {
            for (j, e) in self.elements.iter().enumerate() {
                if used[j] {
                    continue;
                }
                if let Some((At::Row(a), At::Row(b))) = two(e) {
                    let ends = [a.as_str(), b.as_str()];
                    if ends.contains(&p.upper.0.as_str()) && ends.contains(&p.lower.0.as_str()) {
                        p.bias.push(j);
                        used[j] = true;
                    }
                }
            }
        }
        let pierce_pins: BTreeSet<String> = pierces
            .iter()
            .flat_map(|p| [p.upper.0.clone(), p.lower.0.clone()])
            .collect();

        // Parts between two host pins: one bridge per pair, a pin in
        // one bridge.
        let mut bridges: Vec<(String, String, Vec<usize>)> = Vec::new();
        for (i, e) in self.elements.iter().enumerate() {
            if used[i] {
                continue;
            }
            let Some((At::Row(a), At::Row(b))) = two(e) else {
                continue;
            };
            if a == b || pierce_pins.contains(&a) || pierce_pins.contains(&b) {
                continue;
            }
            if let Some(br) = bridges
                .iter_mut()
                .find(|(x, y, _)| (*x == a && *y == b) || (*x == b && *y == a))
            {
                br.2.push(i);
                used[i] = true;
            } else if !bridges
                .iter()
                .any(|(x, y, _)| [x, y].contains(&&a) || [x, y].contains(&&b))
            {
                bridges.push((a, b, vec![i]));
                used[i] = true;
            }
        }

        // Two-pin parts on rows.
        let mut free = Vec::new();
        let mut far: Vec<(usize, At, At, String, String)> = Vec::new();
        for (i, e) in self.elements.iter().enumerate() {
            if used[i] {
                continue;
            }
            let Some((a, b)) = two(e) else { continue };
            let (la, lb) = (
                Self::end_label(&e.terms[0].1),
                Self::end_label(&e.terms[1].1),
            );
            // Host pin first.
            let (a, b, la, lb, flip) = match (&a, &b) {
                (At::Row(_), _) => (a, b, la, lb, false),
                (_, At::Row(_)) => (b, a, lb, la, true),
                _ => (a, b, la, lb, false),
            };
            match (&a, &b) {
                (At::Row(p), At::Gnd) if !pierce_pins.contains(p) => {
                    get(self, &mut rows, p).down.push(i);
                    used[i] = true;
                }
                (At::Row(p), At::Net(net)) if !pierce_pins.contains(p) => {
                    let pull = self.power.contains(net) && e.sym != Sym::Inductor;
                    let r = get(self, &mut rows, p);
                    if !pull && r.series.is_none() {
                        r.series = Some((i, net.clone(), flip));
                        used[i] = true;
                    } else if self.power.contains(net) {
                        r.up.push((i, net.clone()));
                        used[i] = true;
                    }
                }
                _ => {}
            }
            if !used[i] {
                far.push((i, a, b, la, lb));
            }
        }
        // Parts from a net to ground hang from the pin they are placed
        // near, a pin tied to the net, or a series part's far end.
        for (i, a, b, la, lb) in far {
            let net = match (&a, &b) {
                (At::Net(m), At::Gnd) | (At::Gnd, At::Net(m)) => Some(m.clone()),
                (At::Row(p), At::Gnd) | (At::Gnd, At::Row(p)) if pierce_pins.contains(p) => None,
                _ => None,
            };
            let hang = net.and_then(|m| {
                let ok_pin = |p: &str| !pierce_pins.contains(p);
                let far_of = |rows: &BTreeMap<String, Row>, role: Option<&str>| {
                    rows.values()
                        .find(|r| {
                            matches!(&r.series, Some((j, n, _)) if *n == m
                                && role.is_none_or(|role| self.elements[*j].role == role))
                        })
                        .map(|r| r.pin.clone())
                };
                let tied: Vec<&str> = self.tied(&m).into_iter().filter(|p| ok_pin(p)).collect();
                let hinted = match &self.elements[i].near {
                    Some(Near::Role(role)) => far_of(&rows, Some(role)).map(Hang::Far),
                    Some(Near::Pin(p)) if tied.contains(&p.as_str()) => Some(Hang::Down(p.clone())),
                    Some(Near::Pin(p)) => rows
                        .get(p)
                        .filter(|r| matches!(&r.series, Some((_, n, _)) if *n == m))
                        .map(|r| Hang::Far(r.pin.clone())),
                    None => None,
                };
                hinted
                    .or_else(|| {
                        // The tied pin with the fewest parts so far.
                        tied.iter()
                            .min_by_key(|p| rows.get(**p).map_or(0, |r| r.down.len() + r.up.len()))
                            .map(|p| Hang::Down(p.to_string()))
                    })
                    .or_else(|| far_of(&rows, None).map(Hang::Far))
            });
            match hang {
                Some(Hang::Down(p)) => get(self, &mut rows, &p).down.push(i),
                Some(Hang::Far(p)) => get(self, &mut rows, &p).far_down.push(i),
                None => free.push(Free::Part(i, a, b, la, lb)),
            }
            used[i] = true;
        }

        // Host pins tied to nets.
        for c in &self.connects {
            if pierce_pins.contains(&c.pin) {
                continue;
            }
            let local = c.net.as_deref().is_some_and(|n| self.is_local(n));
            let r = get(self, &mut rows, &c.pin);
            let tip = Some((c.level, c.tip.clone()));
            match &c.net {
                None => r.end = RowEnd::Nc(c.level, c.tip.clone()),
                Some(net) if self.ground.contains(net) => r.end = RowEnd::Gnd(tip),
                Some(net) => {
                    let ends_elsewhere =
                        r.series.as_ref().is_some_and(|(_, m, _)| !self.is_local(m));
                    if ends_elsewhere {
                        if !(local && *net == r.pin) {
                            r.link = Some(net.clone());
                        }
                    } else if local {
                        // The pin's own node: named unless the name is the pin's.
                        if *net != r.pin || r.bare() {
                            r.end = RowEnd::Local(net.clone(), tip);
                        }
                    } else {
                        r.end = RowEnd::Label(net.clone(), tip);
                    }
                }
            }
        }
        // A series part to an interface net ends at its label.
        for r in rows.values_mut() {
            if let Some((_, net, _)) = &r.series {
                if !self.is_local(net) && matches!(r.end, RowEnd::Open) {
                    r.end = RowEnd::Label(net.clone(), None);
                }
            }
        }

        // Blocks (and parts we could not place): their pins join by labels.
        for (i, e) in self.elements.iter().enumerate() {
            if used[i] {
                continue;
            }
            used[i] = true;
            let mut brows = Vec::new();
            let mut top = Vec::new();
            for (pin, t) in &e.terms {
                let mut r = Row {
                    pin: pin.clone(),
                    ..Row::default()
                };
                match self.at(t) {
                    At::Gnd => r.end = RowEnd::Gnd(None),
                    At::Row(p) => {
                        r.end = RowEnd::Name(p.clone());
                        let hr = get(self, &mut rows, &p);
                        if hr.bare() {
                            hr.end = RowEnd::Name(p.clone());
                        } else if hr.link.is_none() {
                            hr.link = Some(p.clone());
                        }
                    }
                    At::Net(net) if self.power.contains(&net) => {
                        top.push((pin.clone(), net));
                        continue;
                    }
                    At::Net(net) if self.is_local(&net) => r.end = RowEnd::Local(net, None),
                    At::Net(net) => r.end = RowEnd::Label(net, None),
                }
                brows.push(r);
            }
            // Signal pins in host-pin order, ground last.
            brows.sort_by_key(|r| match &r.end {
                RowEnd::Name(p) => (0, self.pin_index(p)),
                RowEnd::Gnd(_) => (2, 0),
                _ => (1, 0),
            });
            // Ground pins go below the box; the rest on both sides.
            let (gnd, mut sig): (Vec<Row>, Vec<Row>) = brows
                .into_iter()
                .partition(|r| matches!(r.end, RowEnd::Gnd(_)));
            let right = if sig.len() > 3 {
                sig.split_off(sig.len().div_ceil(2))
            } else {
                Vec::new()
            };
            sig.extend(gnd);
            free.push(Free::Block(i, [sig, right], top));
        }

        // Bridged pins: both rows (a ground pin too) side by side.
        let mut units: Vec<(usize, Unit)> = Vec::new();
        for (a, b, parts) in bridges {
            let ra = rows.remove(&a).unwrap_or_else(|| Row {
                pin: a.clone(),
                numbers: self.numbers(&a),
                ..Row::default()
            });
            let rb = rows.remove(&b).unwrap_or_else(|| Row {
                pin: b.clone(),
                numbers: self.numbers(&b),
                ..Row::default()
            });
            // The order that keeps them closest; pin order on a tie.
            let ab = self.row_extent(&ra).1 + self.row_extent(&rb).0;
            let ba = self.row_extent(&rb).1 + self.row_extent(&ra).0;
            let in_order = self.pin_index(&a) <= self.pin_index(&b);
            let (upper, lower) = if ab < ba || (ab == ba && in_order) {
                (ra, rb)
            } else {
                (rb, ra)
            };
            let at = self.pin_index(&a).min(self.pin_index(&b));
            units.push((
                at,
                Unit::Bridge(Bridge {
                    upper,
                    lower,
                    parts,
                }),
            ));
        }

        // Ground-only pins go to the bottom of the box.
        let mut bottom = Vec::new();
        for (pin, r) in rows {
            if pierce_pins.contains(&pin) {
                continue;
            }
            let bare =
                r.up.is_empty() && r.down.is_empty() && r.series.is_none() && r.link.is_none();
            if let (true, RowEnd::Gnd(tip)) = (bare, &r.end) {
                bottom.push((pin.clone(), r.numbers.clone(), tip.clone()));
                continue;
            }
            units.push((self.pin_index(&pin), Unit::Row(r)));
        }
        bottom.sort_by_key(|(p, _, _)| self.pin_index(p));
        for p in pierces {
            let at = self.pin_index(&p.upper.0).min(self.pin_index(&p.lower.0));
            units.push((at, Unit::Pierce(p)));
        }
        units.sort_by_key(|(at, _)| *at);
        let units: Vec<Unit> = units.into_iter().map(|(_, u)| u).collect();
        let sides = self.split(units);
        Plan {
            sides,
            bottom,
            free,
        }
    }

    /// Left and right: inputs left, outputs right, the rest so that the
    /// box is short and the sides about as many pins (few rows all on
    /// the right).
    fn split(&self, units: Vec<Unit>) -> [Vec<Unit>; 2] {
        fn hint(pin: &str) -> Option<usize> {
            let up = pin.to_uppercase();
            let tokens: Vec<&str> = up.split(|c: char| !c.is_ascii_alphanumeric()).collect();
            if tokens
                .iter()
                .any(|t| *t == "IN" || *t == "INPUT" || t.starts_with("VIN"))
            {
                Some(0)
            } else if tokens
                .iter()
                .any(|t| *t == "OUT" || *t == "OUTPUT" || t.starts_with("VOUT"))
            {
                Some(1)
            } else {
                None
            }
        }
        let hints: Vec<Option<usize>> = units
            .iter()
            .map(|u| match u {
                Unit::Row(r) => hint(&r.pin),
                Unit::Pierce(_) => Some(1),
                Unit::Bridge(_) => None,
            })
            .collect();
        let any_hint = units
            .iter()
            .zip(&hints)
            .any(|(u, h)| matches!(u, Unit::Row(_)) && h.is_some());
        if units.len() <= 3 && !any_hint {
            return [Vec::new(), units];
        }
        let heights: Vec<f64> = units
            .iter()
            .map(|u| {
                let (a, b) = self.extent(u);
                a + b
            })
            .collect();
        let pins: Vec<f64> = units.iter().map(|u| u.pins().len() as f64).collect();
        let mut fixed = [(0.0, 0.0), (0.0, 0.0)];
        for ((h, p), s) in heights.iter().zip(&pins).zip(&hints) {
            if let Some(s) = s {
                fixed[*s].0 += h;
                fixed[*s].1 += p;
            }
        }
        let open: Vec<usize> = (0..units.len()).filter(|i| hints[*i].is_none()).collect();
        let mut best = (f64::MAX, 0);
        for k in 0..=open.len() {
            let sum = |s: &[usize], f: &Vec<f64>| s.iter().map(|i| f[*i]).sum::<f64>();
            let left = fixed[0].0 + sum(&open[..k], &heights);
            let right = fixed[1].0 + sum(&open[k..], &heights);
            let (lp, rp) = (
                fixed[0].1 + sum(&open[..k], &pins),
                fixed[1].1 + sum(&open[k..], &pins),
            );
            let cost = left.max(right) + 0.25 * (left - right).abs() + 15.0 * (lp - rp).abs();
            if cost < best.0 - 1e-9 {
                best = (cost, k);
            }
        }
        let mut out: [Vec<Unit>; 2] = [Vec::new(), Vec::new()];
        let left_open: BTreeSet<usize> = open[..best.1].iter().copied().collect();
        for (i, u) in units.into_iter().enumerate() {
            let s = hints[i].unwrap_or(if left_open.contains(&i) { 0 } else { 1 });
            out[s].push(u);
        }
        out
    }
}

// ---------------------------------------------------------------- layout

const STUB: f64 = 20.0;
const PIERCE_LOW: f64 = 30.0; // lower row below the upper one

fn snap(v: f64) -> f64 {
    (v / 10.0).ceil() * 10.0
}

/// Horizontal room a tap (a part hanging from or standing on a row)
/// takes: its wire, its text and a gap to the next one.
fn tap_w(beside: f64) -> f64 {
    (beside + 20.0).max(36.0)
}

struct Ctx<'a> {
    scene: &'a Scene,
}

impl Ctx<'_> {
    fn el(&self, i: usize) -> &Element {
        &self.scene.elements[i]
    }

    fn value_width(&self, i: usize) -> f64 {
        self.scene.lines_width(i)
    }

    #[allow(clippy::too_many_arguments)]
    /// A part from (x, y) going `vdir` (+1 down, -1 up) over 2*HALF,
    /// leads 10 long on each side; its value beside it towards `tdir`.
    fn vertical(&self, pen: &mut Pen, i: usize, x: f64, y: f64, vdir: f64, tdir: f64, flip: bool) {
        let e = self.el(i);
        pen.open(&format!("p {}", level_class(e.level)), &e.tip);
        let (y1, y2) = (y + vdir * 10.0, y + vdir * (10.0 + 2.0 * HALF));
        pen.line(x, y, x, y1);
        // A switch's plunger on the text side.
        let flip = flip ^ (side_reach(e.sym) > 0.0 && (tdir * vdir) < 0.0);
        let (a, b) = if flip {
            ((x, y2), (x, y1))
        } else {
            ((x, y1), (x, y2))
        };
        symbol(pen, e.sym, a, b);
        let lines = self.scene.lines(i);
        let mid = (y1 + y2) / 2.0;
        let top = mid + 4.0 - (lines.len() as f64 - 1.0) * 6.5;
        let off = 13.0 + side_reach(e.sym);
        for (k, l) in lines.iter().enumerate() {
            pen.text(
                x + tdir * off,
                top + k as f64 * 13.0,
                if tdir > 0.0 {
                    Anchor::Start
                } else {
                    Anchor::End
                },
                "val",
                T_VAL,
                l,
            );
        }
        pen.close();
    }

    /// A shunt to ground hanging from (x, y).
    fn down(&self, pen: &mut Pen, i: usize, x: f64, y: f64, tdir: f64) {
        self.vertical(pen, i, x, y, 1.0, tdir, false);
        let bottom = y + 10.0 + 2.0 * HALF;
        pen.line(x, bottom, x, bottom + 6.0);
        ground(pen, x, bottom + 6.0);
    }

    /// A part up to a power symbol, standing on (x, y).
    fn up(&self, pen: &mut Pen, i: usize, net: &str, x: f64, y: f64, tdir: f64) {
        self.vertical(pen, i, x, y, -1.0, tdir, false);
        let top = y - 10.0 - 2.0 * HALF;
        pen.line(x, top, x, top - 6.0);
        power(pen, x, top - 6.0, net);
    }

    /// A part in a row from `x(c - half)` to `x(c + half)`, its value
    /// above.
    fn inline(&self, pen: &mut Pen, i: usize, a: (f64, f64), b: (f64, f64), flip: bool) {
        let e = self.el(i);
        pen.open(&format!("p {}", level_class(e.level)), &e.tip);
        let (s, t) = if flip { (b, a) } else { (a, b) };
        symbol(pen, e.sym, s, t);
        let (cx, y) = ((a.0 + b.0) / 2.0, a.1);
        for (k, l) in self.scene.lines(i).iter().rev().enumerate() {
            pen.text(
                cx,
                y - 11.0 - k as f64 * 12.0,
                Anchor::Middle,
                "val",
                T_VAL,
                l,
            );
        }
        pen.close();
    }

    /// One row from the box edge at `ox` going `dir`; its parts start
    /// at `start` (past a bridge's parts) and its wire reaches at least
    /// `reach`. Returns its end.
    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        pen: &mut Pen,
        r: &Row,
        ox: f64,
        y: f64,
        dir: f64,
        start: f64,
        reach: f64,
    ) -> f64 {
        let x = |u: f64| ox + dir * u;
        let anchor = if dir > 0.0 {
            Anchor::Start
        } else {
            Anchor::End
        };
        if !r.numbers.is_empty() {
            pen.text(x(3.0), y - 3.5, anchor, "num", T_NUM, &r.numbers.join(", "));
        }
        let numbers_w = text_width(&r.numbers.join(", "), T_NUM) + 6.0;
        let mut cur = STUB.max(start);
        if let RowEnd::Nc(level, tip) = &r.end {
            let end = STUB.max(reach);
            pen.line(x(0.0), y, x(end), y);
            pen.open(&format!("nc {}", level_class(*level)), tip);
            no_connect(pen, x(end), y);
            pen.close();
            return x(end + 5.0);
        }
        if let Some(name) = &r.link {
            cur = cur.max(numbers_w);
            let class = if self.scene.is_local(name) {
                "lbl loc"
            } else {
                "lbl"
            };
            pen.open(class, name);
            pen.text(x(cur + 4.0), y - 4.0, anchor, "", T_VAL, name);
            pen.close();
            cur += text_width(name, T_VAL) + 12.0;
        }
        // Taps: (position, what).
        enum Tap<'a> {
            Up(usize, &'a str),
            Down(usize),
        }
        let mut taps: Vec<(f64, Tap)> = Vec::new();
        if !r.up.is_empty() || !r.down.is_empty() {
            cur = cur.max(numbers_w - 6.0);
        }
        // A power symbol's name reaches back past its wire.
        let mut prev_reach = 0.0f64;
        for (i, net) in &r.up {
            let at = (cur + 12.0).max(prev_reach + power_half(net) + 8.0);
            taps.push((at, Tap::Up(*i, net)));
            prev_reach = at + power_half(net).max(self.scene.beside(*i));
            cur = at + tap_w(self.scene.beside(*i)).max(power_half(net) + 14.0) - 12.0;
        }
        for i in &r.down {
            let at = cur + 12.0;
            taps.push((at, Tap::Down(*i)));
            cur = at + tap_w(self.scene.beside(*i)) - 12.0;
        }
        let mut series_at = None;
        if let Some((i, _, flip)) = &r.series {
            let w = self.value_width(*i);
            let half = HALF.max(w / 2.0 + 4.0);
            let c = cur + 10.0 + half;
            series_at = Some((c, *i, *flip));
            cur = c + half;
            for j in &r.far_down {
                let at = cur + 14.0;
                taps.push((at, Tap::Down(*j)));
                cur = at + tap_w(self.scene.beside(*j)) - 14.0;
            }
        }
        // The end, clear of the pin numbers.
        cur = cur.max(numbers_w).max(reach);
        let tail = |pen: &mut Pen, end_u: f64| {
            // The wire, broken where the series part sits.
            match series_at {
                Some((c, _, _)) => {
                    pen.line(x(0.0), y, x(c - HALF), y);
                    if end_u > c + HALF {
                        pen.line(x(c + HALF), y, x(end_u), y);
                    }
                }
                None => pen.line(x(0.0), y, x(end_u), y),
            }
        };
        let open_end = matches!(r.end, RowEnd::Open);
        match &r.end {
            RowEnd::Label(net, lvl) => {
                let end_u = cur + 12.0;
                let (class, tip) = match lvl {
                    Some((l, t)) => (level_class(*l), t.clone()),
                    None => ("", net.clone()),
                };
                tail(pen, end_u);
                let w = flag(pen, x(end_u), y, dir, net, class, &tip);
                cur = end_u + w;
            }
            RowEnd::Local(net, lvl) => {
                let end_u = cur + 12.0;
                let tip = lvl.as_ref().map_or(net.as_str(), |(_, t)| t.as_str());
                tail(pen, end_u);
                let w = local_name(pen, x(end_u), y, dir, net, tip);
                cur = end_u + w;
            }
            RowEnd::Gnd(lvl) => {
                let end_u = cur + 12.0;
                tail(pen, end_u);
                let (class, tip) = match lvl {
                    Some((l, t)) => (level_class(*l), t.as_str()),
                    None => ("", "GND"),
                };
                pen.open(&format!("lbl {class}"), tip);
                pen.line(x(end_u), y, x(end_u), y + 8.0);
                ground(pen, x(end_u), y + 8.0);
                pen.close();
                cur = end_u + 8.0;
            }
            RowEnd::Name(name) => {
                let s = cur.max(numbers_w);
                let w = text_width(name, T_VAL);
                pen.text(x(s + 4.0), y - 4.0, anchor, "lbl", T_VAL, name);
                let end_u = s + w + 10.0;
                tail(pen, end_u);
                cur = end_u;
            }
            RowEnd::Open | RowEnd::Nc(..) => {
                // Ends at the last part: the wire turns into it.
                let mut end_u = taps.last().map_or(STUB, |(at, _)| *at);
                if let Some((c, _, _)) = series_at {
                    if r.far_down.is_empty() {
                        end_u = c + HALF;
                    }
                }
                let end_u = end_u.max(reach);
                tail(pen, end_u);
            }
        }
        let last = taps.last().map(|(at, _)| *at);
        for (at, tap) in &taps {
            if Some(*at) != last || !open_end || reach > *at {
                pen.dot(x(*at), y);
            }
            match tap {
                Tap::Up(i, net) => self.up(pen, *i, net, x(*at), y, dir),
                Tap::Down(i) => self.down(pen, *i, x(*at), y, dir),
            }
        }
        if let Some((c, i, flip)) = series_at {
            self.inline(pen, i, (x(c - HALF), y), (x(c + HALF), y), flip);
        }
        x(cur)
    }

    /// Two rows with parts between them, from `ox` going `dir`.
    fn bridge(&self, pen: &mut Pen, b: &Bridge, ox: f64, y: f64, dir: f64) {
        let x = |u: f64| ox + dir * u;
        let gap = self.scene.bridge_gap(b);
        let (yu, yl) = (y, y + gap);
        let numbers_w = [&b.upper, &b.lower]
            .iter()
            .map(|r| text_width(&r.numbers.join(", "), T_NUM) + 6.0)
            .fold(0.0, f64::max);
        let mut u = STUB.max(numbers_w) + 12.0;
        let mut cols = Vec::new();
        for i in &b.parts {
            cols.push((u, *i));
            u += tap_w(self.scene.beside(*i));
        }
        let last = cols.last().map_or(STUB, |c| c.0);
        let more = |r: &Row| !r.bare();
        for (k, (u, i)) in cols.iter().enumerate() {
            let is_last = k + 1 == cols.len();
            for (r, yy) in [(&b.upper, yu), (&b.lower, yl)] {
                if !is_last || more(r) {
                    pen.dot(x(*u), yy);
                }
            }
            // The part centred between the rows, its leads to them.
            let mid = (yu + yl) / 2.0;
            let e = self.el(*i);
            pen.open(&format!("p {}", level_class(e.level)), &e.tip);
            pen.line(x(*u), yu, x(*u), mid - HALF);
            pen.line(x(*u), mid + HALF, x(*u), yl);
            let flip = side_reach(e.sym) > 0.0 && dir < 0.0;
            let (s, t) = ((x(*u), mid - HALF), (x(*u), mid + HALF));
            let (s, t) = if flip { (t, s) } else { (s, t) };
            symbol(pen, e.sym, s, t);
            let lines = self.scene.lines(*i);
            let top = mid + 4.0 - (lines.len() as f64 - 1.0) * 6.5;
            for (k, l) in lines.iter().enumerate() {
                pen.text(
                    x(*u + 13.0 + side_reach(e.sym)),
                    top + k as f64 * 13.0,
                    if dir > 0.0 {
                        Anchor::Start
                    } else {
                        Anchor::End
                    },
                    "val",
                    T_VAL,
                    l,
                );
            }
            pen.close();
        }
        // The rows' own parts beyond the bridge's text.
        let start = cols
            .last()
            .map_or(STUB, |(u, i)| u + self.scene.beside(*i) + 8.0);
        self.row(pen, &b.upper, ox, yu, dir, start, last);
        self.row(pen, &b.lower, ox, yl, dir, start, last);
    }

    /// A crystal between two rows, from `ox` going `dir`.
    fn pierce(&self, pen: &mut Pen, p: &Pierce, ox: f64, y: f64, dir: f64) -> f64 {
        let x = |u: f64| ox + dir * u;
        let start = if dir > 0.0 {
            Anchor::Start
        } else {
            Anchor::End
        };
        let (bias_dy, drop) = self.scene.pierce_depths(p);
        let (yu, yl) = (y, y + PIERCE_LOW);
        let yb = yl + drop;
        for (nums, yy) in [(&p.upper.1, yu), (&p.lower.1, yl)] {
            if !nums.is_empty() {
                pen.text(x(3.0), yy - 3.5, start, "num", T_NUM, &nums.join(", "));
            }
        }
        // Lower row: pin, series part, then down to the crystal line.
        let mut cur = STUB;
        if let Some(i) = p.lower.2 {
            let w = self.value_width(i);
            let half = HALF.max(w / 2.0 + 4.0);
            let c = cur + 10.0 + half;
            self.inline(pen, i, (x(c - HALF), yl), (x(c + HALF), yl), false);
            pen.line(x(0.0), yl, x(c - HALF), yl);
            cur = c + HALF;
        }
        let lower_caps_w = p
            .lower
            .3
            .iter()
            .map(|i| self.value_width(*i))
            .fold(0.0, f64::max);
        let xb = (cur + 20.0).max(STUB + lower_caps_w + 26.0);
        pen.line(
            x(if p.lower.2.is_some() { cur } else { 0.0 }),
            yl,
            x(xb),
            yl,
        );
        pen.line(x(xb), yl, x(xb), yb);
        // The crystal (and what is across it).
        let cw = self.value_width(p.crystal);
        let bw = p
            .bias
            .iter()
            .map(|i| self.value_width(*i))
            .fold(0.0, f64::max);
        let span = (2.0 * HALF + 40.0).max(cw + 30.0).max(bw + 30.0);
        let xa = xb + span;
        let c = (xa + xb) / 2.0;
        pen.line(x(0.0), yu, x(xa), yu);
        pen.line(x(xa), yu, x(xa), yb);
        for (k, i) in p.bias.iter().enumerate() {
            let yr = yl + bias_dy + 30.0 * k as f64;
            pen.dot(x(xb), yr);
            pen.dot(x(xa), yr);
            pen.line(x(xb), yr, x(c - HALF), yr);
            pen.line(x(c + HALF), yr, x(xa), yr);
            self.inline(pen, *i, (x(c - HALF), yr), (x(c + HALF), yr), false);
        }
        pen.line(x(xb), yb, x(c - HALF), yb);
        pen.line(x(c + HALF), yb, x(xa), yb);
        let e = self.el(p.crystal);
        pen.open(&format!("p {}", level_class(e.level)), &e.tip);
        let (l, r) = ((x(c - HALF), yb), (x(c + HALF), yb));
        // The crystal's first terminal on its own side.
        let (s, t) = if p.first_lower { (l, r) } else { (r, l) };
        symbol(pen, Sym::Crystal, s, t);
        for (k, line) in self.scene.lines(p.crystal).iter().rev().enumerate() {
            pen.text(
                x(c),
                yb - 12.0 - k as f64 * 12.0,
                Anchor::Middle,
                "val",
                T_VAL,
                line,
            );
        }
        if p.grounded {
            pen.line(x(c), yb + 8.0, x(c), yb + 20.0);
            ground(pen, x(c), yb + 20.0);
        }
        pen.close();
        // Load capacitors: the outer side's text outside.
        for (k, i) in p.lower.3.iter().enumerate() {
            if k == 0 {
                pen.dot(x(xb), yb);
            }
            self.down(pen, *i, x(xb - k as f64 * 30.0), yb, -dir);
        }
        let mut end = xa;
        for (k, i) in p.upper.3.iter().enumerate() {
            if k == 0 {
                pen.dot(x(xa), yb);
            }
            self.down(pen, *i, x(xa + k as f64 * 30.0), yb, dir);
            end = xa + k as f64 * 30.0 + self.scene.beside(*i);
        }
        x(end)
    }

    /// A box with rows on its sides, ground pins below and power pins
    /// above; its top-left corner at the origin.
    fn host(
        &self,
        title: &[String],
        title_tip: Option<(&str, &str)>,
        sides: &[Vec<Unit>; 2],
        bottom: &[GroundPin],
        top: &[(String, String)],
    ) -> Pen {
        let mut pen = Pen::default();
        let side_w = |s: &Vec<Unit>| {
            s.iter()
                .flat_map(|u| u.pins())
                .map(|p| text_width(p, T_NAME))
                .fold(0.0, f64::max)
        };
        let (lw, rw) = (side_w(&sides[0]), side_w(&sides[1]));
        let title_w = title
            .iter()
            .map(|t| text_width(t, T_HEAD))
            .fold(0.0, f64::max);
        // Ground pins below: each wide enough for its numbers.
        let slots: Vec<f64> = bottom
            .iter()
            .map(|(_, nums, _)| (text_width(&nums.join(","), T_NUM) + 14.0).max(30.0))
            .collect();
        let bottom_w = slots.iter().sum::<f64>() + 10.0;
        let top_w = top
            .iter()
            .map(|(p, _)| (text_width(p, T_NAME) + 10.0).max(30.0))
            .sum::<f64>()
            + 10.0;
        let width = snap(
            (lw + rw + 30.0)
                .max(title_w + 24.0)
                .max(bottom_w)
                .max(top_w)
                .max(70.0),
        );
        // Pin rows.
        let head = 18.0 + title.len() as f64 * 15.0 + if top.is_empty() { 0.0 } else { 14.0 };
        let mut ys: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
        let mut last = 0.0f64;
        for (s, units) in sides.iter().enumerate() {
            let mut y = head;
            let mut prev_below: Option<f64> = None;
            for u in units {
                let (above, below) = self.scene.extent(u);
                if let Some(pb) = prev_below {
                    y += snap((pb + above).max(20.0));
                } else {
                    y = y.max(snap(head + 6.0));
                }
                ys[s].push(y);
                let lowest = match u {
                    Unit::Pierce(_) => y + PIERCE_LOW,
                    Unit::Bridge(b) => y + self.scene.bridge_gap(b),
                    Unit::Row(_) => y,
                };
                last = last.max(lowest);
                prev_below = Some(below - (lowest - y));
            }
        }
        let bottom_names = bottom
            .iter()
            .map(|(p, _, _)| text_width(p, T_NAME))
            .fold(0.0, f64::max);
        let height = snap(
            last + 20.0
                + if bottom.is_empty() {
                    0.0
                } else {
                    bottom_names + 10.0
                },
        );
        let height = height.max(snap(head + 10.0));
        // The box.
        if let Some((class, tip)) = title_tip {
            pen.open(class, tip);
        }
        pen.rect(0.0, 0.0, width, height, "host b");
        for (k, t) in title.iter().enumerate() {
            let y = 18.0 + if top.is_empty() { 0.0 } else { 14.0 } + k as f64 * 15.0;
            pen.text(
                width / 2.0,
                y,
                Anchor::Middle,
                if k == 0 { "head" } else { "val" },
                if k == 0 { T_HEAD } else { T_VAL },
                t,
            );
        }
        if title_tip.is_some() {
            pen.close();
        }
        for (s, units) in sides.iter().enumerate() {
            let (ox, dir, a) = if s == 0 {
                (0.0, -1.0, Anchor::Start)
            } else {
                (width, 1.0, Anchor::End)
            };
            let name_x = if s == 0 { 5.0 } else { width - 5.0 };
            for (u, y) in units.iter().zip(&ys[s]) {
                match u {
                    Unit::Row(r) => {
                        pen.text(name_x, y + 3.5, a, "pn", T_NAME, &r.pin);
                        self.row(&mut pen, r, ox, *y, dir, STUB, 0.0);
                    }
                    Unit::Pierce(p) => {
                        pen.text(name_x, y + 3.5, a, "pn", T_NAME, &p.upper.0);
                        pen.text(name_x, y + PIERCE_LOW + 3.5, a, "pn", T_NAME, &p.lower.0);
                        self.pierce(&mut pen, p, ox, *y, dir);
                    }
                    Unit::Bridge(b) => {
                        let gap = self.scene.bridge_gap(b);
                        pen.text(name_x, y + 3.5, a, "pn", T_NAME, &b.upper.pin);
                        pen.text(name_x, y + gap + 3.5, a, "pn", T_NAME, &b.lower.pin);
                        self.bridge(&mut pen, b, ox, *y, dir);
                    }
                }
            }
        }
        // Ground pins below (numbers right of each stub), power pins above.
        let x_start = width / 2.0 - slots.iter().sum::<f64>() / 2.0;
        let mut prefix = 0.0;
        for (k, (pin, nums, lvl)) in bottom.iter().enumerate() {
            let x = x_start + prefix + (slots[k] / 2.0).min(15.0);
            prefix += slots[k];
            pen.text_up(x + 3.5, height - 5.0, "pn", T_NAME, pin);
            if !nums.is_empty() {
                pen.text(
                    x + 4.0,
                    height + 10.0,
                    Anchor::Start,
                    "num",
                    T_NUM,
                    &nums.join(","),
                );
            }
            pen.line(x, height, x, height + STUB);
            let (class, tip) = match lvl {
                Some((l, t)) => (level_class(*l), t.as_str()),
                None => ("", "GND"),
            };
            pen.open(&format!("lbl {class}"), tip);
            ground(&mut pen, x, height + STUB);
            pen.close();
        }
        let tslots: Vec<f64> = top
            .iter()
            .map(|(p, net)| {
                (text_width(p, T_NAME) + 10.0)
                    .max(text_width(net, T_VAL) + 10.0)
                    .max(30.0)
            })
            .collect();
        let mut x =
            width / 2.0 - (tslots.iter().sum::<f64>() - tslots.first().unwrap_or(&0.0)) / 2.0;
        for (k, (pin, net)) in top.iter().enumerate() {
            if k > 0 {
                x += (tslots[k - 1] + tslots[k]) / 2.0;
            }
            pen.text(x, 13.0, Anchor::Middle, "pn", T_NAME, pin);
            pen.line(x, 0.0, x, -STUB);
            power(&mut pen, x, -STUB, net);
        }
        pen
    }

    fn free(&self, f: &Free) -> Pen {
        let mut pen = Pen::default();
        match f {
            Free::Part(i, a, b, la, lb) => {
                let e = self.el(*i);
                if e.sym == Sym::Block || e.terms.is_empty() {
                    let mut lines = e.value.clone();
                    if lines.is_empty() {
                        lines.push(e.role.clone());
                    }
                    let w = lines
                        .iter()
                        .enumerate()
                        .map(|(k, l)| text_width(l, if k == 0 { T_HEAD } else { T_VAL }))
                        .fold(0.0, f64::max)
                        + 24.0;
                    let h = 14.0 + lines.len() as f64 * 15.0;
                    pen.open(&format!("p {}", level_class(e.level)), &e.tip);
                    pen.rect(0.0, 0.0, w, h, "host b");
                    for (k, l) in lines.iter().enumerate() {
                        pen.text(
                            w / 2.0,
                            20.0 + k as f64 * 15.0,
                            Anchor::Middle,
                            if k == 0 { "head" } else { "val" },
                            if k == 0 { T_HEAD } else { T_VAL },
                            l,
                        );
                    }
                    pen.close();
                    return pen;
                }
                match (a, b) {
                    (_, At::Gnd) | (At::Gnd, _) => {
                        // A shunt: power above, ground below.
                        let flip = matches!(a, At::Gnd);
                        let net = if flip { lb } else { la };
                        pen.line(0.0, 0.0, 0.0, 6.0);
                        power(&mut pen, 0.0, 0.0, net);
                        self.vertical(&mut pen, *i, 0.0, 6.0, 1.0, 1.0, flip);
                        let bottom = 6.0 + 10.0 + 2.0 * HALF;
                        pen.line(0.0, bottom, 0.0, bottom + 6.0);
                        ground(&mut pen, 0.0, bottom + 6.0);
                    }
                    _ => {
                        let w = self.value_width(*i);
                        let half = HALF.max(w / 2.0 + 4.0);
                        let label = |at: &At, l: &str, x: f64, dir: f64, pen: &mut Pen| match at {
                            At::Net(net) if self.scene.is_local(net) => {
                                local_name(pen, x, 0.0, dir, l, l);
                            }
                            _ => {
                                flag(pen, x, 0.0, dir, l, "", l);
                            }
                        };
                        label(a, la, 0.0, -1.0, &mut pen);
                        pen.line(0.0, 0.0, 10.0 + half - HALF, 0.0);
                        self.inline(
                            &mut pen,
                            *i,
                            (10.0 + half - HALF, 0.0),
                            (10.0 + half + HALF, 0.0),
                            false,
                        );
                        let end = 20.0 + 2.0 * half;
                        pen.line(10.0 + half + HALF, 0.0, end, 0.0);
                        label(b, lb, end, 1.0, &mut pen);
                    }
                }
            }
            Free::Block(i, [left, right], top) => {
                let e = self.el(*i);
                let mut title = e.value.clone();
                if title.is_empty() {
                    title.push(e.role.clone());
                }
                let gnd: Vec<GroundPin> = left
                    .iter()
                    .filter(|r| matches!(r.end, RowEnd::Gnd(_)))
                    .map(|r| (r.pin.clone(), Vec::new(), None))
                    .collect();
                let side = |rows: &Vec<Row>| -> Vec<Unit> {
                    rows.iter()
                        .filter(|r| !matches!(r.end, RowEnd::Gnd(_)))
                        .cloned()
                        .map(Unit::Row)
                        .collect()
                };
                let class = format!("p {}", level_class(e.level));
                return self.host(
                    &title,
                    Some((&class, &e.tip)),
                    &[side(left), side(right)],
                    &gnd,
                    top,
                );
            }
        }
        pen
    }
}

/// The scene as an SVG document. `standalone`: also styled for a dark
/// colour scheme on its own (inlined, it follows the page).
pub fn render(scene: &Scene, standalone: bool) -> String {
    render_sections(
        &[Section {
            title: None,
            scene: scene.clone(),
        }],
        standalone,
    )
}

/// Sections stacked, each under its title; empty when nothing is drawn.
pub fn render_sections(sections: &[Section], standalone: bool) -> String {
    render_all(sections).map_or_else(String::new, |pen| finish(pen, standalone))
}

/// What is wrong with the drawing (estimated text widths): texts that
/// overlap each other or cross a wire or outline, and anything outside
/// the view box. The tests keep this empty for every circuit in the
/// data.
pub fn problems(sections: &[Section]) -> Vec<String> {
    let Some(pen) = render_all(sections) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, (a, ta)) in pen.texts.iter().enumerate() {
        for (b, tb) in &pen.texts[i + 1..] {
            if a.overlaps(b) {
                out.push(format!("text {ta:?} overlaps text {tb:?}"));
            }
        }
        let inner = a.shrunk(0.6);
        for (p, q) in &pen.segs {
            if inner.crossed_by(*p, *q) {
                out.push(format!(
                    "text {ta:?} crosses a line ({}, {})-({}, {})",
                    n(p.0),
                    n(p.1),
                    n(q.0),
                    n(q.1)
                ));
                break;
            }
        }
    }
    let view = view_box(&pen);
    for (b, t) in &pen.texts {
        if !view.contains(b) {
            out.push(format!("text {t:?} outside the view box"));
        }
    }
    for (p, q) in &pen.segs {
        let s = BBox {
            x0: p.0.min(q.0),
            y0: p.1.min(q.1),
            x1: p.0.max(q.0),
            y1: p.1.max(q.1),
        };
        if !view.contains(&s) {
            out.push("a line outside the view box".into());
        }
    }
    out
}

/// The two-pin parts drawn apart from the host (between net labels):
/// none of them touches a host pin or is placed near one.
pub fn detached(scene: &Scene) -> Vec<String> {
    scene
        .plan()
        .free
        .iter()
        .filter_map(|f| match f {
            Free::Part(i, ..) => Some(scene.elements[*i].role.clone()),
            Free::Block(..) => None,
        })
        .collect()
}

fn render_all(sections: &[Section]) -> Option<Pen> {
    let mut pen = Pen::default();
    let mut y = 0.0;
    let mut any = false;
    for s in sections {
        let Some(part) = render_pen(&s.scene) else {
            continue;
        };
        if any {
            // A rule between sections.
            y += 14.0;
        }
        let bb = part.bbox();
        if let Some(t) = &s.title {
            pen.text(0.0, y + 12.0, Anchor::Start, "sec", T_HEAD, t);
            y += 26.0;
        }
        pen.place(part, -bb.x0, y - bb.y0);
        y += bb.y1 - bb.y0 + 20.0;
        any = true;
    }
    any.then_some(pen)
}

fn render_pen(scene: &Scene) -> Option<Pen> {
    let plan = scene.plan();
    let ctx = Ctx { scene };
    let mut pen = Pen::default();
    let has_host = plan.sides.iter().any(|s| !s.is_empty()) || !plan.bottom.is_empty();
    if !has_host && plan.free.is_empty() {
        return None;
    }
    let mut y_free = 0.0;
    if has_host {
        let host = ctx.host(
            std::slice::from_ref(&scene.host),
            None,
            &plan.sides,
            &plan.bottom,
            &[],
        );
        let bb = host.bbox();
        pen.place(host, -bb.x0, -bb.y0);
        y_free = bb.y1 - bb.y0 + 34.0;
    }
    // Parts beside the host, in a line under it (wrapping).
    let max_w = pen.bbox().x1.max(560.0);
    let (mut x, mut row_h) = (0.0, 0.0f64);
    for f in &plan.free {
        let item = ctx.free(f);
        let bb = item.bbox();
        let (w, h) = (bb.x1 - bb.x0, bb.y1 - bb.y0);
        if x > 0.0 && x + w > max_w {
            x = 0.0;
            y_free += row_h + 30.0;
            row_h = 0.0;
        }
        pen.place(item, x - bb.x0, y_free - bb.y0);
        x += w + 40.0;
        row_h = row_h.max(h);
    }
    Some(pen)
}

const STYLE: &str = "\
.cd{font-family:system-ui,-apple-system,'Segoe UI',Roboto,sans-serif}\
.cd path,.cd circle{fill:none;stroke:currentColor;stroke-width:1.2;stroke-linecap:round;stroke-linejoin:round}\
.cd .b{stroke-width:1.5}\
.cd .host{stroke-width:1.6;fill:var(--color-bg-subtle,#f7f7f8)}\
.cd .dot{fill:currentColor;stroke:none}\
.cd text{fill:currentColor;stroke:none;font-size:10.5px}\
.cd .head{font-size:12px;font-weight:600}\
.cd .sec{font-size:12px;font-weight:600;fill:var(--color-text-muted,#5c5c66)}\
.cd .pn{font-size:10px}\
.cd .num{font-size:9px;fill:var(--color-text-muted,#5c5c66)}\
.cd .val{fill:var(--color-text-muted,#5c5c66)}\
.cd .loc text{font-style:italic}\
.cd .lvl-recommended{color:var(--color-accent,#2456e0)}\
.cd .lvl-example,.cd .lvl-optional{color:var(--color-text-muted,#5c5c66)}\
.cd .lvl-example .b{stroke-dasharray:3 2.2}\
.cd .lvl-optional .b{stroke-dasharray:0.5 2.6}\
.cd .p .val{fill:currentColor}\
.cd .p:hover,.cd .lbl:hover{opacity:.7}";

const DARK: &str = "\
@media (prefers-color-scheme:dark){svg.cd{color:#e4e4e8;background:#16161a}\
.cd .host{fill:#1f1f24}.cd .num,.cd .val,.cd .sec{fill:#a0a0aa}\
.cd .lvl-recommended{color:#7ea2ff}.cd .lvl-example,.cd .lvl-optional{color:#a0a0aa}}";

/// Display size per drawing unit (the drawing is laid out in CSS pixels
/// at a 10.5 px text size; shown a little larger).
const SCALE: f64 = 1.2;

/// The drawing's bounds with a margin (wider on the right, where the
/// estimated text widths may fall short).
fn view_box(pen: &Pen) -> BBox {
    let bb = pen.bbox();
    BBox {
        x0: bb.x0 - 8.0,
        y0: bb.y0 - 8.0,
        x1: bb.x1 + 14.0,
        y1: bb.y1 + 8.0,
    }
}

fn finish(pen: Pen, standalone: bool) -> String {
    let v = view_box(&pen);
    let (x, y, w, h) = (v.x0, v.y0, v.x1 - v.x0, v.y1 - v.y0);
    let mut s = String::new();
    let _ = write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"cd\" viewBox=\"{} {} {} {}\" width=\"{}\" height=\"{}\" role=\"img\"><style>{STYLE}{}</style>{}</svg>",
        n(x),
        n(y),
        n(w),
        n(h),
        n((w * SCALE).ceil()),
        n((h * SCALE).ceil()),
        if standalone { DARK } else { "" },
        pen.s
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(role: &str, sym: Sym, value: &str, level: LevelValue, terms: &[(&str, End)]) -> Element {
        Element {
            role: role.into(),
            sym,
            value: vec![value.into()],
            count: 1,
            level,
            tip: role.into(),
            terms: terms
                .iter()
                .map(|(p, e)| (p.to_string(), e.clone()))
                .collect(),
            near: None,
        }
    }
    fn pin(p: &str) -> End {
        End::Pin(p.into())
    }
    fn net(p: &str) -> End {
        End::Net(p.into())
    }
    fn tie(s: &mut Scene, pin: &str, net: Option<&str>) {
        s.connects.push(Connection {
            pin: pin.into(),
            net: net.map(Into::into),
            level: LevelValue::Required,
            tip: pin.into(),
        });
    }

    fn clean(s: &Scene) {
        let p = problems(&[Section {
            title: None,
            scene: s.clone(),
        }]);
        assert!(p.is_empty(), "{p:#?}");
    }

    fn crystal() -> Scene {
        Scene {
            host: "RP2040".into(),
            pins: vec![("20".into(), "XIN".into()), ("21".into(), "XOUT".into())],
            ground: ["GND".to_string()].into(),
            power: BTreeSet::new(),
            local: ["XTAL_OUT".to_string()].into(),
            connects: vec![],
            elements: vec![
                el(
                    "C_XIN",
                    Sym::Capacitor,
                    "15 pF",
                    LevelValue::Recommended,
                    &[("1", pin("XIN")), ("2", net("GND"))],
                ),
                el(
                    "C_XOUT",
                    Sym::Capacitor,
                    "15 pF",
                    LevelValue::Recommended,
                    &[("1", net("XTAL_OUT")), ("2", net("GND"))],
                ),
                el(
                    "R_XOUT",
                    Sym::Resistor,
                    "1 kΩ",
                    LevelValue::Recommended,
                    &[("1", pin("XOUT")), ("2", net("XTAL_OUT"))],
                ),
                el(
                    "X1",
                    Sym::Crystal,
                    "12 MHz",
                    LevelValue::Recommended,
                    &[
                        ("GND", net("GND")),
                        ("XTAL1", pin("XIN")),
                        ("XTAL2", net("XTAL_OUT")),
                    ],
                ),
            ],
        }
    }

    fn decoupling() -> Scene {
        let mut s = Scene {
            host: "RP2040".into(),
            pins: vec![
                ("1".into(), "IOVDD".into()),
                ("10".into(), "IOVDD".into()),
                ("19".into(), "TESTEN".into()),
                ("23".into(), "DVDD".into()),
                ("44".into(), "VREG_VIN".into()),
                ("45".into(), "VREG_VOUT".into()),
                ("46".into(), "USB_DM".into()),
                ("57".into(), "GND".into()),
            ],
            ground: ["GND".to_string()].into(),
            power: ["VDD_3V3".to_string(), "VDD_CORE".to_string()].into(),
            local: BTreeSet::new(),
            connects: vec![],
            elements: vec![],
        };
        for (p, netn, lvl) in [
            ("IOVDD", Some("VDD_3V3"), LevelValue::Example),
            ("DVDD", Some("VDD_CORE"), LevelValue::Required),
            ("TESTEN", Some("GND"), LevelValue::Required),
            ("GND", Some("GND"), LevelValue::Required),
            ("VREG_VIN", Some("VDD_3V3"), LevelValue::Required),
            ("VREG_VOUT", Some("VDD_CORE"), LevelValue::Required),
        ] {
            s.connects.push(Connection {
                pin: p.into(),
                net: netn.map(Into::into),
                level: lvl,
                tip: p.into(),
            });
        }
        let mut c = el(
            "C_IOVDD",
            Sym::Capacitor,
            "100 nF",
            LevelValue::Recommended,
            &[("1", pin("IOVDD")), ("2", net("GND"))],
        );
        c.count = 2;
        s.elements.push(c);
        s.elements.push(el(
            "C_VREG_VIN",
            Sym::Capacitor,
            "1 µF",
            LevelValue::Required,
            &[("1", pin("VREG_VIN")), ("2", net("GND"))],
        ));
        s.elements.push(el(
            "C_VREG_VOUT",
            Sym::Capacitor,
            "1 µF",
            LevelValue::Required,
            &[("1", pin("VREG_VOUT")), ("2", net("GND"))],
        ));
        s.elements.push(el(
            "R_DM",
            Sym::Resistor,
            "27 Ω",
            LevelValue::Required,
            &[("1", pin("USB_DM")), ("2", net("USB_DM"))],
        ));
        s.elements.push(el(
            "C_BULK",
            Sym::Capacitor,
            "10 µF",
            LevelValue::Example,
            &[("1", net("VDD_3V3")), ("2", net("GND"))],
        ));
        s
    }

    fn side_of(plan: &Plan, pin: &str) -> Option<usize> {
        plan.sides
            .iter()
            .position(|s| s.iter().any(|u| u.pins().contains(&pin)))
    }

    #[test]
    fn crystal_is_a_pierce_network() {
        let s = crystal();
        let plan = s.plan();
        assert!(plan.free.is_empty());
        assert!(
            matches!(plan.sides[1].as_slice(), [Unit::Pierce(p)] if p.upper.0 == "XIN" && p.lower.0 == "XOUT" && p.lower.2 == Some(2) && p.upper.3 == vec![0] && p.lower.3 == vec![1] && p.grounded)
        );
        clean(&s);
        let svg = render(&s, false);
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        assert!(svg.contains(">12 MHz<") && svg.contains(">1 kΩ<"));
        assert_eq!(svg, render(&s, false), "deterministic");
    }

    #[test]
    fn a_bias_resistor_sits_across_the_crystal() {
        let mut s = crystal();
        s.elements.push(el(
            "R_BIAS",
            Sym::Resistor,
            "1 MΩ",
            LevelValue::Optional,
            &[("1", pin("XIN")), ("2", pin("XOUT"))],
        ));
        let plan = s.plan();
        assert!(plan.free.is_empty());
        assert!(matches!(plan.sides[1].as_slice(), [Unit::Pierce(p)] if p.bias == vec![4]));
        clean(&s);
    }

    #[test]
    fn decoupling_rows_and_labels() {
        let s = decoupling();
        let plan = s.plan();
        // Ground-only pins at the bottom; VIN left, VOUT right.
        let bottom: Vec<&str> = plan.bottom.iter().map(|b| b.0.as_str()).collect();
        assert_eq!(bottom, ["TESTEN", "GND"]);
        assert_eq!(side_of(&plan, "VREG_VIN"), Some(0));
        assert_eq!(side_of(&plan, "VREG_VOUT"), Some(1));
        // The bulk cap on a net several pins share hangs from one of them.
        assert!(plan.free.is_empty());
        clean(&s);
        let svg = render(&s, true);
        assert!(svg.contains("100 nF ×2"));
        assert!(svg.contains("1, 10"), "both IOVDD pin numbers");
        assert!(svg.contains("prefers-color-scheme"));
        assert!(svg.contains("lvl-example"));
    }

    #[test]
    fn net_parts_hang_from_the_pin_they_are_near() {
        let mut s = decoupling();
        s.elements[4].near = Some(Near::Pin("VREG_VIN".into()));
        let plan = s.plan();
        let row = plan
            .sides
            .iter()
            .flatten()
            .find_map(|u| match u {
                Unit::Row(r) if r.pin == "VREG_VIN" => Some(r),
                _ => None,
            })
            .unwrap();
        assert_eq!(row.down, vec![1, 4]);
    }

    #[test]
    fn parts_between_two_pins_bridge_their_rows() {
        let mut s = Scene {
            host: "ESP32".into(),
            pins: vec![
                ("47".into(), "CAP2".into()),
                ("48".into(), "CAP1".into()),
                ("49".into(), "VDDA".into()),
            ],
            ground: ["GND".to_string()].into(),
            power: ["VDD".to_string()].into(),
            ..Scene::default()
        };
        tie(&mut s, "VDDA", Some("VDD"));
        s.elements = vec![
            el(
                "C_CAP1",
                Sym::Capacitor,
                "10 nF",
                LevelValue::Required,
                &[("1", pin("CAP1")), ("2", net("GND"))],
            ),
            el(
                "C_CAP2",
                Sym::Capacitor,
                "3.3 nF",
                LevelValue::Optional,
                &[("1", pin("CAP2")), ("2", pin("CAP1"))],
            ),
            el(
                "R_CAP2",
                Sym::Resistor,
                "20 kΩ",
                LevelValue::Optional,
                &[("1", pin("CAP2")), ("2", pin("CAP1"))],
            ),
        ];
        let plan = s.plan();
        assert!(plan.free.is_empty());
        let b = plan
            .sides
            .iter()
            .flatten()
            .find_map(|u| match u {
                Unit::Bridge(b) => Some(b),
                _ => None,
            })
            .expect("a bridge");
        assert_eq!(
            (b.upper.pin.as_str(), b.lower.pin.as_str()),
            ("CAP2", "CAP1")
        );
        assert_eq!(b.parts, vec![1, 2]);
        assert_eq!(b.lower.down, vec![0]);
        clean(&s);
        let svg = render(&s, false);
        assert!(svg.contains(">CAP2<") && svg.contains(">CAP1<"));
    }

    #[test]
    fn a_bridge_to_a_ground_pin_keeps_its_row() {
        let mut s = Scene {
            host: "LPC845".into(),
            pins: vec![
                ("8".into(), "VSS".into()),
                ("42".into(), "VREFP".into()),
                ("43".into(), "VREFN".into()),
            ],
            ground: ["GND".to_string()].into(),
            power: ["VDD".to_string()].into(),
            ..Scene::default()
        };
        tie(&mut s, "VSS", Some("GND"));
        tie(&mut s, "VREFN", Some("GND"));
        tie(&mut s, "VREFP", Some("VDD"));
        s.elements = vec![el(
            "C_VREF",
            Sym::Capacitor,
            "100 nF",
            LevelValue::Example,
            &[("1", pin("VREFP")), ("2", pin("VREFN"))],
        )];
        let plan = s.plan();
        let bottom: Vec<&str> = plan.bottom.iter().map(|b| b.0.as_str()).collect();
        assert_eq!(bottom, ["VSS"]);
        assert!(plan
            .sides
            .iter()
            .flatten()
            .any(|u| matches!(u, Unit::Bridge(b) if b.parts == vec![0])));
        clean(&s);
    }

    #[test]
    fn local_nets_are_not_flags() {
        let mut s = Scene {
            host: "ESP32".into(),
            pins: vec![("3".into(), "VDD3P3".into())],
            ground: ["GND".to_string()].into(),
            power: ["VDD_3V3".to_string()].into(),
            local: ["VDD_RF".to_string()].into(),
            ..Scene::default()
        };
        tie(&mut s, "VDD3P3", Some("VDD_RF"));
        s.elements = vec![el(
            "L",
            Sym::Inductor,
            "2 nH",
            LevelValue::Recommended,
            &[("1", net("VDD_3V3")), ("2", net("VDD_RF"))],
        )];
        let plan = s.plan();
        let Unit::Row(r) = &plan.sides[1][0] else {
            panic!()
        };
        // The inductor in the row, to the supply's label.
        assert!(matches!(&r.series, Some((0, n, _)) if n == "VDD_3V3"));
        assert_eq!(r.link.as_deref(), Some("VDD_RF"));
        clean(&s);
    }

    #[test]
    fn blocks_join_by_labels() {
        let s = Scene {
            host: "MCU".into(),
            pins: vec![("1".into(), "CS".into()), ("2".into(), "CLK".into())],
            ground: ["GND".to_string()].into(),
            power: ["VDD".to_string()].into(),
            local: BTreeSet::new(),
            connects: vec![],
            elements: vec![
                el(
                    "R_PU",
                    Sym::Resistor,
                    "10 kΩ",
                    LevelValue::Optional,
                    &[("1", net("VDD")), ("2", pin("CS"))],
                ),
                Element {
                    sym: Sym::Block,
                    ..el(
                        "U1",
                        Sym::Block,
                        "Flash",
                        LevelValue::Required,
                        &[
                            ("/CS", pin("CS")),
                            ("CLK", pin("CLK")),
                            ("VCC", net("VDD")),
                            ("GND", net("GND")),
                        ],
                    )
                },
            ],
        };
        let plan = s.plan();
        assert!(
            matches!(&plan.free[..], [Free::Block(1, [rows, right], top)] if rows.len() == 3 && right.is_empty() && top.len() == 1)
        );
        clean(&s);
    }

    #[test]
    fn sections_stack_under_titles() {
        let sections = [
            Section {
                title: Some("Crystal".into()),
                scene: crystal(),
            },
            Section {
                title: Some("Nothing".into()),
                scene: Scene::default(),
            },
            Section {
                title: Some("Supply".into()),
                scene: decoupling(),
            },
        ];
        let svg = render_sections(&sections, false);
        assert!(svg.contains(">Crystal<") && svg.contains(">Supply<"));
        assert!(!svg.contains(">Nothing<"), "an empty section is left out");
        assert!(problems(&sections).is_empty(), "{:?}", problems(&sections));
    }

    #[test]
    fn segments_cross_boxes() {
        let b = BBox {
            x0: 0.0,
            y0: 0.0,
            x1: 10.0,
            y1: 10.0,
        };
        assert!(b.crossed_by((-5.0, 5.0), (15.0, 5.0)));
        assert!(b.crossed_by((5.0, 5.0), (6.0, 6.0)));
        assert!(!b.crossed_by((-5.0, 12.0), (15.0, 12.0)));
        assert!(!b.crossed_by((-5.0, -5.0), (-1.0, 20.0)));
    }

    #[test]
    fn nothing_to_draw() {
        assert_eq!(render(&Scene::default(), false), "");
    }
}
