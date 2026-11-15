//! Pin compatibility: two Devices' pin maps compared pad by pad, with
//! the names vendors give one function folded together (GND/VSS/COMMON,
//! VIN/IN/VI/INPUT, VOUT/OUT/VO/OUTPUT, EN/CE/SHDN with polarity, NC).
//!
//! A pad is compared by the function its names stand for, not by its
//! spelling: `IN` and `VIN` are the same pin, `ADJ/GND` covers `GND`.
//! Anything that could break the board is `differs` (a pinout that only
//! looks alike, like two SOT-23-5 regulators with VIN and GND swapped);
//! what is probably fine but worth a look is a `note`.

use openparts_core::{Device, Pin, PinType};
use serde::Serialize;
use std::collections::BTreeSet;

/// A pin function, after synonyms are folded.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Func {
    Ground,
    Vin,
    Vout,
    Vdd,
    Enable,
    Adjust,
    Bypass,
    PowerGood,
    Reset,
    Nc,
    /// Any other name, normalised (upper case, no separators).
    Other(String),
}

/// An enable pin's active level, from its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Polarity {
    /// `EN`, `CE`, `ON`: high turns the part on.
    High,
    /// `EN#`, `/EN`, `nEN`: low turns it on.
    Low,
    /// `SHDN`, `SD`: datasheets draw the bar the name text lacks, so the
    /// level can't be told from the name.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Functions {
    funcs: BTreeSet<Func>,
    enable: Option<Polarity>,
}

/// Folds one name token (already upper case) to its function, with
/// whether it carried a negation mark and the bare name.
fn token_func(raw: &str) -> Option<(Func, bool, String)> {
    let mut t = raw.trim().to_string();
    if t.is_empty() {
        return None;
    }
    let mut negated = false;
    for p in ['/', '~', '!'] {
        if let Some(r) = t.strip_prefix(p) {
            t = r.to_string();
            negated = true;
        }
    }
    for s in ["#", "*", "_N", "_B", "_L"] {
        if t.len() > s.len() {
            if let Some(r) = t.strip_suffix(s) {
                t = r.to_string();
                negated = true;
            }
        }
    }
    let t: String = t
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '+')
        .collect();
    if t.is_empty() {
        return None;
    }
    let controls = ["EN", "ENABLE", "CE", "SHDN", "SD", "RST", "RESET"];
    let t = match t.strip_prefix('N') {
        Some(r) if controls.contains(&r) => {
            negated = true;
            r.to_string()
        }
        _ => t,
    };
    let f = match t.as_str() {
        "GND" | "VSS" | "GROUND" | "COMMON" | "AGND" | "PGND" | "DGND" | "GNDPAD" | "EPGND"
        | "EP" | "EPAD" | "PAD" | "THERMALPAD" | "VEE" => Func::Ground,
        "VIN" | "IN" | "VI" | "INPUT" | "VCCIN" | "VDDIN" => Func::Vin,
        "VOUT" | "OUT" | "VO" | "OUTPUT" => Func::Vout,
        "VDD" | "VCC" | "VS" | "V+" | "VPLUS" | "VDDIO" | "VBAT" => Func::Vdd,
        "EN" | "ENABLE" | "CE" | "ON" | "SHDN" | "SD" | "SHUTDOWN" | "OFF" | "ONOFF" => {
            Func::Enable
        }
        "ADJ" | "ADJUST" | "ADJUSTMENT" => Func::Adjust,
        "BYP" | "BP" | "BYPASS" | "NR" | "NRSS" => Func::Bypass,
        "PG" | "PGOOD" | "POWERGOOD" | "PWRGD" | "PGD" => Func::PowerGood,
        "RST" | "RESET" | "MR" => Func::Reset,
        "NC" | "NOTCONNECTED" | "NOCONNECT" | "DNC" | "NIC" => Func::Nc,
        other => Func::Other(other.to_string()),
    };
    Some((f, negated, t))
}

fn functions(pin: &Pin) -> Functions {
    let mut funcs = BTreeSet::new();
    let mut enable = None;
    let upper = pin.name.trim().to_uppercase();
    // `N/C` and `NOT CONNECTED` before the name is split on `/` and spaces.
    let joined: String = upper.chars().filter(|c| !c.is_whitespace()).collect();
    if pin.pin_type == PinType::Nc || matches!(joined.as_str(), "N/C" | "NOTCONNECTED" | "N.C.") {
        funcs.insert(Func::Nc);
        return Functions { funcs, enable };
    }
    // `ADJ/GND`, `Adj (GND)`, `Adjust/Ground`: each alternative names
    // the pin. A leading `/` is a negation mark, not a separator.
    let mut tokens: Vec<String> = Vec::new();
    for word in upper.split(['(', ')', ',', ' ']) {
        let (lead, body) = match word.strip_prefix('/') {
            Some(r) => ("/", r),
            None => ("", word),
        };
        for (i, tok) in body.split('/').enumerate() {
            if !tok.is_empty() {
                tokens.push(if i == 0 {
                    format!("{lead}{tok}")
                } else {
                    tok.to_string()
                });
            }
        }
    }
    for t in tokens {
        if let Some((f, negated, base)) = token_func(&t) {
            if f == Func::Enable {
                let shutdown = matches!(base.as_str(), "SHDN" | "SD" | "SHUTDOWN" | "OFF");
                enable = Some(match (shutdown, negated) {
                    (false, false) => Polarity::High,
                    (false, true) => Polarity::Low,
                    // `/SHDN`: low shuts down, so high enables.
                    (true, true) => Polarity::High,
                    (true, false) => Polarity::Unknown,
                });
            }
            funcs.insert(f);
        }
    }
    if pin.pin_type == PinType::Ground {
        funcs.insert(Func::Ground);
    }
    Functions { funcs, enable }
}

/// A pin as the comparison shows it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PinBrief {
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: PinType,
}

/// How one pad compares.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PadStatus {
    /// The same name (or both unconnected).
    Same,
    /// A synonym of the same function (`IN` / `VIN`).
    Equivalent,
    /// Probably compatible; check the detail.
    Note,
    /// A different function: not pin-compatible.
    Differs,
}

/// Why a pad is a `note` or `differs`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PadReason {
    /// The names stand for different functions.
    FunctionDiffers,
    /// One side is ground, the other isn't.
    GroundMismatch,
    /// Both are enables, of opposite active level.
    PolarityDiffers,
    /// Both are enables; the level of one can't be told from its name
    /// (`SHDN`).
    PolarityUnverified,
    /// The original leaves the pad unconnected; the candidate uses it
    /// for a passive function (a bypass capacitor's pin) that works
    /// floating only with reduced performance.
    CandidateUsesNcPad,
    /// The original leaves the pad unconnected; the candidate needs it.
    CandidateNeedsNcPad,
    /// The candidate leaves unconnected a pad the original uses (what
    /// the board connects there goes nowhere).
    UnusedOnCandidate,
    /// A pad of one land pattern has no pin on the other Device.
    MissingPin,
}

/// One pad of the comparison.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PadDiff {
    /// The original's pad number.
    pub pad: String,
    /// The candidate's pad at the same place (usually the same number).
    pub candidate_pad: String,
    pub original: Option<PinBrief>,
    pub candidate: Option<PinBrief>,
    pub status: PadStatus,
    pub reason: Option<PadReason>,
}

/// A pad-by-pad comparison of two pinouts on one land pattern.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PinComparison {
    /// No pad `differs`.
    pub compatible: bool,
    /// Pads that differ.
    pub differing: Vec<String>,
    /// Pads with a note.
    pub noted: Vec<String>,
    pub pads: Vec<PadDiff>,
}

fn brief(p: &Pin) -> PinBrief {
    PinBrief {
        name: p.name.clone(),
        pin_type: p.pin_type,
    }
}

fn is_nc(p: &Pin) -> bool {
    functions(p).funcs.contains(&Func::Nc) || p.pin_type == PinType::Reserved
}

/// Compares one pad's pins.
fn compare_pins(o: Option<&Pin>, c: Option<&Pin>) -> (PadStatus, Option<PadReason>) {
    use PadReason as R;
    use PadStatus as S;
    match (o, c) {
        (None, None) => (S::Same, None),
        (Some(p), None) | (None, Some(p)) if is_nc(p) => (S::Same, None),
        (Some(_), None) | (None, Some(_)) => (S::Differs, Some(R::MissingPin)),
        (Some(o), Some(c)) => {
            let (fo, fc) = (functions(o), functions(c));
            let (nc_o, nc_c) = (is_nc(o), is_nc(c));
            if nc_o && nc_c {
                return (S::Same, None);
            }
            if nc_o {
                let passive =
                    matches!(c.pin_type, PinType::Passive) || fc.funcs.contains(&Func::Bypass);
                return if passive {
                    (S::Note, Some(R::CandidateUsesNcPad))
                } else {
                    (S::Differs, Some(R::CandidateNeedsNcPad))
                };
            }
            if nc_c {
                return (S::Note, Some(R::UnusedOnCandidate));
            }
            let ground_o = fo.funcs.contains(&Func::Ground);
            let ground_c = fc.funcs.contains(&Func::Ground);
            if ground_o != ground_c {
                // `ADJ/GND` on a fixed regulator is ground; `ADJUST` on
                // an adjustable one is not.
                return (S::Differs, Some(R::GroundMismatch));
            }
            if fo.funcs.is_disjoint(&fc.funcs) {
                return (S::Differs, Some(R::FunctionDiffers));
            }
            if let (Some(a), Some(b)) = (fo.enable, fc.enable) {
                if fo.funcs.contains(&Func::Enable) && fc.funcs.contains(&Func::Enable) {
                    match (a, b) {
                        (Polarity::Unknown, _) | (_, Polarity::Unknown) if a != b => {
                            return (S::Note, Some(R::PolarityUnverified))
                        }
                        _ if a != b => return (S::Differs, Some(R::PolarityDiffers)),
                        _ => {}
                    }
                }
            }
            let same_name = o.name.trim().eq_ignore_ascii_case(c.name.trim());
            if same_name {
                (S::Same, None)
            } else {
                (S::Equivalent, None)
            }
        }
    }
}

/// Compares `orig`'s and `cand`'s pins on matched pads: `pads` pairs
/// each original pad number with the candidate's pad at the same place.
pub fn compare(orig: &Device, cand: &Device, pads: &[(String, String)]) -> PinComparison {
    let mut out = PinComparison {
        compatible: true,
        differing: Vec::new(),
        noted: Vec::new(),
        pads: Vec::new(),
    };
    let mut seen = BTreeSet::new();
    for (po, pc) in pads {
        // A tab repeating a lead's number is one pin, compared once.
        if !seen.insert((po.clone(), pc.clone())) {
            continue;
        }
        let o = orig.pins.get(po);
        let c = cand.pins.get(pc);
        let (status, reason) = compare_pins(o, c);
        // A tab numbered like a lead is listed once.
        match status {
            PadStatus::Differs => {
                out.compatible = false;
                if !out.differing.contains(po) {
                    out.differing.push(po.clone());
                }
            }
            PadStatus::Note if !out.noted.contains(po) => out.noted.push(po.clone()),
            _ => {}
        }
        out.pads.push(PadDiff {
            pad: po.clone(),
            candidate_pad: pc.clone(),
            original: o.map(brief),
            candidate: c.map(brief),
            status,
            reason,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(name: &str, t: PinType) -> Pin {
        Pin {
            name: name.into(),
            pin_type: t,
            alternate_functions: vec![],
            connected_to: None,
        }
    }

    fn status(a: (&str, PinType), b: (&str, PinType)) -> PadStatus {
        compare_pins(Some(&pin(a.0, a.1)), Some(&pin(b.0, b.1))).0
    }

    use PinType as T;

    #[test]
    fn synonyms_fold_together() {
        assert_eq!(
            status(("VIN", T::Power), ("IN", T::Power)),
            PadStatus::Equivalent
        );
        assert_eq!(
            status(("INPUT", T::Power), ("VI", T::Power)),
            PadStatus::Equivalent
        );
        assert_eq!(
            status(("VOUT", T::Power), ("OUT", T::PowerOut)),
            PadStatus::Equivalent
        );
        assert_eq!(
            status(("GND", T::Ground), ("VSS", T::Ground)),
            PadStatus::Equivalent
        );
        assert_eq!(
            status(("COMMON", T::Ground), ("GND", T::Ground)),
            PadStatus::Equivalent
        );
        assert_eq!(
            status(("ADJ/GND", T::Ground), ("GND", T::Ground)),
            PadStatus::Equivalent
        );
        assert_eq!(
            status(("Adj (GND)", T::Ground), ("Adjust/Ground", T::Ground)),
            PadStatus::Equivalent
        );
        assert_eq!(
            status(("EN", T::Input), ("CE", T::Input)),
            PadStatus::Equivalent
        );
        assert_eq!(status(("EN", T::Input), ("EN", T::Input)), PadStatus::Same);
    }

    #[test]
    fn functions_that_differ_are_flagged() {
        assert_eq!(
            status(("VIN", T::Power), ("GND", T::Ground)),
            PadStatus::Differs
        );
        assert_eq!(
            status(("VOUT", T::Power), ("VIN", T::Power)),
            PadStatus::Differs
        );
        // An adjustable regulator's ADJUST where a fixed one's pin is ground.
        assert_eq!(
            status(("ADJ/GND", T::Ground), ("ADJUST", T::Input)),
            PadStatus::Differs
        );
        assert_eq!(
            status(("EN", T::Input), ("BYP", T::Passive)),
            PadStatus::Differs
        );
    }

    #[test]
    fn enable_polarity() {
        assert_eq!(
            status(("EN", T::Input), ("EN#", T::Input)),
            PadStatus::Differs
        );
        assert_eq!(
            status(("EN", T::Input), ("/SHDN", T::Input)),
            PadStatus::Equivalent
        );
        assert_eq!(
            status(("nEN", T::Input), ("/EN", T::Input)),
            PadStatus::Equivalent
        );
        assert_eq!(
            status(("EN", T::Input), ("SHDN", T::Input)),
            PadStatus::Note
        );
    }

    #[test]
    fn unconnected_pads() {
        assert_eq!(status(("NC", T::Nc), ("N/C", T::Nc)), PadStatus::Same);
        assert_eq!(
            status(("NC", T::Nc), ("NOT CONNECTED", T::Nc)),
            PadStatus::Same
        );
        // A bypass pin where the original has none: works, noisier.
        assert_eq!(status(("NC", T::Nc), ("BYP", T::Passive)), PadStatus::Note);
        // An adjust pin left floating: broken.
        assert_eq!(status(("NC", T::Nc), ("ADJ", T::Input)), PadStatus::Differs);
        assert_eq!(status(("PG", T::Output), ("NC", T::Nc)), PadStatus::Note);
    }
}
