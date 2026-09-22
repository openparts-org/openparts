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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pad {
    pub number: String,
    pub position: Point2,
    pub size: (f64, f64),
    pub shape: PadShape,
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

/// Rough per-character glyph width for KiCad's default 1.27mm pin-name
/// font, used only to estimate how much room a top/bottom pin's label
/// needs along the axis it's stacked on (its text is rotated 90° there,
/// so it runs parallel to that axis and can collide with a neighbor's
/// text if they're packed at a fixed grid step regardless of name
/// length). Not exact font metrics -- consistent with this project's
/// existing "structurally representative, not pixel-perfect" approach to
/// generated geometry.
const CHAR_WIDTH_MM: f64 = 1.0;

fn text_half_extent(name: &str) -> f64 {
    (name.chars().count() as f64 * CHAR_WIDTH_MM / 2.0).max(GRID_MM / 2.0)
}

/// A pin's required half-extent along a packing axis, plus an optional
/// group key (used by `pack_centered` to insert extra gaps between
/// different groups).
type PinSpec = (f64, Option<String>);

/// Minimum clearance guaranteed between a side's outermost pin and the
/// adjacent side's outermost pin at each of the symbol's four corners
/// (e.g. the last power pin on top vs. the first bus pin on the right).
///
/// KiCad draws more than just the pin name next to each pin -- it also
/// shows the pin's electrical-type description (e.g. "power input"),
/// which this crate has no way to measure the exact rendered size of.
/// Rather than trend the per-character text-width estimate elsewhere in
/// this file (already an approximation) even higher across the board,
/// this margin targets exactly the place that's actually at risk: two
/// perpendicular sides meeting at a corner, where the estimate errors
/// on each axis compound. See `widen_corner_ends`.
const CORNER_MARGIN_MM: f64 = 4.0;

/// Widens the first and last entries of `specs` (a side's outermost
/// pins, which sit closest to the symbol's corners) to at least
/// [`CORNER_MARGIN_MM`], leaving every pin in between untouched.
///
/// Growing a corner pin's own half-extent genuinely increases its
/// clearance from an adjacent side's corner pin, not just the overall
/// symbol size: each side's outermost pin sits exactly `half_extent`
/// away from the body edge on its own axis (by construction, see
/// `widest_reach` in `build_symbol`), so two perpendicular corner pins
/// are `sqrt(a.half_extent² + b.half_extent²)` apart regardless of how
/// large the body itself is. Guaranteeing each individually clears
/// `CORNER_MARGIN_MM` guarantees their combined distance does too.
fn widen_corner_ends(specs: &mut [PinSpec]) {
    if let Some(first) = specs.first_mut() {
        first.0 = first.0.max(CORNER_MARGIN_MM);
    }
    if let Some(last) = specs.last_mut() {
        last.0 = last.0.max(CORNER_MARGIN_MM);
    }
}

/// Packs `specs` (each pin's required half-extent along the packing
/// axis, plus an optional group key) end to end -- consecutive pins get
/// exactly `half_extent[i] + half_extent[i+1]` apart, plus one extra
/// `GRID_MM` whenever the group key changes between them -- then shifts
/// the whole run so it's centered on 0.
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
                GRID_MM
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

/// Above this many pins, the bus family (e.g. a GPIO bank) is split
/// across multiple KiCad symbol units rather than stacked on one
/// ever-taller right edge -- see [`build_symbol`]'s docs.
const BUS_UNIT_MAX_PINS: usize = 32;

/// Builds a Symbol from a Device's pins, grouped onto sides the way a
/// hand-authored MCU symbol is: `power`-type pins on top, the
/// `ground`-type pin(s) on bottom, the single largest same-family signal
/// group (e.g. a GPIO bus) on the right, and every other pin -- grouped
/// by family via [`pin_group_key`], each family kept contiguous and
/// separated from the next by an extra gap -- on the left. Pins on each
/// side are packed via [`pack_centered`]: top/bottom spacing grows with
/// label length (so long power-pin names don't collide), left/right
/// spacing stays a fixed row height but widens between different
/// families. Body width/height follow from the resulting extents,
/// never forcing a square.
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

    // Top/bottom: spacing driven by label length, same family-boundary
    // gap treatment as left/right -- e.g. RP2040's six IOVDD pins pack
    // tightly together, then get a gap before DVDD, ADC_AVDD, the
    // VREG_VIN/VREG_VOUT pair (sharing the "VREG" family), and USB_VDD.
    let mut top_specs: Vec<PinSpec> = top
        .iter()
        .map(|n| {
            (
                text_half_extent(&device.pins[*n].name),
                Some(pin_group_key(&device.pins[*n].name)),
            )
        })
        .collect();
    let mut bottom_specs: Vec<PinSpec> = bottom
        .iter()
        .map(|n| {
            (
                text_half_extent(&device.pins[*n].name),
                Some(pin_group_key(&device.pins[*n].name)),
            )
        })
        .collect();
    // Left: fixed row height, but grouped -- an extra gap opens up
    // wherever the pin family changes (e.g. between QSPI_* and USB_*).
    let mut left_specs: Vec<PinSpec> = left
        .iter()
        .map(|n| (GRID_MM / 2.0, Some(pin_group_key(&device.pins[*n].name))))
        .collect();
    // Guarantee corner clearance (see CORNER_MARGIN_MM) before packing --
    // widening an outermost pin's half-extent shifts where it actually
    // sits, so this must happen before pack_centered runs, not after.
    widen_corner_ends(&mut top_specs);
    widen_corner_ends(&mut bottom_specs);
    widen_corner_ends(&mut left_specs);

    let top_pos = pack_centered(&top_specs);
    let bottom_pos = pack_centered(&bottom_specs);
    let left_pos = pack_centered(&left_specs);

    // The bus (`right`) is split into units of at most
    // BUS_UNIT_MAX_PINS once it exceeds that size -- see this
    // function's docs/flowchart. Below that size this is just one
    // "chunk" containing every bus pin, identical to the pre-split
    // behavior.
    let bus_chunks: Vec<&[&String]> = right.chunks(BUS_UNIT_MAX_PINS).collect();
    let splitting = bus_chunks.len() > 1;
    let chunk_specs_pos: Vec<(Vec<PinSpec>, Vec<f64>)> = bus_chunks
        .iter()
        .map(|chunk| {
            let mut specs: Vec<PinSpec> = chunk
                .iter()
                .map(|n| (GRID_MM / 2.0, Some(pin_group_key(&device.pins[*n].name))))
                .collect();
            widen_corner_ends(&mut specs);
            let pos = pack_centered(&specs);
            (specs, pos)
        })
        .collect();

    // The body must reach past each outermost pin's own label extent,
    // not just its center position -- otherwise the outermost label
    // hangs off past the drawn rectangle edge.
    let widest_reach = |specs: &[PinSpec], positions: &[f64]| {
        specs
            .iter()
            .zip(positions.iter())
            .map(|((half, _), pos)| pos.abs() + half)
            .fold(GRID_MM, f64::max)
    };
    let half_width =
        widest_reach(&top_specs, &top_pos).max(widest_reach(&bottom_specs, &bottom_pos));
    // Every unit shares one rectangle, so this must cover the largest
    // bus chunk, not just whichever chunk happens to be last.
    let half_height = chunk_specs_pos
        .iter()
        .map(|(s, p)| widest_reach(s, p))
        .fold(widest_reach(&left_specs, &left_pos), f64::max);

    let mut pins = Vec::with_capacity(numbers.len());
    let mut push_pin = |number: &String, position: Point2, side: PinOrientation, unit: u32| {
        let pin = &device.pins[number];
        pins.push(SymbolPin {
            number: number.clone(),
            name: pin.name.clone(),
            electrical_type: pin.pin_type.into(),
            position,
            length: PIN_LENGTH_MM,
            orientation: side,
            unit,
        });
    };
    // Top/bottom/left are shared across every unit once splitting, so
    // they land in KiCad's unit-0 sentinel; otherwise (the common case)
    // everything is unit 1, exactly as before this feature existed.
    let shared_unit: u32 = if splitting { 0 } else { 1 };
    // `pack_centered` lays its first item at the most-negative position;
    // left/top's first item is conventionally the "start" (top-most /
    // right-most) side, the opposite direction, so their axis is negated.
    for (number, &y) in left.iter().zip(left_pos.iter()) {
        push_pin(
            number,
            Point2 {
                x: -half_width - PIN_LENGTH_MM,
                y: -y,
            },
            PinOrientation::Left,
            shared_unit,
        );
    }
    for (number, &x) in bottom.iter().zip(bottom_pos.iter()) {
        push_pin(
            number,
            Point2 {
                x,
                y: -half_height - PIN_LENGTH_MM,
            },
            PinOrientation::Bottom,
            shared_unit,
        );
    }
    for (number, &x) in top.iter().zip(top_pos.iter()) {
        push_pin(
            number,
            Point2 {
                x: -x,
                y: half_height + PIN_LENGTH_MM,
            },
            PinOrientation::Top,
            shared_unit,
        );
    }
    for (chunk_index, chunk) in bus_chunks.iter().enumerate() {
        let (_, chunk_pos) = &chunk_specs_pos[chunk_index];
        let unit = if splitting { chunk_index as u32 + 1 } else { 1 };
        for (number, &y) in chunk.iter().zip(chunk_pos.iter()) {
            push_pin(
                number,
                Point2 {
                    x: half_width + PIN_LENGTH_MM,
                    y,
                },
                PinOrientation::Right,
                unit,
            );
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
pub fn build_footprint(name: &str, geometry: &MechanicalGeometry) -> PcbFootprint {
    let pads = geometry
        .leads
        .iter()
        .map(|lead| Pad {
            number: lead.number.clone(),
            position: Point2 {
                x: lead.position.x,
                y: lead.position.y,
            },
            size: (lead.size.x, lead.size.y),
            shape: PadShape::Rect,
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
        assert!((cross_group_gap - (2.0 + GRID_MM)).abs() < 1e-9); // + GRID_MM
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
    fn widen_corner_ends_only_touches_the_first_and_last_entries() {
        let mut specs: Vec<PinSpec> = vec![
            (1.0, Some("A".into())),
            (1.0, Some("A".into())),
            (1.0, Some("A".into())),
        ];
        widen_corner_ends(&mut specs);
        assert_eq!(specs[0].0, CORNER_MARGIN_MM, "first entry should widen");
        assert_eq!(specs[1].0, 1.0, "middle entry must stay untouched");
        assert_eq!(specs[2].0, CORNER_MARGIN_MM, "last entry should widen");
    }

    #[test]
    fn widen_corner_ends_never_shrinks_an_already_wide_entry() {
        let mut specs: Vec<PinSpec> = vec![(CORNER_MARGIN_MM + 5.0, None)];
        widen_corner_ends(&mut specs);
        assert_eq!(specs[0].0, CORNER_MARGIN_MM + 5.0);
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
    /// that `widen_corner_ends`'s own tests above cover directly.
    #[test]
    fn corner_adjacent_pins_from_different_sides_keep_a_safety_margin() {
        let symbol = build_symbol("MCU", &device_shaped_like_an_mcu());

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
                gap >= CORNER_MARGIN_MM,
                "{label} corner: adjacent-side pins only {gap:.2}mm apart \
                 (want >= {CORNER_MARGIN_MM}mm)"
            );
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
        use openparts_mcad::{Body, Lead, MechanicalGeometry, Point3, Size3};
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
            }],
            markers: vec![],
        };
        let footprint = build_footprint("EX1", &geometry);
        assert_eq!(footprint.pads.len(), 1);
        assert_eq!(footprint.pads[0].number, "1");
        assert_eq!(footprint.pads[0].position, Point2 { x: 1.0, y: 2.0 });
        assert_eq!(footprint.pads[0].size, (0.3, 0.2));
    }
}
