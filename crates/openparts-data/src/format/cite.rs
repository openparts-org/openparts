//! Citations (`src`) and quantity strings of the 0.2 file format.
//!
//! A citation is `[<source-id>] [p<page>] ["<locator>"]`, e.g.
//! `p2 "Electrical Characteristics, RDS(ON)"`; the source id defaults to
//! the file's `source:`. The full form is a mapping
//! `{ source, page, locator }`; several citations are a list.
//!
//! A quantity is `<number> <unit>` (`26.5 mΩ`), a tolerance `±<number>
//! <unit>`, a condition range `<a>..<b> <unit>`.

use serde_yaml::{Mapping, Number, Value};

/// One citation as the in-memory provenance entry (`source`, `page`,
/// `locator`).
#[derive(Debug, Clone, PartialEq)]
pub struct Cite {
    pub source: String,
    pub page: Option<u64>,
    pub locator: Option<String>,
}

impl Cite {
    pub fn to_value(&self) -> Value {
        let mut m = Mapping::new();
        m.insert("source".into(), self.source.clone().into());
        if let Some(p) = self.page {
            m.insert("page".into(), Value::Number(p.into()));
        }
        if let Some(l) = &self.locator {
            m.insert("locator".into(), l.clone().into());
        }
        Value::Mapping(m)
    }

    pub fn from_value(v: &Value) -> Result<Cite, String> {
        let m = v
            .as_mapping()
            .ok_or_else(|| "a provenance entry must be a mapping".to_string())?;
        let source = m
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| "a provenance entry needs a source".to_string())?
            .to_string();
        let page = match m.get("page") {
            None | Some(Value::Null) => None,
            Some(p) => Some(
                p.as_u64()
                    .ok_or_else(|| "page must be a whole number".to_string())?,
            ),
        };
        let locator = match m.get("locator") {
            None | Some(Value::Null) => None,
            Some(l) => Some(
                l.as_str()
                    .ok_or_else(|| "locator must be text".to_string())?
                    .to_string(),
            ),
        };
        for k in m.keys() {
            let k = k.as_str().unwrap_or("");
            if !matches!(k, "source" | "page" | "locator") {
                return Err(format!("unknown field \"{k}\" in a citation"));
            }
        }
        Ok(Cite {
            source,
            page,
            locator,
        })
    }

    /// The shorthand, omitting the source when it is `default`; `None`
    /// when the shorthand can't say it (an id that reads as a page or
    /// has spaces).
    pub fn shorthand(&self, default: Option<&str>) -> Option<String> {
        let id_ok = !self.source.is_empty()
            && !self.source.contains(char::is_whitespace)
            && !self.source.starts_with('"')
            && !is_page(&self.source);
        if !id_ok {
            return None;
        }
        let mut parts = Vec::new();
        if default != Some(self.source.as_str()) {
            parts.push(self.source.clone());
        }
        if let Some(p) = self.page {
            parts.push(format!("p{p}"));
        }
        if let Some(l) = &self.locator {
            parts.push(format!("\"{l}\""));
        }
        if parts.is_empty() {
            parts.push(self.source.clone());
        }
        Some(parts.join(" "))
    }
}

fn is_page(token: &str) -> bool {
    token.len() > 1 && token.starts_with('p') && token[1..].bytes().all(|b| b.is_ascii_digit())
}

/// Parses one shorthand citation.
pub fn parse_shorthand(s: &str, default: Option<&str>) -> Result<Cite, String> {
    let mut rest = s.trim_start();
    let mut source = None;
    let mut page = None;
    let token_end = |r: &str| r.find(char::is_whitespace).unwrap_or(r.len());
    if !rest.starts_with('"') && !rest.is_empty() {
        let end = token_end(rest);
        let token = &rest[..end];
        if is_page(token) {
            page = Some(
                token[1..]
                    .parse()
                    .map_err(|_| format!("bad page in {s:?}"))?,
            );
        } else {
            source = Some(token.to_string());
        }
        rest = rest[end..].trim_start();
    }
    if page.is_none() && !rest.starts_with('"') && !rest.is_empty() {
        let end = token_end(rest);
        let token = &rest[..end];
        if !is_page(token) {
            return Err(format!(
                "citation {s:?}: expected `p<page>` or a \"quoted locator\" after the source"
            ));
        }
        page = Some(
            token[1..]
                .parse()
                .map_err(|_| format!("bad page in {s:?}"))?,
        );
        rest = rest[end..].trim_start();
    }
    let locator = if rest.is_empty() {
        None
    } else if rest.len() >= 2 && rest.starts_with('"') && rest.ends_with('"') {
        Some(rest[1..rest.len() - 1].to_string())
    } else {
        return Err(format!(
            "citation {s:?}: the locator must be in double quotes, at the end"
        ));
    };
    let source = match source.or_else(|| default.map(String::from)) {
        Some(s) => s,
        None => {
            return Err(format!(
                "citation {s:?} names no source and the file has no `source:`"
            ))
        }
    };
    if page.is_none() && locator.is_none() && s.trim().is_empty() {
        return Err("an empty citation".into());
    }
    Ok(Cite {
        source,
        page,
        locator,
    })
}

/// A `src` value: one citation (shorthand or mapping) or a list.
pub fn parse_src(v: &Value, default: Option<&str>) -> Result<Vec<Cite>, String> {
    let one = |v: &Value| -> Result<Cite, String> {
        match v {
            Value::String(s) => parse_shorthand(s, default),
            Value::Mapping(m) => {
                let mut m = m.clone();
                if !m.contains_key("source") {
                    let d = default
                        .ok_or("a citation names no source and the file has no `source:`")?;
                    m.insert("source".into(), d.into());
                }
                Cite::from_value(&Value::Mapping(m))
            }
            _ => Err("src must be a citation or a list of citations".into()),
        }
    };
    match v {
        Value::Sequence(items) if !items.is_empty() => items.iter().map(one).collect(),
        Value::Sequence(_) => Err("src is an empty list".into()),
        other => Ok(vec![one(other)?]),
    }
}

/// `<number> <unit>` -> (number, unit).
pub fn parse_quantity(s: &str) -> Result<(f64, String), String> {
    let t = s.trim().replace('−', "-");
    let starts_ok = t
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.'));
    let mut best = None;
    if starts_ok {
        for (i, _) in t.char_indices().skip(1).chain([(t.len(), ' ')]) {
            if let Ok(v) = t[..i].trim_end().parse::<f64>() {
                if v.is_finite() {
                    best = Some((i, v));
                }
            }
        }
    }
    let Some((i, v)) = best else {
        return Err(format!(
            "{s:?} is not a quantity (`<number> <unit>`, e.g. \"26.5 mΩ\")"
        ));
    };
    let unit = t[i..].trim();
    if unit.is_empty() {
        return Err(format!("{s:?} has no unit (write e.g. \"{s} V\")"));
    }
    if unit.contains(char::is_whitespace) {
        return Err(format!("{s:?}: the unit can't contain spaces"));
    }
    Ok((v, unit.to_string()))
}

/// The shortest decimal that reads back as `v`.
pub fn number(v: f64) -> String {
    openparts_core::parameter::plain_number(v)
}

pub fn quantity(v: f64, unit: &str) -> String {
    format!("{} {unit}", number(v))
}

pub fn num_value(v: f64) -> Value {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        Value::Number(Number::from(v as i64))
    } else {
        Value::Number(Number::from(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorthand_round_trips() {
        let d = Some("aos/AO3400A-DS");
        for (text, cite) in [
            (
                "p1 \"Title\"",
                Cite {
                    source: "aos/AO3400A-DS".into(),
                    page: Some(1),
                    locator: Some("Title".into()),
                },
            ),
            (
                "aos/PO-00001 p2 \"Table \"A\", row b\"",
                Cite {
                    source: "aos/PO-00001".into(),
                    page: Some(2),
                    locator: Some("Table \"A\", row b".into()),
                },
            ),
            (
                "\"Pin table\"",
                Cite {
                    source: "aos/AO3400A-DS".into(),
                    page: None,
                    locator: Some("Pin table".into()),
                },
            ),
            (
                "p7",
                Cite {
                    source: "aos/AO3400A-DS".into(),
                    page: Some(7),
                    locator: None,
                },
            ),
            (
                "st/DS1@rev2",
                Cite {
                    source: "st/DS1@rev2".into(),
                    page: None,
                    locator: None,
                },
            ),
            (
                "p3 \"\"",
                Cite {
                    source: "aos/AO3400A-DS".into(),
                    page: Some(3),
                    locator: Some(String::new()),
                },
            ),
        ] {
            assert_eq!(parse_shorthand(text, d).unwrap(), cite, "{text}");
            assert_eq!(cite.shorthand(d).unwrap(), text);
        }
        assert!(parse_shorthand("p1 Title", d).is_err());
        assert!(parse_shorthand("p1 \"x\"", None).is_err());
    }

    #[test]
    fn quantities() {
        assert_eq!(parse_quantity("26.5 mΩ").unwrap(), (26.5, "mΩ".into()));
        assert_eq!(parse_quantity("-0.525 mm").unwrap(), (-0.525, "mm".into()));
        assert_eq!(parse_quantity("0.4mm").unwrap(), (0.4, "mm".into()));
        assert_eq!(parse_quantity("10 %").unwrap(), (10.0, "%".into()));
        assert_eq!(parse_quantity("1e-3 F").unwrap(), (1e-3, "F".into()));
        assert!(parse_quantity("30").is_err());
        assert!(parse_quantity("V").is_err());
        assert!(parse_quantity("inf V").is_err());
        assert_eq!(quantity(26.5, "mΩ"), "26.5 mΩ");
        assert_eq!(quantity(30.0, "V"), "30 V");
    }
}
