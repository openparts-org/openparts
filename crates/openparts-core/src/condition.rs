//! Measurement conditions of a parameter value (Canonical Data
//! Specification section 11.2): the `at` map of a value, symbol ->
//! quantity, e.g. `at: { VGS: 10 V, ID: 5.7 A }`.
//!
//! Symbols come from a closed vocabulary ([`CONDITION_SYMBOLS`]), each
//! with one physical quantity whose unit table it uses. A condition is
//! one value (`10 V`) or a range (`-40..85 °C`).

use crate::parameter::Quantity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One condition: a value, or a `min`..`max` range, in `unit`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    pub unit: String,
}

/// Symbol -> condition.
pub type Conditions = BTreeMap<String, Condition>;

/// One condition symbol of the vocabulary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionSymbol {
    pub symbol: &'static str,
    pub quantity: Quantity,
    pub meaning: &'static str,
}

const fn sym(symbol: &'static str, quantity: Quantity, meaning: &'static str) -> ConditionSymbol {
    ConditionSymbol {
        symbol,
        quantity,
        meaning,
    }
}

/// The closed symbol vocabulary, in display order. Grows like the
/// parameter vocabulary: add a symbol here and in the spec table.
pub const CONDITION_SYMBOLS: &[ConditionSymbol] = &[
    sym("VGS", Quantity::Voltage, "gate-source voltage"),
    sym("VDS", Quantity::Voltage, "drain-source voltage"),
    sym("VCE", Quantity::Voltage, "collector-emitter voltage"),
    sym("VCC", Quantity::Voltage, "supply voltage"),
    sym("VDD", Quantity::Voltage, "supply voltage"),
    sym(
        "VS",
        Quantity::Voltage,
        "total supply voltage V+ - V- (an amplifier's ±15 V is 30 V)",
    ),
    sym("VIN", Quantity::Voltage, "input voltage"),
    sym(
        "VCM",
        Quantity::Voltage,
        "input common-mode voltage (a current-sense or difference amplifier)",
    ),
    sym("VOUT", Quantity::Voltage, "output voltage"),
    sym("VR", Quantity::Voltage, "reverse voltage"),
    sym("VF", Quantity::Voltage, "forward voltage"),
    sym("VOD", Quantity::Voltage, "input overdrive (comparators)"),
    sym("VM", Quantity::Voltage, "motor (bridge) supply voltage"),
    sym(
        "VBAT",
        Quantity::Voltage,
        "backup-battery voltage (an RTC's backup current)",
    ),
    sym("ID", Quantity::Current, "drain current"),
    sym("IC", Quantity::Current, "collector current"),
    sym("IB", Quantity::Current, "base current"),
    sym("IF", Quantity::Current, "forward current"),
    sym("IR", Quantity::Current, "reverse current"),
    sym("IZT", Quantity::Current, "zener test current"),
    sym("IPP", Quantity::Current, "peak pulse current"),
    sym("IOUT", Quantity::Current, "output (load) current"),
    sym("IL", Quantity::Current, "load current"),
    sym("CL", Quantity::Capacitance, "load capacitance"),
    sym("RL", Quantity::Resistance, "load resistance"),
    sym("TA", Quantity::Temperature, "ambient temperature"),
    sym("TJ", Quantity::Temperature, "junction temperature"),
    sym("TC", Quantity::Temperature, "case temperature"),
    sym("TL", Quantity::Temperature, "lead temperature"),
    sym("TS", Quantity::Temperature, "solder-point temperature"),
    sym("TT", Quantity::Temperature, "terminal temperature"),
    sym("f", Quantity::Frequency, "frequency"),
    sym(
        "FCLK",
        Quantity::Frequency,
        "clock frequency (an interface or core clock a supply current is stated at)",
    ),
    sym("t", Quantity::Time, "duration"),
    sym(
        "THD",
        Quantity::Ratio,
        "total harmonic distortion + noise (an audio amplifier's output power)",
    ),
    sym(
        "d",
        Quantity::Length,
        "measuring distance (a buzzer's sound pressure)",
    ),
];

/// The vocabulary entry for `symbol` (case-sensitive).
pub fn condition_symbol(symbol: &str) -> Option<&'static ConditionSymbol> {
    CONDITION_SYMBOLS.iter().find(|s| s.symbol == symbol)
}

/// Position of `symbol` in the vocabulary (unknown symbols last).
pub fn symbol_order(symbol: &str) -> usize {
    CONDITION_SYMBOLS
        .iter()
        .position(|s| s.symbol == symbol)
        .unwrap_or(usize::MAX)
}

/// Why a condition is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionError {
    UnknownSymbol(String),
    IncompatibleUnit {
        symbol: String,
        unit: String,
    },
    /// Neither a value nor a range, or both, or `min > max`.
    Shape(String),
}

impl std::fmt::Display for ConditionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConditionError::UnknownSymbol(s) => {
                let known: Vec<&str> = CONDITION_SYMBOLS.iter().map(|s| s.symbol).collect();
                write!(
                    f,
                    "\"{s}\" is not a condition symbol (one of: {})",
                    known.join(", ")
                )
            }
            ConditionError::IncompatibleUnit { symbol, unit } => {
                write!(f, "unit \"{unit}\" does not fit condition {symbol}")
            }
            ConditionError::Shape(s) => write!(f, "condition {s}: a value or a min..max range"),
        }
    }
}

/// A condition normalised to SI (`value`, `min`, `max`), comparable.
pub type NormalizedCondition = (Option<f64>, Option<f64>, Option<f64>);

impl Condition {
    /// The condition in SI units, after checking its symbol and unit.
    pub fn normalize(&self, symbol: &str) -> Result<NormalizedCondition, ConditionError> {
        let s = condition_symbol(symbol)
            .ok_or_else(|| ConditionError::UnknownSymbol(symbol.to_string()))?;
        let factor =
            s.quantity
                .unit_factor(&self.unit)
                .ok_or_else(|| ConditionError::IncompatibleUnit {
                    symbol: symbol.to_string(),
                    unit: self.unit.clone(),
                })?;
        let ok = match (self.value, self.min, self.max) {
            (Some(_), None, None) => true,
            (None, Some(a), Some(b)) => a <= b,
            (None, Some(_), None) | (None, None, Some(_)) => true,
            _ => false,
        };
        if !ok {
            return Err(ConditionError::Shape(symbol.to_string()));
        }
        let si = |v: Option<f64>| v.map(|v| crate::parameter::round_significant(v * factor, 12));
        Ok((si(self.value), si(self.min), si(self.max)))
    }

    /// `10 V`, `-40..85 °C`, `≥ 1 mA`.
    pub fn label(&self) -> String {
        let n = crate::parameter::plain_number;
        let u = &self.unit;
        match (self.value, self.min, self.max) {
            (Some(v), _, _) => format!("{} {u}", n(v)),
            (None, Some(a), Some(b)) => format!("{}..{} {u}", n(a), n(b)),
            (None, Some(a), None) => format!("≥ {} {u}", n(a)),
            (None, None, Some(b)) => format!("≤ {} {u}", n(b)),
            _ => String::new(),
        }
    }
}

/// Every condition checked; the first error.
pub fn check_conditions(conditions: &Conditions) -> Result<(), ConditionError> {
    for (symbol, c) in conditions {
        c.normalize(symbol)?;
    }
    Ok(())
}

/// The symbols in vocabulary order.
pub fn ordered(conditions: &Conditions) -> Vec<(&String, &Condition)> {
    let mut v: Vec<_> = conditions.iter().collect();
    v.sort_by_key(|(s, _)| (symbol_order(s), s.as_str()));
    v
}

/// Serialises conditions in vocabulary order (`VGS` before `ID`), so
/// JSON readers that keep key order show them as the data reads.
pub fn serialize_ordered<S: serde::Serializer>(c: &Conditions, s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    let mut m = s.serialize_map(Some(c.len()))?;
    for (k, v) in ordered(c) {
        m.serialize_entry(k, v)?;
    }
    m.end()
}

/// The compact display form: `VGS 10 V, ID 5.7 A` (empty without
/// conditions).
pub fn conditions_label(conditions: &Conditions) -> String {
    ordered(conditions)
        .into_iter()
        .map(|(s, c)| format!("{s} {}", c.label()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The conditions in SI, for like-for-like comparison: two values are
/// compared only when these are equal. `None` when a condition is
/// invalid (such a value is never compared).
pub fn comparable(conditions: &Conditions) -> Option<Vec<(String, NormalizedCondition)>> {
    ordered(conditions)
        .into_iter()
        .map(|(s, c)| c.normalize(s).ok().map(|n| (s.clone(), n)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(value: f64, unit: &str) -> Condition {
        Condition {
            value: Some(value),
            unit: unit.into(),
            ..Default::default()
        }
    }

    #[test]
    fn symbols_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for s in CONDITION_SYMBOLS {
            assert!(seen.insert(s.symbol), "{}", s.symbol);
        }
    }

    #[test]
    fn checks_symbol_and_unit() {
        assert_eq!(c(10.0, "V").normalize("VGS"), Ok((Some(10.0), None, None)));
        assert_eq!(c(5.7, "mA").normalize("ID"), Ok((Some(0.0057), None, None)));
        assert_eq!(c(25.0, "°C").normalize("TA"), Ok((Some(25.0), None, None)));
        assert!(matches!(
            c(10.0, "A").normalize("VGS"),
            Err(ConditionError::IncompatibleUnit { .. })
        ));
        assert!(matches!(
            c(10.0, "V").normalize("VXX"),
            Err(ConditionError::UnknownSymbol(_))
        ));
        let range = Condition {
            min: Some(-40.0),
            max: Some(85.0),
            unit: "°C".into(),
            ..Default::default()
        };
        assert_eq!(range.label(), "-40..85 °C");
        assert!(range.normalize("TA").is_ok());
    }

    #[test]
    fn label_is_in_vocabulary_order() {
        let mut at = Conditions::new();
        at.insert("ID".into(), c(5.7, "A"));
        at.insert("VGS".into(), c(10.0, "V"));
        assert_eq!(conditions_label(&at), "VGS 10 V, ID 5.7 A");
        let mut milli = Conditions::new();
        milli.insert("ID".into(), c(5700.0, "mA"));
        milli.insert("VGS".into(), c(10.0, "V"));
        assert_eq!(comparable(&at), comparable(&milli));
    }
}
