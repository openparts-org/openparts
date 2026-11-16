//! Canonical Data Specification section 11.1: typed electrical
//! parameters.
//!
//! A Part (and, for facts shared by a whole family, its Device) may
//! carry a `parameters` map. Its keys come from a closed vocabulary
//! ([`PARAMETERS`]); each key has one fixed shape -- a physical quantity
//! with an SI base unit, or an enumerated text value -- so a value can
//! always be normalised to SI and compared across manufacturers. A key
//! holds one value or a list of condition variants (`at`, section 11.2).
//!
//! In memory (and in the API) a value is `min`/`typ`/`max` + `unit`;
//! the data file writes `{ typ: 100 nF }` (openparts-data's format).
//!
//! Nothing here reads a format: the vocabulary, unit table, SI
//! normalisation, engineering-prefix formatting and the parser for
//! typed filter values are plain Rust.

use crate::category::ComponentCategory;
use crate::condition::Conditions;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One parameter value as the data states it. A numeric key uses
/// `min`/`typ`/`max` + `unit` (like a [`crate::Dimension`]; none of
/// the three is invented when the source doesn't state it); an
/// enumerated key uses `value`. Which shape a key takes is fixed by the
/// vocabulary; the validator rejects the other one. `at` holds the
/// measurement conditions the source states for the value.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ParameterValue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// The typical value (schema 0.1: `nominal`).
    #[serde(default, alias = "nominal", skip_serializing_if = "Option::is_none")]
    pub typ: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Unit of `min`/`typ`/`max`, e.g. `nF`, `kΩ`, `%`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The value of an enumerated key, e.g. `X7R`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Measurement conditions (section 11.2), e.g. `VGS: 10 V`.
    #[serde(
        default,
        skip_serializing_if = "Conditions::is_empty",
        serialize_with = "crate::condition::serialize_ordered"
    )]
    pub at: Conditions,
}

/// The values of one parameter key: one, or several condition variants
/// (`rds_on` at VGS 10 V and at 4.5 V). The first is the *primary*
/// value, the one the catalog, search and filters use. Never empty.
///
/// Reads one value or a list; always serialises as a list.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ParameterValues(pub Vec<ParameterValue>);

impl ParameterValues {
    pub fn one(v: ParameterValue) -> Self {
        ParameterValues(vec![v])
    }

    /// The primary value (the first).
    pub fn primary(&self) -> &ParameterValue {
        &self.0[0]
    }

    pub fn primary_mut(&mut self) -> &mut ParameterValue {
        &mut self.0[0]
    }

    pub fn iter(&self) -> std::slice::Iter<'_, ParameterValue> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The variant stated under exactly the conditions `at` (compared
    /// in SI), if any: the like-for-like counterpart of another value.
    pub fn at(&self, at: &Conditions) -> Option<&ParameterValue> {
        let want = crate::condition::comparable(at)?;
        self.0
            .iter()
            .find(|v| crate::condition::comparable(&v.at).as_ref() == Some(&want))
    }
}

impl From<ParameterValue> for ParameterValues {
    fn from(v: ParameterValue) -> Self {
        ParameterValues::one(v)
    }
}

impl<'a> IntoIterator for &'a ParameterValues {
    type Item = &'a ParameterValue;
    type IntoIter = std::slice::Iter<'a, ParameterValue>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'de> Deserialize<'de> for ParameterValues {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{value, Error, MapAccess, SeqAccess, Visitor};
        struct OneOrMany;
        impl<'de> Visitor<'de> for OneOrMany {
            type Value = Vec<ParameterValue>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "a parameter value or a list of them")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                Ok(vec![ParameterValue::deserialize(
                    value::MapAccessDeserializer::new(map),
                )?])
            }
            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                Vec::<ParameterValue>::deserialize(value::SeqAccessDeserializer::new(seq))
            }
        }
        let list = d.deserialize_any(OneOrMany)?;
        if list.is_empty() {
            return Err(D::Error::custom("a parameter needs at least one value"));
        }
        Ok(ParameterValues(list))
    }
}

/// Parameter key -> its values.
pub type Parameters = BTreeMap<String, ParameterValues>;

/// The physical quantity of a numeric key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantity {
    Capacitance,
    Resistance,
    Inductance,
    Voltage,
    Current,
    Power,
    Frequency,
    /// A dimensionless ratio (tolerances); SI value 1 = 100 %.
    Ratio,
    /// Relative change per kelvin (resistor TCR); SI unit 1/K.
    TemperatureCoefficient,
    Length,
    Force,
    /// Storage capacity in bits. `Kbit`/`Mbit`/`Gbit` are binary
    /// (2^10, 2^20, 2^30), as memory datasheets use them (JEDEC).
    Information,
    /// Luminous intensity (an LED's brightness), in candela.
    LuminousIntensity,
    /// A serial data rate in bit/s. `kbps`/`Kbps`/`Mbps` are decimal
    /// (10^3, 10^6), as interface datasheets use them.
    DataRate,
    /// An amplifier's output slew rate, in V/s (shown in V/µs).
    SlewRate,
    /// A plane angle in degrees (an LED's viewing angle; not SI).
    Angle,
    /// A temperature, in °C (not SI): measurement conditions and
    /// operating ranges.
    Temperature,
    /// A duration: measurement conditions and delays.
    Time,
}

impl Quantity {
    /// Every quantity.
    pub const ALL: &'static [Quantity] = &[
        Quantity::Capacitance,
        Quantity::Resistance,
        Quantity::Inductance,
        Quantity::Voltage,
        Quantity::Current,
        Quantity::Power,
        Quantity::Frequency,
        Quantity::Ratio,
        Quantity::TemperatureCoefficient,
        Quantity::Length,
        Quantity::Force,
        Quantity::Information,
        Quantity::LuminousIntensity,
        Quantity::DataRate,
        Quantity::SlewRate,
        Quantity::Angle,
        Quantity::Temperature,
        Quantity::Time,
    ];

    /// The SI (base) unit symbol normalised values are in.
    pub fn si_unit(self) -> &'static str {
        match self {
            Quantity::Capacitance => "F",
            Quantity::Resistance => "Ω",
            Quantity::Inductance => "H",
            Quantity::Voltage => "V",
            Quantity::Current => "A",
            Quantity::Power => "W",
            Quantity::Frequency => "Hz",
            Quantity::Ratio => "1",
            Quantity::TemperatureCoefficient => "1/K",
            Quantity::Length => "m",
            Quantity::Force => "N",
            Quantity::Information => "bit",
            Quantity::LuminousIntensity => "cd",
            Quantity::DataRate => "bit/s",
            Quantity::SlewRate => "V/s",
            Quantity::Angle => "°",
            Quantity::Temperature => "°C",
            Quantity::Time => "s",
        }
    }

    /// Every accepted unit spelling with its factor to the SI unit.
    /// Spellings are case-sensitive (`mΩ` is milli-, `MΩ` mega-ohm).
    pub fn units(self) -> &'static [(&'static str, f64)] {
        match self {
            Quantity::Capacitance => &[
                ("pF", 1e-12),
                ("nF", 1e-9),
                ("uF", 1e-6),
                ("µF", 1e-6),
                ("μF", 1e-6),
                ("mF", 1e-3),
                ("F", 1.0),
            ],
            Quantity::Resistance => &[
                ("mΩ", 1e-3),
                ("mohm", 1e-3),
                ("Ω", 1.0),
                ("Ω", 1.0),
                ("ohm", 1.0),
                ("kΩ", 1e3),
                ("kohm", 1e3),
                ("MΩ", 1e6),
                ("Mohm", 1e6),
            ],
            Quantity::Inductance => &[
                ("nH", 1e-9),
                ("uH", 1e-6),
                ("µH", 1e-6),
                ("μH", 1e-6),
                ("mH", 1e-3),
                ("H", 1.0),
            ],
            Quantity::Voltage => &[
                ("uV", 1e-6),
                ("µV", 1e-6),
                ("μV", 1e-6),
                ("mV", 1e-3),
                ("V", 1.0),
                ("kV", 1e3),
            ],
            Quantity::Current => &[
                ("fA", 1e-15),
                ("pA", 1e-12),
                ("nA", 1e-9),
                ("uA", 1e-6),
                ("µA", 1e-6),
                ("μA", 1e-6),
                ("mA", 1e-3),
                ("A", 1.0),
            ],
            Quantity::Power => &[
                ("uW", 1e-6),
                ("µW", 1e-6),
                ("μW", 1e-6),
                ("mW", 1e-3),
                ("W", 1.0),
            ],
            Quantity::Frequency => &[("Hz", 1.0), ("kHz", 1e3), ("MHz", 1e6), ("GHz", 1e9)],
            Quantity::Ratio => &[("%", 1e-2), ("ppm", 1e-6)],
            Quantity::TemperatureCoefficient => &[
                ("ppm/K", 1e-6),
                ("ppm/°C", 1e-6),
                ("%/K", 1e-2),
                ("%/°C", 1e-2),
            ],
            Quantity::Length => &[
                ("nm", 1e-9),
                ("um", 1e-6),
                ("µm", 1e-6),
                ("μm", 1e-6),
                ("mm", 1e-3),
                ("m", 1.0),
            ],
            Quantity::Force => &[("mN", 1e-3), ("N", 1.0), ("gf", 9.80665e-3)],
            Quantity::Information => &[
                ("bit", 1.0),
                ("Kbit", 1024.0),
                ("kbit", 1024.0),
                ("Mbit", 1048576.0),
                ("Gbit", 1073741824.0),
            ],
            Quantity::LuminousIntensity => {
                &[("ucd", 1e-6), ("µcd", 1e-6), ("mcd", 1e-3), ("cd", 1.0)]
            }
            Quantity::DataRate => &[
                ("bit/s", 1.0),
                ("bps", 1.0),
                ("kbit/s", 1e3),
                ("kbps", 1e3),
                ("Kbps", 1e3),
                ("Mbit/s", 1e6),
                ("Mbps", 1e6),
            ],
            Quantity::SlewRate => &[
                ("V/s", 1.0),
                ("V/ms", 1e3),
                ("V/us", 1e6),
                ("V/µs", 1e6),
                ("V/μs", 1e6),
            ],
            Quantity::Angle => &[("°", 1.0), ("deg", 1.0)],
            Quantity::Temperature => &[("°C", 1.0)],
            Quantity::Time => &[
                ("ps", 1e-12),
                ("ns", 1e-9),
                ("us", 1e-6),
                ("µs", 1e-6),
                ("ms", 1e-3),
                ("s", 1.0),
            ],
        }
    }

    /// The factor from `unit` to the SI unit, or `None` if `unit` is not
    /// a unit of this quantity.
    pub fn unit_factor(self, unit: &str) -> Option<f64> {
        self.units()
            .iter()
            .find(|(u, _)| *u == unit)
            .map(|(_, f)| *f)
    }

    /// Display prefixes (symbol, factor) for engineering notation,
    /// smallest first, with the unit symbol they precede.
    fn display(self) -> (&'static [(&'static str, f64)], &'static str) {
        const SMALL: &[(&str, f64)] = &[
            ("p", 1e-12),
            ("n", 1e-9),
            ("µ", 1e-6),
            ("m", 1e-3),
            ("", 1.0),
            ("k", 1e3),
            ("M", 1e6),
            ("G", 1e9),
        ];
        match self {
            Quantity::Capacitance => (&SMALL[..5], "F"),
            Quantity::Resistance => (&SMALL[3..], "Ω"),
            Quantity::Inductance => (&SMALL[1..5], "H"),
            Quantity::Voltage => (&SMALL[2..6], "V"),
            Quantity::Current => (&SMALL[..5], "A"),
            Quantity::Power => (&SMALL[2..5], "W"),
            Quantity::Frequency => (&SMALL[4..], "Hz"),
            Quantity::Ratio => (&[("", 1e-2)], "%"),
            Quantity::TemperatureCoefficient => (&[("", 1e-6)], "ppm/K"),
            Quantity::Length => (&SMALL[1..5], "m"),
            Quantity::Force => (&[("", 1.0)], "N"),
            Quantity::Information => (
                &[
                    ("", 1.0),
                    ("K", 1024.0),
                    ("M", 1048576.0),
                    ("G", 1073741824.0),
                ],
                "bit",
            ),
            Quantity::LuminousIntensity => (&SMALL[2..5], "cd"),
            Quantity::DataRate => (&SMALL[4..7], "bit/s"),
            Quantity::SlewRate => (&[("", 1.0)], "V/s"),
            Quantity::Angle => (&[("", 1.0)], "°"),
            Quantity::Temperature => (&[("", 1.0)], "°C"),
            Quantity::Time => (&SMALL[..5], "s"),
        }
    }
}

/// What a key's value is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterShape {
    /// `typ`/`min`/`max` + `unit` of this quantity.
    Numeric(Quantity),
    /// `value`, one of these (case-sensitive) spellings.
    Enumerated(&'static [&'static str]),
}

/// One vocabulary entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParameterSpec {
    pub key: &'static str,
    pub shape: ParameterShape,
    /// The device categories the key is plausible for (a category's
    /// sub-categories included). Anything else is a validator warning,
    /// not an error.
    pub categories: &'static [ComponentCategory],
    /// A fixed display unit (`%`, `ppm`, `mm`, `nm`) instead of
    /// engineering prefixes; also the unit a bare number in a filter
    /// is read in.
    pub display_unit: Option<&'static str>,
    /// The value is a deviation around a target (a tolerance): data
    /// states `min`/`max` (e.g. -10/+10 %), and the value it is
    /// compared and filtered by is the larger magnitude of the two.
    pub deviation: bool,
    /// The plausible SI range (inclusive) of every stated number
    /// (`min`, `typ`, `max`; a deviation's magnitude): a value outside
    /// is almost always a unit slip (`20 %` for `20 ppm`), so the
    /// validator warns (`parameter.implausible_value`).
    pub range: Option<(f64, f64)>,
    /// The value is an operating range (`min`..`max`: a supply voltage,
    /// an operating temperature): a catalog range filter asks whether
    /// it contains the asked span, and alternatives must cover it.
    pub span: bool,
}

impl ParameterSpec {
    /// Whether SI value `v` lies in the key's plausible range (always,
    /// without one).
    pub fn in_range(&self, v: f64) -> bool {
        let v = if self.deviation { v.abs() } else { v };
        self.range.is_none_or(|(lo, hi)| {
            let eps = 1e-9 * lo.abs().max(hi.abs());
            v >= lo - eps && v <= hi + eps
        })
    }
}

use ComponentCategory as C;
use ParameterShape::{Enumerated, Numeric};
use Quantity as Q;

const fn num(key: &'static str, q: Quantity, categories: &'static [C]) -> ParameterSpec {
    ParameterSpec {
        key,
        shape: Numeric(q),
        categories,
        display_unit: None,
        deviation: false,
        range: None,
        span: false,
    }
}

const fn shown_in(mut spec: ParameterSpec, unit: &'static str) -> ParameterSpec {
    spec.display_unit = Some(unit);
    spec
}

const fn deviation(mut spec: ParameterSpec) -> ParameterSpec {
    spec.deviation = true;
    spec
}

/// An operating-range key (see [`ParameterSpec::span`]).
const fn span(mut spec: ParameterSpec) -> ParameterSpec {
    spec.span = true;
    spec
}

/// The plausible SI range of the key's values (see [`ParameterSpec::range`]).
const fn within(mut spec: ParameterSpec, lo: f64, hi: f64) -> ParameterSpec {
    spec.range = Some((lo, hi));
    spec
}

const fn enumerated(
    key: &'static str,
    values: &'static [&'static str],
    categories: &'static [C],
) -> ParameterSpec {
    ParameterSpec {
        key,
        shape: Enumerated(values),
        categories,
        display_unit: None,
        deviation: false,
        range: None,
        span: false,
    }
}

const PASSIVE_RLC: &[C] = &[C::Capacitor, C::Resistor, C::Inductor];
/// Forward/reverse ratings: diodes, LEDs and an optocoupler's input LED.
const SEMIS_VF: &[C] = &[C::Diode, C::Led, C::Optocoupler];
/// Collector ratings: BJTs and an optocoupler's phototransistor output.
const COLLECTOR: &[C] = &[C::Transistor, C::Optocoupler];
/// Frequency-control parts: crystals (resonators) and oscillators.
const XTAL: &[C] = &[C::Crystal, C::Oscillator];
/// Integrated circuits (a group covers its subcategories) and LEDs
/// with an integrated driver (addressable RGB LEDs).
const ICS_AND_LED: &[C] = &[
    C::Led,
    C::Microcontroller,
    C::Power,
    C::Memory,
    C::Sensor,
    C::Analog,
    C::Interface,
    C::Logic,
    C::Timing,
    C::Driver,
];
/// A supply-voltage range: ICs and driver LEDs, but not regulators
/// (their input is `input_voltage_max`).
const SUPPLIED: &[C] = &[
    C::Led,
    C::Microcontroller,
    C::Memory,
    C::Sensor,
    C::Analog,
    C::Interface,
    C::Logic,
    C::Timing,
    C::Driver,
];
/// Amplifier inputs: op-amps, comparators, current-sense amplifiers.
const AMPS: &[C] = &[C::OpAmp, C::Comparator, C::CurrentSenseAmplifier];
/// Digital outputs with a timing: comparators, logic, interface ICs.
const DIGITAL: &[C] = &[C::Comparator, C::Logic, C::Interface];
/// Parts made of identical channels (amplifiers, gates, transceivers).
const CHANNELS: &[C] = &[C::Analog, C::Interface, C::Logic, C::Driver];
/// `yes` / `no` flags (ranked: `yes` is better).
const YES_NO: &[&str] = &["yes", "no"];
/// ±1000 ppm: a frequency deviation beyond it is a unit slip.
const PPM_1000: f64 = 1e-3;

/// The closed parameter vocabulary, in display order. Extend it (with
/// the spec, Canonical Data Specification section 11.1) when a new
/// component type needs a key; never reuse a key for another quantity.
pub const PARAMETERS: &[ParameterSpec] = &[
    // Capacitors
    num("capacitance", Q::Capacitance, &[C::Capacitor]),
    deviation(shown_in(num("tolerance", Q::Ratio, PASSIVE_RLC), "%")),
    deviation(num(
        "capacitance_tolerance",
        Q::Capacitance,
        &[C::Capacitor],
    )),
    num(
        "rated_voltage",
        Q::Voltage,
        &[C::Capacitor, C::Fuse, C::Resistor, C::Switch],
    ),
    enumerated(
        "dielectric",
        &[
            "C0G", "NP0", "U2J", "X5R", "X5S", "X6S", "X6T", "X7R", "X7S", "X7T", "X8L", "X8R",
            "Y5V", "Z5U",
        ],
        &[C::Capacitor],
    ),
    num("esr", Q::Resistance, &[C::Capacitor, C::Crystal]),
    // Resistors
    num("resistance", Q::Resistance, &[C::Resistor]),
    num("power_rating", Q::Power, &[C::Resistor, C::Diode, C::Led]),
    deviation(shown_in(
        num(
            "temperature_coefficient",
            Q::TemperatureCoefficient,
            &[C::Resistor],
        ),
        "ppm/K",
    )),
    // Inductors
    num("inductance", Q::Inductance, &[C::Inductor]),
    num(
        "rated_current",
        Q::Current,
        &[C::Inductor, C::Fuse, C::Switch],
    ),
    num("saturation_current", Q::Current, &[C::Inductor]),
    num("dcr", Q::Resistance, &[C::Inductor]),
    // Crystals and oscillators
    num("frequency", Q::Frequency, XTAL),
    within(
        num("load_capacitance", Q::Capacitance, &[C::Crystal]),
        0.0,
        1e-9,
    ),
    within(
        deviation(shown_in(num("frequency_tolerance", Q::Ratio, XTAL), "ppm")),
        0.0,
        PPM_1000,
    ),
    // Over the operating temperature range (`at: { TA: -40..85 °C }`).
    within(
        deviation(shown_in(num("frequency_stability", Q::Ratio, XTAL), "ppm")),
        0.0,
        PPM_1000,
    ),
    // Frequency change in the first year (ppm/year as printed).
    within(
        deviation(shown_in(num("aging", Q::Ratio, XTAL), "ppm")),
        0.0,
        PPM_1000,
    ),
    within(
        shown_in(num("drive_level", Q::Power, &[C::Crystal]), "µW"),
        0.0,
        1e-2,
    ),
    within(
        num("shunt_capacitance", Q::Capacitance, &[C::Crystal]),
        0.0,
        1e-10,
    ),
    enumerated(
        "clock_output",
        &[
            "cmos",
            "ttl",
            "lvds",
            "lvpecl",
            "hcsl",
            "cml",
            "clipped_sine",
            "sine",
        ],
        &[C::Oscillator],
    ),
    // Diodes and LEDs
    num("forward_voltage", Q::Voltage, SEMIS_VF),
    num("reverse_voltage", Q::Voltage, SEMIS_VF),
    num("forward_current", Q::Current, SEMIS_VF),
    num("zener_voltage", Q::Voltage, &[C::Diode]),
    num("zener_test_current", Q::Current, &[C::Diode]),
    enumerated(
        "color",
        &[
            "red",
            "orange",
            "amber",
            "yellow",
            "green",
            "blue",
            "white",
            "infrared",
            "ultraviolet",
            "rgb",
        ],
        &[C::Led],
    ),
    shown_in(num("wavelength", Q::Length, &[C::Led]), "nm"),
    within(
        shown_in(num("peak_wavelength", Q::Length, &[C::Led]), "nm"),
        1e-7,
        1e-5,
    ),
    num("luminous_intensity", Q::LuminousIntensity, &[C::Led]),
    // Full angle at half intensity (2θ½).
    within(num("viewing_angle", Q::Angle, &[C::Led]), 0.0, 360.0),
    // RGB LEDs: one key per emitter colour (dominant wavelength,
    // luminous intensity); addressable ones (an integrated driver) also
    // state their supply and data input.
    shown_in(num("wavelength_red", Q::Length, &[C::Led]), "nm"),
    shown_in(num("wavelength_green", Q::Length, &[C::Led]), "nm"),
    shown_in(num("wavelength_blue", Q::Length, &[C::Led]), "nm"),
    num("luminous_intensity_red", Q::LuminousIntensity, &[C::Led]),
    num("luminous_intensity_green", Q::LuminousIntensity, &[C::Led]),
    num("luminous_intensity_blue", Q::LuminousIntensity, &[C::Led]),
    enumerated("data_interface", &["single_wire", "clock_data"], &[C::Led]),
    // Transistors
    enumerated(
        "polarity",
        &["npn", "pnp", "n_channel", "p_channel"],
        &[C::Transistor],
    ),
    num("drain_source_voltage", Q::Voltage, &[C::Transistor]),
    num("drain_current", Q::Current, &[C::Transistor]),
    num("rds_on", Q::Resistance, &[C::Transistor]),
    num("gate_threshold_voltage", Q::Voltage, &[C::Transistor]),
    num("collector_emitter_voltage", Q::Voltage, COLLECTOR),
    num("collector_current", Q::Current, COLLECTOR),
    // Optocouplers (input LED: forward_voltage/forward_current/
    // reverse_voltage; output: collector_emitter_voltage/collector_current)
    num("isolation_voltage", Q::Voltage, &[C::Optocoupler]),
    shown_in(
        num("current_transfer_ratio", Q::Ratio, &[C::Optocoupler]),
        "%",
    ),
    // Regulators
    enumerated("output_type", &["fixed", "adjustable"], &[C::Power]),
    num("output_voltage", Q::Voltage, &[C::Power]),
    num("input_voltage_max", Q::Voltage, &[C::Power]),
    num("output_current", Q::Current, &[C::Power]),
    num("dropout_voltage", Q::Voltage, &[C::LinearRegulator]),
    // Memory
    num("capacity", Q::Information, &[C::Memory]),
    // Switches
    num("actuation_force", Q::Force, &[C::Switch]),
    shown_in(num("travel", Q::Length, &[C::Switch]), "mm"),
    // Integrated circuits in general. The supply as a range (min/max);
    // an amplifier's is the total span V+ - V- (±18 V is 36 V).
    within(span(num("supply_voltage", Q::Voltage, SUPPLIED)), -1e3, 1e3),
    within(num("supply_current", Q::Current, ICS_AND_LED), 0.0, 10.0),
    within(
        span(num("operating_temperature", Q::Temperature, C::ALL)),
        -100.0,
        300.0,
    ),
    // Human-body-model ESD rating of all pins.
    within(num("esd_hbm", Q::Voltage, ICS_AND_LED), 1e2, 1e5),
    // Amplifiers and comparators
    enumerated(
        "channel_count",
        &["1", "2", "3", "4", "6", "8", "16"],
        CHANNELS,
    ),
    within(
        num(
            "gain_bandwidth",
            Q::Frequency,
            &[C::OpAmp, C::CurrentSenseAmplifier],
        ),
        0.0,
        1e11,
    ),
    within(
        shown_in(
            num(
                "slew_rate",
                Q::SlewRate,
                &[C::OpAmp, C::CurrentSenseAmplifier],
            ),
            "V/µs",
        ),
        0.0,
        1e11,
    ),
    // Magnitudes: the printed ±3 mV max is `max: 3 mV`.
    within(num("input_offset_voltage", Q::Voltage, AMPS), 0.0, 1.0),
    within(num("input_bias_current", Q::Current, AMPS), 0.0, 0.1),
    within(
        num(
            "supply_current_per_channel",
            Q::Current,
            &[C::OpAmp, C::Comparator],
        ),
        0.0,
        1.0,
    ),
    enumerated("rail_to_rail_input", YES_NO, &[C::OpAmp, C::Comparator]),
    enumerated("rail_to_rail_output", YES_NO, &[C::OpAmp, C::Comparator]),
    within(num("propagation_delay", Q::Time, DIGITAL), 0.0, 1e-3),
    enumerated(
        "output_structure",
        &["push_pull", "open_drain", "open_collector"],
        DIGITAL,
    ),
    // Logic
    enumerated(
        "logic_function",
        &[
            "buffer",
            "inverter",
            "and",
            "nand",
            "or",
            "nor",
            "xor",
            "xnor",
            "bus_transceiver",
            "flip_flop",
            "latch",
            "shift_register",
            "counter",
            "decoder",
            "multiplexer",
        ],
        &[C::Logic],
    ),
    // Output drive, as magnitudes (IOH -24 mA is 24 mA).
    within(
        num("output_current_high", Q::Current, &[C::Logic, C::Interface]),
        0.0,
        1.0,
    ),
    within(
        num("output_current_low", Q::Current, &[C::Logic, C::Interface]),
        0.0,
        1.0,
    ),
    // Interface (and addressable LEDs' data input)
    within(
        num("data_rate", Q::DataRate, &[C::Led, C::Interface]),
        0.0,
        1e12,
    ),
    // Bus pins' fault-protection range (abs max, min/max as printed).
    within(
        span(num("bus_fault_voltage", Q::Voltage, &[C::Interface])),
        -1e3,
        1e3,
    ),
];

/// The vocabulary entry for `key`.
pub fn parameter_spec(key: &str) -> Option<&'static ParameterSpec> {
    PARAMETERS.iter().find(|s| s.key == key)
}

/// Why a value can't be normalised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterError {
    /// The key is not in [`PARAMETERS`].
    UnknownKey,
    /// A numeric key without `unit`.
    MissingUnit,
    /// `unit` is not a unit of the key's quantity.
    IncompatibleUnit { unit: String, quantity: Quantity },
    /// The value has the other key shape's fields (or neither).
    WrongShape(&'static str),
    /// An enumerated value outside the key's vocabulary.
    UnknownValue(String),
    /// `min <= typ <= max` does not hold.
    OutOfOrder,
}

impl std::fmt::Display for ParameterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParameterError::UnknownKey => write!(f, "not a parameter key of the vocabulary"),
            ParameterError::MissingUnit => write!(f, "a numeric parameter needs a unit"),
            ParameterError::IncompatibleUnit { unit, quantity } => {
                let units: Vec<&str> = quantity.units().iter().map(|u| u.0).collect();
                write!(
                    f,
                    "unit \"{unit}\" is not a unit of {quantity:?} (one of: {})",
                    units.join(", ")
                )
            }
            ParameterError::WrongShape(why) => write!(f, "{why}"),
            ParameterError::UnknownValue(v) => write!(f, "\"{v}\" is not one of the key's values"),
            ParameterError::OutOfOrder => write!(f, "min <= typ <= max does not hold"),
        }
    }
}

/// A value normalised to SI.
#[derive(Debug, Clone, PartialEq)]
pub enum Normalized {
    Numeric {
        typ: Option<f64>,
        min: Option<f64>,
        max: Option<f64>,
    },
    /// The vocabulary's spelling.
    Text(&'static str),
}

impl ParameterValue {
    /// The value in SI units (numeric keys) or its vocabulary spelling.
    pub fn normalize(&self, spec: &ParameterSpec) -> Result<Normalized, ParameterError> {
        match spec.shape {
            Numeric(q) => {
                if self.value.is_some() {
                    return Err(ParameterError::WrongShape(
                        "a numeric parameter takes typ/min/max + unit, not value",
                    ));
                }
                if self.typ.is_none() && self.min.is_none() && self.max.is_none() {
                    return Err(ParameterError::WrongShape(
                        "a numeric parameter needs typ, min or max",
                    ));
                }
                let unit = self.unit.as_deref().ok_or(ParameterError::MissingUnit)?;
                let factor =
                    q.unit_factor(unit)
                        .ok_or_else(|| ParameterError::IncompatibleUnit {
                            unit: unit.to_string(),
                            quantity: q,
                        })?;
                let (n, lo, hi) = (self.typ, self.min, self.max);
                let le = |a: Option<f64>, b: Option<f64>| match (a, b) {
                    (Some(a), Some(b)) => a <= b,
                    _ => true,
                };
                if !(le(lo, n) && le(n, hi) && le(lo, hi)) {
                    return Err(ParameterError::OutOfOrder);
                }
                // Twelve significant digits: `10 µF` is 1e-5, not
                // 9.999999999999999e-6.
                let si = |v: Option<f64>| v.map(|v| round_significant(v * factor, 12));
                Ok(Normalized::Numeric {
                    typ: si(n),
                    min: si(lo),
                    max: si(hi),
                })
            }
            Enumerated(values) => {
                if self.typ.is_some()
                    || self.min.is_some()
                    || self.max.is_some()
                    || self.unit.is_some()
                {
                    return Err(ParameterError::WrongShape(
                        "an enumerated parameter takes value, not typ/min/max/unit",
                    ));
                }
                let v = self.value.as_deref().ok_or(ParameterError::WrongShape(
                    "an enumerated parameter needs value",
                ))?;
                values
                    .iter()
                    .find(|x| **x == v)
                    .map(|x| Normalized::Text(x))
                    .ok_or_else(|| ParameterError::UnknownValue(v.to_string()))
            }
        }
    }
}

impl Normalized {
    /// The one number a numeric value is compared, sorted and filtered
    /// by (SI): a deviation key's larger magnitude; otherwise
    /// `typ`, else the middle of `min`/`max`, else its one limit.
    pub fn stated(&self, spec: &ParameterSpec) -> Option<f64> {
        let Normalized::Numeric { typ, min, max } = *self else {
            return None;
        };
        if spec.deviation {
            return match (typ, min, max) {
                (Some(n), None, None) => Some(n.abs()),
                _ => [min, max]
                    .into_iter()
                    .flatten()
                    .map(f64::abs)
                    .reduce(f64::max),
            };
        }
        typ.or(match (min, max) {
            (Some(a), Some(b)) => Some((a + b) / 2.0),
            (a, b) => a.or(b),
        })
    }

    /// The display form: `100 nF`, `±10%`, `X7R`, `1.2 V typ`... Ranges
    /// show as `min–max`; a lone limit as `≥ x` / `≤ x`.
    pub fn label(&self, spec: &ParameterSpec) -> String {
        self.label_digits(spec, 4)
    }

    /// [`Normalized::label`] keeping every significant digit of a value
    /// as written (`32.768 kHz`, not `32.77 kHz`).
    pub fn label_exact(&self, spec: &ParameterSpec) -> String {
        self.label_digits(spec, 9)
    }

    fn label_digits(&self, spec: &ParameterSpec, digits: i32) -> String {
        let (typ, min, max) = match *self {
            Normalized::Text(t) => return t.to_string(),
            Normalized::Numeric { typ, min, max } => (typ, min, max),
        };
        let f = |v: f64| format_value_digits(v, spec, digits);
        if spec.deviation {
            return match (typ, min, max) {
                (_, Some(a), Some(b)) if (a + b).abs() <= 1e-12 * b.abs().max(1e-30) => {
                    format!("±{}", f(b.abs()))
                }
                (_, Some(a), Some(b)) => format!("{} / +{}", f(a), f(b)),
                (Some(n), None, None) => format!("±{}", f(n.abs())),
                (_, Some(a), None) => f(a),
                (_, None, Some(b)) => format!("+{}", f(b)),
                _ => String::new(),
            };
        }
        match (typ, min, max) {
            (Some(n), _, _) => f(n),
            (None, Some(a), Some(b)) => format!("{}–{}", f(a), f(b)),
            (None, Some(a), None) => format!("≥ {}", f(a)),
            (None, None, Some(b)) => format!("≤ {}", f(b)),
            _ => String::new(),
        }
    }
}

/// `v` (SI) in the key's display unit or with an engineering prefix,
/// four significant digits at most: `1e-7` F -> `100 nF`, `0.01` ->
/// `1%`, `4.7e3` Ω -> `4.7 kΩ`, `134217728` bit -> `128 Mbit`.
pub fn format_value(v: f64, spec: &ParameterSpec) -> String {
    format_value_digits(v, spec, 4)
}

fn format_value_digits(v: f64, spec: &ParameterSpec, digits: i32) -> String {
    let Numeric(q) = spec.shape else {
        return trim_number(v, digits);
    };
    if let Some(unit) = spec.display_unit {
        let factor = q.unit_factor(unit).unwrap_or(1.0);
        let n = trim_number(v / factor, digits);
        return if unit == "%" || unit == "°" {
            format!("{n}{unit}")
        } else {
            format!("{n} {unit}")
        };
    }
    let (prefixes, symbol) = q.display();
    let mag = v.abs();
    let mut chosen = prefixes[0];
    for p in prefixes {
        if mag >= p.1 * (1.0 - 1e-9) {
            chosen = *p;
        }
    }
    if mag == 0.0 {
        chosen = *prefixes
            .iter()
            .find(|p| p.0.is_empty())
            .unwrap_or(&prefixes[0]);
    }
    let n = trim_number(v / chosen.1, digits);
    if symbol == "%" || symbol == "°" {
        format!("{n}{symbol}")
    } else {
        format!("{n} {}{symbol}", chosen.0)
    }
}

/// `v` rounded to `digits` significant digits.
pub(crate) fn round_significant(v: f64, digits: i32) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return v;
    }
    let exp = digits - 1 - v.abs().log10().floor() as i32;
    // Scale by an exact power of ten in the direction that keeps it
    // an integer, so the result is the nearest double to the decimal.
    let r = if exp >= 0 {
        let s = 10f64.powi(exp);
        (v * s).round() / s
    } else {
        let s = 10f64.powi(-exp);
        (v / s).round() * s
    };
    format!("{r:e}").parse().unwrap_or(r)
}

/// The shortest decimal that reads back as `v` (`26.5`, `30`, `0.0057`).
pub fn plain_number(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    format!("{v}")
}

/// `sig` significant digits (four for display), trailing zeros dropped
/// (`100`, `4.7`, `0.25`).
fn trim_number(v: f64, sig: i32) -> String {
    if v == 0.0 || !v.is_finite() {
        return "0".into();
    }
    let digits = sig - 1 - v.abs().log10().floor() as i32;
    let s = if digits > 0 {
        format!("{:.*}", digits as usize, v)
    } else {
        let scale = 10f64.powi(-digits);
        format!("{}", (v / scale).round() * scale)
    };
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// Parses a typed filter value for `spec` into SI: a number with an
/// optional prefix and/or unit, in any of the forms people type --
/// `100nF`, `100 nF`, `0.1u`, `10n`, `10k`, `4k7`, `4R7`, `1e-7`, `1%`.
/// A bare number is in the key's display unit (`%` for `tolerance`),
/// else in the SI unit. `None` when it isn't a value of this quantity.
pub fn parse_value(input: &str, spec: &ParameterSpec) -> Option<f64> {
    let Numeric(q) = spec.shape else {
        return None;
    };
    let s: String = input
        .trim()
        .trim_start_matches('±')
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    if s.is_empty() {
        return None;
    }
    // `4k7`, `2n2`, `4R7`: the prefix letter stands for the decimal point.
    let infix = {
        let chars: Vec<char> = s.chars().collect();
        let pos = chars.iter().position(|c| !(c.is_ascii_digit()));
        match pos {
            Some(i)
                if i > 0
                    && i + 1 < chars.len()
                    && chars[i + 1..].iter().all(|c| c.is_ascii_digit())
                    && "pnuµμmkKMGRr".contains(chars[i]) =>
            {
                let head: String = chars[..i].iter().collect();
                let tail: String = chars[i + 1..].iter().collect();
                let prefix = if matches!(chars[i], 'R' | 'r') {
                    String::new()
                } else {
                    chars[i].to_string()
                };
                Some(format!("{head}.{tail}{prefix}"))
            }
            _ => None,
        }
    };
    let s = infix.unwrap_or(s);
    // The longest leading number (with an optional exponent).
    let mut end = 0;
    for i in (1..=s.len()).rev() {
        if s.is_char_boundary(i) && s[..i].parse::<f64>().is_ok() {
            end = i;
            break;
        }
    }
    let number: f64 = s[..end].parse().ok()?;
    let rest = &s[end..];
    if !number.is_finite() {
        return None;
    }
    if rest.is_empty() {
        let unit = spec.display_unit.unwrap_or_else(|| q.si_unit());
        return Some(number * q.unit_factor(unit).unwrap_or(1.0));
    }
    if let Some(f) = q.unit_factor(rest) {
        return Some(number * f);
    }
    // Case-insensitive unit spelling, when that is unambiguous.
    let lower = rest.to_lowercase();
    let matches: Vec<f64> = q
        .units()
        .iter()
        .filter(|(u, _)| u.to_lowercase() == lower)
        .map(|(_, f)| *f)
        .collect();
    if !matches.is_empty() && matches.iter().all(|f| (f - matches[0]).abs() < 1e-30) {
        return Some(number * matches[0]);
    }
    // A prefix alone, or a prefix + the unit symbol in any case.
    let (prefixes, symbol) = q.display();
    let symbols: &[&str] = match q {
        Quantity::Resistance => &["Ω", "Ω", "ohm", "ohms", "r"],
        _ => &[symbol],
    };
    let (prefix, unit_part) = split_prefix(rest);
    let unit_ok = unit_part.is_empty()
        || symbols
            .iter()
            .any(|s| s.to_lowercase() == unit_part.to_lowercase());
    if !unit_ok {
        return None;
    }
    let factor = match prefix {
        "" => 1.0,
        "meg" | "Meg" | "MEG" => 1e6,
        p => {
            let p = match p {
                "u" | "μ" => "µ",
                "K" if q != Quantity::Information => "k",
                "k" if q == Quantity::Information => "K",
                other => other,
            };
            prefixes.iter().find(|x| x.0 == p)?.1
        }
    };
    if prefix.is_empty() && unit_part.is_empty() {
        return None;
    }
    Some(number * factor)
}

/// `rest` as (prefix, unit): `nF` -> (`n`, `F`), `kohm` -> (`k`, `ohm`),
/// `F` -> (``, `F`), `meg` -> (`meg`, ``).
fn split_prefix(rest: &str) -> (&str, &str) {
    for p in ["meg", "Meg", "MEG"] {
        if let Some(r) = rest.strip_prefix(p) {
            return (p, r);
        }
    }
    let mut chars = rest.chars();
    match chars.next() {
        Some(c) if "pnuµμmkKMG".contains(c) => rest.split_at(c.len_utf8()),
        _ => ("", rest),
    }
}

/// The parameters in force for a Part: its Device's family-wide values
/// overlaid with the Part's own (a Part value replaces the Device's for
/// the same key). The flag is `true` for a value from the Device.
pub fn effective_parameters<'a>(
    part: &'a Parameters,
    device: &'a Parameters,
) -> BTreeMap<&'a str, (&'a ParameterValues, bool)> {
    let mut out: BTreeMap<&str, (&ParameterValues, bool)> = device
        .iter()
        .map(|(k, v)| (k.as_str(), (v, true)))
        .collect();
    for (k, v) in part {
        out.insert(k.as_str(), (v, false));
    }
    out
}

/// Whether `key` is plausible for a device of `category` (a listed
/// group covers its subcategories).
pub fn plausible_for(spec: &ParameterSpec, category: ComponentCategory) -> bool {
    spec.categories.iter().any(|c| category.is_within(*c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(key: &str) -> &'static ParameterSpec {
        parameter_spec(key).unwrap()
    }

    fn val(typ: Option<f64>, min: Option<f64>, max: Option<f64>, unit: &str) -> ParameterValue {
        ParameterValue {
            typ,
            min,
            max,
            unit: Some(unit.into()),
            value: None,
            at: Default::default(),
        }
    }

    #[test]
    fn keys_are_unique_and_units_resolve() {
        let mut seen = std::collections::BTreeSet::new();
        for s in PARAMETERS {
            assert!(seen.insert(s.key), "duplicate key {}", s.key);
            if let (Numeric(q), Some(u)) = (s.shape, s.display_unit) {
                assert!(q.unit_factor(u).is_some(), "{}: {u}", s.key);
            }
        }
    }

    #[test]
    fn normalises_to_si() {
        let n = val(Some(100.0), None, None, "nF")
            .normalize(spec("capacitance"))
            .unwrap();
        let v = n.stated(spec("capacitance")).unwrap();
        assert!((v - 1e-7).abs() < 1e-18);
        assert_eq!(n.label(spec("capacitance")), "100 nF");
        let n = val(Some(4.7), None, None, "kΩ")
            .normalize(spec("resistance"))
            .unwrap();
        assert_eq!(n.label(spec("resistance")), "4.7 kΩ");
        let n = val(Some(128.0), None, None, "Mbit")
            .normalize(spec("capacity"))
            .unwrap();
        assert_eq!(n.stated(spec("capacity")), Some(134217728.0));
        assert_eq!(n.label(spec("capacity")), "128 Mbit");
        // Every digit as written, when asked for.
        let n = val(Some(32.768), None, None, "kHz")
            .normalize(spec("frequency"))
            .unwrap();
        assert_eq!(n.label(spec("frequency")), "32.77 kHz");
        assert_eq!(n.label_exact(spec("frequency")), "32.768 kHz");
        let n = val(Some(100.0), None, None, "nF")
            .normalize(spec("capacitance"))
            .unwrap();
        assert_eq!(n.label_exact(spec("capacitance")), "100 nF");
    }

    #[test]
    fn tolerances_are_deviations() {
        let t = spec("tolerance");
        let n = val(None, Some(-10.0), Some(10.0), "%")
            .normalize(t)
            .unwrap();
        assert_eq!(n.label(t), "±10%");
        assert!((n.stated(t).unwrap() - 0.1).abs() < 1e-12);
        let n = val(None, Some(-20.0), Some(80.0), "%")
            .normalize(t)
            .unwrap();
        assert_eq!(n.label(t), "-20% / +80%");
        let tc = spec("temperature_coefficient");
        let n = val(None, Some(-200.0), Some(200.0), "ppm/°C")
            .normalize(tc)
            .unwrap();
        assert_eq!(n.label(tc), "±200 ppm/K");
    }

    #[test]
    fn rejects_bad_values() {
        let c = spec("capacitance");
        assert_eq!(
            val(Some(1.0), None, None, "V").normalize(c),
            Err(ParameterError::IncompatibleUnit {
                unit: "V".into(),
                quantity: Quantity::Capacitance
            })
        );
        assert_eq!(
            val(Some(5.0), Some(6.0), None, "nF").normalize(c),
            Err(ParameterError::OutOfOrder)
        );
        let mut no_unit = val(Some(1.0), None, None, "nF");
        no_unit.unit = None;
        assert_eq!(no_unit.normalize(c), Err(ParameterError::MissingUnit));
        let d = spec("dielectric");
        let x7r = ParameterValue {
            value: Some("X7R".into()),
            ..Default::default()
        };
        assert_eq!(x7r.normalize(d), Ok(Normalized::Text("X7R")));
        let bad = ParameterValue {
            value: Some("X9Z".into()),
            ..Default::default()
        };
        assert!(matches!(
            bad.normalize(d),
            Err(ParameterError::UnknownValue(_))
        ));
        assert!(matches!(
            x7r.normalize(c),
            Err(ParameterError::WrongShape(_))
        ));
    }

    #[test]
    fn formats_with_engineering_prefixes() {
        let f = |v: f64, k: &str| format_value(v, spec(k));
        assert_eq!(f(1e-7, "capacitance"), "100 nF");
        assert_eq!(f(4.7e-6, "capacitance"), "4.7 µF");
        assert_eq!(f(1.5e-11, "capacitance"), "15 pF");
        assert_eq!(f(1.37e3, "resistance"), "1.37 kΩ");
        assert_eq!(f(0.05, "resistance"), "50 mΩ");
        assert_eq!(f(1e6, "resistance"), "1 MΩ");
        assert_eq!(f(12e6, "frequency"), "12 MHz");
        assert_eq!(f(0.05, "power_rating"), "50 mW");
        assert_eq!(f(0.25e-3, "travel"), "0.25 mm");
        assert_eq!(f(20e-6, "frequency_tolerance"), "20 ppm");
        assert_eq!(f(16.0, "rated_voltage"), "16 V");
    }

    #[test]
    fn parses_typed_filter_values() {
        let p = |s: &str, k: &str| parse_value(s, spec(k));
        let close = |a: Option<f64>, b: f64| a.is_some_and(|a| (a - b).abs() <= b.abs() * 1e-9);
        assert!(close(p("10n", "capacitance"), 1e-8));
        assert!(close(p("0.1u", "capacitance"), 1e-7));
        assert!(close(p("100nF", "capacitance"), 1e-7));
        assert!(close(p("100 nf", "capacitance"), 1e-7));
        assert!(close(p("0.1µF", "capacitance"), 1e-7));
        assert!(close(p("1e-7", "capacitance"), 1e-7));
        assert!(close(p("10k", "resistance"), 1e4));
        assert!(close(p("4k7", "resistance"), 4.7e3));
        assert!(close(p("4R7", "resistance"), 4.7));
        assert!(close(p("1M", "resistance"), 1e6));
        assert!(close(p("1m", "resistance"), 1e-3));
        assert!(close(p("10kohm", "resistance"), 1e4));
        assert!(close(p("2.2kΩ", "resistance"), 2.2e3));
        assert!(close(p("1", "tolerance"), 0.01));
        assert!(close(p("1%", "tolerance"), 0.01));
        assert!(close(p("±5%", "tolerance"), 0.05));
        assert!(close(p("12MHz", "frequency"), 12e6));
        assert!(close(p("12m", "frequency"), 12e6) || p("12m", "frequency").is_none());
        assert!(close(p("128M", "capacity"), 134217728.0));
        assert!(close(p("256Kbit", "capacity"), 262144.0));
        assert!(close(p("0.25", "travel"), 0.25e-3));
        assert_eq!(p("10V", "capacitance"), None);
        assert_eq!(p("abc", "capacitance"), None);
        assert_eq!(p("X7R", "dielectric"), None);
    }

    #[test]
    fn zener_and_optocoupler_keys() {
        let vz = spec("zener_voltage");
        assert!(plausible_for(vz, C::Diode));
        assert!(!plausible_for(vz, C::Optocoupler));
        let n = val(Some(5.1), Some(4.8), Some(5.4), "V")
            .normalize(vz)
            .unwrap();
        assert_eq!(n.label(vz), "5.1 V");
        assert_eq!(n.stated(vz), Some(5.1));
        let izt = spec("zener_test_current");
        let n = val(Some(5.0), None, None, "mA").normalize(izt).unwrap();
        assert_eq!(n.label(izt), "5 mA");

        let ctr = spec("current_transfer_ratio");
        assert!(!ctr.deviation);
        assert!(plausible_for(ctr, C::Optocoupler));
        assert!(!plausible_for(ctr, C::Transistor));
        let n = val(None, Some(50.0), Some(600.0), "%")
            .normalize(ctr)
            .unwrap();
        assert_eq!(n.label(ctr), "50%–600%");
        assert!((n.stated(ctr).unwrap() - 3.25).abs() < 1e-12);
        assert!((parse_value("100", ctr).unwrap() - 1.0).abs() < 1e-12);

        let iso = spec("isolation_voltage");
        let n = val(Some(5000.0), None, None, "V").normalize(iso).unwrap();
        assert_eq!(n.label(iso), "5 kV");
        assert!(plausible_for(iso, C::Optocoupler));

        for key in [
            "forward_voltage",
            "forward_current",
            "reverse_voltage",
            "collector_emitter_voltage",
            "collector_current",
        ] {
            assert!(plausible_for(spec(key), C::Optocoupler), "{key}");
        }
        assert!(plausible_for(
            spec("collector_emitter_voltage"),
            C::Transistor
        ));
    }

    #[test]
    fn timing_amplifier_and_logic_keys() {
        let f = |v: f64, k: &str| format_value(v, spec(k));
        let lab = |v: ParameterValue, k: &str| v.normalize(spec(k)).unwrap().label(spec(k));
        assert_eq!(
            lab(val(Some(0.6), None, None, "V/us"), "slew_rate"),
            "0.6 V/µs"
        );
        assert_eq!(f(13e6, "slew_rate"), "13 V/µs");
        assert_eq!(
            lab(val(Some(1.0), None, None, "pA"), "input_bias_current"),
            "1 pA"
        );
        assert_eq!(
            lab(val(Some(3.5), None, None, "ns"), "propagation_delay"),
            "3.5 ns"
        );
        assert_eq!(
            lab(val(Some(120.0), None, None, "°"), "viewing_angle"),
            "120°"
        );
        assert_eq!(
            lab(val(Some(100.0), None, None, "µW"), "drive_level"),
            "100 µW"
        );
        assert_eq!(
            lab(val(Some(940.0), None, None, "nm"), "peak_wavelength"),
            "940 nm"
        );
        let t = spec("operating_temperature");
        let n = val(None, Some(-40.0), Some(125.0), "°C")
            .normalize(t)
            .unwrap();
        assert_eq!(n.label(t), "-40 °C–125 °C");
        let st = spec("frequency_stability");
        let n = val(None, Some(-50.0), Some(50.0), "ppm")
            .normalize(st)
            .unwrap();
        assert_eq!(n.label(st), "±50 ppm");
        assert!(st.in_range(-50e-6) && !st.in_range(0.2));
        assert!(close_to(parse_value("20V/us", spec("slew_rate")), 20e6));
        assert!(close_to(parse_value("0.5", spec("slew_rate")), 0.5e6));
        assert!(close_to(
            parse_value("10p", spec("input_bias_current")),
            1e-11
        ));
        // Generalised keys: no longer LED-only.
        for (key, c) in [
            ("supply_voltage", C::OpAmp),
            ("supply_voltage", C::Oscillator),
            ("supply_voltage", C::Led),
            ("data_rate", C::CanTransceiver),
            ("data_rate", C::Led),
            ("frequency", C::Oscillator),
            ("propagation_delay", C::Comparator),
            ("propagation_delay", C::Logic),
            ("operating_temperature", C::Capacitor),
        ] {
            assert!(plausible_for(spec(key), c), "{key} for {c:?}");
        }
        assert!(!plausible_for(spec("supply_voltage"), C::LinearRegulator));
        assert!(!plausible_for(spec("slew_rate"), C::Logic));
    }

    fn close_to(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() <= b.abs() * 1e-9)
    }

    #[test]
    fn part_values_override_device_values() {
        let mut device = Parameters::new();
        device.insert(
            "capacity".into(),
            val(Some(128.0), None, None, "Mbit").into(),
        );
        device.insert(
            "rated_voltage".into(),
            val(Some(3.6), None, None, "V").into(),
        );
        let mut part = Parameters::new();
        part.insert(
            "rated_voltage".into(),
            val(Some(1.95), None, None, "V").into(),
        );
        let eff = effective_parameters(&part, &device);
        assert!(eff["capacity"].1);
        assert_eq!(eff["rated_voltage"].0.primary().typ, Some(1.95));
        assert!(!eff["rated_voltage"].1);
    }
}
