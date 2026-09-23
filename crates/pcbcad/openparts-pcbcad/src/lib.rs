//! openparts-pcbcad: the CAD-independent PCB Symbol/Footprint
//! Intermediate Representation (Architecture Specification section 12).
//! EDA-specific concepts stay in the adapter crates (openparts-kicad,
//! openparts-librepcb, openparts-altium); this crate only knows about
//! Device pins and Mechanical Geometry leads.

use openparts_core::{Device, PinType};
use openparts_mcad::MechanicalGeometry;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinElectricalType {
    Input,
    Output,
    Bidirectional,
    PowerIn,
    Passive,
    NoConnect,
    Unspecified,
}

impl From<PinType> for PinElectricalType {
    fn from(t: PinType) -> Self {
        match t {
            PinType::Power | PinType::Ground => PinElectricalType::PowerIn,
            PinType::Input | PinType::Clock | PinType::Reset => PinElectricalType::Input,
            PinType::Output => PinElectricalType::Output,
            PinType::Io => PinElectricalType::Bidirectional,
            PinType::Analog | PinType::Reserved | PinType::Passive => PinElectricalType::Passive,
            PinType::Nc => PinElectricalType::NoConnect,
            PinType::Unknown => PinElectricalType::Unspecified,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinOrientation {
    /// Pin stub points toward +X, pin sits on the symbol's left edge.
    Left,
    /// Pin stub points toward -X, pin sits on the symbol's right edge.
    Right,
    /// Pin stub points toward +Y, pin sits on the symbol's bottom edge.
    Bottom,
    /// Pin stub points toward -Y, pin sits on the symbol's top edge.
    Top,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolPin {
    pub number: String,
    pub name: String,
    pub electrical_type: PinElectricalType,
    pub position: Point2,
    pub length: f64,
    pub orientation: PinOrientation,
    /// Which KiCad symbol unit this pin belongs to. `1` for an ordinary
    /// (non-split) symbol. `0` is KiCad's "common to every unit" sentinel
    /// -- used only when [`build_symbol`] splits a very large pin bus
    /// across multiple units (see its docs), to hold the pins (power,
    /// ground, misc signals) shared by every numbered unit.
    pub unit: u32,
    /// True for every member of a stacked name-group (see
    /// `group_by_exact_name`) after the first: KiCad draws every pin's
    /// name/number text at its own `(at ...)` coordinate with no
    /// overlap-avoidance of its own, so multiple *visible* pins stacked
    /// at the identical position render as garbled overlapping text --
    /// confirmed directly against KiCad's own official RP2040 symbol
    /// (gitlab.com/kicad/libraries/kicad-symbols
    /// `MCU_RaspberryPi.kicad_symdir/RP2040.kicad_sym`): only the first
    /// pin in each stack (e.g. IOVDD pin 1) is visible; the other five
    /// IOVDD pins (10/22/33/42/49) each carry `(hide yes)`. Hidden pins
    /// are still fully real, individually-numbered connection points --
    /// KiCad just doesn't draw their text -- so nothing about netlist
    /// correctness changes, only what's drawn.
    pub hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphicKind {
    Rectangle { start: Point2, end: Point2 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Graphic {
    pub kind: GraphicKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolUnit {
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcbSymbol {
    pub name: String,
    pub units: Vec<SymbolUnit>,
    pub pins: Vec<SymbolPin>,
    pub graphics: Vec<Graphic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadShape {
    Rect,
    Circle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pad {
    pub number: String,
    pub position: Point2,
    pub size: (f64, f64),
    pub shape: PadShape,
    /// SMD or through-hole -- taken straight from the generating
    /// `Lead`'s own `mounting` (already a dependency of this crate).
    pub mounting: openparts_mcad::Mounting,
    /// Drill diameter (mm). `Some` only when `mounting` is
    /// `ThroughHole`.
    pub drill: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Courtyard {
    pub outline: Vec<Point2>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcbFootprint {
    pub name: String,
    pub pads: Vec<Pad>,
    pub graphics: Vec<Graphic>,
    pub courtyard: Option<Courtyard>,
}

const GRID_MM: f64 = 2.54;
const PIN_LENGTH_MM: f64 = 2.54;

/// Extracts a pin's "family" for grouping same-kind pins onto the same
/// side: drop anything from `/` onward (an alternate-function suffix,
/// e.g. `GPIO26/ADC0` -> `GPIO26`), take the text before the first `_`
/// (e.g. `QSPI_SD0` -> `QSPI`), then strip trailing ASCII digits
/// (`GPIO26` -> `GPIO`). A name with no shared prefix (`TESTEN`, `RUN`)
/// becomes its own singleton group, same as any other family.
///
/// Deliberately simple/deterministic rather than a fuzzy label-similarity
/// match: common MCU pin-naming conventions across vendors (STM32
/// `PA0`/`PA1`, ESP32 `GPIO0`, RP2040 `QSPI_SD0`) already follow this
/// "prefix + delimiter" pattern, and fuzzy matching would trade that for
/// grouping decisions that are harder to predict or test.
fn pin_group_key(name: &str) -> String {
    let base = name.split('/').next().unwrap_or(name);
    let first_word = base.split('_').next().unwrap_or(base);
    let trimmed = first_word.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed.is_empty() {
        first_word.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Groups `numbers` (device pin-number references) into runs sharing
/// the exact same pin name, preserving first-appearance order -- e.g.
/// RP2040's six IOVDD pins (device pin numbers 1, 10, 22, 33, 42, 49)
/// become one group.
///
/// Used to place pins wired to the same net at a single shared
/// schematic position, matching KiCad's own official RP2040 library
/// symbol (confirmed by the maintainer against their real KiCad 10
/// install) rather than giving each physical pin its own row: KiCad
/// 10+ native pin stacking places multiple separate pin objects at the
/// identical coordinate, each still keeping its own correct number, and
/// recognizes them as one connection point in the schematic. This is
/// purely a schematic/netlist-declaration technique -- the footprint
/// (`build_footprint`, built independently from `MechanicalGeometry`,
/// never from pin names) still has one full-fledged, individually
/// numbered copper pad per physical pin, so nothing about the real PCB
/// connections changes; wiring the one visible stacked pin still nets
/// every underlying pin number together, exactly as if each had been
/// wired separately.
///
/// This is a different, narrower concept from [`pin_group_key`]'s
/// family grouping: `QSPI_SD0`/`QSPI_SD1` share a *family* (kept
/// visually close, never merged, since they're different signals), but
/// only pins sharing the exact same *name* -- the same net -- stack.
fn group_by_exact_name<'a>(numbers: &[&'a String], device: &Device) -> Vec<Vec<&'a String>> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<&'a String>> =
        std::collections::HashMap::new();
    for &number in numbers {
        let name = device.pins[number].name.clone();
        groups.entry(name.clone()).or_insert_with(|| {
            order.push(name.clone());
            Vec::new()
        });
        groups.get_mut(&name).unwrap().push(number);
    }
    order
        .into_iter()
        .map(|name| groups.remove(&name).unwrap())
        .collect()
}

/// Rough per-character glyph width for KiCad's default 1.27mm pin-name
/// font (cross-checked against the real widths in KiCad's own Newstroke
/// font data, gitlab.com/kicad/code/kicad `common/newstroke_font.cpp`:
/// digits/most capitals are ~20-22 font units wide against a ~21-unit
/// cap height, i.e. width ≈ height, which is what this constant already
/// assumed). Not exact font metrics -- consistent with this project's
/// existing "structurally representative, not pixel-perfect" approach to
/// generated geometry.
///
/// Only the pin *name* is sized this way -- an earlier version of this
/// file also modeled KiCad's separate "electrical type" description
/// text (e.g. "Power input"), but that text is drawn on
/// `LAYER_PRIVATE_NOTES` and gated by
/// `PIN_LAYOUT_CACHE::m_showElectricalType`
/// (eeschema/pin_layout_cache.cpp: `if( !m_showElectricalType ) return
/// std::nullopt;`) -- confirmed by reading eeschema/sch_painter.cpp (the
/// schematic-*sheet* canvas, as opposed to the separate Symbol/Library
/// Editor window) directly: nothing there ever sets that flag, so this
/// text never renders in the view a placed symbol is actually seen in.
/// Modeling it was solving an invisible problem and inflating every
/// symbol's size for no visible benefit; removed.
const CHAR_WIDTH_MM: f64 = 1.0;

/// A pin's required half-extent along a packing axis, plus an optional
/// group key (used by `pack_centered` to insert extra gaps between
/// different groups).
type PinSpec = (f64, Option<String>);

/// Every pin's half-extent along its side's packing axis: a flat
/// `GRID_MM / 2.0` (KiCad's own 100mil pin pitch), the same rule for all
/// four sides. A pin's NAME text is the only label that can compete for
/// space along the packing axis (see `CHAR_WIDTH_MM`'s docs for why the
/// electrical-type text doesn't count), and on top/bottom pins that name
/// rotates to read vertically, so it barely uses any packing-axis space
/// there either -- the flat grid pitch is what actually governs same-side
/// neighbor spacing on every side.
const PIN_PACKING_HALF_EXTENT_MM: f64 = GRID_MM / 2.0;

/// Extra gap `pack_centered` inserts between two consecutive pins whose
/// group differs, on top of their natural half-extent sum -- the same
/// value for every side.
///
/// Cross-checked directly against KiCad's own official RP2040 symbol
/// (gitlab.com/kicad/libraries/kicad-symbols
/// `MCU_RaspberryPi.kicad_symdir/RP2040.kicad_sym`): its left column
/// leaves exactly 7.62mm (3x `GRID_MM`) between four of its five
/// distinct signal-group boundaries (TESTEN | RUN | USB_DM/DP | QSPI
/// bus | XIN | XOUT | SWCLK/SWDIO), while staying a tight 1x `GRID_MM`
/// *within* a functionally-paired group -- exactly what
/// [`pin_group_key`] already keeps together as one family (USB_DM/DP
/// share the "USB" key, the QSPI bus pins share "QSPI"). Reproducing
/// that 3x-grid boundary rule here (`GRID_MM` natural neighbor spacing +
/// this `2 * GRID_MM` extra) lands the left column's total span within
/// ~4% of the real symbol's (68.58mm vs. 66.04mm on that reference
/// file) -- not just directionally right, but quantitatively close.
/// This is exactly the "the bus side already makes the body tall, so
/// don't cram the left column just because it has fewer pins" room a
/// maintainer screenshot showed being wasted under a tighter boundary
/// gap.
const GROUP_GAP_MM: f64 = GRID_MM * 2.0;

/// Packs `specs` (each pin's required half-extent along the packing
/// axis, plus an optional group key) end to end -- consecutive pins get
/// exactly `half_extent[i] + half_extent[i+1]` apart, plus
/// [`GROUP_GAP_MM`] whenever the group key changes between them -- then
/// shifts the whole run so it's centered on 0.
///
/// This is the closed-form solution to "space these out just enough to
/// avoid collision, centered, with a gap between different groups": for
/// a 1D layout it can be solved exactly in one pass, so unlike an
/// iterative force/spring relaxation there's no convergence tolerance or
/// iteration count to pick and the result is exactly reproducible.
fn pack_centered(specs: &[PinSpec]) -> Vec<f64> {
    if specs.is_empty() {
        return Vec::new();
    }
    let mut positions = Vec::with_capacity(specs.len());
    let mut cursor = 0.0;
    let mut prev_half = 0.0;
    let mut prev_group: Option<&String> = None;
    for (i, (half, group)) in specs.iter().enumerate() {
        if i > 0 {
            let gap = if group.is_some() && group.as_ref() != prev_group {
                GROUP_GAP_MM
            } else {
                0.0
            };
            cursor += prev_half + half + gap;
        }
        positions.push(cursor);
        prev_half = *half;
        prev_group = group.as_ref();
    }
    let shift = (positions[0] + positions[positions.len() - 1]) / 2.0;
    for p in &mut positions {
        *p -= shift;
    }
    positions
}

/// The corner-clearance margin used by [`corner_boosted_reach`]: the
/// widest a single pin *name* on this device actually is (in mm, via
/// [`CHAR_WIDTH_MM`]), computed per-device rather than a flat worst-case
/// constant. A device with only short names doesn't pay for a cross-
/// locale/cross-type worst case it doesn't have; a device with a
/// genuinely long name (e.g. RP2040's own `VREG_VOUT`) still gets a
/// margin sized to fit it. This is what actually shrinks the rectangle
/// for typical devices versus a flat guessed margin, while still
/// resolving the real cross-side overlap risk at each corner.
fn corner_margin_mm(device: &Device) -> f64 {
    device
        .pins
        .values()
        .map(|p| p.name.chars().count() as f64 * CHAR_WIDTH_MM)
        .fold(GRID_MM, f64::max)
}

/// Computes the same thing as `widest_reach` (in `build_symbol`), but
/// the first/last item -- a side's outermost pin, which sits closest to
/// the symbol's corners -- has its contribution to the reach boosted to
/// at least `margin` (see [`corner_margin_mm`]), every other item using
/// its own natural half-extent unchanged.
///
/// This intentionally only affects the *rectangle size* this reach
/// feeds into, not the corner pin's actual packed position:
/// `pack_centered` was previously given the boosted half-extent
/// directly, which genuinely fixed cross-side corner clearance but as
/// a side effect also pushed the corner pin unnecessarily far from its
/// own same-side neighbor (e.g. GPIO29 from GPIO28) -- a real,
/// maintainer-reported regression. Boosting only the reach, not the
/// packing input, keeps every same-side gap exactly as tight as
/// `pack_centered` naturally makes it, while still growing the
/// rectangle enough that the *other* (perpendicular) side's corner pin
/// -- whose position is fixed relative to this rectangle's size, not
/// to this side's packing -- ends up far enough away. Each side's
/// outermost pin's reach contribution is `pos.abs() + half_extent`; two
/// perpendicular corner pins end up `sqrt(a² + b²)` apart regardless of
/// the body's overall size, so guaranteeing each clears `margin`
/// guarantees their combined distance does too.
fn corner_boosted_reach(specs: &[PinSpec], positions: &[f64], margin: f64) -> f64 {
    let last_index = specs.len().saturating_sub(1);
    specs
        .iter()
        .zip(positions.iter())
        .enumerate()
        .map(|(i, ((half, _), pos))| {
            let half = if i == 0 || i == last_index {
                half.max(margin)
            } else {
                *half
            };
            pos.abs() + half
        })
        .fold(GRID_MM, f64::max)
}

/// Above this many pins, the bus family (e.g. a GPIO bank) is split
/// across multiple KiCad symbol units rather than stacked on one
/// ever-taller right edge -- see [`build_symbol`]'s docs.
const BUS_UNIT_MAX_PINS: usize = 32;

/// Builds a Symbol in four stages: **group** pins onto sides the way a
/// hand-authored MCU symbol is (`power`-type on top, `ground`-type on
/// bottom, the single largest same-family signal group -- e.g. a GPIO
/// bus -- on the right, everything else -- grouped by family via
/// [`pin_group_key`], each family kept contiguous -- on the left);
/// **stack** pins sharing the exact same name (e.g. RP2040's six IOVDD
/// pins) onto one shared position via [`group_by_exact_name`]; **place**
/// each side's groups with [`pack_centered`] (uniform grid-pitch row
/// height everywhere, an extra [`GROUP_GAP_MM`] between different
/// families); then **resolve** the one overlap risk that per-side
/// packing alone can't see -- two perpendicular sides' outermost pins
/// meeting at a corner -- via [`corner_boosted_reach`], which grows the
/// rectangle (never a side's own packed spacing) by exactly this
/// device's own longest-name margin ([`corner_margin_mm`]). Body
/// width/height follow from the resulting extents, never forcing a
/// square.
///
/// When the bus exceeds [`BUS_UNIT_MAX_PINS`], it's split across
/// multiple KiCad symbol units instead of making one unit taller
/// without limit -- confirmed against the official KiCad Library
/// Convention rather than invented (klc.kicad.org, S3.8 "Multi unit
/// symbols": large parts should be split into units, and pins shared by
/// every unit -- power here -- belong in a dedicated unit). KiCad's own
/// `.kicad_sym` format already supports this directly: a sub-symbol
/// named `"{name}_0_1"` (unit 0) is automatically composited into
/// *every* unit the user places, so top/bottom/left pins only need to
/// be written once, tagged unit 0, rather than repeated per bus chunk.
///
/// ```mermaid
/// flowchart TD
///     A[build_symbol] --> B["bucket pins: power -> top, ground -> bottom,<br/>rest -> grouped by name family"]
///     B --> C["find the bus: largest family in `rest`"]
///     C --> D{"bus pin count > 32?"}
///     D -- "no" --> E["single unit (unit 1):<br/>top + bottom + left + bus, as today"]
///     D -- "yes" --> F["unit 0 (shared, composited into every<br/>unit KiCad places): top + bottom + left,<br/>no bus pins"]
///     F --> G["split the bus into chunks of &le; 32,<br/>in ascending pin-number order"]
///     G --> H["unit 1..N: one chunk each,<br/>right side, shared rectangle from unit 0"]
/// ```
pub fn build_symbol(name: &str, device: &Device) -> PcbSymbol {
    let mut numbers: Vec<&String> = device.pins.keys().collect();
    numbers.sort_by_key(|n| n.parse::<u32>().unwrap_or(u32::MAX));

    let mut top: Vec<&String> = Vec::new();
    let mut bottom: Vec<&String> = Vec::new();
    let mut rest: Vec<&String> = Vec::new();
    for number in &numbers {
        match device.pins[*number].pin_type {
            PinType::Power => top.push(number),
            PinType::Ground => bottom.push(number),
            _ => rest.push(number),
        }
    }

    let mut group_order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<&String>> =
        std::collections::HashMap::new();
    for number in &rest {
        let key = pin_group_key(&device.pins[*number].name);
        groups.entry(key.clone()).or_insert_with(|| {
            group_order.push(key.clone());
            Vec::new()
        });
        groups.get_mut(&key).unwrap().push(number);
    }

    let bus_key = group_order.iter().max_by_key(|k| groups[*k].len()).cloned();

    let mut left: Vec<&String> = Vec::new();
    let mut right: Vec<&String> = Vec::new();
    for key in &group_order {
        let target = if Some(key) == bus_key.as_ref() {
            &mut right
        } else {
            &mut left
        };
        target.extend(groups[key].iter().copied());
    }

    // Stack pins that share the exact same name (e.g. RP2040's six
    // IOVDD pins) at a single schematic position -- see
    // group_by_exact_name's docs. From here on, pack_centered operates
    // on one PinSpec per name-group rather than per physical pin; each
    // group's packed position is applied to every member pin when
    // SymbolPins are finally emitted below.
    let top_groups = group_by_exact_name(&top, device);
    let bottom_groups = group_by_exact_name(&bottom, device);
    let left_groups = group_by_exact_name(&left, device);

    // Every side uses the same flat grid-pitch half-extent (see
    // PIN_PACKING_HALF_EXTENT_MM's docs); the only per-side difference
    // left is which family-boundary gaps apply, e.g. RP2040's IOVDD
    // group packs tightly against DVDD, then gets a gap before
    // ADC_AVDD, the VREG_VIN/VREG_VOUT pair (sharing the "VREG"
    // family), and USB_VDD.
    let top_specs: Vec<PinSpec> = top_groups
        .iter()
        .map(|g| {
            (
                PIN_PACKING_HALF_EXTENT_MM,
                Some(pin_group_key(&device.pins[g[0]].name)),
            )
        })
        .collect();
    let bottom_specs: Vec<PinSpec> = bottom_groups
        .iter()
        .map(|g| {
            (
                PIN_PACKING_HALF_EXTENT_MM,
                Some(pin_group_key(&device.pins[g[0]].name)),
            )
        })
        .collect();
    // Left: fixed row height, but grouped -- an extra gap opens up
    // wherever the pin family changes (e.g. between QSPI_* and USB_*).
    let left_specs: Vec<PinSpec> = left_groups
        .iter()
        .map(|g| {
            (
                PIN_PACKING_HALF_EXTENT_MM,
                Some(pin_group_key(&device.pins[g[0]].name)),
            )
        })
        .collect();

    let top_pos = pack_centered(&top_specs);
    let bottom_pos = pack_centered(&bottom_specs);
    let left_pos = pack_centered(&left_specs);

    // The bus (`right`) is split into units of at most
    // BUS_UNIT_MAX_PINS name-groups once it exceeds that size -- see
    // this function's docs/flowchart. Below that size this is just one
    // "chunk" containing every bus group, identical to the pre-split
    // behavior. Grouped before chunking so a stacked bus pin (unlikely
    // in practice -- bus signals are normally uniquely named -- but
    // possible) never gets split across a unit boundary.
    let right_groups = group_by_exact_name(&right, device);
    let bus_chunks: Vec<&[Vec<&String>]> = right_groups.chunks(BUS_UNIT_MAX_PINS).collect();
    let splitting = bus_chunks.len() > 1;
    let chunk_specs_pos: Vec<(Vec<PinSpec>, Vec<f64>)> = bus_chunks
        .iter()
        .map(|chunk| {
            let specs: Vec<PinSpec> = chunk
                .iter()
                .map(|g| {
                    (
                        PIN_PACKING_HALF_EXTENT_MM,
                        Some(pin_group_key(&device.pins[g[0]].name)),
                    )
                })
                .collect();
            let pos = pack_centered(&specs);
            (specs, pos)
        })
        .collect();

    // The body must reach past each outermost pin's own label extent
    // (otherwise the outermost label hangs off past the drawn rectangle
    // edge) and, at the four corners specifically, past this device's
    // own corner_margin_mm -- see corner_boosted_reach's docs for why
    // that's computed separately from each side's own packed positions
    // above.
    let margin = corner_margin_mm(device);
    let half_width = corner_boosted_reach(&top_specs, &top_pos, margin).max(corner_boosted_reach(
        &bottom_specs,
        &bottom_pos,
        margin,
    ));
    // Every unit shares one rectangle, so this must cover the largest
    // bus chunk, not just whichever chunk happens to be last.
    let half_height = chunk_specs_pos
        .iter()
        .map(|(s, p)| corner_boosted_reach(s, p, margin))
        .fold(
            corner_boosted_reach(&left_specs, &left_pos, margin),
            f64::max,
        );

    let mut pins = Vec::with_capacity(numbers.len());
    // Only the first pin in a stacked name-group (see `SymbolPin::hidden`'s
    // docs) is drawn with its real electrical type; every other member
    // is hidden and downgraded to `Passive`, matching KiCad's own
    // official RP2040 symbol exactly (its five hidden duplicate IOVDD
    // pins are each `pin passive line ... (hide yes)`, not
    // `pin power_in line`). Each still keeps its own correct number, so
    // netlisting is unaffected -- only what KiCad draws changes.
    let mut push_pin =
        |number: &String, position: Point2, side: PinOrientation, unit: u32, is_primary: bool| {
            let pin = &device.pins[number];
            pins.push(SymbolPin {
                number: number.clone(),
                name: pin.name.clone(),
                electrical_type: if is_primary {
                    pin.pin_type.into()
                } else {
                    PinElectricalType::Passive
                },
                position,
                length: PIN_LENGTH_MM,
                orientation: side,
                unit,
                hidden: !is_primary,
            });
        };
    // Top/bottom/left are shared across every unit once splitting, so
    // they land in KiCad's unit-0 sentinel; otherwise (the common case)
    // everything is unit 1, exactly as before this feature existed.
    let shared_unit: u32 = if splitting { 0 } else { 1 };
    // `pack_centered` lays its first item at the most-negative position;
    // left/top's first item is conventionally the "start" (top-most /
    // right-most) side, the opposite direction, so their axis is
    // negated. Every member of a name-group shares its group's one
    // packed position (native KiCad pin stacking -- see
    // group_by_exact_name's docs).
    for (group, &y) in left_groups.iter().zip(left_pos.iter()) {
        for (i, &number) in group.iter().enumerate() {
            push_pin(
                number,
                Point2 {
                    x: -half_width - PIN_LENGTH_MM,
                    y: -y,
                },
                PinOrientation::Left,
                shared_unit,
                i == 0,
            );
        }
    }
    for (group, &x) in bottom_groups.iter().zip(bottom_pos.iter()) {
        for (i, &number) in group.iter().enumerate() {
            push_pin(
                number,
                Point2 {
                    x,
                    y: -half_height - PIN_LENGTH_MM,
                },
                PinOrientation::Bottom,
                shared_unit,
                i == 0,
            );
        }
    }
    for (group, &x) in top_groups.iter().zip(top_pos.iter()) {
        for (i, &number) in group.iter().enumerate() {
            push_pin(
                number,
                Point2 {
                    x: -x,
                    y: half_height + PIN_LENGTH_MM,
                },
                PinOrientation::Top,
                shared_unit,
                i == 0,
            );
        }
    }
    for (chunk_index, chunk) in bus_chunks.iter().enumerate() {
        let (_, chunk_pos) = &chunk_specs_pos[chunk_index];
        let unit = if splitting { chunk_index as u32 + 1 } else { 1 };
        for (group, &y) in chunk.iter().zip(chunk_pos.iter()) {
            for (i, &number) in group.iter().enumerate() {
                push_pin(
                    number,
                    Point2 {
                        x: half_width + PIN_LENGTH_MM,
                        y,
                    },
                    PinOrientation::Right,
                    unit,
                    i == 0,
                );
            }
        }
    }

    let units = if splitting {
        (1..=bus_chunks.len() as u32)
            .map(|index| SymbolUnit { index })
            .collect()
    } else {
        vec![SymbolUnit { index: 1 }]
    };

    PcbSymbol {
        name: name.to_string(),
        units,
        pins,
        graphics: vec![Graphic {
            kind: GraphicKind::Rectangle {
                start: Point2 {
                    x: -half_width,
                    y: half_height,
                },
                end: Point2 {
                    x: half_width,
                    y: -half_height,
                },
            },
        }],
    }
}

/// Builds a Footprint by projecting Mechanical Geometry leads onto the
/// XY plane. Lead size/position from the generator already encode the
/// correct per-side orientation, so no rotation is needed here.
///
/// Pad shape: an SMD lead always gets `Rect` (unchanged from before
/// through-hole support existed). A through-hole lead's shape marks
/// polarity the same way a real KiCad footprint does -- confirmed
/// directly against `KiCad/kicad-footprints`'
/// `Capacitor_THT.pretty/CP_Radial_D5.0mm_P2.00mm.kicad_mod`: pin "1"
/// (positive) is `rect`, every other pin is `circle`. Applied
/// generically here (any through-hole part's pin 1, not hardcoded to
/// radial capacitors specifically), matching how `MarkerKind::Pin1Dot`
/// is already a generic, family-agnostic concept.
pub fn build_footprint(name: &str, geometry: &MechanicalGeometry) -> PcbFootprint {
    let pads = geometry
        .leads
        .iter()
        .map(|lead| {
            let shape = match lead.mounting {
                openparts_mcad::Mounting::Smd => PadShape::Rect,
                openparts_mcad::Mounting::ThroughHole if lead.number == "1" => PadShape::Rect,
                openparts_mcad::Mounting::ThroughHole => PadShape::Circle,
            };
            Pad {
                number: lead.number.clone(),
                position: Point2 {
                    x: lead.position.x,
                    y: lead.position.y,
                },
                size: (lead.size.x, lead.size.y),
                shape,
                mounting: lead.mounting,
                drill: lead.drill,
            }
        })
        .collect();

    PcbFootprint {
        name: name.to_string(),
        pads,
        graphics: vec![],
        courtyard: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Device, DeviceId, Kind, ManufacturerId, Pin};
    use std::collections::BTreeMap;

    fn device_8_pins() -> Device {
        let mut pins = BTreeMap::new();
        for i in 1..=8u32 {
            pins.insert(
                i.to_string(),
                Pin {
                    name: format!("P{i}"),
                    pin_type: PinType::Io,
                    alternate_functions: vec![],
                },
            );
        }
        Device {
            schema_version: "0.1".into(),
            kind: Kind::Device,
            id: DeviceId::from("ex/DEV8"),
            manufacturer: ManufacturerId::from("ex"),
            family: None,
            pins,
            revisions: BTreeMap::new(),
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn pin_group_key_strips_suffixes_and_trailing_digits() {
        assert_eq!(pin_group_key("GPIO0"), "GPIO");
        assert_eq!(pin_group_key("GPIO26/ADC0"), "GPIO");
        assert_eq!(pin_group_key("QSPI_SS"), "QSPI");
        assert_eq!(pin_group_key("QSPI_SD0"), "QSPI");
        assert_eq!(pin_group_key("USB_DM"), "USB");
        assert_eq!(pin_group_key("USB_DP"), "USB");
        assert_eq!(pin_group_key("TESTEN"), "TESTEN");
    }

    #[test]
    fn group_by_exact_name_groups_and_preserves_first_appearance_order() {
        let device = device_shaped_like_an_mcu();
        let numbers: Vec<&String> = ["15", "16", "17", "14"]
            .iter()
            .map(|n| {
                device
                    .pins
                    .keys()
                    .find(|k| k.as_str() == *n)
                    .expect("fixture has this pin")
            })
            .collect();
        let groups = group_by_exact_name(&numbers, &device);
        // 15/16/17 are all "IOVDD" (one group of 3); 14 is "TESTEN"
        // (its own singleton group), in first-appearance order.
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 3);
        assert!(groups[0].iter().all(|n| device.pins[*n].name == "IOVDD"));
        assert_eq!(groups[1].len(), 1);
        assert_eq!(device.pins[groups[1][0]].name, "TESTEN");
    }

    #[test]
    fn pack_centered_spaces_neighbors_by_their_combined_half_extent() {
        let specs = vec![(2.0, None), (3.0, None), (1.0, None)];
        let positions = pack_centered(&specs);
        assert_eq!(positions.len(), 3);
        assert!((positions[1] - positions[0] - 5.0).abs() < 1e-9); // 2.0+3.0
        assert!((positions[2] - positions[1] - 4.0).abs() < 1e-9); // 3.0+1.0
                                                                   // Centered: the run's midpoint sits at 0.
        assert!((positions[0] + positions[2]).abs() < 1e-9);
    }

    #[test]
    fn pack_centered_adds_a_gap_only_at_group_boundaries() {
        let specs = vec![
            (1.0, Some("A".to_string())),
            (1.0, Some("A".to_string())),
            (1.0, Some("B".to_string())),
        ];
        let positions = pack_centered(&specs);
        let within_group_gap = positions[1] - positions[0];
        let cross_group_gap = positions[2] - positions[1];
        assert!((within_group_gap - 2.0).abs() < 1e-9); // 1.0+1.0, no extra gap
        assert!((cross_group_gap - (2.0 + GROUP_GAP_MM)).abs() < 1e-9); // + GROUP_GAP_MM
    }

    #[test]
    fn pack_centered_handles_empty_and_single_item() {
        assert_eq!(pack_centered(&[]), Vec::<f64>::new());
        let single = pack_centered(&[(3.0, None)]);
        assert_eq!(single, vec![0.0]);
    }

    #[test]
    fn top_pins_dont_overlap_when_names_are_long() {
        // Every pin here is `power`-type with a long, distinct name --
        // exactly the RP2040 top-row scenario that used to overlap under
        // a fixed 2.54mm grid step.
        let mut pins = BTreeMap::new();
        for (n, name) in [
            ("1", "USB_VDD"),
            ("2", "ADC_AVDD"),
            ("3", "VREG_VIN"),
            ("4", "VREG_VOUT"),
        ] {
            pins.insert(
                n.into(),
                Pin {
                    name: name.into(),
                    pin_type: PinType::Power,
                    alternate_functions: vec![],
                },
            );
        }
        let device = Device {
            schema_version: "0.1".into(),
            kind: Kind::Device,
            id: DeviceId::from("ex/LONGNAMES"),
            manufacturer: ManufacturerId::from("ex"),
            family: None,
            pins,
            revisions: BTreeMap::new(),
            provenance: BTreeMap::new(),
        };
        let symbol = build_symbol("LONGNAMES", &device);
        let mut xs: Vec<f64> = symbol.pins.iter().map(|p| p.position.x).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.windows(2) {
            let gap = pair[1] - pair[0];
            // Every name here is at least 7 characters; the gap between
            // any two neighbors must be at least enough for both their
            // estimated text widths, never the old fixed 2.54mm.
            assert!(gap > GRID_MM, "adjacent top pins only {gap}mm apart");
        }
    }

    /// A device shaped like RP2040 in miniature: a large `io` family
    /// sharing a name prefix (the "GPIO bus"), a couple of small
    /// families, several same-named `power` pins, and one `ground` pin.
    fn device_shaped_like_an_mcu() -> Device {
        let mut pins = BTreeMap::new();
        for i in 0..10u32 {
            pins.insert(
                (i + 1).to_string(),
                Pin {
                    name: format!("GPIO{i}"),
                    pin_type: PinType::Io,
                    alternate_functions: vec![],
                },
            );
        }
        for (n, name) in [("11", "QSPI_SS"), ("12", "QSPI_SCLK"), ("13", "QSPI_SD0")] {
            pins.insert(
                n.into(),
                Pin {
                    name: name.into(),
                    pin_type: PinType::Io,
                    alternate_functions: vec![],
                },
            );
        }
        pins.insert(
            "14".into(),
            Pin {
                name: "TESTEN".into(),
                pin_type: PinType::Reserved,
                alternate_functions: vec![],
            },
        );
        for n in ["15", "16", "17"] {
            pins.insert(
                n.into(),
                Pin {
                    name: "IOVDD".into(),
                    pin_type: PinType::Power,
                    alternate_functions: vec![],
                },
            );
        }
        pins.insert(
            "EP".into(),
            Pin {
                name: "GND".into(),
                pin_type: PinType::Ground,
                alternate_functions: vec![],
            },
        );

        Device {
            schema_version: "0.1".into(),
            kind: Kind::Device,
            id: DeviceId::from("ex/MCU"),
            manufacturer: ManufacturerId::from("ex"),
            family: None,
            pins,
            revisions: BTreeMap::new(),
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn power_pins_land_on_top_and_ground_on_bottom() {
        let symbol = build_symbol("MCU", &device_shaped_like_an_mcu());
        for pin in &symbol.pins {
            if pin.name == "IOVDD" {
                assert_eq!(pin.orientation, PinOrientation::Top, "pin {}", pin.number);
            }
            if pin.name == "GND" {
                assert_eq!(
                    pin.orientation,
                    PinOrientation::Bottom,
                    "pin {}",
                    pin.number
                );
            }
        }
    }

    #[test]
    fn same_named_pins_stack_at_one_position_without_losing_any() {
        // KiCad 10+ native pin stacking (see group_by_exact_name's
        // docs): pins sharing an exact name -- this fixture's three
        // IOVDD pins (device pin numbers 15, 16, 17) -- share one
        // schematic position, but each still exists as its own
        // SymbolPin with its own correct number. Confirms the "1
        // SymbolPin object per device pin" invariant genuinely holds
        // (this is not the same thing as merging pin *count*).
        let device = device_shaped_like_an_mcu();
        let symbol = build_symbol("MCU", &device);
        assert_eq!(
            symbol.pins.len(),
            device.pins.len(),
            "every device pin must still produce its own SymbolPin"
        );

        let iovdd: Vec<&SymbolPin> = symbol.pins.iter().filter(|p| p.name == "IOVDD").collect();
        assert_eq!(iovdd.len(), 3);
        let numbers: std::collections::BTreeSet<&str> =
            iovdd.iter().map(|p| p.number.as_str()).collect();
        assert_eq!(
            numbers,
            ["15", "16", "17"]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>(),
            "each stacked pin must keep its own correct number"
        );
        let positions: std::collections::HashSet<(i64, i64)> = iovdd
            .iter()
            .map(|p| {
                (
                    (p.position.x * 1000.0).round() as i64,
                    (p.position.y * 1000.0).round() as i64,
                )
            })
            .collect();
        assert_eq!(
            positions.len(),
            1,
            "all three IOVDD pins should share one schematic position, got {iovdd:?}"
        );
    }

    #[test]
    fn the_largest_family_lands_together_on_one_side() {
        let symbol = build_symbol("MCU", &device_shaped_like_an_mcu());
        let gpio_sides: std::collections::HashSet<PinOrientation> = symbol
            .pins
            .iter()
            .filter(|p| p.name.starts_with("GPIO"))
            .map(|p| p.orientation)
            .collect();
        assert_eq!(
            gpio_sides.len(),
            1,
            "all 10 GPIO pins should share one side, got {gpio_sides:?}"
        );
        // The 10-pin GPIO bus is strictly larger than any other
        // non-power/ground family (QSPI has 3, TESTEN has 1), so it
        // must be the side chosen as the "bus" side.
        let bus_side = *gpio_sides.iter().next().unwrap();
        let qspi_sides: std::collections::HashSet<PinOrientation> = symbol
            .pins
            .iter()
            .filter(|p| p.name.starts_with("QSPI"))
            .map(|p| p.orientation)
            .collect();
        assert_eq!(qspi_sides.len(), 1, "QSPI pins should share one side");
        assert_ne!(
            bus_side,
            *qspi_sides.iter().next().unwrap(),
            "the smaller QSPI family should not share the bus side"
        );
    }

    #[test]
    fn different_families_on_the_left_get_an_extra_gap() {
        let symbol = build_symbol("MCU", &device_shaped_like_an_mcu());
        let mut left: Vec<(String, f64)> = symbol
            .pins
            .iter()
            .filter(|p| p.orientation == PinOrientation::Left)
            .map(|p| (pin_group_key(&p.name), p.position.y))
            .collect();
        left.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap()); // top to bottom

        let gaps: Vec<(bool, f64)> = left
            .windows(2)
            .map(|pair| {
                let is_boundary = pair[0].0 != pair[1].0;
                (is_boundary, pair[0].1 - pair[1].1)
            })
            .collect();
        let boundary_gap = gaps
            .iter()
            .find(|(is_boundary, _)| *is_boundary)
            .map(|(_, g)| *g)
            .expect("the left side has more than one family, so a boundary must exist");
        let within_group_gap = gaps
            .iter()
            .find(|(is_boundary, _)| !*is_boundary)
            .map(|(_, g)| *g)
            .expect("QSPI has 3 pins, so a within-group gap must exist");
        assert!(
            boundary_gap > within_group_gap,
            "expected a family boundary ({boundary_gap}mm) to be wider than a \
             within-group gap ({within_group_gap}mm)"
        );
    }

    #[test]
    fn body_is_not_forced_square_when_side_counts_differ() {
        let symbol = build_symbol("MCU", &device_shaped_like_an_mcu());
        let Graphic {
            kind: GraphicKind::Rectangle { start, end },
        } = symbol.graphics[0];
        let width = (end.x - start.x).abs();
        let height = (end.y - start.y).abs();
        // 10 GPIO + 4 (QSPI+TESTEN) pins split left/right vs. 3 power +
        // 1 ground pins split top/bottom -- these must not coincide.
        assert!(
            (width - height).abs() > 1.0,
            "expected a non-square body, got {width} x {height}"
        );
    }

    /// A device with `gpio_count` GPIO pins (the entire non-power/ground
    /// bus, so there's no `left` side content to muddy comparisons), one
    /// IOVDD power pin, and one GND pin.
    fn device_with_a_gpio_count(gpio_count: u32) -> Device {
        let mut pins = BTreeMap::new();
        for i in 0..gpio_count {
            pins.insert(
                (i + 1).to_string(),
                Pin {
                    name: format!("GPIO{i}"),
                    pin_type: PinType::Io,
                    alternate_functions: vec![],
                },
            );
        }
        pins.insert(
            (gpio_count + 1).to_string(),
            Pin {
                name: "IOVDD".into(),
                pin_type: PinType::Power,
                alternate_functions: vec![],
            },
        );
        pins.insert(
            "EP".into(),
            Pin {
                name: "GND".into(),
                pin_type: PinType::Ground,
                alternate_functions: vec![],
            },
        );
        Device {
            schema_version: "0.1".into(),
            kind: Kind::Device,
            id: DeviceId::from("ex/BIGMCU"),
            manufacturer: ManufacturerId::from("ex"),
            family: None,
            pins,
            revisions: BTreeMap::new(),
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn bus_at_the_threshold_stays_a_single_unit() {
        let symbol = build_symbol("BIGMCU", &device_with_a_gpio_count(32));
        assert_eq!(symbol.units, vec![SymbolUnit { index: 1 }]);
        assert!(symbol.pins.iter().all(|p| p.unit == 1));
    }

    #[test]
    fn bus_over_the_threshold_splits_into_multiple_units() {
        let symbol = build_symbol("BIGMCU", &device_with_a_gpio_count(70));
        assert_eq!(
            symbol.units,
            vec![
                SymbolUnit { index: 1 },
                SymbolUnit { index: 2 },
                SymbolUnit { index: 3 },
            ]
        );

        // IOVDD/GND are shared -- unit 0, appearing exactly once each,
        // never duplicated per chunk.
        let iovdd: Vec<&SymbolPin> = symbol.pins.iter().filter(|p| p.name == "IOVDD").collect();
        assert_eq!(iovdd.len(), 1);
        assert_eq!(iovdd[0].unit, 0);
        let gnd: Vec<&SymbolPin> = symbol.pins.iter().filter(|p| p.name == "GND").collect();
        assert_eq!(gnd.len(), 1);
        assert_eq!(gnd[0].unit, 0);

        // The 70 GPIO pins partition into three chunks (32 + 32 + 6),
        // each pin appearing exactly once, ascending order preserved.
        for (unit, expected_range) in [(1u32, 0..32), (2, 32..64), (3, 64..70)] {
            let mut numbers_in_unit: Vec<u32> = symbol
                .pins
                .iter()
                .filter(|p| p.name.starts_with("GPIO") && p.unit == unit)
                .map(|p| p.name.trim_start_matches("GPIO").parse().unwrap())
                .collect();
            numbers_in_unit.sort();
            let expected: Vec<u32> = expected_range.collect();
            assert_eq!(numbers_in_unit, expected, "unit {unit}");
        }
    }

    #[test]
    fn shared_rectangle_covers_the_largest_bus_chunk() {
        // With no `left` side content in this fixture, the body height
        // is driven entirely by the bus. A single 32-pin unit and a
        // split-into-3 (32+32+6) symbol both have 32 as their largest
        // chunk, so they must produce the same body height.
        let single = build_symbol("BIGMCU", &device_with_a_gpio_count(32));
        let split = build_symbol("BIGMCU", &device_with_a_gpio_count(70));
        let height_of = |s: &PcbSymbol| {
            let Graphic {
                kind: GraphicKind::Rectangle { start, end },
            } = s.graphics[0];
            (end.y - start.y).abs()
        };
        assert!((height_of(&single) - height_of(&split)).abs() < 1e-9);
    }

    #[test]
    fn top_and_bottom_group_boundaries_get_the_unified_group_gap() {
        // Every side now uses the same flat grid-pitch half-extent and
        // the same GROUP_GAP_MM between different families -- top/bottom
        // is no longer a special case. Uses the same fixture shape as
        // `top_pins_dont_overlap_when_names_are_long` (distinctly-named
        // Power pins, so each is its own family) to get more than one
        // top-row position to compare.
        let mut pins = BTreeMap::new();
        for (n, name) in [("1", "USB_VDD"), ("2", "ADC_AVDD")] {
            pins.insert(
                n.into(),
                Pin {
                    name: name.into(),
                    pin_type: PinType::Power,
                    alternate_functions: vec![],
                },
            );
        }
        let device = Device {
            schema_version: "0.1".into(),
            kind: Kind::Device,
            id: DeviceId::from("ex/TWOPOWER"),
            manufacturer: ManufacturerId::from("ex"),
            family: None,
            pins,
            revisions: BTreeMap::new(),
            provenance: BTreeMap::new(),
        };
        let symbol = build_symbol("TWOPOWER", &device);
        let mut top_xs: Vec<f64> = symbol
            .pins
            .iter()
            .filter(|p| p.orientation == PinOrientation::Top)
            .map(|p| p.position.x)
            .collect();
        top_xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(top_xs.len(), 2);
        let gap = top_xs[1] - top_xs[0];
        let expected = 2.0 * PIN_PACKING_HALF_EXTENT_MM + GROUP_GAP_MM;
        assert!(
            (gap - expected).abs() < 1e-9,
            "expected {expected}mm between top-row family boundaries, got {gap}mm"
        );
    }

    #[test]
    fn corner_boosted_reach_only_boosts_the_first_and_last_entries() {
        // Three items 3.0mm apart center-to-center (half-extent 1.0
        // each -> positions -3, 0, 3 after centering). Reach should
        // equal 3.0 + 1.0 = 4.0 *without* boosting, since 1.0 < margin
        // only matters for the boosted first/last entries.
        let specs: Vec<PinSpec> = vec![
            (1.0, Some("A".into())),
            (1.0, Some("A".into())),
            (1.0, Some("A".into())),
        ];
        let margin = 5.0;
        let positions = pack_centered(&specs);
        let reach = corner_boosted_reach(&specs, &positions, margin);
        let outermost = positions.iter().map(|p| p.abs()).fold(0.0, f64::max);
        assert_eq!(
            reach,
            outermost + margin,
            "reach should use the boosted (corner-margin) half-extent for the outermost item"
        );
    }

    #[test]
    fn corner_boosted_reach_never_shrinks_an_already_wide_entry() {
        let margin = 5.0;
        let specs: Vec<PinSpec> = vec![(margin + 5.0, None)];
        let positions = pack_centered(&specs);
        assert_eq!(
            corner_boosted_reach(&specs, &positions, margin),
            margin + 5.0
        );
    }

    #[test]
    fn corner_boosting_does_not_change_same_side_neighbor_spacing() {
        // Regression guard for the real bug this fixed: boosting the
        // outermost pin's half-extent for the *rectangle-size*
        // calculation must not also inflate the gap to its own
        // same-side neighbor (e.g. GPIO29 pulling away from GPIO28) --
        // pack_centered must only ever see each pin's natural
        // half-extent, never the corner-boosted one.
        let symbol = build_symbol("MCU", &device_shaped_like_an_mcu());
        let mut right_ys: Vec<f64> = symbol
            .pins
            .iter()
            .filter(|p| p.orientation == PinOrientation::Right)
            .map(|p| p.position.y)
            .collect();
        right_ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in right_ys.windows(2) {
            let gap = pair[1] - pair[0];
            assert!(
                (gap - GRID_MM).abs() < 1e-9,
                "expected uniform {GRID_MM}mm GPIO row spacing throughout, \
                 including near the corners, got {gap}mm"
            );
        }
    }

    #[test]
    fn corner_margin_scales_with_this_devices_own_longest_name() {
        // corner_margin_mm is computed per-device now, not a flat
        // worst-case constant -- a device with a longer name gets a
        // bigger margin, one with only short names gets a smaller one
        // (and the rectangle shrinks accordingly).
        let short = device_with_names(&["A", "B"]);
        let long = device_with_names(&["VERY_LONG_PIN_NAME", "B"]);
        assert!(corner_margin_mm(&long) > corner_margin_mm(&short));
        // Never shrinks below the grid pitch, even for single-character
        // names.
        assert!(corner_margin_mm(&short) >= GRID_MM);
    }

    fn device_with_names(names: &[&str]) -> Device {
        let mut pins = BTreeMap::new();
        for (i, name) in names.iter().enumerate() {
            pins.insert(
                (i + 1).to_string(),
                Pin {
                    name: name.to_string(),
                    pin_type: PinType::Io,
                    alternate_functions: vec![],
                },
            );
        }
        Device {
            schema_version: "0.1".into(),
            kind: Kind::Device,
            id: DeviceId::from("ex/NAMES"),
            manufacturer: ManufacturerId::from("ex"),
            family: None,
            pins,
            revisions: BTreeMap::new(),
            provenance: BTreeMap::new(),
        }
    }

    /// Boundary-labeling sanity check (see this crate's discussion of
    /// prior art in build_symbol's docs): pack_centered's spacing only
    /// guards same-side neighbors, so this checks the four corners,
    /// where a side's outermost pin sits next to the *adjacent* side's
    /// outermost pin. Note this measures raw pin-stub-tip distance, not
    /// full label reach -- PIN_LENGTH alone already keeps stub tips
    /// comfortably apart for this fixture's short names, so this test
    /// mainly guards against a gross regression (e.g. corner pins ending
    /// up literally coincident), not the finer label-overlap margin
    /// that `corner_boosted_reach`'s own tests above cover directly.
    #[test]
    fn corner_adjacent_pins_from_different_sides_keep_a_safety_margin() {
        let device = device_shaped_like_an_mcu();
        let margin = corner_margin_mm(&device);
        let symbol = build_symbol("MCU", &device);

        let extreme = |side: PinOrientation, pick_max: bool, axis_x: bool| -> &SymbolPin {
            symbol
                .pins
                .iter()
                .filter(|p| p.orientation == side)
                .max_by(|a, b| {
                    let (av, bv) = if axis_x {
                        (a.position.x, b.position.x)
                    } else {
                        (a.position.y, b.position.y)
                    };
                    let ord = av.partial_cmp(&bv).unwrap();
                    if pick_max {
                        ord
                    } else {
                        ord.reverse()
                    }
                })
                .unwrap()
        };
        let distance = |a: &SymbolPin, b: &SymbolPin| {
            let dx = a.position.x - b.position.x;
            let dy = a.position.y - b.position.y;
            (dx * dx + dy * dy).sqrt()
        };

        // (side near this corner, pick the pin closest to the corner on
        // its own axis) for each of the four corners.
        let top_right = distance(
            extreme(PinOrientation::Top, true, true), // top pin closest to +x
            extreme(PinOrientation::Right, true, false), // right pin closest to +y
        );
        let top_left = distance(
            extreme(PinOrientation::Top, false, true), // top pin closest to -x
            extreme(PinOrientation::Left, true, false), // left pin closest to +y
        );
        let bottom_right = distance(
            extreme(PinOrientation::Bottom, true, true), // bottom pin closest to +x
            extreme(PinOrientation::Right, false, false), // right pin closest to -y
        );
        let bottom_left = distance(
            extreme(PinOrientation::Bottom, false, true), // bottom pin closest to -x
            extreme(PinOrientation::Left, false, false),  // left pin closest to -y
        );

        for (label, gap) in [
            ("top-right", top_right),
            ("top-left", top_left),
            ("bottom-right", bottom_right),
            ("bottom-left", bottom_left),
        ] {
            assert!(
                gap >= margin,
                "{label} corner: adjacent-side pins only {gap:.2}mm apart \
                 (want >= {margin}mm)"
            );
        }
    }

    #[test]
    fn no_two_pin_labels_overlap_anywhere_in_the_symbol() {
        // Holistic safety net, checking the actual end property rather
        // than any one mechanism: every pin's label -- its stub tip,
        // plus a name-length reach along the side's outward axis -- as
        // an axis-aligned box, and no two boxes anywhere in the whole
        // symbol may intersect. This is the single check that would
        // have caught the real corner-margin regression from earlier in
        // this file's history regardless of which piece of code
        // produced the positions, and it's what any future layout
        // change (this one included) has to keep satisfying.
        struct Box2 {
            min_x: f64,
            max_x: f64,
            min_y: f64,
            max_y: f64,
        }
        let device = device_shaped_like_an_mcu();
        let symbol = build_symbol("MCU", &device);
        // `p.position` is already the pin's outer tip -- build_symbol
        // bakes PIN_LENGTH_MM into it when placing each pin (e.g. left
        // pins get `x: -half_width - PIN_LENGTH_MM`) -- so the label
        // reach below starts from `p.position` directly, with no second
        // length addition.
        let boxes: Vec<Box2> = symbol
            .pins
            .iter()
            .filter(|p| !p.hidden)
            .map(|p| {
                let reach = p.name.chars().count() as f64 * CHAR_WIDTH_MM;
                match p.orientation {
                    PinOrientation::Left => Box2 {
                        min_x: p.position.x - reach,
                        max_x: p.position.x,
                        min_y: p.position.y - GRID_MM / 2.0,
                        max_y: p.position.y + GRID_MM / 2.0,
                    },
                    PinOrientation::Right => Box2 {
                        min_x: p.position.x,
                        max_x: p.position.x + reach,
                        min_y: p.position.y - GRID_MM / 2.0,
                        max_y: p.position.y + GRID_MM / 2.0,
                    },
                    PinOrientation::Top => Box2 {
                        min_x: p.position.x - GRID_MM / 2.0,
                        max_x: p.position.x + GRID_MM / 2.0,
                        min_y: p.position.y,
                        max_y: p.position.y + reach,
                    },
                    PinOrientation::Bottom => Box2 {
                        min_x: p.position.x - GRID_MM / 2.0,
                        max_x: p.position.x + GRID_MM / 2.0,
                        min_y: p.position.y - reach,
                        max_y: p.position.y,
                    },
                }
            })
            .collect();

        let named: Vec<(&str, &Box2)> = symbol
            .pins
            .iter()
            .filter(|p| !p.hidden)
            .map(|p| p.name.as_str())
            .zip(boxes.iter())
            .collect();
        // Same-side neighbors are packed to touch exactly (zero gap) by
        // design -- an epsilon absorbs the floating-point noise from
        // `pack_centered`'s centering shift without masking a real
        // overlap of any meaningful size.
        const EPS: f64 = 1e-6;
        for (i, (na, a)) in named.iter().enumerate() {
            for (nb, b) in &named[i + 1..] {
                let separated = a.max_x <= b.min_x + EPS
                    || b.max_x <= a.min_x + EPS
                    || a.max_y <= b.min_y + EPS
                    || b.max_y <= a.min_y + EPS;
                assert!(
                    separated,
                    "two pin labels overlap in the generated symbol: {na} [{:.2},{:.2}]x[{:.2},{:.2}] vs {nb} [{:.2},{:.2}]x[{:.2},{:.2}]",
                    a.min_x, a.max_x, a.min_y, a.max_y, b.min_x, b.max_x, b.min_y, b.max_y
                );
            }
        }
    }

    #[test]
    fn symbol_has_one_pin_per_device_pin() {
        let symbol = build_symbol("EX8", &device_8_pins());
        assert_eq!(symbol.pins.len(), 8);
        let numbers: std::collections::BTreeSet<&str> =
            symbol.pins.iter().map(|p| p.number.as_str()).collect();
        for i in 1..=8 {
            assert!(numbers.contains(i.to_string().as_str()));
        }
    }

    #[test]
    fn footprint_pad_count_matches_lead_count() {
        use openparts_mcad::{Body, BodyShape, Lead, MechanicalGeometry, Mounting, Point3, Size3};
        let geometry = MechanicalGeometry {
            body: Body {
                position: Point3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                size: Size3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
                shape: BodyShape::Box,
            },
            leads: vec![Lead {
                number: "1".into(),
                position: Point3 {
                    x: 1.0,
                    y: 2.0,
                    z: 0.0,
                },
                size: Size3 {
                    x: 0.3,
                    y: 0.2,
                    z: 0.1,
                },
                mounting: Mounting::Smd,
                drill: None,
            }],
            markers: vec![],
        };
        let footprint = build_footprint("EX1", &geometry);
        assert_eq!(footprint.pads.len(), 1);
        assert_eq!(footprint.pads[0].number, "1");
        assert_eq!(footprint.pads[0].position, Point2 { x: 1.0, y: 2.0 });
        assert_eq!(footprint.pads[0].size, (0.3, 0.2));
    }

    #[test]
    fn through_hole_pin_1_is_squared_and_the_rest_are_round() {
        // Matches a real KiCad reference footprint's own convention
        // (KiCad/kicad-footprints, Capacitor_THT.pretty/
        // CP_Radial_D5.0mm_P2.00mm.kicad_mod: pin 1 "rect", pin 2
        // "circle") -- applied generically to any through-hole part's
        // pin 1, not hardcoded to radial capacitors.
        use openparts_mcad::{Body, BodyShape, Lead, MechanicalGeometry, Mounting, Point3, Size3};
        let lead = |number: &str, x: f64| Lead {
            number: number.into(),
            position: Point3 { x, y: 0.0, z: 0.0 },
            size: Size3 {
                x: 1.6,
                y: 1.6,
                z: 3.0,
            },
            mounting: Mounting::ThroughHole,
            drill: Some(0.8),
        };
        let geometry = MechanicalGeometry {
            body: Body {
                position: Point3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                size: Size3 {
                    x: 5.0,
                    y: 5.0,
                    z: 11.0,
                },
                shape: BodyShape::Cylinder,
            },
            leads: vec![lead("1", -1.0), lead("2", 1.0)],
            markers: vec![],
        };
        let footprint = build_footprint("EX1", &geometry);
        let pad1 = footprint.pads.iter().find(|p| p.number == "1").unwrap();
        let pad2 = footprint.pads.iter().find(|p| p.number == "2").unwrap();
        assert_eq!(pad1.shape, PadShape::Rect);
        assert_eq!(pad2.shape, PadShape::Circle);
        for pad in &footprint.pads {
            assert_eq!(pad.mounting, Mounting::ThroughHole);
            assert_eq!(pad.drill, Some(0.8));
        }
    }

    #[test]
    fn smd_leads_stay_rect_regardless_of_pin_number() {
        use openparts_mcad::{Body, BodyShape, Lead, MechanicalGeometry, Mounting, Point3, Size3};
        let lead = |number: &str, x: f64| Lead {
            number: number.into(),
            position: Point3 { x, y: 0.0, z: 0.0 },
            size: Size3 {
                x: 0.5,
                y: 0.3,
                z: 0.1,
            },
            mounting: Mounting::Smd,
            drill: None,
        };
        let geometry = MechanicalGeometry {
            body: Body {
                position: Point3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                size: Size3 {
                    x: 1.6,
                    y: 0.8,
                    z: 0.45,
                },
                shape: BodyShape::Box,
            },
            leads: vec![lead("1", -0.5), lead("2", 0.5)],
            markers: vec![],
        };
        let footprint = build_footprint("EX1", &geometry);
        for pad in &footprint.pads {
            assert_eq!(pad.shape, PadShape::Rect);
            assert_eq!(pad.mounting, Mounting::Smd);
            assert_eq!(pad.drill, None);
        }
    }
}
