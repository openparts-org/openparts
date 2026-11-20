//! IR symbol -> `pcbcraft-symbol/1` (one unit) or `/4` (several).
//!
//! PCBCraft pins sit on the 1.27 mm grid, at most 12.7 mm from the
//! symbol's centre on each axis, each at its own position within a
//! unit; coordinates are nanometres with Y pointing down. The IR's own
//! drawing (KiCad's, Y up) is kept when it fits. A larger symbol
//! (RP2040's 56 pins span 70 mm) is laid out again into units of at
//! most 8 pins per side, keeping each pin's side and order: PCBCraft
//! supports up to 32 units sharing one terminal table, and placing a
//! pin elsewhere in the drawing changes no connection.
//!
//! Pins the IR stacks at one position (same name: RP2040's six IOVDD
//! pins) become one terminal, numbered by the visible pin, which the
//! part's package map binds to every stacked pin's pad.

use crate::record::{
    Pin, PinAppearance, Shape, Symbol, Terminal, Unit, SYMBOL_FORMAT_V1, SYMBOL_FORMAT_V4,
};
use crate::{check_terminal_id, clean_text, nm, Refuse};
use openparts_pcbcad::{GraphicKind, PcbSymbol, PinElectricalType, PinOrientation, SymbolPin};

const GRID_NM: i64 = 1_270_000;
const PIN_REACH_NM: i64 = 12_700_000;
const MAX_DRAWING_NM: i64 = 100_000_000;
const PIN_LENGTH_NM: i64 = 2_540_000;
const TEXT_HEIGHT_NM: i64 = 1_270_000;
/// Re-laid-out units: pin rows/columns per side, centred on the body.
const SLOTS: [i64; 8] = [-7, -5, -3, -1, 1, 3, 5, 7];
const BODY_NM: i64 = 10_160_000;
const MAX_UNITS: usize = 32;

/// A converted symbol (id and revision still unset) and, per terminal,
/// the device pin numbers it stands for.
#[derive(Debug, Clone)]
pub(crate) struct SymbolDraft {
    pub record: Symbol,
    pub terminals: Vec<(String, Vec<String>)>,
}

fn electrical_type(t: PinElectricalType) -> &'static str {
    match t {
        PinElectricalType::Input => "input",
        PinElectricalType::Output => "output",
        PinElectricalType::Bidirectional => "bidirectional",
        PinElectricalType::PowerIn => "power_in",
        PinElectricalType::PowerOut => "power_out",
        PinElectricalType::Passive => "passive",
        PinElectricalType::NoConnect => "no_connect",
        PinElectricalType::Unspecified => "unspecified",
    }
}

/// PCBCraft's orientation "points from attachment toward the body":
/// a pin on the left edge points right.
fn orientation(o: PinOrientation) -> &'static str {
    match o {
        PinOrientation::Left => "right",
        PinOrientation::Right => "left",
        // The IR is Y up: a top-edge pin's stub points down to the body.
        PinOrientation::Top => "down",
        PinOrientation::Bottom => "up",
    }
}

fn appearance(o: &'static str, length_nm: i64) -> PinAppearance {
    PinAppearance {
        orientation: o,
        length_nm,
        style: "line",
        show_name: true,
        show_number: true,
        name_height_nm: TEXT_HEIGHT_NM,
        number_height_nm: TEXT_HEIGHT_NM,
    }
}

/// One terminal: a visible IR pin and the pins stacked under it.
struct Stack<'a> {
    primary: &'a SymbolPin,
    numbers: Vec<String>,
    /// The IR position, PCBCraft frame (Y down), nm.
    at: [i64; 2],
}

pub(crate) fn convert(symbol: &PcbSymbol, name: &str) -> Result<SymbolDraft, Refuse> {
    if symbol.pins.is_empty() {
        return Err("the symbol has no pins".into());
    }
    // Stack by (unit, position); the visible pin names the terminal.
    let mut stacks: Vec<Stack> = Vec::new();
    for pin in &symbol.pins {
        let at = [nm(pin.position.x)?, -nm(pin.position.y)?];
        let unit = pin.unit;
        let found = stacks.iter_mut().find(|s| {
            s.at == at && s.primary.unit == unit && s.primary.orientation == pin.orientation
        });
        match found {
            Some(s) => {
                if !pin.hidden && s.primary.hidden {
                    s.primary = pin;
                }
                if pin.name != s.primary.name && !pin.hidden && !s.primary.hidden {
                    return Err(format!(
                        "pins {} and {} share a symbol position with different names",
                        s.numbers[0], pin.number
                    ));
                }
                s.numbers.push(pin.number.clone());
            }
            None => stacks.push(Stack {
                primary: pin,
                numbers: vec![pin.number.clone()],
                at,
            }),
        }
    }
    if stacks.len() > 1024 {
        return Err(format!(
            "{} terminals: PCBCraft symbols hold at most 1024",
            stacks.len()
        ));
    }
    let mut terminals = Vec::with_capacity(stacks.len());
    let mut records = Vec::with_capacity(stacks.len());
    for s in &stacks {
        let id = s.primary.number.clone();
        check_terminal_id(&id).map_err(|e| format!("pin number {e}"))?;
        if terminals
            .iter()
            .any(|(t, _): &(String, Vec<String>)| *t == id)
        {
            return Err(format!("pin number {id} appears twice in the symbol"));
        }
        let pin_name = s.primary.name.trim();
        let name = if pin_name.is_empty() {
            None
        } else {
            Some(clean_text("pin name", pin_name, 32)?)
        };
        records.push(Terminal {
            id: id.clone(),
            name,
            electrical_type: electrical_type(s.primary.electrical_type),
        });
        terminals.push((id, s.numbers.clone()));
    }

    let units = match native_unit(symbol, &stacks)? {
        Some(unit) => vec![unit],
        None => relayout(&stacks)?,
    };
    Ok(SymbolDraft {
        record: Symbol {
            format: if units.len() > 1 {
                SYMBOL_FORMAT_V4
            } else {
                SYMBOL_FORMAT_V1
            },
            id: String::new(),
            revision: 0,
            name: clean_text("symbol name", name, 64)?,
            reference_prefix: reference_prefix(&symbol.reference)?,
            default_value: clean_text("symbol value", name, 64)?,
            terminals: records,
            units,
        },
        terminals,
    })
}

fn reference_prefix(prefix: &str) -> Result<String, Refuse> {
    if (1..=4).contains(&prefix.len()) && prefix.chars().all(|c| c.is_ascii_uppercase()) {
        Ok(prefix.to_string())
    } else {
        Err(format!(
            "reference prefix {prefix:?} is not 1-4 capital letters"
        ))
    }
}

/// The IR drawing itself, when it is one unit whose pins all sit on
/// PCBCraft's grid within its reach.
fn native_unit(symbol: &PcbSymbol, stacks: &[Stack]) -> Result<Option<Unit>, Refuse> {
    let one_unit = symbol.pins.iter().all(|p| p.unit == symbol.pins[0].unit);
    let fits = |v: i64| v % GRID_NM == 0 && v.abs() <= PIN_REACH_NM;
    if !one_unit || !stacks.iter().all(|s| fits(s.at[0]) && fits(s.at[1])) {
        return Ok(None);
    }
    let mut drawing = Vec::new();
    for g in &symbol.graphics {
        let GraphicKind::Rectangle { start, end } = g.kind;
        let (x0, x1) = (nm(start.x)?, nm(end.x)?);
        let (y0, y1) = (-nm(start.y)?, -nm(end.y)?);
        let min_nm = [x0.min(x1), y0.min(y1)];
        let max_nm = [x0.max(x1), y0.max(y1)];
        if min_nm
            .iter()
            .chain(&max_nm)
            .any(|v| v.abs() > MAX_DRAWING_NM)
        {
            return Ok(None);
        }
        if min_nm[0] < max_nm[0] && min_nm[1] < max_nm[1] {
            drawing.push(Shape::Rect { min_nm, max_nm });
        }
    }
    let mut pins = Vec::with_capacity(stacks.len());
    for s in stacks {
        let length = nm(s.primary.length)?;
        if !(100_000..=100_000_000).contains(&length) {
            return Ok(None);
        }
        pins.push(Pin {
            terminal: s.primary.number.clone(),
            at_nm: s.at,
            appearance: appearance(orientation(s.primary.orientation), length),
        });
    }
    Ok(Some(Unit {
        id: "main".into(),
        drawing,
        pins,
    }))
}

/// Units of up to 8 pins per side around a 20.32 mm body, each side's
/// pins in the IR's order (left/right top to bottom, top/bottom left to
/// right).
fn relayout(stacks: &[Stack]) -> Result<Vec<Unit>, Refuse> {
    let side = |o: PinOrientation| -> Vec<&Stack> {
        let mut v: Vec<&Stack> = stacks
            .iter()
            .filter(|s| s.primary.orientation == o)
            .collect();
        match o {
            PinOrientation::Left | PinOrientation::Right => v.sort_by_key(|s| s.at[1]),
            PinOrientation::Top | PinOrientation::Bottom => v.sort_by_key(|s| s.at[0]),
        }
        v
    };
    let sides = [
        (side(PinOrientation::Left), PinOrientation::Left),
        (side(PinOrientation::Right), PinOrientation::Right),
        (side(PinOrientation::Top), PinOrientation::Top),
        (side(PinOrientation::Bottom), PinOrientation::Bottom),
    ];
    let count = sides
        .iter()
        .map(|(v, _)| v.len().div_ceil(SLOTS.len()))
        .max()
        .unwrap_or(0);
    if count > MAX_UNITS {
        return Err(format!(
            "the symbol needs {count} units of 32 pins: PCBCraft allows {MAX_UNITS}"
        ));
    }
    let mut units = Vec::with_capacity(count);
    for k in 0..count {
        let mut pins = Vec::new();
        for (list, o) in &sides {
            let chunk: Vec<&&Stack> = list
                .iter()
                .skip(k * SLOTS.len())
                .take(SLOTS.len())
                .collect();
            let offset = (SLOTS.len() - chunk.len()) / 2;
            for (i, s) in chunk.iter().enumerate() {
                let along = SLOTS[offset + i] * GRID_NM;
                let at_nm = match o {
                    PinOrientation::Left => [-PIN_REACH_NM, along],
                    PinOrientation::Right => [PIN_REACH_NM, along],
                    PinOrientation::Top => [along, -PIN_REACH_NM],
                    PinOrientation::Bottom => [along, PIN_REACH_NM],
                };
                pins.push(Pin {
                    terminal: s.primary.number.clone(),
                    at_nm,
                    appearance: appearance(orientation(*o), PIN_LENGTH_NM),
                });
            }
        }
        units.push(Unit {
            id: if count == 1 {
                "main".into()
            } else {
                format!("u{}", k + 1)
            },
            drawing: vec![Shape::Rect {
                min_nm: [-BODY_NM, -BODY_NM],
                max_nm: [BODY_NM, BODY_NM],
            }],
            pins,
        });
    }
    Ok(units)
}
