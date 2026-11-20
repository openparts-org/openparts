//! OpenParts parameters -> PCBCraft typed specifications.
//!
//! A specification is written only when its value is exactly one
//! canonical base-unit decimal (`library-common-v1#/$defs/decimal`: no
//! exponent, at most 36 digits and 18 fractional places). Conversion
//! is decimal digit arithmetic on the value as the data writes it (its
//! shortest round-trip spelling), never binary floating-point scaling,
//! so `4.7 kΩ` becomes exactly `"4700"`. Anything else -- a quantity
//! PCBCraft has no unit for (`1/K`, dB, bits...), disjoint ranges, a
//! multi-valued key, an asymmetric deviation, a value outside the
//! decimal's precision -- is left out: an unknown value is never a zero
//! or a guess.

use crate::record::{FieldType, ParameterValue};
use openparts_core::parameter::{parameter_spec, ParameterShape, Parameters, Quantity};

/// An exact decimal `m * 10^e`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Dec {
    m: i128,
    e: i32,
}

impl Dec {
    /// The decimal a float's shortest round-trip spelling states
    /// (`0.1` -> 1e-1, not 0.1000000000000000055...).
    pub(crate) fn from_f64(v: f64) -> Option<Dec> {
        if !v.is_finite() {
            return None;
        }
        let s = format!("{v:e}");
        let (mantissa, exp) = s.split_once('e')?;
        let exp: i32 = exp.parse().ok()?;
        let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let m: i128 = format!("{int}{frac}").parse().ok()?;
        Some(Dec {
            m,
            e: exp - frac.len() as i32,
        })
    }

    pub(crate) fn parse(s: &str) -> Option<Dec> {
        let (int, frac) = s.split_once('.').unwrap_or((s, ""));
        Some(Dec {
            m: format!("{int}{frac}").parse().ok()?,
            e: -(frac.len() as i32),
        })
    }

    pub(crate) fn mul(self, o: Dec) -> Option<Dec> {
        Some(Dec {
            m: self.m.checked_mul(o.m)?,
            e: self.e.checked_add(o.e)?,
        })
    }

    pub(crate) fn add(self, o: Dec) -> Option<Dec> {
        let e = self.e.min(o.e);
        let scale = |d: Dec| -> Option<i128> {
            10i128
                .checked_pow(u32::try_from(d.e - e).ok()?)?
                .checked_mul(d.m)
        };
        Some(Dec {
            m: scale(self)?.checked_add(scale(o)?)?,
            e,
        })
    }

    pub(crate) fn abs(self) -> Dec {
        Dec {
            m: self.m.abs(),
            e: self.e,
        }
    }

    /// The canonical spelling, or `None` beyond the decimal's 36 digits
    /// / 18 fractional places.
    pub(crate) fn canonical(self) -> Option<String> {
        let (mut m, mut e) = (self.m, self.e);
        if m == 0 {
            return Some("0".into());
        }
        while m % 10 == 0 {
            m /= 10;
            e += 1;
        }
        let digits = m.unsigned_abs().to_string();
        let sign = if m < 0 { "-" } else { "" };
        let body = if e >= 0 {
            format!("{digits}{}", "0".repeat(e as usize))
        } else {
            let frac = (-e) as usize;
            if frac > 18 {
                return None;
            }
            if digits.len() > frac {
                let (i, f) = digits.split_at(digits.len() - frac);
                format!("{i}.{f}")
            } else {
                format!("0.{}{digits}", "0".repeat(frac - digits.len()))
            }
        };
        if body.chars().filter(char::is_ascii_digit).count() > 36 {
            return None;
        }
        Some(format!("{sign}{body}"))
    }
}

/// PCBCraft's standard quantity and stored unit for an OpenParts one.
fn pcb_quantity(q: Quantity) -> Option<(&'static str, &'static str)> {
    Some(match q {
        Quantity::Resistance => ("resistance", "Ohm"),
        Quantity::Capacitance => ("capacitance", "F"),
        Quantity::Inductance => ("inductance", "H"),
        Quantity::Voltage => ("voltage", "V"),
        Quantity::Current => ("current", "A"),
        Quantity::Power => ("power", "W"),
        Quantity::Frequency => ("frequency", "Hz"),
        Quantity::Time => ("time", "s"),
        Quantity::Length => ("length", "m"),
        Quantity::Temperature => ("temperature", "K"),
        Quantity::Ratio => ("ratio", "1"),
        _ => return None,
    })
}

/// `v` in `unit` as a base-unit decimal of `q`.
fn to_base(q: Quantity, v: f64, unit: &str) -> Option<Dec> {
    let v = Dec::from_f64(v)?;
    if q == Quantity::Temperature {
        // The only temperature unit the data uses is °C: K = °C + 273.15
        // (exact in decimal).
        return match unit {
            "°C" => v.add(Dec::parse("273.15")?),
            _ => None,
        };
    }
    let factor = Dec::from_f64(q.unit_factor(unit)?)?;
    v.mul(factor)
}

/// One field a specification fills.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FieldDef {
    pub id: String,
    pub name: String,
    pub value_type: FieldType,
}

/// Which part of an OpenParts value a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bound {
    Typ,
    Min,
    Max,
    /// A deviation's symmetric magnitude (a ±1 % tolerance: 0.01).
    Magnitude,
}

/// PCBCraft's own field IDs (those its sources and examples define),
/// used where an OpenParts key means the same thing.
fn builtin(key: &str, bound: Bound) -> Option<(&'static str, &'static str)> {
    Some(match (key, bound) {
        ("resistance", Bound::Typ) => ("pcbcraft:resistance", "Resistance"),
        ("tolerance", Bound::Magnitude) => ("pcbcraft:tolerance", "Tolerance"),
        ("supply_voltage", Bound::Min) => ("pcbcraft:supply-voltage-min", "Minimum supply voltage"),
        ("supply_voltage", Bound::Max) => ("pcbcraft:supply-voltage-max", "Maximum supply voltage"),
        ("rated_current", Bound::Typ) => ("pcbcraft:rated-current", "Rated current"),
        _ => return None,
    })
}

fn humanize(key: &str) -> String {
    let mut s = key.replace('_', " ");
    if let Some(first) = s.get(0..1) {
        let up = first.to_ascii_uppercase();
        s.replace_range(0..1, &up);
    }
    s
}

/// `openparts:<key>` (with a bound suffix) unless PCBCraft defines it.
fn field(key: &str, bound: Bound, value_type: FieldType) -> FieldDef {
    if let Some((id, name)) = builtin(key, bound) {
        return FieldDef {
            id: id.into(),
            name: name.into(),
            value_type,
        };
    }
    let slug = key.replace('_', "-");
    let (suffix, label) = match bound {
        Bound::Typ => ("", String::new()),
        Bound::Min => ("-min", " (min)".to_string()),
        Bound::Max => ("-max", " (max)".to_string()),
        Bound::Magnitude => ("", " (±)".to_string()),
    };
    FieldDef {
        id: format!("openparts:{slug}{suffix}"),
        name: format!("{}{label}", humanize(key)),
        value_type,
    }
}

/// The representable specifications of `params` (a part's effective
/// parameters): each key's primary value, one field per stated bound.
/// Measurement conditions (`at`) are not carried; PCBCraft has no
/// place for them.
pub(crate) fn specifications(params: &Parameters) -> Vec<(FieldDef, ParameterValue)> {
    let mut out = Vec::new();
    for (key, values) in params {
        let Some(spec) = parameter_spec(key) else {
            continue;
        };
        if values.is_empty() {
            continue;
        }
        let v = values.primary();
        if v.normalize(spec).is_err() {
            continue;
        }
        match spec.shape {
            ParameterShape::Numeric(q) => {
                let Some((quantity, unit)) = pcb_quantity(q) else {
                    continue;
                };
                if !v.ranges.is_empty() {
                    continue;
                }
                let Some(data_unit) = v.unit.as_deref() else {
                    continue;
                };
                let base = |x: Option<f64>| x.map(|x| to_base(q, x, data_unit));
                let (typ, min, max) = (base(v.typ), base(v.min), base(v.max));
                let mut push = |bound: Bound, d: Dec| {
                    if let Some(value) = d.canonical() {
                        out.push((
                            field(key, bound, FieldType::Quantity { quantity, unit }),
                            ParameterValue::Quantity { value, unit },
                        ));
                    }
                };
                if spec.deviation {
                    // Symmetric only: ±x, or a lone stated x.
                    let magnitude = match (typ, min, max) {
                        (Some(Some(t)), None, None) => Some(t.abs()),
                        (None, Some(Some(lo)), Some(Some(hi))) if lo.abs() == hi.abs() => {
                            Some(hi.abs())
                        }
                        _ => None,
                    };
                    if let Some(m) = magnitude {
                        push(Bound::Magnitude, m);
                    }
                    continue;
                }
                for (bound, d) in [(Bound::Typ, typ), (Bound::Min, min), (Bound::Max, max)] {
                    if let Some(Some(d)) = d {
                        push(bound, d);
                    }
                }
            }
            ParameterShape::Enumerated(choices) => {
                let value = match (&v.value, v.values.as_slice()) {
                    (Some(value), []) => value.clone(),
                    (None, [one]) => one.clone(),
                    _ => continue,
                };
                let fits = |s: &str| !s.is_empty() && s.chars().count() <= 64;
                let value_type = if choices.len() <= 64 && choices.iter().all(|c| fits(c)) {
                    FieldType::Choice {
                        choices: choices.iter().map(|c| c.to_string()).collect(),
                    }
                } else {
                    FieldType::Text
                };
                let value = match value_type {
                    FieldType::Choice { .. } => ParameterValue::Choice { value },
                    _ => ParameterValue::Text { value },
                };
                out.push((field(key, Bound::Typ, value_type), value));
            }
        }
    }
    out
}

/// The field a category's parts are valued by (its `primary_value`).
pub(crate) fn primary_field(category: openparts_core::ComponentCategory) -> Option<&'static str> {
    use openparts_core::ComponentCategory as C;
    Some(match category {
        C::Resistor => "pcbcraft:resistance",
        C::Capacitor => "openparts:capacitance",
        C::Inductor => "openparts:inductance",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canon(v: f64, unit: &str, q: Quantity) -> Option<String> {
        to_base(q, v, unit)?.canonical()
    }

    #[test]
    fn values_convert_by_decimal_digits() {
        assert_eq!(canon(4.7, "kΩ", Quantity::Resistance).unwrap(), "4700");
        assert_eq!(
            canon(100.0, "nF", Quantity::Capacitance).unwrap(),
            "0.0000001"
        );
        assert_eq!(canon(0.1, "Ω", Quantity::Resistance).unwrap(), "0.1");
        assert_eq!(canon(1.0, "%", Quantity::Ratio).unwrap(), "0.01");
        assert_eq!(canon(-40.0, "°C", Quantity::Temperature).unwrap(), "233.15");
        assert_eq!(canon(85.0, "°C", Quantity::Temperature).unwrap(), "358.15");
        assert_eq!(canon(0.0, "V", Quantity::Voltage).unwrap(), "0");
        assert_eq!(canon(32.768, "kHz", Quantity::Frequency).unwrap(), "32768");
        // Below the decimal's 18 fractional places: left out.
        assert_eq!(
            Dec::parse("0.000000000000000001")
                .unwrap()
                .canonical()
                .unwrap(),
            "0.000000000000000001"
        );
        assert_eq!(
            canon(1.5, "pF", Quantity::Capacitance).unwrap(),
            "0.0000000000015"
        );
        assert_eq!(Dec::from_f64(1.5e-19).unwrap().canonical(), None);
    }
}
