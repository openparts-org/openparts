//! Where PCBCraft draws a symbol unit's labels, and whether any two
//! collide.
//!
//! PCBCraft generates each pin's lead, number and name itself
//! (`PinAppearance::graphics` in its `schematic/pins.rs`) and letters all
//! text with its bundled stroke font, "PCBcraft Technical"
//! (`board/silkscreen/technical.rs`): proportional, about 0.83 x the
//! height per capital. The boxes here follow those rules and the font's
//! own metrics, so a layout can be checked before it is written: no
//! label may touch another label, the body outline or another pin's
//! lead.

use crate::record::{Geometry, Pin, Shape, Unit};

/// The font's capital height, in its own units.
const CAP: f64 = 680.0;

/// Printable ASCII from `' '`: advance, then the ink's x range and its
/// y range downward from the capital top, in font units. Derived from
/// PCBCraft's `assets/fonts/pcbcraft-technical/strokes.json` (an
/// OFL-1.1 derivative of Relief SingleLine). Other characters are drawn
/// as `?`, as PCBCraft does.
#[rustfmt::skip]
const GLYPHS: [(f64, Option<[f64; 4]>); 95] = [
    (320.0, None), // space
    (341.0, Some([130.0, 211.0, 0.0, 690.0])), // !
    (393.0, Some([80.0, 333.0, -65.0, 144.0])), // "
    (611.0, Some([60.0, 551.0, 0.0, 680.0])), // #
    (566.0, Some([95.0, 476.0, -138.0, 818.0])), // $
    (717.0, Some([70.0, 647.0, 0.0, 680.0])), // %
    (685.0, Some([100.0, 605.0, 0.0, 680.0])), // &
    (218.0, Some([80.0, 158.0, -65.0, 144.0])), // '
    (300.0, Some([100.0, 230.0, -51.0, 796.0])), // (
    (300.0, Some([70.0, 200.0, -51.0, 796.0])), // )
    (432.0, Some([60.0, 372.0, 0.0, 295.0])), // *
    (635.0, Some([85.0, 550.0, 120.0, 585.0])), // +
    (258.0, Some([90.0, 168.0, 613.0, 822.0])), // ,
    (417.0, Some([100.0, 317.0, 402.0, 402.0])), // -
    (301.0, Some([110.0, 191.0, 608.0, 690.0])), // .
    (407.0, Some([50.0, 357.0, -10.0, 690.0])), // /
    (622.0, Some([90.0, 532.0, 0.0, 680.0])), // 0
    (448.0, Some([70.0, 288.0, 10.0, 680.0])), // 1
    (550.0, Some([65.0, 465.0, 0.0, 680.0])), // 2
    (566.0, Some([75.0, 456.0, 0.0, 681.0])), // 3
    (569.0, Some([65.0, 509.0, 10.0, 680.0])), // 4
    (558.0, Some([95.0, 467.0, 10.0, 680.0])), // 5
    (567.0, Some([90.0, 492.0, 0.0, 680.0])), // 6
    (495.0, Some([40.0, 445.0, 10.0, 680.0])), // 7
    (559.0, Some([90.0, 469.0, 0.0, 680.0])), // 8
    (567.0, Some([75.0, 477.0, 0.0, 680.0])), // 9
    (301.0, Some([110.0, 191.0, 145.0, 690.0])), // :
    (294.0, Some([110.0, 204.0, 145.0, 822.0])), // ;
    (652.0, Some([100.0, 552.0, 158.0, 630.0])), // <
    (554.0, Some([100.0, 454.0, 258.0, 458.0])), // =
    (652.0, Some([100.0, 552.0, 158.0, 630.0])), // >
    (493.0, Some([70.0, 383.0, 0.0, 690.0])), // ?
    (862.0, Some([95.0, 767.0, 48.0, 766.0])), // @
    (676.0, Some([70.0, 606.0, 5.0, 680.0])), // A
    (634.0, Some([140.0, 534.0, 10.0, 670.0])), // B
    (665.0, Some([95.0, 580.0, 0.0, 680.0])), // C
    (693.0, Some([140.0, 608.0, 10.0, 670.0])), // D
    (591.0, Some([140.0, 501.0, 10.0, 670.0])), // E
    (551.0, Some([140.0, 481.0, 10.0, 680.0])), // F
    (676.0, Some([95.0, 566.0, 0.0, 680.0])), // G
    (706.0, Some([140.0, 566.0, 0.0, 680.0])), // H
    (280.0, Some([139.0, 141.0, 0.0, 680.0])), // I
    (361.0, Some([50.0, 231.0, 0.0, 680.0])), // J
    (577.0, Some([140.0, 527.0, 0.0, 680.0])), // K
    (508.0, Some([140.0, 458.0, 0.0, 670.0])), // L
    (862.0, Some([140.0, 722.0, 5.0, 680.0])), // M
    (740.0, Some([140.0, 600.0, 0.0, 680.0])), // N
    (716.0, Some([95.0, 621.0, 0.0, 680.0])), // O
    (590.0, Some([140.0, 525.0, 10.0, 680.0])), // P
    (716.0, Some([95.0, 621.0, 0.0, 818.0])), // Q
    (604.0, Some([140.0, 519.0, 10.0, 680.0])), // R
    (566.0, Some([95.0, 476.0, 0.0, 680.0])), // S
    (520.0, Some([40.0, 480.0, 10.0, 680.0])), // T
    (690.0, Some([126.0, 560.0, 0.0, 680.0])), // U
    (641.0, Some([70.0, 571.0, 0.0, 675.0])), // V
    (910.0, Some([80.0, 830.0, 0.0, 675.0])), // W
    (607.0, Some([70.0, 537.0, 0.0, 680.0])), // X
    (575.0, Some([50.0, 525.0, 0.0, 680.0])), // Y
    (609.0, Some([110.0, 509.0, 10.0, 670.0])), // Z
    (295.0, Some([100.0, 225.0, -51.0, 796.0])), // [
    (407.0, Some([50.0, 357.0, -10.0, 690.0])), // \\
    (295.0, Some([70.0, 195.0, -51.0, 796.0])), // ]
    (438.0, Some([60.0, 378.0, 0.0, 319.0])), // ^
    (410.0, Some([10.0, 400.0, 835.0, 835.0])), // _
    (155.0, Some([10.0, 145.0, -111.0, 22.0])), // `
    (561.0, Some([90.0, 441.0, 125.0, 680.0])), // a
    (611.0, Some([138.0, 521.0, -65.0, 680.0])), // b
    (531.0, Some([90.0, 446.0, 125.0, 680.0])), // c
    (613.0, Some([90.0, 473.0, -65.0, 680.0])), // d
    (561.0, Some([90.0, 476.0, 125.0, 680.0])), // e
    (333.0, Some([50.0, 328.0, -65.0, 680.0])), // f
    (608.0, Some([90.0, 473.0, 125.0, 870.0])), // g
    (608.0, Some([140.0, 473.0, -65.0, 680.0])), // h
    (280.0, Some([139.0, 141.0, -105.0, 680.0])), // i
    (293.0, Some([0.0, 158.0, -105.0, 870.0])), // j
    (528.0, Some([140.0, 478.0, -65.0, 680.0])), // k
    (307.0, Some([135.0, 252.0, -65.0, 680.0])), // l
    (941.0, Some([140.0, 806.0, 125.0, 680.0])), // m
    (608.0, Some([140.0, 473.0, 125.0, 680.0])), // n
    (592.0, Some([90.0, 502.0, 125.0, 680.0])), // o
    (613.0, Some([140.0, 523.0, 125.0, 870.0])), // p
    (613.0, Some([90.0, 473.0, 125.0, 870.0])), // q
    (394.0, Some([140.0, 338.0, 125.0, 680.0])), // r
    (521.0, Some([90.0, 436.0, 125.0, 680.0])), // s
    (387.0, Some([50.0, 327.0, 20.0, 680.0])), // t
    (603.0, Some([135.0, 463.0, 125.0, 680.0])), // u
    (539.0, Some([60.0, 479.0, 125.0, 675.0])), // v
    (817.0, Some([70.0, 747.0, 125.0, 675.0])), // w
    (540.0, Some([70.0, 470.0, 125.0, 680.0])), // x
    (549.0, Some([70.0, 489.0, 125.0, 870.0])), // y
    (530.0, Some([90.0, 450.0, 135.0, 670.0])), // z
    (381.0, Some([100.0, 331.0, -34.0, 804.0])), // {
    (280.0, Some([139.0, 141.0, -65.0, 870.0])), // |
    (381.0, Some([50.0, 281.0, -34.0, 804.0])), // }
    (645.0, Some([100.0, 545.0, 324.0, 396.0])), // ~
];

/// PCBCraft's stroke for pin graphics, and so for their lettering.
pub(crate) const PIN_STROKE_NM: i64 = 120_000;
/// The most room asked around a pin number.
const NUMBER_CLEARANCE_NM: i64 = 250_000;
/// The outline stroke assumed for body rectangles.
const BODY_STROKE_NM: i64 = 150_000;

/// An axis-aligned box, nm, PCBCraft frame (Y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rect {
    pub x0: i64,
    pub y0: i64,
    pub x1: i64,
    pub y1: i64,
}

impl Rect {
    fn grow(self, d: i64) -> Rect {
        Rect {
            x0: self.x0 - d,
            y0: self.y0 - d,
            x1: self.x1 + d,
            y1: self.y1 + d,
        }
    }
    pub fn hits(&self, o: &Rect, clearance: i64) -> bool {
        self.x0 < o.x1 + clearance
            && o.x0 < self.x1 + clearance
            && self.y0 < o.y1 + clearance
            && o.y0 < self.y1 + clearance
    }
    fn segment(a: [i64; 2], b: [i64; 2], stroke: i64) -> Rect {
        Rect {
            x0: a[0].min(b[0]),
            y0: a[1].min(b[1]),
            x1: a[0].max(b[0]),
            y1: a[1].max(b[1]),
        }
        .grow(stroke / 2)
    }
}

/// The ink box of `text` lettered `height_nm` high with a pin stroke,
/// anchored at `at` (its vertical centre), aligned along the text, and
/// turned `rotation` degrees (0 or 270).
pub(crate) fn text_box(
    text: &str,
    height_nm: i64,
    at: [i64; 2],
    align: &str,
    rotation: u16,
) -> Rect {
    let stroke = PIN_STROKE_NM.min(height_nm / 3).max(1);
    let scale = (height_nm - stroke) as f64 / CAP;
    let gap = (stroke + height_nm / 6) as f64;
    let (mut pen, mut right): (f64, Option<f64>) = (0.0, None);
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for c in text.chars() {
        let i = c as usize;
        let (advance, ink) = if (32..127).contains(&i) {
            GLYPHS[i - 32]
        } else {
            GLYPHS[usize::from(b'?') - 32]
        };
        if let Some([l, r, t, b]) = ink {
            if let Some(previous) = right {
                pen = pen.max(previous + gap - l * scale);
            }
            x0 = x0.min(pen + l * scale);
            x1 = x1.max(pen + r * scale);
            y0 = y0.min(t * scale);
            y1 = y1.max(b * scale);
            right = Some(pen + r * scale);
        }
        pen += advance * scale;
    }
    if x0 > x1 {
        return Rect {
            x0: at[0],
            y0: at[1],
            x1: at[0],
            y1: at[1],
        };
    }
    let width = (x1 - x0).round() as i64;
    let (along0, along1) = match align {
        "start" => (0, width),
        "end" => (-width, 0),
        _ => (-width / 2, width - width / 2),
    };
    // Lettering puts the capital top half a stroke below 0; PCBCraft
    // then centres the text's nominal height on the anchor.
    let top = (stroke as f64 / 2.0 + y0).round() as i64 - height_nm / 2;
    let bottom = (stroke as f64 / 2.0 + y1).round() as i64 - height_nm / 2;
    let local = match rotation {
        // x along the text, y across it.
        0 => Rect {
            x0: at[0] + along0,
            y0: at[1] + top,
            x1: at[0] + along1,
            y1: at[1] + bottom,
        },
        // (x, y) -> (y, -x): the text runs up the page, its top to the left.
        270 => Rect {
            x0: at[0] + top,
            y0: at[1] - along1,
            x1: at[0] + bottom,
            y1: at[1] - along0,
        },
        _ => unreachable!("only upright and quarter-turned text is drawn"),
    };
    local.grow(stroke / 2)
}

/// The body end of a pin's lead.
pub(crate) fn lead_end(pin: &Pin) -> [i64; 2] {
    let [x, y] = pin.at_nm;
    let l = pin.appearance.length_nm;
    match pin.appearance.orientation {
        "right" => [x + l, y],
        "left" => [x - l, y],
        "down" => [x, y + l],
        _ => [x, y - l],
    }
}

/// What a unit draws: per item its box and the pin it belongs to.
struct Item {
    rect: Rect,
    pin: Option<usize>,
    kind: Kind,
    /// A pin number (else a name or other text).
    number: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Kind {
    Label,
    Body,
    Lead,
}

fn items(unit: &Unit, name_of: &dyn Fn(&str) -> Option<String>) -> Vec<Item> {
    let mut out = Vec::new();
    for shape in &unit.drawing {
        match shape {
            Shape::Rect { min_nm, max_nm } => {
                let [x0, y0] = *min_nm;
                let [x1, y1] = *max_nm;
                for (a, b) in [
                    ([x0, y0], [x1, y0]),
                    ([x1, y0], [x1, y1]),
                    ([x0, y1], [x1, y1]),
                    ([x0, y0], [x0, y1]),
                ] {
                    out.push(Item {
                        rect: Rect::segment(a, b, BODY_STROKE_NM),
                        pin: None,
                        kind: Kind::Body,
                        number: false,
                    });
                }
            }
            Shape::Graphic { graphic } => {
                if let Geometry::Text {
                    at,
                    text,
                    height_nm,
                    rotation,
                    align,
                } = &graphic.geometry
                {
                    // A pin's name moved into the drawing belongs to the
                    // pin whose lead ends next to it.
                    let anchor = [at.x_nm, at.y_nm];
                    let pin = unit.pins.iter().position(|p| {
                        let e = lead_end(p);
                        e[0] == anchor[0] && (e[1] - anchor[1]).abs() < 1_000_000
                    });
                    out.push(Item {
                        rect: text_box(text, *height_nm, anchor, align, *rotation),
                        pin,
                        kind: Kind::Label,
                        number: false,
                    });
                }
            }
        }
    }
    for (i, p) in unit.pins.iter().enumerate() {
        let a = &p.appearance;
        let end = lead_end(p);
        out.push(Item {
            rect: Rect::segment(p.at_nm, end, PIN_STROKE_NM),
            pin: Some(i),
            kind: Kind::Lead,
            number: false,
        });
        let vertical = matches!(a.orientation, "up" | "down");
        if a.show_number {
            let mut mid = [(p.at_nm[0] + end[0]) / 2, (p.at_nm[1] + end[1]) / 2];
            let align = if vertical {
                mid[0] += 400_000;
                "start"
            } else {
                mid[1] -= 600_000;
                "middle"
            };
            out.push(Item {
                rect: text_box(&p.terminal, a.number_height_nm, mid, align, 0),
                pin: Some(i),
                kind: Kind::Label,
                number: true,
            });
        }
        if a.show_name {
            if let Some(name) = name_of(&p.terminal).filter(|n| !n.is_empty()) {
                let inset = if a.style.contains("clock") {
                    700_000
                } else {
                    400_000
                };
                let (at, align) = match a.orientation {
                    "right" => ([end[0] + inset, end[1]], "start"),
                    "left" => ([end[0] - inset, end[1]], "end"),
                    "up" => ([end[0], end[1] - 700_000], "middle"),
                    _ => ([end[0], end[1] + 700_000], "middle"),
                };
                out.push(Item {
                    rect: text_box(&name, a.name_height_nm, at, align, 0),
                    pin: Some(i),
                    kind: Kind::Label,
                    number: false,
                });
            }
        }
    }
    out
}

/// Every collision in `unit`: a label within `clearance` of another
/// pin's label (touching one of its own pin's), or touching the body outline or another pin's lead. Each as
/// (label's pin index, what it hits, the other's pin index).
pub(crate) fn collisions(
    unit: &Unit,
    name_of: &dyn Fn(&str) -> Option<String>,
    clearance: i64,
) -> Vec<(Option<usize>, Kind, Option<usize>)> {
    let items = items(unit, name_of);
    let mut out = Vec::new();
    for (i, a) in items.iter().enumerate() {
        if a.kind != Kind::Label {
            continue;
        }
        for (j, b) in items.iter().enumerate() {
            let hit = match b.kind {
                Kind::Label => {
                    // PCBCraft places a pin's own number and name; a
                    // number, outside the body, needs less room.
                    let gap = if a.pin.is_some() && a.pin == b.pin {
                        0
                    } else if a.number || b.number {
                        clearance.min(NUMBER_CLEARANCE_NM)
                    } else {
                        clearance
                    };
                    j > i && a.rect.hits(&b.rect, gap)
                }
                Kind::Body => a.rect.hits(&b.rect, 0),
                Kind::Lead => b.pin != a.pin && a.rect.hits(&b.rect, 0),
            };
            if hit {
                out.push((a.pin, b.kind, b.pin));
            }
        }
    }
    out
}
