//! The deterministic YAML writer of the 0.2 format: block style for the
//! top level and for long entries, flow style (`{ typ: 18 mΩ, src: p2
//! "..." }`) for an entry that fits on one line, aligned values within a
//! nested mapping, blank lines between groups of top-level entries, and
//! a scalar quoted only when a plain one would read back differently.

use super::comments::Comments;
use serde_yaml::{Mapping, Value};

/// Longest line a flow entry may make.
pub const WIDTH: usize = 100;
/// Values of a nested mapping align up to this key width.
const ALIGN_MAX: usize = 26;

fn plain_ok(s: &str, flow: bool) -> bool {
    if s.is_empty()
        || s.trim() != s
        || s.contains(['\n', '\r', '\t'])
        || s.chars().any(char::is_control)
    {
        return false;
    }
    let doc = if flow {
        format!("[{s}]")
    } else {
        format!("k: {s}")
    };
    match serde_yaml::from_str::<Value>(&doc) {
        Ok(Value::Sequence(v)) if flow => v.len() == 1 && v[0].as_str() == Some(s),
        Ok(Value::Mapping(m)) if !flow => {
            m.len() == 1 && m.get("k").and_then(Value::as_str) == Some(s)
        }
        _ => false,
    }
}

fn quote(s: &str) -> String {
    let control = s.chars().any(|c| c.is_control());
    if s.contains('"') && !control {
        return format!("'{}'", s.replace('\'', "''"));
    }
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn scalar(v: &Value, flow: bool) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                n.to_string()
            } else {
                super::cite::number(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => {
            if plain_ok(s, flow) {
                s.clone()
            } else {
                quote(s)
            }
        }
        Value::Tagged(t) => scalar(&t.value, flow),
        _ => String::new(),
    }
}

fn key_text(k: &Value) -> String {
    scalar(k, true)
}

/// The one-line flow form of `v`.
pub fn flow(v: &Value) -> String {
    match v {
        Value::Mapping(m) if m.is_empty() => "{}".into(),
        Value::Mapping(m) => {
            let items: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("{}: {}", key_text(k), flow(v)))
                .collect();
            format!("{{ {} }}", items.join(", "))
        }
        Value::Sequence(s) => {
            let items: Vec<String> = s.iter().map(flow).collect();
            format!("[{}]", items.join(", "))
        }
        other => scalar(other, true),
    }
}

fn is_collection(v: &Value) -> bool {
    matches!(v, Value::Mapping(_) | Value::Sequence(_))
}

/// A top-level mapping of records (pins, parameters, dimensions) or a
/// list of records: always one entry per line.
fn force_block(v: &Value) -> bool {
    match v {
        Value::Mapping(m) => {
            let records: Vec<&Value> = m
                .iter()
                .filter(|(k, _)| k.as_str() != Some("src"))
                .map(|(_, v)| v)
                .collect();
            records.len() > 1 && records.iter().all(|v| is_collection(v))
        }
        Value::Sequence(s) => s.iter().any(Value::is_mapping),
        _ => false,
    }
}

fn single_line(v: &Value) -> Option<String> {
    match v {
        Value::Mapping(_) | Value::Sequence(_) => Some(flow(v)),
        Value::String(s) if s.contains('\n') => None,
        other => Some(scalar(other, false)),
    }
}

/// `key: value` at `indent`, block or flow.
fn entry(out: &mut String, indent: usize, key: &str, pad: usize, v: &Value, top: bool) {
    let lead = " ".repeat(indent);
    let label = format!("{key}:");
    let w = pad.max(key.chars().count()) + 1;
    let width_ok = |s: &str| indent + w + 1 + s.chars().count() <= WIDTH;
    let simple = !is_collection(v)
        || matches!(v, Value::Mapping(m) if m.is_empty())
        || matches!(v, Value::Sequence(s) if s.is_empty());
    if simple {
        out.push_str(&format!("{lead}{label:w$} {}\n", scalar_block(v)));
        return;
    }
    if !(top && force_block(v)) {
        if let Some(line) = single_line(v) {
            if width_ok(&line) {
                out.push_str(&format!("{lead}{label:w$} {line}\n"));
                return;
            }
        }
    }
    out.push_str(&format!("{lead}{key}:\n"));
    block(out, indent + 2, v);
}

fn scalar_block(v: &Value) -> String {
    match v {
        Value::Mapping(m) if m.is_empty() => "{}".into(),
        Value::Sequence(s) if s.is_empty() => "[]".into(),
        other => scalar(other, false),
    }
}

/// Whether `entry` would write `v` after `key` on one line.
fn fits(indent: usize, key_width: usize, v: &Value) -> bool {
    if !is_collection(v) {
        return !matches!(v, Value::String(s) if s.contains('\n'));
    }
    single_line(v).is_some_and(|s| indent + key_width + 2 + s.chars().count() <= WIDTH)
}

/// The children of a nested collection.
fn block(out: &mut String, indent: usize, v: &Value) {
    match v {
        Value::Mapping(m) => {
            let keys: Vec<String> = m.keys().map(key_text).collect();
            // Tables (pins, parameters, dimensions) align their values;
            // a record's fields don't.
            let table = m
                .iter()
                .all(|(k, v)| k.as_str() == Some("src") || is_collection(v));
            let widest = if table {
                keys.iter().map(|k| k.chars().count()).max().unwrap_or(0)
            } else {
                0
            };
            let try_pad = widest.min(ALIGN_MAX);
            let one_line: Vec<bool> = m.values().map(|v| fits(indent, try_pad, v)).collect();
            let pad = keys
                .iter()
                .zip(&one_line)
                .filter(|(k, one)| table && **one && k.chars().count() <= ALIGN_MAX)
                .map(|(k, _)| k.chars().count())
                .max()
                .unwrap_or(0);
            for ((k, v), one) in keys.iter().zip(m.values()).zip(one_line) {
                let p = if one && k.chars().count() <= pad {
                    pad
                } else {
                    0
                };
                entry(out, indent, k, p, v, false);
            }
        }
        Value::Sequence(items) => {
            let lead = " ".repeat(indent);
            for item in items {
                let line = single_line(item);
                match line {
                    Some(l) if indent + 2 + l.chars().count() <= WIDTH || !is_collection(item) => {
                        out.push_str(&format!("{lead}- {l}\n"));
                    }
                    _ => match item {
                        Value::Mapping(m) if !m.is_empty() => {
                            let mut inner = String::new();
                            block(&mut inner, indent + 2, item);
                            // `- ` takes the place of the first key's indent.
                            let inner =
                                inner.replacen(&format!("{lead}  "), &format!("{lead}- "), 1);
                            out.push_str(&inner);
                        }
                        _ => {
                            out.push_str(&format!("{lead}-\n"));
                            block(out, indent + 2, item);
                        }
                    },
                }
            }
        }
        other => out.push_str(&format!("{}{}\n", " ".repeat(indent), scalar(other, false))),
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Class {
    Plain,
    Flow,
    Block,
}

/// The whole document: comments, then the top-level entries with a
/// blank line between groups (scalars, one-line records, blocks).
pub fn document(doc: &Mapping, comments: &Comments) -> String {
    let mut out = String::new();
    for line in &comments.header {
        out.push_str(line);
        out.push('\n');
    }
    let mut prev: Option<Class> = None;
    for (k, v) in doc {
        let key = key_text(k);
        let mut text = String::new();
        entry(&mut text, 0, &key, 0, v, true);
        // A wrapped scalar fact (`{ value: x, src: ... }`) reads as a
        // scalar.
        let wrapped = matches!(v, Value::Mapping(m)
            if m.len() == 2 && m.contains_key("value") && m.contains_key("src")
                && !m.get("value").is_some_and(|x| x.is_mapping() || x.is_sequence()));
        let class = if text.lines().count() > 1 {
            Class::Block
        } else if matches!(v, Value::Mapping(m) if !m.is_empty()) && !wrapped {
            Class::Flow
        } else {
            Class::Plain
        };
        let note = comments
            .before
            .get(k.as_str().unwrap_or(""))
            .filter(|c| !c.is_empty());
        if let Some(p) = prev {
            if p != class || class == Class::Block || note.is_some() {
                out.push('\n');
            }
        }
        if let Some(lines) = note {
            for line in lines {
                out.push_str(line);
                out.push('\n');
            }
        }
        out.push_str(&text);
        prev = Some(class);
    }
    if !comments.trailer.is_empty() {
        out.push('\n');
        for line in &comments.trailer {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_only_when_needed() {
        let s = |x: &str, f: bool| scalar(&Value::String(x.into()), f);
        assert_eq!(s("aos/AO3400A", false), "aos/AO3400A");
        assert_eq!(s("0.2", false), "\"0.2\"");
        assert_eq!(s("1", true), "\"1\"");
        assert_eq!(s("true", false), "\"true\"");
        assert_eq!(s("p1 \"Title\"", false), "p1 \"Title\"");
        assert_eq!(s("p1 \"Abs, VDS\"", true), "'p1 \"Abs, VDS\"'");
        assert_eq!(s("p1 \"a: b\"", false), "'p1 \"a: b\"'");
        assert_eq!(s("a #b", false), "\"a #b\"");
        assert_eq!(s("26.5 mΩ", true), "26.5 mΩ");
        assert_eq!(s("±10 %", true), "±10 %");
        assert_eq!(s("-0.5 mm", true), "-0.5 mm");
        assert_eq!(s("", false), "\"\"");
    }

    #[test]
    fn writes_flow_when_short_and_block_when_long() {
        let doc: Mapping = serde_yaml::from_str(
            "kind: part\nexistence: { status: probable, src: p1 \"Title\" }\nparameters:\n  polarity: { value: n_channel, src: p1 \"Title\" }\n  rds_on: { typ: 18 mΩ, max: 26.5 mΩ, at: { VGS: 10 V }, src: 'p2 \"Electrical Characteristics, RDS(ON) Static Drain-Source On-Resistance\"' }\n",
        )
        .unwrap();
        let text = document(&doc, &Comments::default());
        assert_eq!(
            text,
            "kind: part\n\nexistence: { status: probable, src: p1 \"Title\" }\n\nparameters:\n  polarity: { value: n_channel, src: p1 \"Title\" }\n  rds_on:\n    typ: 18 mΩ\n    max: 26.5 mΩ\n    at: { VGS: 10 V }\n    src: p2 \"Electrical Characteristics, RDS(ON) Static Drain-Source On-Resistance\"\n"
        );
        let back: Mapping = serde_yaml::from_str(&text).unwrap();
        assert_eq!(back, doc);
    }
}
