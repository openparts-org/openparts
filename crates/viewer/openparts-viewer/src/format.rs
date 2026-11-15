//! Numbers as the dimension labels show them.

use openparts_core::Dimension;

/// At most three decimals, no trailing zeros: 0.5, 3, 0.203.
pub fn number(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let mut s = format!("{r:.3}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

/// A datasheet dimension: "7 mm", "0.18 (0.13–0.23) mm", "3–3.2 mm",
/// "≤11.5 mm", "≥0.2 mm"; `None` when it states no value at all.
pub fn dimension(d: &Dimension) -> Option<String> {
    let u = if d.unit.is_empty() { "mm" } else { &d.unit };
    let value = match (d.typ, d.min, d.max) {
        (Some(n), Some(lo), Some(hi)) => {
            format!("{} ({}–{})", number(n), number(lo), number(hi))
        }
        (Some(n), Some(lo), None) => format!("{} (≥{})", number(n), number(lo)),
        (Some(n), None, Some(hi)) => format!("{} (≤{})", number(n), number(hi)),
        (Some(n), None, None) => number(n),
        (None, Some(lo), Some(hi)) => format!("{}–{}", number(lo), number(hi)),
        (None, Some(lo), None) => format!("≥{}", number(lo)),
        (None, None, Some(hi)) => format!("≤{}", number(hi)),
        (None, None, None) => return None,
    };
    Some(format!("{value} {u}"))
}

/// A length measured on the model (not stated by the datasheet).
pub fn measured(v: f64) -> String {
    format!("≈{} mm", number(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dim(nominal: Option<f64>, min: Option<f64>, max: Option<f64>) -> Dimension {
        Dimension {
            typ: nominal,
            min,
            max,
            unit: "mm".into(),
        }
    }

    #[test]
    fn formats_numbers_and_ranges() {
        assert_eq!(number(0.5000000001), "0.5");
        assert_eq!(number(3.0), "3");
        assert_eq!(number(-0.0001), "0");
        assert_eq!(dimension(&dim(Some(7.0), None, None)).unwrap(), "7 mm");
        assert_eq!(
            dimension(&dim(Some(0.18), Some(0.13), Some(0.23))).unwrap(),
            "0.18 (0.13–0.23) mm"
        );
        assert_eq!(dimension(&dim(None, None, Some(11.5))).unwrap(), "≤11.5 mm");
        assert_eq!(
            dimension(&dim(None, Some(0.0), Some(0.05))).unwrap(),
            "0–0.05 mm"
        );
        assert_eq!(dimension(&dim(None, None, None)), None);
        assert_eq!(measured(0.9), "≈0.9 mm");
    }
}
