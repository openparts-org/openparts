//! IR symbol -> `pcbcraft-symbol/1` (one unit) or `/4` (several).
//!
//! PCBCraft pins sit on the 1.27 mm grid, at most 12.7 mm from the
//! symbol's centre on each axis, each at its own position within a
//! unit; coordinates are nanometres with Y pointing down. The IR's own
//! drawing (KiCad's, Y up) is kept when it fits. A larger symbol
//! (RP2040's 56 pins span 70 mm) is laid out again into units of at
//! most 8 pins per side, by the IR's own pin families (see
//! [`relayout`]): PCBCraft supports up to 32 units sharing one terminal
//! table, and placing a pin elsewhere in the drawing changes no
//! connection.
//!
//! Pins the IR stacks at one position (same name: RP2040's six IOVDD
//! pins) become one terminal, numbered by the visible pin, which the
//! part's package map binds to every stacked pin's pad.

use crate::record::{
    Pin, PinAppearance, Shape, Symbol, Terminal, Unit, SYMBOL_FORMAT_V1, SYMBOL_FORMAT_V4,
};
use crate::{check_terminal_id, clean_text, nm, Refuse};
use openparts_pcbcad::{
    natural_cmp, GraphicKind, PcbSymbol, PinElectricalType, PinFamily, PinGroups, PinOrientation,
    SymbolPin,
};

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
        None => relayout(symbol, &stacks)?,
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

/// Pins per side of a re-laid-out unit.
const SIDE: usize = SLOTS.len();

/// One family to place: its key and its terminals (indices into the
/// stacks), in drawing order.
struct Group {
    key: String,
    terms: Vec<usize>,
}

/// A re-laid-out unit: per side (left, right, top, bottom) its runs of
/// one family each, in drawing order (top to bottom, left to right).
#[derive(Default, Clone)]
struct Plan {
    sides: [Vec<Vec<usize>>; 4],
}

const LEFT: usize = 0;
const RIGHT: usize = 1;
const TOP: usize = 2;
const BOTTOM: usize = 3;

impl Plan {
    fn used(&self, side: usize) -> usize {
        self.sides[side].iter().map(Vec::len).sum()
    }
    fn is_empty(&self) -> bool {
        self.sides.iter().all(Vec::is_empty)
    }
}

/// Fills units' left then right sides with whole families, in order.
#[derive(Clone)]
struct Packer {
    units: Vec<Plan>,
    /// The side being filled in the last unit (`LEFT` or `RIGHT`).
    side: usize,
}

impl Packer {
    fn new_unit(&mut self) {
        self.units.push(Plan::default());
        self.side = LEFT;
    }

    fn unit(&mut self) -> &mut Plan {
        self.units.last_mut().expect("a unit")
    }

    /// Places one family: on the current side when it fits, else on the
    /// next side or unit. A family longer than a side fills an empty
    /// unit's left then right side; one longer than a unit is cut, in
    /// order, into as few unit-sized pieces as it takes. `splittable`
    /// (a connector's pin sequence) fills every side in turn instead.
    fn place(&mut self, terms: &[usize], splittable: bool) {
        if terms.is_empty() {
            return;
        }
        if splittable {
            let mut rest = terms;
            while !rest.is_empty() {
                let side = self.side;
                let room = SIDE - self.unit().used(side);
                if room == 0 {
                    self.advance();
                    continue;
                }
                let (now, later) = rest.split_at(room.min(rest.len()));
                self.unit().sides[side].push(now.to_vec());
                rest = later;
            }
            return;
        }
        if terms.len() > 2 * SIDE {
            for piece in terms.chunks(2 * SIDE) {
                self.place(piece, false);
            }
            return;
        }
        if terms.len() > SIDE {
            if !self.unit().is_vertical_empty() {
                self.new_unit();
            }
            let (left, right) = terms.split_at(SIDE);
            let unit = self.unit();
            unit.sides[LEFT].push(left.to_vec());
            unit.sides[RIGHT].push(right.to_vec());
            self.side = RIGHT;
            return;
        }
        loop {
            let side = self.side;
            if SIDE - self.unit().used(side) >= terms.len() {
                self.unit().sides[side].push(terms.to_vec());
                return;
            }
            self.advance();
        }
    }

    /// Places related families in one unit when they fit in one
    /// (ESP32-S3's JTAG pins `MTCK`, `MTDO`, `MTDI`, `MTMS`).
    fn place_block(&mut self, block: &[Group], splittable: bool) {
        let mut trial = self.clone();
        for g in block {
            trial.place(&g.terms, splittable);
        }
        if trial.units.len() > self.units.len() && !splittable {
            let mut fresh = Packer {
                units: vec![Plan::default()],
                side: LEFT,
            };
            for g in block {
                fresh.place(&g.terms, false);
            }
            if fresh.units.len() == 1 {
                self.new_unit();
                for g in block {
                    self.place(&g.terms, false);
                }
                return;
            }
        }
        *self = trial;
    }

    fn advance(&mut self) {
        if self.side == LEFT {
            self.side = RIGHT;
        } else {
            self.new_unit();
        }
    }
}

impl Plan {
    fn is_vertical_empty(&self) -> bool {
        self.sides[LEFT].is_empty() && self.sides[RIGHT].is_empty()
    }
}

/// Places `families` on one side of `plan` (power on top, ground at the
/// bottom) while they fit; returns the rest, for the left and right.
fn fill_edge(plan: &mut Plan, side: usize, families: Vec<Group>) -> Vec<Group> {
    let mut rest = Vec::new();
    for g in families {
        let room = SIDE - plan.used(side);
        if !rest.is_empty() || room == 0 {
            rest.push(g);
        } else if g.terms.len() <= room {
            plan.sides[side].push(g.terms);
        } else if room == SIDE {
            let (now, later) = g.terms.split_at(SIDE);
            plan.sides[side].push(now.to_vec());
            rest.push(Group {
                key: g.key,
                terms: later.to_vec(),
            });
        } else {
            rest.push(g);
        }
    }
    rest
}

/// Runs of consecutive families whose keys share their first two
/// characters (`SPICS`, `SPIHD`, `SPICLK`; `U0TXD`, `U0RXD`).
fn related_runs(families: &[Group]) -> Vec<&[Group]> {
    let prefix = |k: &str| {
        k.get(..2)
            .filter(|_| k.chars().count() >= 2)
            .map(str::to_owned)
    };
    let mut runs = Vec::new();
    let mut start = 0;
    for i in 1..=families.len() {
        let joined = i < families.len()
            && prefix(&families[i].key).is_some()
            && prefix(&families[i].key) == prefix(&families[i - 1].key);
        if !joined {
            runs.push(&families[start..i]);
            start = i;
        }
    }
    runs
}

/// Whether `key` names a port of the same series as `bus` (`PC` next to
/// `PA`, `P5.` next to `P1.`): as long, one character different.
fn sibling_port(key: &str, bus: &str) -> bool {
    key.len() == bus.len() && key.chars().zip(bus.chars()).filter(|(a, b)| a != b).count() == 1
}

/// Units of up to 8 pins per side around a 20.32 mm body, grouped as the
/// IR is ([`PinGroups`]: KiCad's layout). Everything in one unit when it
/// fits; otherwise a first unit holds power (top), ground (bottom) and
/// the other signal families (left, then right), and the buses -- the
/// IR's bus, every family filling a side and the ports of their series
/// (STM32 `PA`/`PB`/`PC`), in natural order -- follow in units of their
/// own. Families stay together on one side unless longer than a side; a
/// family longer than a unit is cut into unit-sized pieces in order. A
/// symbol with no families (a connector: every name its own) keeps pin
/// number order.
fn relayout(symbol: &PcbSymbol, stacks: &[Stack]) -> Result<Vec<Unit>, Refuse> {
    let groups = PinGroups::of_symbol(symbol);
    let mut index: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (i, s) in stacks.iter().enumerate() {
        for n in &s.numbers {
            index.insert(n.as_str(), i);
        }
    }
    let mut placed = vec![false; stacks.len()];
    let mut convert = |families: &[PinFamily]| -> Vec<Group> {
        families
            .iter()
            .map(|f| Group {
                key: f.key.clone(),
                terms: f
                    .stacks
                    .iter()
                    .filter_map(|s| index.get(s.numbers[0].as_str()).copied())
                    .filter(|&i| !std::mem::replace(&mut placed[i], true))
                    .collect(),
            })
            .filter(|g| !g.terms.is_empty())
            .collect()
    };
    let power = convert(&groups.top);
    let ground = convert(&groups.bottom);
    let bus_keys: Vec<String> = groups.right.iter().map(|f| f.key.clone()).collect();
    let mut signals = convert(&groups.left);
    signals.extend(convert(&groups.right));
    // Anything the grouping missed (never, for an IR from build_symbol).
    let missed: Vec<usize> = (0..stacks.len()).filter(|&i| !placed[i]).collect();
    if !missed.is_empty() {
        signals.push(Group {
            key: String::new(),
            terms: missed,
        });
    }

    let connector = signals.iter().all(|g| g.terms.len() == 1);
    let (misc, mut buses): (Vec<Group>, Vec<Group>) = if connector {
        let mut terms: Vec<usize> = signals.into_iter().flat_map(|g| g.terms).collect();
        terms.sort_by(|&a, &b| {
            pin_number_order(&stacks[a].primary.number, &stacks[b].primary.number)
        });
        (
            vec![Group {
                key: String::new(),
                terms,
            }],
            Vec::new(),
        )
    } else {
        let big: Vec<String> = signals
            .iter()
            .filter(|g| g.terms.len() >= SIDE)
            .map(|g| g.key.clone())
            .chain(bus_keys)
            .collect();
        signals
            .into_iter()
            .partition(|g| !big.iter().any(|b| g.key == *b || sibling_port(&g.key, b)))
    };
    buses.sort_by(|a, b| natural_cmp(&a.key, &b.key));

    let mut first = Plan::default();
    let mut vertical = fill_edge(&mut first, TOP, power);
    vertical.extend(fill_edge(&mut first, BOTTOM, ground));
    let mut packer = Packer {
        units: vec![first],
        side: LEFT,
    };
    for g in &vertical {
        packer.place(&g.terms, false);
    }
    for block in related_runs(&misc) {
        packer.place_block(block, connector);
    }
    let mut all_in_one = packer.clone();
    for g in &buses {
        all_in_one.place(&g.terms, false);
    }
    if all_in_one.units.len() == 1 {
        packer = all_in_one;
    } else if !buses.is_empty() {
        // Buses never share the power unit.
        packer.new_unit();
        for g in &buses {
            packer.place(&g.terms, false);
        }
    }
    let plans: Vec<Plan> = packer.units.into_iter().filter(|p| !p.is_empty()).collect();
    if plans.len() > MAX_UNITS {
        return Err(format!(
            "the symbol needs {} units of 32 pins: PCBCraft allows {MAX_UNITS}",
            plans.len()
        ));
    }

    let count = plans.len();
    let mut units = Vec::with_capacity(count);
    for (k, plan) in plans.iter().enumerate() {
        let mut pins = Vec::new();
        for (side, o) in [
            (LEFT, PinOrientation::Left),
            (RIGHT, PinOrientation::Right),
            (TOP, PinOrientation::Top),
            (BOTTOM, PinOrientation::Bottom),
        ] {
            let runs = &plan.sides[side];
            let used = plan.used(side);
            // An empty slot between families, when the side has room.
            let gaps = used + runs.len().saturating_sub(1) <= SIDE;
            let total = if gaps {
                used + runs.len().saturating_sub(1)
            } else {
                used
            };
            let mut slot = (SIDE - total) / 2;
            for (r, run) in runs.iter().enumerate() {
                if gaps && r > 0 {
                    slot += 1;
                }
                for &t in run {
                    let along = SLOTS[slot] * GRID_NM;
                    slot += 1;
                    let at_nm = match o {
                        PinOrientation::Left => [-PIN_REACH_NM, along],
                        PinOrientation::Right => [PIN_REACH_NM, along],
                        PinOrientation::Top => [along, -PIN_REACH_NM],
                        PinOrientation::Bottom => [along, PIN_REACH_NM],
                    };
                    pins.push(Pin {
                        terminal: stacks[t].primary.number.clone(),
                        at_nm,
                        appearance: appearance(orientation(o), PIN_LENGTH_NM),
                    });
                }
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

/// Pin numbers in drawing order: numerically, then as text.
fn pin_number_order(a: &str, b: &str) -> std::cmp::Ordering {
    let n = |s: &str| s.parse::<u32>().unwrap_or(u32::MAX);
    n(a).cmp(&n(b)).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{ComponentCategory, Device, DeviceId, Kind, ManufacturerId, PinType};
    use std::collections::{BTreeMap, BTreeSet};

    /// A device from `"number:name:type"` entries (type: p power,
    /// po power_out, g ground, i io, c clock, r reset, x reserved,
    /// in input, a passive).
    fn device(category: ComponentCategory, pins: &[&str]) -> Device {
        let pins = pins
            .iter()
            .map(|e| {
                let mut f = e.split(':');
                let (number, name, t) = (f.next().unwrap(), f.next().unwrap(), f.next().unwrap());
                let pin_type = match t {
                    "p" => PinType::Power,
                    "po" => PinType::PowerOut,
                    "g" => PinType::Ground,
                    "i" => PinType::Io,
                    "c" => PinType::Clock,
                    "r" => PinType::Reset,
                    "x" => PinType::Reserved,
                    "in" => PinType::Input,
                    "o" => PinType::Output,
                    _ => PinType::Passive,
                };
                (
                    number.to_string(),
                    openparts_core::Pin {
                        name: name.into(),
                        pin_type,
                        alternate_functions: vec![],
                        connected_to: None,
                        diff_pair: None,
                    },
                )
            })
            .collect();
        Device {
            schema_version: "0.2".into(),
            kind: Kind::Device,
            parameters: Default::default(),
            id: DeviceId::from("ex/DEV"),
            manufacturer: ManufacturerId::from("ex"),
            category,
            family: None,
            pins,
            revisions: BTreeMap::new(),
            provenance: BTreeMap::new(),
            review: None,
        }
    }

    fn rp2040() -> Device {
        let mut pins: Vec<String> = Vec::new();
        let gpio = [(2, 0..8), (11, 8..16), (27, 16..22), (34, 22..30)];
        for (first, range) in gpio {
            for (k, g) in range.enumerate() {
                pins.push(format!("{}:GPIO{g}:i", first + k));
            }
        }
        for e in [
            "1:IOVDD:p",
            "10:IOVDD:p",
            "22:IOVDD:p",
            "33:IOVDD:p",
            "42:IOVDD:p",
            "49:IOVDD:p",
            "23:DVDD:p",
            "50:DVDD:p",
            "43:ADC_AVDD:p",
            "44:VREG_VIN:p",
            "45:VREG_VOUT:po",
            "48:USB_VDD:p",
            "19:TESTEN:x",
            "20:XIN:c",
            "21:XOUT:c",
            "24:SWCLK:c",
            "25:SWD:i",
            "26:RUN:r",
            "46:USB_DM:i",
            "47:USB_DP:i",
            "51:QSPI_SD3:i",
            "52:QSPI_SCLK:o",
            "53:QSPI_SD0:i",
            "54:QSPI_SD2:i",
            "55:QSPI_SD1:i",
            "56:QSPI_CSn:i",
            "EP:GND:g",
        ] {
            pins.push(e.to_string());
        }
        let refs: Vec<&str> = pins.iter().map(String::as_str).collect();
        device(ComponentCategory::Microcontroller, &refs)
    }

    fn stm32f103c8() -> Device {
        let mut pins: Vec<String> = [
            "1:VBAT:p",
            "2:PC13-TAMPER-RTC:i",
            "3:PC14-OSC32_IN:i",
            "4:PC15-OSC32_OUT:i",
            "5:OSC_IN:c",
            "6:OSC_OUT:c",
            "7:NRST:r",
            "8:VSSA:g",
            "9:VDDA:p",
            "10:PA0-WKUP:i",
            "23:VSS_1:g",
            "24:VDD_1:p",
            "35:VSS_2:g",
            "36:VDD_2:p",
            "44:BOOT0:in",
            "47:VSS_3:g",
            "48:VDD_3:p",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let ports: [(&str, &[(u32, u32)]); 2] = [
            (
                "PA",
                &[
                    (11, 1),
                    (12, 2),
                    (13, 3),
                    (14, 4),
                    (15, 5),
                    (16, 6),
                    (17, 7),
                    (29, 8),
                    (30, 9),
                    (31, 10),
                    (32, 11),
                    (33, 12),
                    (34, 13),
                    (37, 14),
                    (38, 15),
                ],
            ),
            (
                "PB",
                &[
                    (18, 0),
                    (19, 1),
                    (20, 2),
                    (21, 10),
                    (22, 11),
                    (25, 12),
                    (26, 13),
                    (27, 14),
                    (28, 15),
                    (39, 3),
                    (40, 4),
                    (41, 5),
                    (42, 6),
                    (43, 7),
                    (45, 8),
                    (46, 9),
                ],
            ),
        ];
        for (port, list) in ports {
            for (n, k) in list {
                pins.push(format!("{n}:{port}{k}:i"));
            }
        }
        let refs: Vec<&str> = pins.iter().map(String::as_str).collect();
        device(ComponentCategory::Microcontroller, &refs)
    }

    fn export(device: &Device) -> SymbolDraft {
        let ir = openparts_pcbcad::build_symbol("DEV", device);
        convert(&ir, "DEV").expect("converts")
    }

    /// Each unit's pin names: left top to bottom, right top to bottom,
    /// then top and bottom left to right.
    fn units(draft: &SymbolDraft) -> Vec<Vec<String>> {
        let names: BTreeMap<&str, &str> = draft
            .record
            .terminals
            .iter()
            .map(|t| (t.id.as_str(), t.name.as_deref().unwrap_or("")))
            .collect();
        draft
            .record
            .units
            .iter()
            .map(|u| {
                let mut pins: Vec<(u8, i64, String)> = u
                    .pins
                    .iter()
                    .map(|p| {
                        let (side, along) = match p.appearance.orientation {
                            "right" => (0, p.at_nm[1]),
                            "left" => (1, p.at_nm[1]),
                            "down" => (2, p.at_nm[0]),
                            _ => (3, p.at_nm[0]),
                        };
                        (side, along, names[p.terminal.as_str()].to_string())
                    })
                    .collect();
                pins.sort();
                pins.into_iter().map(|(_, _, n)| n).collect()
            })
            .collect()
    }

    fn gpio(range: std::ops::Range<u32>) -> Vec<String> {
        range.map(|i| format!("GPIO{i}")).collect()
    }

    #[test]
    fn rp2040_units_follow_the_function_groups() {
        let units = units(&export(&rp2040()));
        assert_eq!(units.len(), 3, "{units:?}");
        let first: BTreeSet<&str> = units[0].iter().map(String::as_str).collect();
        let expected: BTreeSet<&str> = [
            "IOVDD",
            "DVDD",
            "ADC_AVDD",
            "VREG_VIN",
            "VREG_VOUT",
            "USB_VDD",
            "GND",
            "TESTEN",
            "XIN",
            "XOUT",
            "SWCLK",
            "SWD",
            "RUN",
            "USB_DM",
            "USB_DP",
            "QSPI_CSn",
            "QSPI_SCLK",
            "QSPI_SD0",
            "QSPI_SD1",
            "QSPI_SD2",
            "QSPI_SD3",
        ]
        .into_iter()
        .collect();
        assert_eq!(first, expected);
        // The QSPI family stays together, in natural order.
        let qspi: Vec<&String> = units[0].iter().filter(|n| n.starts_with("QSPI")).collect();
        assert_eq!(
            qspi,
            [
                "QSPI_CSn",
                "QSPI_SCLK",
                "QSPI_SD0",
                "QSPI_SD1",
                "QSPI_SD2",
                "QSPI_SD3"
            ]
        );
        assert_eq!(units[1], gpio(0..16));
        assert_eq!(units[2], gpio(16..30));
    }

    #[test]
    fn stm32_ports_get_units_of_their_own() {
        let units = units(&export(&stm32f103c8()));
        assert_eq!(units.len(), 4, "{units:?}");
        let port = |n: &str| n.len() >= 3 && n.starts_with('P') && n.as_bytes()[2].is_ascii_digit();
        assert!(!units[0].iter().any(|n| port(n)), "{:?}", units[0]);
        let mut pa = vec!["PA0-WKUP".to_string()];
        pa.extend((1..16).map(|i| format!("PA{i}")));
        assert_eq!(units[1], pa);
        assert_eq!(
            units[2],
            (0..16).map(|i| format!("PB{i}")).collect::<Vec<_>>()
        );
        assert_eq!(
            units[3],
            ["PC13-TAMPER-RTC", "PC14-OSC32_IN", "PC15-OSC32_OUT"]
        );
    }

    #[test]
    fn a_small_ic_stays_one_unit() {
        let d = device(
            ComponentCategory::Other,
            &[
                "1:Q1:o",
                "2:Q2:o",
                "3:Q3:o",
                "4:Q4:o",
                "5:Q5:o",
                "6:Q6:o",
                "7:Q7:o",
                "8:GND:g",
                "9:Q7S:o",
                "10:MR:in",
                "11:SHCP:in",
                "12:STCP:in",
                "13:OE:in",
                "14:DS:in",
                "15:Q0:o",
                "16:VCC:p",
            ],
        );
        let draft = export(&d);
        assert_eq!(draft.record.format, SYMBOL_FORMAT_V1);
        assert_eq!(draft.record.units.len(), 1);
        assert_eq!(draft.record.units[0].id, "main");
        assert_eq!(draft.record.units[0].pins.len(), 16);
    }

    #[test]
    fn a_connector_keeps_pin_number_order() {
        let pins: Vec<String> = (1..=40).map(|i| format!("{i}:{i}:a")).collect();
        let refs: Vec<&str> = pins.iter().map(String::as_str).collect();
        let units = units(&export(&device(ComponentCategory::Connector, &refs)));
        let all: Vec<String> = units.concat();
        assert_eq!(all, (1..=40).map(|i| i.to_string()).collect::<Vec<_>>());
        assert_eq!(units.len(), 3);
    }

    #[test]
    fn relayout_keeps_the_terminals_and_their_pins() {
        for d in [rp2040(), stm32f103c8()] {
            let ir = openparts_pcbcad::build_symbol("DEV", &d);
            let draft = convert(&ir, "DEV").unwrap();
            // The terminals are the IR's stacks: one per visible pin,
            // standing for every pin at its position -- whatever the
            // layout.
            let mut expected: Vec<(String, Vec<String>)> = Vec::new();
            for p in &ir.pins {
                match expected.iter_mut().find(|(_, v)| {
                    let first = ir.pins.iter().find(|q| q.number == v[0]).unwrap();
                    first.position == p.position && first.unit == p.unit
                }) {
                    Some((_, v)) => v.push(p.number.clone()),
                    None => expected.push((p.number.clone(), vec![p.number.clone()])),
                }
            }
            let ids = |t: &[(String, Vec<String>)]| {
                t.iter()
                    .map(|(_, v)| v.iter().cloned().collect::<BTreeSet<_>>())
                    .collect::<BTreeSet<_>>()
            };
            assert_eq!(ids(&draft.terminals), ids(&expected));
            let numbers: BTreeSet<&String> = d.pins.keys().collect();
            let covered: BTreeSet<&String> =
                draft.terminals.iter().flat_map(|(_, v)| v.iter()).collect();
            assert_eq!(covered, numbers);
            // Every terminal drawn once, on the grid, at distinct
            // positions per unit.
            let mut drawn: Vec<&str> = Vec::new();
            for u in &draft.record.units {
                let mut at = BTreeSet::new();
                for p in &u.pins {
                    drawn.push(&p.terminal);
                    assert!(at.insert(p.at_nm), "two pins at {:?}", p.at_nm);
                    for v in p.at_nm {
                        assert!(v % GRID_NM == 0 && v.abs() <= PIN_REACH_NM);
                    }
                }
            }
            drawn.sort();
            let mut terminals: Vec<&str> = draft
                .record
                .terminals
                .iter()
                .map(|t| t.id.as_str())
                .collect();
            terminals.sort();
            assert_eq!(drawn, terminals);
        }
    }
}
