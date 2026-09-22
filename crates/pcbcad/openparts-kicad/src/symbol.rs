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

    for graphic in &symbol.graphics {
        let openparts_pcbcad::GraphicKind::Rectangle { start, end } = graphic.kind;
        out.push_str(&format!("    (symbol \"{}_0_1\"\n", symbol.name));
        out.push_str(&format!(
            "      (rectangle (start {:.2} {:.2}) (end {:.2} {:.2})\n",
            start.x, start.y, end.x, end.y
        ));
        out.push_str("        (stroke (width 0.254) (type default))\n");
        out.push_str("        (fill (type background))))\n");
    }

    out.push_str(&format!("    (symbol \"{}_1_1\"\n", symbol.name));
    for pin in &symbol.pins {
        out.push_str(&format!(
            "      (pin {} line (at {:.2} {:.2} {:.0}) (length {:.2})\n",
            electrical_type_str(pin.electrical_type),
            pin.position.x,
            pin.position.y,
            orientation_angle(pin.orientation),
            pin.length,
        ));
        out.push_str(&format!(
            "        (name \"{}\" (effects (font (size 1.27 1.27))))\n",
            pin.name
        ));
        out.push_str(&format!(
            "        (number \"{}\" (effects (font (size 1.27 1.27)))))\n",
            pin.number
        ));
    }
    out.push_str("    )\n");
    out.push_str("  )\n");
    out.push_str(")\n");
    out
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

        let name_line = lines.next().expect("pin must be followed by a name line");
        let name = extract_quoted(name_line).expect("name line must contain a quoted string");

        let number_line = lines.next().expect("pin must be followed by a number line");
        let number = extract_quoted(number_line).expect("number line must contain a quoted string");

        pins.push(ParsedPin { number, name, electrical_type, x, y, angle });
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
    }
}
