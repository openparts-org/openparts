use openparts_pcbcad::{PcbSymbol, PinElectricalType, PinOrientation};

fn electrical_type_str(t: PinElectricalType) -> &'static str {
    match t {
        PinElectricalType::Input => "input",
        PinElectricalType::Output => "output",
        PinElectricalType::Bidirectional => "bidirectional",
        PinElectricalType::PowerIn => "power_in",
        PinElectricalType::Passive => "passive",
        PinElectricalType::NoConnect => "no_connect",
        PinElectricalType::Unspecified => "unspecified",
    }
}

fn orientation_angle(o: PinOrientation) -> f64 {
    match o {
        PinOrientation::Left => 0.0,
        PinOrientation::Bottom => 90.0,
        PinOrientation::Right => 180.0,
        PinOrientation::Top => 270.0,
    }
}

/// Renders a KiCad 6+ `.kicad_sym` symbol library containing one symbol.
pub fn render_symbol(symbol: &PcbSymbol) -> String {
    let mut out = String::new();
    out.push_str("(kicad_symbol_lib\n");
    out.push_str("  (version 20211014)\n");
    out.push_str("  (generator openparts)\n");
    out.push_str(&format!("  (symbol \"{}\"\n", symbol.name));
    out.push_str("    (in_bom yes) (on_board yes)\n");

    // Unit 0: the body outline, plus any pins shared across every
    // numbered unit (only present when build_symbol split a large bus
    // across multiple units -- see its docs). These must land in one
    // combined block, not two separately-opened-and-closed blocks that
    // happen to share a name: KiCad's `.kicad_sym` format requires each
    // "{name}_{unit}_{style}" sub-symbol name to appear exactly once.
    let has_unit_zero_pins = symbol.pins.iter().any(|p| p.unit == 0);
    if !symbol.graphics.is_empty() || has_unit_zero_pins {
        out.push_str(&format!("    (symbol \"{}_0_1\"\n", symbol.name));
        for graphic in &symbol.graphics {
            let openparts_pcbcad::GraphicKind::Rectangle { start, end } = graphic.kind;
            out.push_str(&format!(
                "      (rectangle (start {:.2} {:.2}) (end {:.2} {:.2})\n",
                start.x, start.y, end.x, end.y
            ));
            out.push_str("        (stroke (width 0.254) (type default))\n");
            out.push_str("        (fill (type background)))\n");
        }
        for pin in symbol.pins.iter().filter(|p| p.unit == 0) {
            write_pin(&mut out, pin);
        }
        out.push_str("    )\n");
    }

    // Ordinarily just unit 1. A symbol whose bus was split across
    // multiple units gets one block per chunk here, each depending on
    // unit 0 (above) for the shared pins KiCad composites in for them.
    let mut unit_indices: Vec<u32> = symbol.units.iter().map(|u| u.index).collect();
    unit_indices.sort_unstable();
    for unit in unit_indices {
        out.push_str(&format!("    (symbol \"{}_{}_1\"\n", symbol.name, unit));
        for pin in symbol.pins.iter().filter(|p| p.unit == unit) {
            write_pin(&mut out, pin);
        }
        out.push_str("    )\n");
    }
    out.push_str("  )\n");
    out.push_str(")\n");
    out
}

fn write_pin(out: &mut String, pin: &openparts_pcbcad::SymbolPin) {
    out.push_str(&format!(
        "      (pin {} line (at {:.2} {:.2} {:.0}) (length {:.2})\n",
        electrical_type_str(pin.electrical_type),
        pin.position.x,
        pin.position.y,
        orientation_angle(pin.orientation),
        pin.length,
    ));
    if pin.hidden {
        out.push_str("        (hide yes)\n");
    }
    out.push_str(&format!(
        "        (name \"{}\" (effects (font (size 1.27 1.27))))\n",
        pin.name
    ));
    out.push_str(&format!(
        "        (number \"{}\" (effects (font (size 1.27 1.27)))))\n",
        pin.number
    ));
}

/// Minimal read-back check used by tests (Testing and Quality
/// Specification section 9: "Independent read-back" / "Semantic
/// checks"). Not a general KiCad parser -- it only understands the exact
/// shape [`render_symbol`] emits, which is enough to verify our own
/// generator round-trips correctly.
#[derive(Debug, PartialEq)]
pub struct ParsedPin {
    pub number: String,
    pub name: String,
    pub electrical_type: String,
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub hidden: bool,
}

pub fn parse_symbol_pins(text: &str) -> Vec<ParsedPin> {
    let mut pins = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("(pin ") {
            continue;
        }
        // (pin <type> line (at <x> <y> <angle>) (length <len>)
        let rest = trimmed.trim_start_matches("(pin ").trim_end();
        let mut parts = rest.split_whitespace();
        let electrical_type = parts.next().unwrap_or_default().to_string();
        let at_idx = rest.find("(at ").expect("pin line must contain (at ...)");
        let after_at = &rest[at_idx + 4..];
        let close = after_at.find(')').expect("(at ...) must be closed");
        let mut nums = after_at[..close].split_whitespace();
        let x: f64 = nums.next().unwrap().parse().unwrap();
        let y: f64 = nums.next().unwrap().parse().unwrap();
        let angle: f64 = nums.next().unwrap().parse().unwrap();

        // An optional `(hide yes)` line sits between the pin's own line
        // and its name -- consume it here rather than mistaking it for
        // the name line.
        let hidden = lines
            .peek()
            .is_some_and(|l| l.trim_start().starts_with("(hide"));
        if hidden {
            lines.next();
        }

        let name_line = lines.next().expect("pin must be followed by a name line");
        let name = extract_quoted(name_line).expect("name line must contain a quoted string");

        let number_line = lines.next().expect("pin must be followed by a number line");
        let number = extract_quoted(number_line).expect("number line must contain a quoted string");

        pins.push(ParsedPin {
            number,
            name,
            electrical_type,
            x,
            y,
            angle,
            hidden,
        });
    }
    pins
}

fn extract_quoted(s: &str) -> Option<String> {
    let start = s.find('"')? + 1;
    let end = start + s[start..].find('"')?;
    Some(s[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_pcbcad::{Graphic, GraphicKind, Point2, SymbolPin, SymbolUnit};

    #[test]
    fn round_trips_pin_number_name_and_position() {
        let symbol = PcbSymbol {
            name: "TEST".into(),
            units: vec![SymbolUnit { index: 1 }],
            pins: vec![SymbolPin {
                number: "7".into(),
                name: "NRST".into(),
                electrical_type: PinElectricalType::Input,
                position: Point2 { x: -10.16, y: 2.54 },
                length: 2.54,
                orientation: PinOrientation::Left,
                unit: 1,
                hidden: false,
            }],
            graphics: vec![Graphic {
                kind: GraphicKind::Rectangle {
                    start: Point2 { x: -7.62, y: 7.62 },
                    end: Point2 { x: 7.62, y: -7.62 },
                },
            }],
        };

        let text = render_symbol(&symbol);
        let parsed = parse_symbol_pins(&text);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].number, "7");
        assert_eq!(parsed[0].name, "NRST");
        assert_eq!(parsed[0].electrical_type, "input");
        assert!((parsed[0].x - (-10.16)).abs() < 1e-6);
        assert!((parsed[0].y - 2.54).abs() < 1e-6);
        assert!((parsed[0].angle - 0.0).abs() < 1e-6);
        assert!(!parsed[0].hidden);
    }

    #[test]
    fn hidden_pins_render_hide_yes_and_still_round_trip() {
        // A stacked duplicate (see openparts-pcbcad's SymbolPin::hidden
        // docs): hidden and hidden pins must not be confused with each
        // other by the reader when they're adjacent in the file.
        let symbol = PcbSymbol {
            name: "TEST".into(),
            units: vec![SymbolUnit { index: 1 }],
            pins: vec![
                SymbolPin {
                    number: "1".into(),
                    name: "IOVDD".into(),
                    electrical_type: PinElectricalType::PowerIn,
                    position: Point2 { x: 0.0, y: 10.0 },
                    length: 2.54,
                    orientation: PinOrientation::Top,
                    unit: 1,
                    hidden: false,
                },
                SymbolPin {
                    number: "10".into(),
                    name: "IOVDD".into(),
                    electrical_type: PinElectricalType::Passive,
                    position: Point2 { x: 0.0, y: 10.0 },
                    length: 2.54,
                    orientation: PinOrientation::Top,
                    unit: 1,
                    hidden: true,
                },
            ],
            graphics: vec![],
        };

        let text = render_symbol(&symbol);
        assert!(text.contains("(hide yes)"));

        let parsed = parse_symbol_pins(&text);
        assert_eq!(parsed.len(), 2);
        assert!(!parsed[0].hidden);
        assert_eq!(parsed[0].number, "1");
        assert!(parsed[1].hidden);
        assert_eq!(parsed[1].number, "10");
        assert_eq!(parsed[1].electrical_type, "passive");
    }

    fn pin(number: &str, name: &str, unit: u32) -> SymbolPin {
        SymbolPin {
            number: number.into(),
            name: name.into(),
            electrical_type: PinElectricalType::Bidirectional,
            position: Point2 { x: 0.0, y: 0.0 },
            length: 2.54,
            orientation: PinOrientation::Right,
            unit,
            hidden: false,
        }
    }

    #[test]
    fn multi_unit_symbols_get_one_block_per_unit_plus_a_shared_unit_zero_block() {
        let symbol = PcbSymbol {
            name: "BIGMCU".into(),
            units: vec![SymbolUnit { index: 1 }, SymbolUnit { index: 2 }],
            pins: vec![
                pin("57", "IOVDD", 0),
                pin("1", "GPIO0", 1),
                pin("2", "GPIO1", 1),
                pin("33", "GPIO32", 2),
            ],
            graphics: vec![Graphic {
                kind: GraphicKind::Rectangle {
                    start: Point2 { x: -7.62, y: 7.62 },
                    end: Point2 { x: 7.62, y: -7.62 },
                },
            }],
        };

        let text = render_symbol(&symbol);
        // Exactly one "_0_1" block: the rectangle graphic and unit-0's
        // shared pins (IOVDD here) must land in the *same* opened block,
        // not two separately-opened blocks that happen to share a name
        // (KiCad requires each sub-symbol name to appear once).
        assert_eq!(text.matches("(symbol \"BIGMCU_0_1\"\n").count(), 1);
        assert!(text.contains("(symbol \"BIGMCU_1_1\"\n"));
        assert!(text.contains("(symbol \"BIGMCU_2_1\"\n"));
        assert!(text.contains("(rectangle "));
        assert!(text.contains("(name \"IOVDD\""));
        // Every opened "(symbol ...)" block is properly closed: the
        // whole file has balanced parentheses.
        assert_eq!(text.matches('(').count(), text.matches(')').count());
        // Unit 0 comes first, since the numbered units depend on it.
        let pos0 = text.find("BIGMCU_0_1").unwrap();
        let pos1 = text.find("BIGMCU_1_1").unwrap();
        let pos2 = text.find("BIGMCU_2_1").unwrap();
        assert!(pos0 < pos1 && pos1 < pos2);

        // Every pin round-trips regardless of which unit block it's in.
        let parsed = parse_symbol_pins(&text);
        let numbers: std::collections::BTreeSet<&str> =
            parsed.iter().map(|p| p.number.as_str()).collect();
        assert_eq!(
            numbers,
            ["57", "1", "2", "33"]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        );
    }
}
