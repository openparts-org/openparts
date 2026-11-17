//! "Meets or exceeds": how a candidate's electrical parameter compares
//! with the original's, one rule per vocabulary key
//! ([`PARAMETER_RULES`], documented in docs/spec/alternatives-and-bom.md).
//!
//! A key the original states and the candidate doesn't is *unverified*,
//! never a pass. A key only the candidate states is not checked (it
//! can't make the candidate worse than what the original promised).

use openparts_core::{
    condition::{comparable, NormalizedCondition},
    conditions_label, effective_parameters, parameter_spec, ComponentCategory, Normalized,
    ParameterSpec, Parameters,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// How a key is compared.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// Must be the same value (within the rule's relative tolerance):
    /// a 100 nF capacitor is not a 220 nF one.
    Equal,
    /// The candidate's value must be at least the original's (a rating).
    AtLeast,
    /// The candidate's value must be at most the original's (a loss, a
    /// spread).
    AtMost,
    /// An enumerated grade: the candidate's rank must be at least the
    /// original's (dielectric C0G > X7R > X5R > Y5V).
    Ranked,
    /// Reported side by side, not judged (direction depends on the
    /// design: gate threshold, actuation force).
    Informational,
    /// A range (`min`..`max`): the candidate's must contain the
    /// original's -- a supply-voltage or operating-temperature range at
    /// least as wide on every side the original states.
    Covers,
}

/// One row of the rule table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParameterRule {
    pub key: &'static str,
    pub kind: RuleKind,
    /// `Equal`: the largest relative difference still counted as equal.
    pub tolerance: f64,
    /// The key says *what* the part is (its capacitance, output
    /// voltage, polarity): a functional equivalent in another package
    /// must state it and match.
    pub identity: bool,
}

const fn rule(key: &'static str, kind: RuleKind) -> ParameterRule {
    ParameterRule {
        key,
        kind,
        tolerance: 1e-6,
        identity: false,
    }
}

const fn identity(mut r: ParameterRule) -> ParameterRule {
    r.identity = true;
    r
}

const fn within(mut r: ParameterRule, tolerance: f64) -> ParameterRule {
    r.tolerance = tolerance;
    r
}

use RuleKind::{AtLeast, AtMost, Covers, Equal, Informational, Ranked};

/// The comparison rule of every key of `openparts_core::PARAMETERS`
/// (a test keeps the two in step).
pub const PARAMETER_RULES: &[ParameterRule] = &[
    // Capacitors
    identity(rule("capacitance", Equal)),
    rule("tolerance", AtMost),
    rule("capacitance_tolerance", AtMost),
    rule("rated_voltage", AtLeast),
    rule("dielectric", Ranked),
    rule("esr", AtMost),
    // Resistors
    identity(rule("resistance", Equal)),
    rule("power_rating", AtLeast),
    rule("temperature_coefficient", AtMost),
    // Inductors
    identity(rule("inductance", Equal)),
    rule("rated_current", AtLeast),
    rule("saturation_current", AtLeast),
    rule("dcr", AtMost),
    // Crystals and oscillators
    identity(rule("frequency", Equal)),
    rule("load_capacitance", Equal),
    rule("frequency_tolerance", AtMost),
    rule("frequency_stability", AtMost),
    rule("aging", AtMost),
    rule("drive_level", AtLeast),
    rule("shunt_capacitance", Informational),
    identity(rule("clock_output", Equal)),
    // Diodes and LEDs
    rule("forward_voltage", AtMost),
    rule("reverse_voltage", AtLeast),
    rule("forward_current", AtLeast),
    identity(within(rule("zener_voltage", Equal), 0.005)),
    rule("zener_test_current", Informational),
    identity(rule("color", Equal)),
    within(rule("wavelength", Equal), 0.02),
    within(rule("peak_wavelength", Equal), 0.02),
    rule("luminous_intensity", Informational),
    rule("viewing_angle", Informational),
    within(rule("wavelength_red", Equal), 0.02),
    within(rule("wavelength_green", Equal), 0.02),
    within(rule("wavelength_blue", Equal), 0.02),
    rule("luminous_intensity_red", Informational),
    rule("luminous_intensity_green", Informational),
    rule("luminous_intensity_blue", Informational),
    identity(rule("data_interface", Equal)),
    // Transistors
    identity(rule("polarity", Equal)),
    rule("drain_source_voltage", AtLeast),
    rule("drain_current", AtLeast),
    rule("rds_on", AtMost),
    rule("gate_threshold_voltage", Informational),
    rule("collector_emitter_voltage", AtLeast),
    rule("collector_current", AtLeast),
    // Optocouplers
    rule("isolation_voltage", AtLeast),
    rule("current_transfer_ratio", AtLeast),
    // Regulators
    identity(rule("output_type", Equal)),
    identity(within(rule("output_voltage", Equal), 0.005)),
    rule("input_voltage_max", AtLeast),
    rule("output_current", AtLeast),
    rule("dropout_voltage", AtMost),
    // Memory
    identity(rule("capacity", Equal)),
    // Switches
    rule("actuation_force", Informational),
    rule("travel", Informational),
    // Integrated circuits in general
    rule("supply_voltage", Covers),
    rule("supply_current", AtMost),
    rule("operating_temperature", Covers),
    rule("esd_hbm", AtLeast),
    // Amplifiers and comparators
    identity(rule("channel_count", Equal)),
    rule("gain_bandwidth", AtLeast),
    rule("slew_rate", AtLeast),
    rule("input_offset_voltage", AtMost),
    rule("input_bias_current", AtMost),
    rule("supply_current_per_channel", AtMost),
    rule("rail_to_rail_input", Ranked),
    rule("rail_to_rail_output", Ranked),
    rule("propagation_delay", AtMost),
    rule("output_structure", Equal),
    // Logic
    identity(rule("logic_function", Equal)),
    rule("output_current_high", AtLeast),
    rule("output_current_low", AtLeast),
    // Interface
    rule("data_rate", AtLeast),
    rule("bus_fault_voltage", Covers),
    // Converters
    rule("resolution", Ranked),
    rule("sample_rate", AtLeast),
    // Motor drivers
    identity(rule("motor_type", Equal)),
    rule("motor_supply_voltage", Covers),
    rule("output_current_peak", AtLeast),
    rule("microstepping", Ranked),
    // LED drivers
    identity(rule("led_driver_type", Equal)),
    // Audio
    rule("amplifier_class", Informational),
    rule("output_power", AtLeast),
    rule("thd_n", AtMost),
    rule("snr", AtLeast),
    rule("sensitivity", Informational),
    rule("sensitivity_digital", Informational),
    rule("acoustic_overload_point", AtLeast),
    rule("sound_pressure", AtLeast),
    identity(rule("buzzer_type", Equal)),
    identity(rule("buzzer_drive", Equal)),
    // Relays
    identity(within(rule("coil_voltage", Equal), 0.005)),
    rule("coil_resistance", Informational),
    rule("contact_current", AtLeast),
    rule("contact_voltage", AtLeast),
    identity(rule("contact_form", Equal)),
    rule("relay_type", Equal),
    // Potentiometers and encoders
    identity(rule("taper", Equal)),
    rule("rotation_angle", Informational),
    rule("resistive_element", Informational),
    identity(rule("pulses_per_revolution", Equal)),
    rule("detents", Equal),
    rule("push_switch", Equal),
    // Ferrite beads, common-mode chokes
    identity(within(rule("impedance", Equal), 0.005)),
    // Sensors
    identity(rule("light_sensor_type", Equal)),
    rule("illuminance_range", Covers),
    identity(rule("target_gas", Equal)),
    rule("sensing_principle", Informational),
    rule("concentration_range", Covers),
    identity(rule("magnetic_sensor_type", Equal)),
    rule("operate_point", Informational),
    rule("release_point", Informational),
    rule("magnetic_range", Covers),
    // RF
    identity(rule("radio_protocol", Equal)),
    rule("frequency_band", Covers),
    rule("tx_power", AtLeast),
    rule("rx_sensitivity", AtMost),
    rule("antenna_type", Informational),
    rule("peak_gain", Informational),
];

/// Dielectric grades, best first: temperature stability of the
/// capacitance (class 1 C0G/NP0 best, Y5V/Z5U worst). Codes of one
/// rank are interchangeable for this check.
pub const DIELECTRIC_RANKS: &[(&str, u8)] = &[
    ("C0G", 100),
    ("NP0", 100),
    ("U2J", 90),
    ("X8R", 80),
    ("X8L", 75),
    ("X7R", 70),
    ("X7S", 65),
    ("X7T", 60),
    ("X6S", 55),
    ("X6T", 50),
    ("X5R", 45),
    ("X5S", 40),
    ("Y5V", 10),
    ("Z5U", 10),
];

/// The rule for `key`.
pub fn rule_for(key: &str) -> Option<&'static ParameterRule> {
    PARAMETER_RULES.iter().find(|r| r.key == key)
}

fn rank(key: &str, value: &str) -> Option<u8> {
    match key {
        "dielectric" => DIELECTRIC_RANKS
            .iter()
            .find(|(v, _)| *v == value)
            .map(|(_, r)| *r),
        // A rail-to-rail input/output serves wherever one without does.
        "rail_to_rail_input" | "rail_to_rail_output" => match value {
            "yes" => Some(1),
            "no" => Some(0),
            _ => None,
        },
        // More bits, finer microsteps rank higher (1/256 step is
        // rank 8).
        "resolution" => value.parse::<u8>().ok(),
        "microstepping" => value.parse::<u16>().ok().map(|n| n.trailing_zeros() as u8),
        _ => None,
    }
}

/// The parameters a part's data is expected to state for its category
/// (the data-coverage indicator): the catalog's columns.
pub fn expected_parameters(category: ComponentCategory) -> &'static [&'static str] {
    use ComponentCategory as C;
    match category {
        C::LinearRegulator => &[
            "output_type",
            "output_voltage",
            "output_current",
            "dropout_voltage",
        ],
        C::Capacitor => &["capacitance", "rated_voltage", "dielectric", "tolerance"],
        C::Resistor => &["resistance", "tolerance", "power_rating"],
        C::Inductor => &["inductance", "rated_current", "saturation_current", "dcr"],
        C::Crystal => &["frequency", "load_capacitance", "frequency_tolerance"],
        C::Oscillator => &["frequency", "supply_voltage", "clock_output"],
        C::OpAmp => &["gain_bandwidth", "supply_voltage", "input_offset_voltage"],
        C::Comparator => &["propagation_delay", "supply_voltage", "output_structure"],
        C::Logic => &["logic_function", "supply_voltage", "propagation_delay"],
        C::CanTransceiver | C::Rs485Transceiver => &["data_rate", "supply_voltage"],
        C::Led => &["color", "forward_voltage"],
        C::Fuse => &["rated_current", "rated_voltage"],
        C::Optocoupler => &["current_transfer_ratio", "isolation_voltage"],
        C::MotorDriver => &["motor_type", "motor_supply_voltage", "output_current"],
        C::LedDriver => &["led_driver_type", "channel_count", "output_current"],
        C::AudioAmplifier => &["amplifier_class", "output_power", "supply_voltage"],
        C::AudioCodec => &["resolution", "sample_rate", "snr"],
        C::Microphone => &["data_interface", "snr"],
        C::Buzzer => &["buzzer_type", "buzzer_drive", "rated_voltage", "frequency"],
        C::Relay => &["coil_voltage", "contact_form", "contact_current"],
        C::Potentiometer => &["resistance", "taper", "power_rating"],
        C::RotaryEncoder => &["pulses_per_revolution", "detents"],
        C::FerriteBead => &["impedance", "rated_current", "dcr"],
        C::CommonModeChoke => &["impedance", "rated_current", "dcr"],
        C::LightSensor => &["light_sensor_type", "data_interface"],
        C::GasSensor => &["target_gas", "data_interface"],
        C::MagneticSensor => &["magnetic_sensor_type", "supply_voltage"],
        C::WirelessModule => &["radio_protocol", "supply_voltage", "antenna_type"],
        C::Antenna => &["frequency_band", "peak_gain"],
        c if c.is_within(C::Memory) => &["capacity"],
        c if c.is_within(C::Power) => &["output_type", "output_voltage", "output_current"],
        _ => &[],
    }
}

/// One value of a key, normalised, with its measurement conditions.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub value: Normalized,
    /// The conditions in SI (`None`: invalid, never compared).
    pub at: Option<Vec<(String, NormalizedCondition)>>,
    /// `VGS 10 V, ID 5.7 A`; empty without conditions.
    pub at_label: String,
}

/// One key in force: its values (the first is the primary one).
#[derive(Debug, Clone, PartialEq)]
pub struct Effective {
    pub spec: &'static ParameterSpec,
    pub variants: Vec<Variant>,
}

impl Effective {
    /// The primary value (what the catalog shows and filters by).
    pub fn primary(&self) -> &Normalized {
        &self.variants[0].value
    }

    /// The primary value's display form, with its conditions:
    /// `18 mΩ @ VGS 10 V`.
    pub fn label(&self) -> String {
        variant_label(&self.variants[0], self.spec)
    }

    /// The variant measured under the same conditions as `other`
    /// (like-for-like; both without conditions also match).
    pub fn like(&self, other: &Variant) -> Option<&Variant> {
        let want = other.at.as_ref()?;
        self.variants.iter().find(|v| v.at.as_ref() == Some(want))
    }
}

fn variant_label(v: &Variant, spec: &ParameterSpec) -> String {
    let value = v.value.label(spec);
    if v.at_label.is_empty() {
        value
    } else {
        format!("{value} @ {}", v.at_label)
    }
}

/// A part's parameters in force (its Device's included), normalised;
/// values that don't normalise are left out (the validator reports them).
pub type EffectiveParameters = BTreeMap<String, Effective>;

pub fn effective(part: &Parameters, device: &Parameters) -> EffectiveParameters {
    effective_parameters(part, device)
        .into_iter()
        .filter_map(|(k, (values, _))| {
            let spec = parameter_spec(k)?;
            let variants: Vec<Variant> = values
                .iter()
                .filter_map(|v| {
                    Some(Variant {
                        value: v.normalize(spec).ok()?,
                        at: comparable(&v.at),
                        at_label: conditions_label(&v.at),
                    })
                })
                .collect();
            (!variants.is_empty()).then(|| (k.to_string(), Effective { spec, variants }))
        })
        .collect()
}

/// The outcome of one key's check.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// Meets the original (equal value, or as good).
    Pass,
    /// Strictly better than the original.
    Better,
    /// Reported only (an `informational` key).
    Info,
    /// The candidate doesn't state the key: not verified.
    Unverified,
    /// Worse than the original, or (an `equal` key) different.
    Fail,
}

/// One key compared.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParameterCheck {
    pub key: String,
    pub rule: RuleKind,
    /// The original's value as displayed (`100 nF`); `None` for an
    /// identity key of the category the original doesn't state.
    pub original: Option<String>,
    /// The candidate's, when it states the key.
    pub candidate: Option<String>,
    pub status: CheckStatus,
    pub identity: bool,
}

/// All parameters of a candidate against the original.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Every key the original states is met or exceeded.
    Meets,
    /// None fails, but some aren't stated by the candidate (or the
    /// original states none at all).
    Unverified,
    /// Some key is worse (or an `equal` key differs).
    Worse,
}

/// The result of [`compare`].
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    pub checks: Vec<ParameterCheck>,
    pub verdict: Verdict,
    /// Some identity key differs: it is a different part, not an
    /// alternative.
    pub identity_mismatch: bool,
    /// Identity keys stated by both and equal.
    pub identity_verified: usize,
    /// Identity keys the original states that the candidate doesn't.
    pub identity_unknown: usize,
    /// Keys strictly better than the original.
    pub better: usize,
    /// Identity keys of the category the original doesn't state (its
    /// output voltage unknown): nothing can be verified to be the same
    /// part.
    pub identity_missing: Vec<&'static str>,
}

impl Comparison {
    /// Every identity key of the category is stated by both and equal.
    pub fn identity_complete(&self) -> bool {
        self.identity_verified > 0
            && self.identity_unknown == 0
            && self.identity_missing.is_empty()
            && !self.identity_mismatch
    }
}

/// The identity keys a part of `category` must state before another
/// part can be verified to be the same function (a capacitor's
/// capacitance, a regulator's output voltage). Other identity keys
/// (`output_type`, `polarity` on a diode...) still must not differ when
/// both state them.
pub fn identity_keys(category: ComponentCategory) -> Vec<&'static str> {
    use ComponentCategory as C;
    match category {
        C::Capacitor => vec!["capacitance"],
        C::Resistor => vec!["resistance"],
        C::Inductor => vec!["inductance"],
        C::Crystal | C::Oscillator => vec!["frequency"],
        C::Led => vec!["color"],
        C::Transistor => vec!["polarity"],
        c if c.is_within(C::Memory) => vec!["capacity"],
        C::LinearRegulator => vec!["output_voltage"],
        C::Potentiometer => vec!["resistance"],
        C::FerriteBead => vec!["impedance"],
        C::MotorDriver => vec!["motor_type"],
        C::Relay => vec!["coil_voltage", "contact_form"],
        C::RotaryEncoder => vec!["pulses_per_revolution"],
        C::WirelessModule => vec!["radio_protocol"],
        _ => vec![],
    }
}

/// The value a numeric rule compares: a rating's guaranteed limit when
/// both sides state it (the larger-is-better key's `min`, the
/// smaller-is-better key's `max`), else the stated value.
fn compared(
    kind: RuleKind,
    a: &Normalized,
    b: &Normalized,
    spec: &ParameterSpec,
) -> Option<(f64, f64)> {
    let (
        Normalized::Numeric {
            min: amin,
            max: amax,
            ..
        },
        Normalized::Numeric {
            min: bmin,
            max: bmax,
            ..
        },
    ) = (a, b)
    else {
        return None;
    };
    if !spec.deviation {
        match kind {
            AtLeast => {
                if let (Some(x), Some(y)) = (amin, bmin) {
                    return Some((*x, *y));
                }
            }
            AtMost => {
                if let (Some(x), Some(y)) = (amax, bmax) {
                    return Some((*x, *y));
                }
            }
            _ => {}
        }
    }
    Some((a.stated(spec)?, b.stated(spec)?))
}

fn check_one(
    rule: &ParameterRule,
    orig: &Normalized,
    cand: &Normalized,
    spec: &ParameterSpec,
) -> CheckStatus {
    if let (Normalized::Text(a), Normalized::Text(b)) = (orig, cand) {
        return match rule.kind {
            Equal => {
                if a == b {
                    CheckStatus::Pass
                } else {
                    CheckStatus::Fail
                }
            }
            Ranked => match (rank(rule.key, a), rank(rule.key, b)) {
                (Some(x), Some(y)) if y > x => CheckStatus::Better,
                (Some(x), Some(y)) if y == x => CheckStatus::Pass,
                (Some(_), Some(_)) => CheckStatus::Fail,
                _ if a == b => CheckStatus::Pass,
                _ => CheckStatus::Unverified,
            },
            Informational => CheckStatus::Info,
            AtLeast | AtMost | Covers => CheckStatus::Unverified,
        };
    }
    if rule.kind == Covers {
        return covers(rule, orig, cand);
    }
    let Some((a, b)) = compared(rule.kind, orig, cand, spec) else {
        return CheckStatus::Unverified;
    };
    let near = (a - b).abs() <= rule.tolerance.max(1e-9) * a.abs().max(b.abs());
    match rule.kind {
        Equal | Ranked => {
            if near {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            }
        }
        AtLeast if near => CheckStatus::Pass,
        AtLeast if b > a => CheckStatus::Better,
        AtMost if near => CheckStatus::Pass,
        AtMost if b < a => CheckStatus::Better,
        AtLeast | AtMost => CheckStatus::Fail,
        Informational => CheckStatus::Info,
        Covers => unreachable!("handled above"),
    }
}

/// A `Covers` check: each limit the original states (`min`, `max`) is
/// met by the candidate's same limit (lower or equal / higher or
/// equal). A limit the candidate doesn't state is unverified; an
/// original with neither (a lone `typ`) can't be judged.
fn covers(rule: &ParameterRule, orig: &Normalized, cand: &Normalized) -> CheckStatus {
    let (
        Normalized::Numeric {
            min: omin,
            max: omax,
            ..
        },
        Normalized::Numeric {
            min: cmin,
            max: cmax,
            ..
        },
    ) = (orig, cand)
    else {
        return CheckStatus::Unverified;
    };
    if omin.is_none() && omax.is_none() {
        return CheckStatus::Unverified;
    }
    let near = |a: f64, b: f64| (a - b).abs() <= rule.tolerance.max(1e-9) * a.abs().max(b.abs());
    let (mut wider, mut unknown) = (false, false);
    // (original limit, candidate limit, candidate is better when lower)
    for (o, c, lower_better) in [(omin, cmin, true), (omax, cmax, false)] {
        let Some(o) = o else { continue };
        match c {
            None => unknown = true,
            Some(c) if near(*o, *c) => {}
            Some(c) if (*c < *o) == lower_better => wider = true,
            Some(_) => return CheckStatus::Fail,
        }
    }
    match (unknown, wider) {
        (true, _) => CheckStatus::Unverified,
        (false, true) => CheckStatus::Better,
        (false, false) => CheckStatus::Pass,
    }
}

/// Compares every key the original states with the candidate's value,
/// and notes the identity keys of `category` the original lacks.
pub fn compare(
    orig: &EffectiveParameters,
    cand: &EffectiveParameters,
    category: Option<ComponentCategory>,
) -> Comparison {
    let mut out = Comparison {
        checks: Vec::new(),
        verdict: Verdict::Meets,
        identity_mismatch: false,
        identity_verified: 0,
        identity_unknown: 0,
        better: 0,
        identity_missing: Vec::new(),
    };
    let mut unverified = orig.is_empty();
    let expected = category.map(identity_keys).unwrap_or_default();
    // Vocabulary order, so the table reads like the catalog.
    for r in PARAMETER_RULES {
        let Some(o) = orig.get(r.key) else {
            if expected.contains(&r.key) {
                unverified = true;
                out.identity_missing.push(r.key);
                out.checks.push(ParameterCheck {
                    key: r.key.to_string(),
                    rule: r.kind,
                    original: None,
                    candidate: cand.get(r.key).map(Effective::label),
                    status: CheckStatus::Unverified,
                    identity: true,
                });
            }
            continue;
        };
        let spec = o.spec;
        let primary = &o.variants[0];
        // Like-for-like only: the candidate's value under the original's
        // conditions; a value under other conditions is not compared.
        let like = cand.get(r.key).and_then(|c| c.like(primary));
        let status = match like {
            None => CheckStatus::Unverified,
            Some(c) => check_one(r, &primary.value, &c.value, spec),
        };
        match status {
            CheckStatus::Fail => {
                out.verdict = Verdict::Worse;
                if r.identity {
                    out.identity_mismatch = true;
                }
            }
            CheckStatus::Unverified => {
                unverified = true;
                if r.identity {
                    out.identity_unknown += 1;
                }
            }
            CheckStatus::Better => out.better += 1,
            _ => {}
        }
        if r.identity && matches!(status, CheckStatus::Pass | CheckStatus::Better) {
            out.identity_verified += 1;
        }
        out.checks.push(ParameterCheck {
            key: r.key.to_string(),
            rule: r.kind,
            original: Some(variant_label(primary, spec)),
            candidate: like
                .map(|c| variant_label(c, spec))
                .or_else(|| cand.get(r.key).map(Effective::label)),
            status,
            identity: r.identity,
        });
    }
    if out.verdict != Verdict::Worse && unverified {
        out.verdict = Verdict::Unverified;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{ParameterValue, PARAMETERS};

    #[test]
    fn every_vocabulary_key_has_a_rule() {
        for spec in PARAMETERS {
            assert!(rule_for(spec.key).is_some(), "no rule for {}", spec.key);
        }
        for r in PARAMETER_RULES {
            assert!(parameter_spec(r.key).is_some(), "{} is not a key", r.key);
        }
        let open_parts_dielectrics = match parameter_spec("dielectric").unwrap().shape {
            openparts_core::ParameterShape::Enumerated(v) => v,
            _ => unreachable!(),
        };
        for d in open_parts_dielectrics {
            assert!(rank("dielectric", d).is_some(), "no rank for {d}");
        }
        // Every value of a ranked key has a rank.
        for r in PARAMETER_RULES.iter().filter(|r| r.kind == Ranked) {
            if let openparts_core::ParameterShape::Enumerated(values) =
                parameter_spec(r.key).unwrap().shape
            {
                for v in values {
                    assert!(rank(r.key, v).is_some(), "no rank for {} {v}", r.key);
                }
            }
        }
        assert!(rank("microstepping", "256") > rank("microstepping", "16"));
        assert!(rank("resolution", "24") > rank("resolution", "16"));
        // Operating ranges are compared by containment, and only they.
        for r in PARAMETER_RULES {
            let spec = parameter_spec(r.key).unwrap();
            assert_eq!(spec.span, r.kind == Covers, "{}", r.key);
        }
    }

    fn params(list: &[(&str, ParameterValue)]) -> EffectiveParameters {
        let p: Parameters = list
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone().into()))
            .collect();
        effective(&p, &Parameters::new())
    }

    fn num(n: f64, unit: &str) -> ParameterValue {
        ParameterValue {
            typ: Some(n),
            unit: Some(unit.into()),
            ..Default::default()
        }
    }

    fn word(v: &str) -> ParameterValue {
        ParameterValue {
            value: Some(v.into()),
            ..Default::default()
        }
    }

    fn tol(pct: f64) -> ParameterValue {
        ParameterValue {
            min: Some(-pct),
            max: Some(pct),
            unit: Some("%".into()),
            ..Default::default()
        }
    }

    fn status(c: &Comparison, key: &str) -> CheckStatus {
        c.checks.iter().find(|x| x.key == key).unwrap().status
    }

    #[test]
    fn capacitor_rules() {
        let orig = params(&[
            ("capacitance", num(100.0, "nF")),
            ("rated_voltage", num(50.0, "V")),
            ("dielectric", word("X7R")),
            ("tolerance", tol(10.0)),
        ]);
        // 0.1 µF is 100 nF; 100 V and C0G beat 50 V and X7R; ±5 % beats ±10 %.
        let better = params(&[
            ("capacitance", num(0.1, "uF")),
            ("rated_voltage", num(100.0, "V")),
            ("dielectric", word("C0G")),
            ("tolerance", tol(5.0)),
        ]);
        let c = compare(&orig, &better, None);
        assert_eq!(c.verdict, Verdict::Meets);
        assert_eq!(status(&c, "capacitance"), CheckStatus::Pass);
        assert_eq!(status(&c, "rated_voltage"), CheckStatus::Better);
        assert_eq!(status(&c, "dielectric"), CheckStatus::Better);
        assert_eq!(status(&c, "tolerance"), CheckStatus::Better);
        assert_eq!(c.identity_verified, 1);

        let worse = params(&[
            ("capacitance", num(100.0, "nF")),
            ("rated_voltage", num(16.0, "V")),
            ("dielectric", word("X5R")),
            ("tolerance", tol(20.0)),
        ]);
        let c = compare(&orig, &worse, None);
        assert_eq!(c.verdict, Verdict::Worse);
        assert!(!c.identity_mismatch);
        for k in ["rated_voltage", "dielectric", "tolerance"] {
            assert_eq!(status(&c, k), CheckStatus::Fail, "{k}");
        }

        let other_value = params(&[("capacitance", num(220.0, "nF"))]);
        assert!(compare(&orig, &other_value, None).identity_mismatch);

        // Unknown counts as unverified, never as a pass.
        let sparse = params(&[("capacitance", num(100.0, "nF"))]);
        let c = compare(&orig, &sparse, None);
        assert_eq!(c.verdict, Verdict::Unverified);
        assert_eq!(status(&c, "rated_voltage"), CheckStatus::Unverified);
    }

    #[test]
    fn smaller_is_better_and_enums() {
        let orig = params(&[
            ("rds_on", num(50.0, "mΩ")),
            ("polarity", word("n_channel")),
            ("drain_source_voltage", num(30.0, "V")),
        ]);
        let cand = params(&[
            ("rds_on", num(35.0, "mΩ")),
            ("polarity", word("n_channel")),
            ("drain_source_voltage", num(30.0, "V")),
        ]);
        let c = compare(&orig, &cand, None);
        assert_eq!(c.verdict, Verdict::Meets);
        assert_eq!(status(&c, "rds_on"), CheckStatus::Better);
        let p = params(&[("polarity", word("p_channel"))]);
        assert!(compare(&orig, &p, None).identity_mismatch);
    }

    #[test]
    fn values_are_compared_like_for_like() {
        let at = |v: ParameterValue, vgs: f64| ParameterValue {
            at: [(
                "VGS".to_string(),
                openparts_core::Condition {
                    value: Some(vgs),
                    unit: "V".into(),
                    ..Default::default()
                },
            )]
            .into(),
            ..v
        };
        let orig = params(&[("rds_on", at(num(50.0, "mΩ"), 10.0))]);
        // The candidate's primary value is at 4.5 V; its 10 V value is
        // the one compared.
        let mut p = Parameters::new();
        p.insert(
            "rds_on".into(),
            openparts_core::ParameterValues(vec![
                at(num(60.0, "mΩ"), 4.5),
                at(num(35.0, "mΩ"), 10.0),
            ]),
        );
        let cand = effective(&p, &Parameters::new());
        let c = compare(&orig, &cand, None);
        assert_eq!(status(&c, "rds_on"), CheckStatus::Better);
        assert_eq!(c.checks[0].original.as_deref(), Some("50 mΩ @ VGS 10 V"));
        assert_eq!(c.checks[0].candidate.as_deref(), Some("35 mΩ @ VGS 10 V"));
        // Only other conditions (or none): not comparable.
        let other = params(&[("rds_on", at(num(20.0, "mΩ"), 4.5))]);
        assert_eq!(
            status(&compare(&orig, &other, None), "rds_on"),
            CheckStatus::Unverified
        );
        let bare = params(&[("rds_on", num(20.0, "mΩ"))]);
        assert_eq!(
            status(&compare(&orig, &bare, None), "rds_on"),
            CheckStatus::Unverified
        );
    }

    #[test]
    fn regulator_dropout_uses_the_guaranteed_limit() {
        let mut orig_dropout = num(250.0, "mV");
        orig_dropout.max = Some(400.0);
        let mut cand_dropout = num(200.0, "mV");
        cand_dropout.max = Some(450.0);
        let orig = params(&[
            ("output_voltage", num(3.3, "V")),
            ("dropout_voltage", orig_dropout),
        ]);
        let cand = params(&[
            ("output_voltage", num(3.3, "V")),
            ("dropout_voltage", cand_dropout),
        ]);
        // Typical 200 < 250 mV, but the guaranteed 450 > 400 mV: worse.
        assert_eq!(
            status(&compare(&orig, &cand, None), "dropout_voltage"),
            CheckStatus::Fail
        );
        let five = params(&[("output_voltage", num(5.0, "V"))]);
        assert!(compare(&orig, &five, None).identity_mismatch);
    }

    #[test]
    fn an_unstated_identity_key_is_never_verified() {
        // Neither side states the output voltage: 3.3 V and 5 V parts
        // could not be told apart.
        let orig = params(&[("output_current", num(800.0, "mA"))]);
        let cand = params(&[("output_current", num(1.0, "A"))]);
        let c = compare(&orig, &cand, Some(ComponentCategory::LinearRegulator));
        assert_eq!(c.verdict, Verdict::Unverified);
        assert!(!c.identity_complete());
        assert_eq!(c.identity_missing, vec!["output_voltage"]);
        assert_eq!(status(&c, "output_voltage"), CheckStatus::Unverified);
    }

    fn range(lo: f64, hi: f64, unit: &str) -> ParameterValue {
        ParameterValue {
            min: Some(lo),
            max: Some(hi),
            unit: Some(unit.into()),
            ..Default::default()
        }
    }

    #[test]
    fn ranges_must_cover_the_original() {
        let orig = params(&[
            ("supply_voltage", range(2.7, 5.5, "V")),
            ("operating_temperature", range(-40.0, 85.0, "°C")),
        ]);
        let wider = params(&[
            ("supply_voltage", range(1.8, 5.5, "V")),
            ("operating_temperature", range(-40.0, 125.0, "°C")),
        ]);
        let c = compare(&orig, &wider, None);
        assert_eq!(c.verdict, Verdict::Meets);
        assert_eq!(status(&c, "supply_voltage"), CheckStatus::Better);
        assert_eq!(status(&c, "operating_temperature"), CheckStatus::Better);
        // 3.0 V minimum: a 2.7 V design is not covered.
        let narrower = params(&[
            ("supply_voltage", range(3.0, 5.5, "V")),
            ("operating_temperature", range(-40.0, 85.0, "°C")),
        ]);
        let c = compare(&orig, &narrower, None);
        assert_eq!(status(&c, "supply_voltage"), CheckStatus::Fail);
        assert_eq!(status(&c, "operating_temperature"), CheckStatus::Pass);
        assert_eq!(c.verdict, Verdict::Worse);
        // The candidate states only a maximum: its minimum is unknown.
        let mut max_only = num(5.5, "V");
        max_only.typ = None;
        max_only.max = Some(5.5);
        let c = compare(&orig, &params(&[("supply_voltage", max_only)]), None);
        assert_eq!(status(&c, "supply_voltage"), CheckStatus::Unverified);
    }

    #[test]
    fn amplifier_and_logic_rules() {
        let orig = params(&[
            ("channel_count", word("2")),
            ("gain_bandwidth", num(1.0, "MHz")),
            ("slew_rate", num(0.5, "V/µs")),
            ("rail_to_rail_output", word("no")),
            ("propagation_delay", num(10.0, "ns")),
        ]);
        let cand = params(&[
            ("channel_count", word("2")),
            ("gain_bandwidth", num(10.0, "MHz")),
            ("slew_rate", num(0.3, "V/us")),
            ("rail_to_rail_output", word("yes")),
            ("propagation_delay", num(5.0, "ns")),
        ]);
        let c = compare(&orig, &cand, None);
        assert_eq!(status(&c, "channel_count"), CheckStatus::Pass);
        assert_eq!(status(&c, "gain_bandwidth"), CheckStatus::Better);
        assert_eq!(status(&c, "slew_rate"), CheckStatus::Fail);
        assert_eq!(status(&c, "rail_to_rail_output"), CheckStatus::Better);
        assert_eq!(status(&c, "propagation_delay"), CheckStatus::Better);
        let quad = params(&[("channel_count", word("4"))]);
        assert!(compare(&orig, &quad, None).identity_mismatch);
        // Rail-to-rail is a grade: "no" does not replace "yes".
        let back = compare(&cand, &orig, None);
        assert_eq!(status(&back, "rail_to_rail_output"), CheckStatus::Fail);
    }
}
