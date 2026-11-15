//! `openparts data migrate --to 0.2`: rewrites a 0.1 file in the 0.2
//! format, and moves the measurement conditions a parameter's locator
//! states into the value's `at` -- only where that is mechanical:
//!
//! - `SYM = <number> <unit>` with `SYM` a condition symbol (or one of its
//!   spellings: `Ta`, `Vin`, `iF`, `IO`...) and a unit of its quantity
//!   (`10V`, `5.7 A`, `25°C`, `+25 ℃`, `200 mAdc`);
//! - not a table-wide default (inside a parenthesis saying "unless
//!   otherwise noted"), not inside a quotation, not a range or a limit
//!   (`TJ = -40 to 125 °C`, `VF = 0.9 V Max`), not the key's own quantity
//!   (`VF` of `forward_voltage`);
//! - every symbol stated once in the locator; otherwise nothing is
//!   taken from it.
//!
//! The taken text (with the `@`/`,` that joined it) is cut from the
//! locator; nothing else in it changes.

use crate::format;
use openparts_core::{condition_symbol, Condition, Conditions, Quantity};
use serde_yaml::Value;

/// What one locator gives.
#[derive(Debug, Clone, PartialEq)]
pub struct Extraction {
    pub conditions: Conditions,
    /// The locator without the taken text.
    pub locator: String,
}

/// Spellings of a condition symbol in sources.
fn canonical_symbol(s: &str) -> Option<&'static str> {
    let alias = match s {
        "Ta" | "Tamb" => "TA",
        "Tj" => "TJ",
        "Tc" => "TC",
        "iF" => "IF",
        "Iout" | "IO" => "IOUT",
        "Vin" | "VI" => "VIN",
        "Vout" | "VO" => "VOUT",
        "Vcc" => "VCC",
        "Vdd" => "VDD",
        "Vce" => "VCE",
        "Vds" => "VDS",
        "Vgs" => "VGS",
        other => other,
    };
    condition_symbol(alias).map(|c| c.symbol)
}

/// Unit spellings -> the vocabulary spelling.
fn canonical_unit(u: &str) -> Option<&'static str> {
    Some(match u {
        "V" | "Vdc" => "V",
        "mV" => "mV",
        "A" | "Adc" => "A",
        "mA" | "mAdc" => "mA",
        "µA" | "uA" | "μA" => "µA",
        "nA" => "nA",
        "°C" | "℃" => "°C",
        "Hz" => "Hz",
        "kHz" => "kHz",
        "MHz" => "MHz",
        "ms" => "ms",
        "µs" | "us" | "μs" => "µs",
        "ns" => "ns",
        _ => return None,
    })
}

/// The symbols that are the key's own quantity, never its condition.
fn own_symbols(key: &str) -> &'static [&'static str] {
    match key {
        "forward_voltage" => &["VF"],
        "reverse_voltage" => &["VR"],
        "forward_current" => &["IF"],
        "drain_source_voltage" => &["VDS"],
        "drain_current" => &["ID"],
        "gate_threshold_voltage" => &["VGS"],
        "output_voltage" => &["VOUT"],
        "output_current" => &["IOUT"],
        "input_voltage_max" => &["VIN"],
        "zener_test_current" => &["IZT"],
        "collector_emitter_voltage" => &["VCE"],
        "collector_current" => &["IC"],
        "rated_current" => &["IR"],
        "frequency" => &["f"],
        _ => &[],
    }
}

/// One `SYM = value unit` in a text.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub start: usize,
    pub end: usize,
    pub symbol: &'static str,
    pub value: f64,
    pub unit: &'static str,
}

/// Every condition-shaped `SYM = value unit` in `text` (byte spans).
pub fn find_conditions(text: &str) -> Vec<Found> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let at = |i: usize| chars.get(i).map(|c| c.1);
    let byte = |i: usize| chars.get(i).map_or(text.len(), |c| c.0);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i].1;
        let boundary =
            i == 0 || at(i - 1).is_some_and(|p| !p.is_alphanumeric() && p != '_' && p != 'Δ');
        if !(c.is_ascii_alphabetic() && boundary) {
            i += 1;
            continue;
        }
        let mut j = i;
        while at(j).is_some_and(|c| c.is_ascii_alphabetic()) {
            j += 1;
        }
        let sym: String = chars[i..j].iter().map(|c| c.1).collect();
        let mut k = j;
        while at(k) == Some(' ') {
            k += 1;
        }
        if at(k) != Some('=') {
            i = j;
            continue;
        }
        k += 1;
        while at(k) == Some(' ') {
            k += 1;
        }
        let num_start = k;
        if matches!(at(k), Some('+' | '-' | '−')) {
            k += 1;
        }
        let digits = k;
        while at(k).is_some_and(|c| c.is_ascii_digit()) {
            k += 1;
        }
        if at(k) == Some('.') && at(k + 1).is_some_and(|c| c.is_ascii_digit()) {
            k += 1;
            while at(k).is_some_and(|c| c.is_ascii_digit()) {
                k += 1;
            }
        }
        if k == digits {
            i = j;
            continue;
        }
        let num: String = chars[num_start..k]
            .iter()
            .map(|c| if c.1 == '−' { '-' } else { c.1 })
            .collect();
        let mut u = k;
        while at(u) == Some(' ') {
            u += 1;
        }
        let unit_start = u;
        while at(u).is_some_and(|c| c.is_alphabetic() || c == '°' || c == '℃') {
            u += 1;
        }
        let unit_text: String = chars[unit_start..u].iter().map(|c| c.1).collect();
        let (Some(symbol), Some(unit), Ok(value)) = (
            canonical_symbol(&sym),
            canonical_unit(&unit_text),
            num.parse::<f64>(),
        ) else {
            i = j;
            continue;
        };
        out.push(Found {
            start: byte(i),
            end: byte(u),
            symbol,
            value,
            unit,
        });
        i = u;
    }
    out
}

/// Byte ranges inside quotations ("...", “...”, '...').
fn quoted(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut open: Option<(char, usize)> = None;
    let mut prev = ' ';
    for (i, c) in text.char_indices() {
        match open {
            Some((q, s)) => {
                let close = match q {
                    '“' => c == '”',
                    '\'' => c == '\'' && !text[i + 1..].starts_with(char::is_alphanumeric),
                    _ => c == q,
                };
                if close {
                    out.push((s, i));
                    open = None;
                }
            }
            None => {
                if c == '"' || c == '“' || (c == '\'' && !prev.is_alphanumeric()) {
                    open = Some((c, i));
                }
            }
        }
        prev = c;
    }
    if let Some((_, s)) = open {
        out.push((s, text.len()));
    }
    out
}

/// Byte ranges of parentheses that say "unless otherwise".
fn table_defaults(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut stack = Vec::new();
    for (i, c) in text.char_indices() {
        match c {
            '(' => stack.push(i),
            ')' => {
                if let Some(s) = stack.pop() {
                    if text[s..i].to_lowercase().contains("unless otherwise") {
                        out.push((s, i));
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(&s) = stack.first() {
        if text[s..].to_lowercase().contains("unless otherwise") {
            out.push((s, text.len()));
        }
    }
    out
}

/// The conditions `locator` states for a value of parameter `key`.
pub fn extract_conditions(locator: &str, key: &str) -> Option<Extraction> {
    let found = find_conditions(locator);
    if found.is_empty() {
        return None;
    }
    // Every symbol once (else which one applies is a judgement).
    for f in &found {
        if found.iter().filter(|g| g.symbol == f.symbol).count() > 1 {
            return None;
        }
    }
    let inside = |spans: &[(usize, usize)], f: &Found| {
        spans.iter().any(|(a, b)| f.start > *a && f.start < *b)
    };
    let (quotes, defaults) = (quoted(locator), table_defaults(locator));
    let own = own_symbols(key);
    let mut taken = Vec::new();
    for f in found {
        if own.contains(&f.symbol) || inside(&quotes, &f) || inside(&defaults, &f) {
            continue;
        }
        let after = locator[f.end..].trim_start();
        let lower = after.to_lowercase();
        let range_or_limit = after.starts_with(['±', '~', '–', '-', '…'])
            || lower.starts_with("to ")
            || lower.starts_with("max")
            || lower.starts_with("min")
            || lower.starts_with("typ")
            || after.starts_with('/');
        if range_or_limit {
            continue;
        }
        let Some(spec) = condition_symbol(f.symbol) else {
            continue;
        };
        if spec.quantity.unit_factor(f.unit).is_none() {
            continue;
        }
        if matches!(spec.quantity, Quantity::Temperature) && f.unit != "°C" {
            continue;
        }
        taken.push(f);
    }
    if taken.is_empty() {
        return None;
    }
    let mut conditions = Conditions::new();
    for f in &taken {
        conditions.insert(
            f.symbol.to_string(),
            Condition {
                value: Some(f.value),
                unit: f.unit.to_string(),
                ..Default::default()
            },
        );
    }
    // Cut the taken text, then the separators it leaves.
    let mut text = String::new();
    let mut last = 0;
    for f in &taken {
        let mut start = f.start;
        // An `@ ` or `at ` that introduced it.
        let before = locator[last..start].trim_end_matches(' ');
        if let Some(b) = before.strip_suffix('@') {
            start = last + b.len();
        } else if let Some(b) = before.strip_suffix(" at") {
            start = last + b.len() + 1;
        }
        text.push_str(&locator[last..start]);
        last = f.end;
    }
    text.push_str(&locator[last..]);
    Some(Extraction {
        conditions,
        locator: tidy(&text),
    })
}

/// Separators left over after a cut: `( , x)` -> `(x)`, `()` -> ``,
/// `x , : y` -> `x: y`, doubled spaces.
fn tidy(s: &str) -> String {
    let mut t = s.to_string();
    loop {
        let before = t.clone();
        for (from, to) in [
            ("  ", " "),
            (" ,", ","),
            (",,", ","),
            (", ,", ","),
            (",:", ":"),
            (", :", ":"),
            (" :", ":"),
            (";,", ";"),
            (",;", ";"),
            ("( ", "("),
            (" )", ")"),
            ("(,", "("),
            (",)", ")"),
            ("(;", "("),
            (";)", ")"),
            ("()", ""),
            ("@:", ":"),
            ("@,", ","),
            ("@)", ")"),
        ] {
            t = t.replace(from, to);
        }
        if t == before {
            break;
        }
    }
    let t = t
        .trim()
        .trim_end_matches([',', ';', '@'])
        .trim_start_matches([',', ';'])
        .trim();
    t.to_string()
}

/// Statistics of a migration.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Stats {
    pub files: usize,
    /// Conditions moved from locators into `at`.
    pub conditions: usize,
    /// Parameter values that got conditions.
    pub values: usize,
}

/// A 0.1 file's text as 0.2 (conditions extracted), or `None` when it is
/// already 0.2.
pub fn migrate_text(text: &str, stats: &mut Stats) -> Result<Option<String>, String> {
    let raw: Value = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
    if raw.get("schema_version").and_then(Value::as_str) == Some(format::SCHEMA_VERSION) {
        return Ok(None);
    }
    let mut doc = format::canonical(format::decode(raw)?)?;
    extract_into(&mut doc, stats);
    stats.files += 1;
    format::write(&doc, &format::comments::extract(text)).map(Some)
}

/// Moves conditions from parameter locators into `at` (in-memory shape).
pub fn extract_into(doc: &mut Value, stats: &mut Stats) {
    let Some(Value::Mapping(params)) = doc.get("parameters").cloned() else {
        return;
    };
    let mut params = params;
    let mut prov = doc.get("provenance").cloned().unwrap_or(Value::Null);
    for (k, values) in params.iter_mut() {
        let key = k.as_str().unwrap_or("").to_string();
        let numeric = openparts_core::parameter_spec(&key)
            .is_some_and(|s| matches!(s.shape, openparts_core::ParameterShape::Numeric(_)));
        let Value::Sequence(list) = values else {
            continue;
        };
        if !numeric || list.len() != 1 {
            continue;
        }
        let ptr = format!("/parameters/{key}");
        let Some(Value::Sequence(entries)) = prov.get_mut(ptr.as_str()) else {
            continue;
        };
        // One citation with a locator: its conditions are the value's.
        if entries.len() != 1 {
            continue;
        }
        let Some(loc) = entries[0]
            .get("locator")
            .and_then(Value::as_str)
            .map(String::from)
        else {
            continue;
        };
        let Some(x) = extract_conditions(&loc, &key) else {
            continue;
        };
        let value = &mut list[0];
        if value
            .get("at")
            .is_some_and(|a| a.as_mapping().is_some_and(|m| !m.is_empty()))
        {
            continue;
        }
        let at = serde_yaml::to_value(&x.conditions).unwrap_or(Value::Null);
        if let Value::Mapping(m) = value {
            m.insert("at".into(), at);
        }
        if let Value::Mapping(e) = &mut entries[0] {
            e.insert("locator".into(), Value::String(x.locator));
        }
        stats.conditions += x.conditions.len();
        stats.values += 1;
    }
    if let Value::Mapping(d) = doc {
        d.insert("parameters".into(), Value::Mapping(params));
        if !prov.is_null() {
            d.insert("provenance".into(), prov);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(l: &str, key: &str) -> Option<(Vec<(String, String)>, String)> {
        extract_conditions(l, key).map(|x| {
            (
                openparts_core::condition::ordered(&x.conditions)
                    .into_iter()
                    .map(|(s, c)| (s.clone(), c.label()))
                    .collect(),
                x.locator,
            )
        })
    }

    #[test]
    fn takes_only_unambiguous_conditions() {
        let pairs = |v: &[(&str, &str)]| -> Vec<(String, String)> {
            v.iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect()
        };
        assert_eq!(
            ex("Electrical Characteristics (TJ=25°C), RDS(ON) Static Drain-Source On-Resistance, VGS=10V, ID=5.7A: Typ 18, Max 26.5 mΩ", "rds_on"),
            Some((
                pairs(&[("VGS", "10 V"), ("ID", "5.7 A"), ("TJ", "25 °C")]),
                "Electrical Characteristics, RDS(ON) Static Drain-Source On-Resistance: Typ 18, Max 26.5 mΩ".into()
            ))
        );
        assert_eq!(
            ex(
                "Absolute Maximum Ratings, Continuous Drain Current ID, TA=25°C: 5.7 A",
                "drain_current"
            ),
            Some((
                pairs(&[("TA", "25 °C")]),
                "Absolute Maximum Ratings, Continuous Drain Current ID: 5.7 A".into()
            ))
        );
        assert_eq!(
            ex("Electrical Characteristics, Forward Voltage Drop VF, B220/A row: Max 0.50 V @ IF = 2.0A, TA = +25°C", "forward_voltage"),
            Some((pairs(&[("IF", "2 A"), ("TA", "25 °C")]), "Electrical Characteristics, Forward Voltage Drop VF, B220/A row: Max 0.50 V".into()))
        );
        // A table default, a quotation, a range, a repeated symbol, the
        // key's own quantity: nothing taken.
        assert_eq!(ex("Absolute Maximum Ratings (TA=25°C unless otherwise noted), Drain-Source Voltage VDS: 30 V", "drain_source_voltage"), None);
        assert_eq!(
            ex(
                "6.6 Electrical Characteristics, \"VO = 5 V, VI = 10 V\"",
                "output_voltage"
            ),
            None
        );
        assert_eq!(
            ex(
                "ORDERING INFORMATION: TJ = -40 °C to +125 °C",
                "output_voltage"
            ),
            None
        );
        assert_eq!(ex("ELECTRICAL CHARACTERISTICS (Vin = 10 V, IO = 500 mA), Dropout Voltage (IO = 1.0 A, TJ = 25°C): Typ 2.0 Vdc", "dropout_voltage"), None);
        assert_eq!(
            ex("Forward Voltage VF = 0.9 V @ IF = 10 mA", "forward_voltage")
                .unwrap()
                .0,
            pairs(&[("IF", "10 mA")])
        );
        assert_eq!(ex("Electro-Optical Characteristics (Ta=25℃), Dominant Wavelength λd @ IF=20mA: Typ. 624 nm", "wavelength"),
            Some((pairs(&[("IF", "20 mA"), ("TA", "25 °C")]), "Electro-Optical Characteristics, Dominant Wavelength λd: Typ. 624 nm".into())));
        assert_eq!(
            ex(
                "Electrical Properties, Rated Current 2 IR 2, ΔT = 40 K: 2000 mA max.",
                "rated_current"
            ),
            None
        );
    }
}
