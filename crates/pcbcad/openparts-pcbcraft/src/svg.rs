//! A `pcbcraft-symbol` record as an SVG image, drawn as PCBCraft draws
//! it: each unit's outline and drawing texts, every pin's lead, and its
//! number and name where PCBCraft letters them (`PinAppearance::graphics`,
//! the rules [`crate::labels`] checks layouts against). Styled like the
//! KiCad preview ([`openparts_pcbcad::symbol_svg`]).

use openparts_pcbcad::symbol_svg::{Anchor, Baseline, Item, SymbolSheet, TextRole, UnitDrawing};
use serde_json::Value;
use std::collections::HashMap;

const NM: f64 = 1e6;

/// The ink's capital height of a text lettered `height_nm` high with a
/// pin stroke (PCBCraft scales the font to the height less the stroke).
fn cap_mm(height_nm: i64) -> f64 {
    let stroke = crate::labels::PIN_STROKE_NM.min(height_nm / 3).max(1);
    (height_nm - stroke) as f64 / NM
}

fn point(v: &Value) -> Option<(f64, f64)> {
    match v {
        Value::Array(a) => Some((a.first()?.as_f64()? / NM, a.get(1)?.as_f64()? / NM)),
        Value::Object(_) => Some((v.get("x_nm")?.as_f64()? / NM, v.get("y_nm")?.as_f64()? / NM)),
        _ => None,
    }
}

fn anchor(align: &str) -> Anchor {
    match align {
        "start" => Anchor::Start,
        "end" => Anchor::End,
        _ => Anchor::Middle,
    }
}

fn drawing(items: &mut Vec<Item>, shape: &Value) -> Option<()> {
    match shape.get("type")?.as_str()? {
        "rect" => {
            let (x0, y0) = point(shape.get("min_nm")?)?;
            let (x1, y1) = point(shape.get("max_nm")?)?;
            items.push(Item::Body { x0, y0, x1, y1 });
        }
        "graphic" => {
            let g = shape.get("graphic")?.get("geometry")?;
            match g.get("type")?.as_str()? {
                "text" => {
                    let rotation = g.get("rotation").and_then(Value::as_u64).unwrap_or(0);
                    items.push(Item::Text {
                        at: point(g.get("at")?)?,
                        text: g.get("text")?.as_str()?.to_string(),
                        size: cap_mm(g.get("height_nm")?.as_i64()?),
                        anchor: anchor(g.get("align").and_then(Value::as_str).unwrap_or("start")),
                        baseline: Baseline::Centre,
                        vertical: rotation == 270,
                        // The exporter moves pin names into the drawing.
                        role: TextRole::PinName,
                    });
                }
                "line" => items.push(Item::Line {
                    from: point(g.get("from")?)?,
                    to: point(g.get("to")?)?,
                }),
                "polyline" => {
                    let points: Vec<(f64, f64)> = g
                        .get("points")?
                        .as_array()?
                        .iter()
                        .filter_map(point)
                        .collect();
                    let closed = g.get("closed").and_then(Value::as_bool).unwrap_or(false);
                    let mut segments: Vec<_> = points.windows(2).map(|w| (w[0], w[1])).collect();
                    if closed && points.len() > 2 {
                        segments.push((points[points.len() - 1], points[0]));
                    }
                    for (from, to) in segments {
                        items.push(Item::Line { from, to });
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    Some(())
}

fn pin(items: &mut Vec<Item>, p: &Value, names: &HashMap<&str, &str>) -> Option<()> {
    let terminal = p.get("terminal")?.as_str()?;
    let at = point(p.get("at_nm")?)?;
    let a = p.get("appearance")?;
    let orientation = a.get("orientation")?.as_str()?;
    let l = a.get("length_nm")?.as_f64()? / NM;
    let end = match orientation {
        "right" => (at.0 + l, at.1),
        "left" => (at.0 - l, at.1),
        "down" => (at.0, at.1 + l),
        _ => (at.0, at.1 - l),
    };
    items.push(Item::Lead { from: at, to: end });
    let flag = |k: &str| a.get(k).and_then(Value::as_bool).unwrap_or(false);
    let height = |k: &str| a.get(k).and_then(Value::as_i64).unwrap_or(1_270_000);
    let vertical = matches!(orientation, "up" | "down");
    if flag("show_number") {
        let mid = ((at.0 + end.0) / 2.0, (at.1 + end.1) / 2.0);
        let (at, anchor) = if vertical {
            ((mid.0 + 0.4, mid.1), Anchor::Start)
        } else {
            ((mid.0, mid.1 - 0.6), Anchor::Middle)
        };
        items.push(Item::Text {
            at,
            text: terminal.to_string(),
            size: cap_mm(height("number_height_nm")),
            anchor,
            baseline: Baseline::Centre,
            vertical: false,
            role: TextRole::PinNumber,
        });
    }
    let name = names.get(terminal).copied().unwrap_or("");
    if flag("show_name") && !name.is_empty() {
        let style = a.get("style").and_then(Value::as_str).unwrap_or("line");
        let inset = if style.contains("clock") { 0.7 } else { 0.4 };
        let (at, anchor) = match orientation {
            "right" => ((end.0 + inset, end.1), Anchor::Start),
            "left" => ((end.0 - inset, end.1), Anchor::End),
            "up" => ((end.0, end.1 - 0.7), Anchor::Middle),
            _ => ((end.0, end.1 + 0.7), Anchor::Middle),
        };
        items.push(Item::Text {
            at,
            text: name.to_string(),
            size: cap_mm(height("name_height_nm")),
            anchor,
            baseline: Baseline::Centre,
            vertical: false,
            role: TextRole::PinName,
        });
    }
    Some(())
}

/// Renders a `pcbcraft-symbol` document (its JSON text): every unit,
/// side by side, labelled with its id when there are several.
pub fn render_symbol_svg(symbol_json: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(symbol_json).map_err(|e| e.to_string())?;
    let bad = || "not a pcbcraft-symbol document".to_string();
    let names: HashMap<&str, &str> = v
        .get("terminals")
        .and_then(Value::as_array)
        .ok_or_else(bad)?
        .iter()
        .filter_map(|t| {
            Some((
                t.get("id")?.as_str()?,
                t.get("name").and_then(Value::as_str).unwrap_or(""),
            ))
        })
        .collect();
    let units = v.get("units").and_then(Value::as_array).ok_or_else(bad)?;
    let several = units.len() > 1;
    let units = units
        .iter()
        .map(|u| {
            let mut items = Vec::new();
            for shape in u
                .get("drawing")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                drawing(&mut items, shape);
            }
            for p in u
                .get("pins")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                pin(&mut items, p, &names);
            }
            let id = u.get("id").and_then(Value::as_str).unwrap_or("");
            UnitDrawing {
                label: several.then(|| format!("Unit {id}")),
                items,
            }
        })
        .collect();
    Ok(SymbolSheet {
        title: v
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        flavour: "pcbcraft",
        units,
    }
    .render())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_units_pins_and_moved_names() {
        let doc = serde_json::json!({
            "format": "pcbcraft-symbol/4",
            "name": "Two & units",
            "terminals": [
                {"id": "1", "name": "VCC", "electrical_type": "power_in"},
                {"id": "2", "name": "OUT", "electrical_type": "output"},
                {"id": "3", "electrical_type": "passive"}
            ],
            "units": [
                {"id": "A", "drawing": [
                    {"type": "rect", "min_nm": [-5080000, -5080000], "max_nm": [5080000, 5080000]},
                    {"type": "graphic", "graphic": {"geometry": {"type": "text",
                        "at": {"x_nm": 0, "y_nm": 3000000}, "text": "VCC", "height_nm": 1270000,
                        "rotation": 270, "align": "end"}, "stroke_nm": 120000, "fill": "none"}}
                 ], "pins": [
                    {"terminal": "1", "at_nm": [0, 7620000], "appearance": {"orientation": "up",
                     "length_nm": 2540000, "style": "line", "show_name": false, "show_number": true,
                     "name_height_nm": 1270000, "number_height_nm": 1270000}},
                    {"terminal": "2", "at_nm": [7620000, 0], "appearance": {"orientation": "left",
                     "length_nm": 2540000, "style": "line", "show_name": true, "show_number": true,
                     "name_height_nm": 1270000, "number_height_nm": 1270000}}
                 ]},
                {"id": "B", "drawing": [], "pins": [
                    {"terminal": "3", "at_nm": [-7620000, 0], "appearance": {"orientation": "right",
                     "length_nm": 2540000, "style": "line", "show_name": true, "show_number": true,
                     "name_height_nm": 1270000, "number_height_nm": 1270000}}
                ]}
            ]
        });
        let svg = render_symbol_svg(&doc.to_string()).unwrap();
        assert!(svg.contains("class=\"sym sym-pcbcraft\""));
        assert!(svg.contains("<title>Two &amp; units</title>"));
        assert_eq!(svg.matches("class=\"lead\"").count(), 3);
        assert_eq!(svg.matches("class=\"num\"").count(), 3);
        // The moved name is drawn turned; the unnamed terminal has none.
        assert!(
            svg.contains("rotate(-90)\" dy=\"0.36em\" font-size=\"1.597\" text-anchor=\"end\">VCC")
        );
        assert_eq!(svg.matches("class=\"name\"").count(), 2);
        assert!(svg.contains(">Unit A</text>") && svg.contains(">Unit B</text>"));
        assert!(render_symbol_svg("{}").is_err());
    }
}
