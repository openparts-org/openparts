//! Canonical Data Specification section 41: a reference circuit (`kind:
//! circuit`), the peripheral circuit a manufacturer's document or a
//! standard gives for a device, as facts: roles, values, connections,
//! constraints, conditions and how strongly the source asks for each.
//!
//! ```yaml
//! roles:
//!   C_VREG_VIN:
//!     per_pin: VREG_VIN                     # one per host pin of that name
//!     requires: { category: capacitor, capacitance: { typ: 1 uF }, src: p157 "2.10.1" }
//!     pins: { "1": $pin, "2": GND }
//!     level: { value: required, word: must, src: p157 "2.10.1" }
//! ```
//!
//! In memory (and in the API) quantities are `min`/`typ`/`max` + `unit`
//! ([`ParameterValue`]) and evidence is the `provenance` map, like every
//! other document.

use crate::category::ComponentCategory;
use crate::ids::{DeviceId, PartId};
use crate::kind::Kind;
use crate::parameter::ParameterValue;
use crate::provenance::ProvenanceMap;
use crate::serde_util::deserialize_no_dup_map;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

/// A reference circuit document.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Circuit {
    pub schema_version: String,
    pub kind: Kind,
    /// `<publisher>/<name>`, e.g. `raspberrypi/rp2040-power`.
    pub id: String,
    pub title: String,
    /// The device the circuit serves.
    pub host: CircuitHost,
    /// Where the circuit is valid (a host pin or net name -> value),
    /// e.g. `IOVDD: { typ: 3.3 V }`; outside them it needs re-tuning.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "deserialize_no_dup_map"
    )]
    pub conditions: BTreeMap<String, ParameterValue>,
    /// The circuit's nets: interface nets the board binds (`power`,
    /// `ground`, `signal`) and `local` ones of each instance.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "deserialize_no_dup_map"
    )]
    pub nets: BTreeMap<String, CircuitNet>,
    /// Host pin name -> net (every host pin of that name).
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "deserialize_no_dup_map"
    )]
    pub connect: BTreeMap<String, Connect>,
    /// The components, by role name.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "deserialize_no_dup_map"
    )]
    pub roles: BTreeMap<String, Role>,
    /// Values the source computes from the roles' values.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "deserialize_no_dup_map"
    )]
    pub formulas: BTreeMap<String, Formula>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placement: Vec<PlacementHint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routing: Vec<RoutingHint>,
    /// Other circuits this one is made of (a composite).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub includes: Vec<Include>,
    #[serde(default, deserialize_with = "deserialize_no_dup_map")]
    pub provenance: ProvenanceMap,
    /// Human review status (section 26.1); `None`: unreviewed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<crate::review::DocumentReview>,
}

/// What a circuit attaches to: one device, or any device that matches.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CircuitHost {
    /// `host: raspberrypi/RP2040`
    Device(DeviceId),
    /// `host: { devices: [...] }` or `{ category: connector, has_pins: [CC1, CC2] }`
    Match(HostMatch),
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostMatch {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<DeviceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<ComponentCategory>,
    /// Pin names the device must have (a generic circuit, e.g. any
    /// USB Type-C receptacle).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub has_pins: Vec<String>,
}

impl CircuitHost {
    /// Whether a device with `id`, `category` and pin names `pins` is a
    /// host of this circuit.
    pub fn matches(
        &self,
        id: &str,
        category: ComponentCategory,
        mut pins: impl FnMut(&str) -> bool,
    ) -> bool {
        match self {
            CircuitHost::Device(d) => d.0 == id,
            CircuitHost::Match(m) => {
                if !m.devices.is_empty() && !m.devices.iter().any(|d| d.0 == id) {
                    return false;
                }
                if m.category.is_some_and(|c| !category.is_within(c)) {
                    return false;
                }
                let generic = m.devices.is_empty();
                (!generic || m.category.is_some() || !m.has_pins.is_empty())
                    && m.has_pins.iter().all(|p| pins(p))
            }
        }
    }

    /// The devices named explicitly.
    pub fn devices(&self) -> Vec<&DeviceId> {
        match self {
            CircuitHost::Device(d) => vec![d],
            CircuitHost::Match(m) => m.devices.iter().collect(),
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetKind {
    Power,
    Ground,
    Signal,
    /// Internal to one instance (prefixed or renamed on expansion).
    Local,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CircuitNet {
    pub kind: NetKind,
    /// The net's working voltage as the source states it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voltage: Option<ParameterValue>,
}

/// How strongly the source asks for a fact (section 41.4).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelValue {
    /// must, shall, required, mandatory, "tie to".
    Required,
    /// should, recommend.
    Recommended,
    /// The reference design has it; no stated rule.
    Example,
    /// may, DNF.
    Optional,
}

impl LevelValue {
    pub fn as_str(self) -> &'static str {
        match self {
            LevelValue::Required => "required",
            LevelValue::Recommended => "recommended",
            LevelValue::Example => "example",
            LevelValue::Optional => "optional",
        }
    }

    /// The modal words a source uses for this level (lower case).
    pub fn words(self) -> &'static [&'static str] {
        match self {
            LevelValue::Required => &[
                "must",
                "shall",
                "required",
                "require",
                "requires",
                "mandatory",
                "tie",
                "important",
                "need",
                "needs",
            ],
            LevelValue::Recommended => &["should", "recommend", "recommended"],
            LevelValue::Example => &[],
            LevelValue::Optional => &["may", "dnf", "optional", "can"],
        }
    }
}

/// A level and the source's own modal word for it. Written `required`,
/// or `{ value: required, word: must }`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    pub value: LevelValue,
    pub word: Option<String>,
}

impl Serialize for Level {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Full<'a> {
            value: LevelValue,
            word: &'a str,
        }
        match &self.word {
            None => self.value.serialize(s),
            Some(w) => Full {
                value: self.value,
                word: w,
            }
            .serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Full {
            value: LevelValue,
            #[serde(default)]
            word: Option<String>,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either {
            Bare(LevelValue),
            Full(Full),
        }
        Ok(match Either::deserialize(d)? {
            Either::Bare(value) => Level { value, word: None },
            Either::Full(f) => Level {
                value: f.value,
                word: f.word,
            },
        })
    }
}

/// A host pin's connection.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connect {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<String>,
    /// Left unconnected.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_connect: bool,
    /// Absent: `required` (a connection the source states).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<Level>,
}

impl Connect {
    pub fn level(&self) -> LevelValue {
        self.level
            .as_ref()
            .map_or(LevelValue::Required, |l| l.value)
    }
}

/// One category, or several the role may take.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Categories {
    One(ComponentCategory),
    Many(Vec<ComponentCategory>),
}

impl Categories {
    pub fn list(&self) -> Vec<ComponentCategory> {
        match self {
            Categories::One(c) => vec![*c],
            Categories::Many(v) => v.clone(),
        }
    }
}

/// What the source states the component must be: a category and
/// catalog parameter values (section 11.1 keys and units; `typ` is the
/// value, `min`/`max` a bound), or a named part.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<Categories>,
    /// A part the source names (`manufacturer/MPN`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<PartId>,
    #[serde(flatten)]
    pub parameters: BTreeMap<String, ParameterValue>,
}

/// What the reference design used without stating a rule. It guides
/// part selection; it never filters.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Example {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<PartId>,
    /// EIA chip size code (`0402`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(flatten)]
    pub parameters: BTreeMap<String, ParameterValue>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Place {
    /// `$pin`, `host.<PIN>` or another role.
    pub near: String,
}

/// One component of the circuit.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    /// One instance per host pin with this name (pin-number order);
    /// `$pin` in `pins` is that pin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_pin: Option<String>,
    /// Instances (default 1); not with `per_pin`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    pub requires: Requirement,
    /// Role pin (the part's pin name or number) -> `$pin`,
    /// `host.<PIN>` or a circuit net. Its evidence is `requires`'.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "deserialize_no_dup_map"
    )]
    pub pins: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<Place>,
    pub level: Level,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<Example>,
    /// Facts about the chosen part the circuit was tuned to (a
    /// crystal's load capacitance); checked against the selection.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "deserialize_no_dup_map"
    )]
    pub expects: BTreeMap<String, ParameterValue>,
}

impl Role {
    /// Instances of this role on a host with these pin numbers per name.
    pub fn instances(&self, pins_named: impl Fn(&str) -> usize) -> usize {
        match &self.per_pin {
            Some(name) => pins_named(name),
            None => self.count.unwrap_or(1) as usize,
        }
    }
}

/// One input of a formula.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FormulaInput {
    /// A role's parameter (its `requires` value, or the selected part's).
    Role(RoleParam),
    /// A value the source assumes (cite it).
    Literal(ParameterValue),
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleParam {
    pub role: String,
    pub param: String,
}

/// A value the source computes: `expr` over `inputs` in SI units.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Formula {
    /// `+ - * /`, parentheses, `sqrt(...)`, numbers and input names.
    pub expr: String,
    #[serde(default, deserialize_with = "deserialize_no_dup_map")]
    pub inputs: BTreeMap<String, FormulaInput>,
    /// What the result should be close to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<RoleParam>,
    /// The source's own worked result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ParameterValue>,
    /// The result's unit (the target's or the stated result's otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementHint {
    /// Keep these nets' or pins' copper short.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep_short: Vec<String>,
    /// Stubs of these host pins at most `max_length`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub short_stub: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<ParameterValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<Level>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingHint {
    /// Two circuit nets or `host.<PIN>`s routed as a pair.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diff_pair: Vec<String>,
    /// Differential impedance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impedance: Option<ParameterValue>,
    /// A net the pair needs unbroken beneath it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_plane: Option<String>,
    /// The source's example geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<RoutingExample>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingExample {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<ParameterValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<ParameterValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board_thickness: Option<ParameterValue>,
}

/// Another circuit as part of this one.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Include {
    pub circuit: String,
    /// Its host is one of this circuit's roles (`{ role: U_LDO }`);
    /// absent: this circuit's host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<IncludeHost>,
    /// Its nets -> this circuit's nets.
    #[serde(
        default,
        rename = "with",
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "deserialize_no_dup_map"
    )]
    pub nets: BTreeMap<String, String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncludeHost {
    pub role: String,
}

/// A role pin target, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target<'a> {
    /// `$pin`: the per-pin instance's host pin.
    ThisPin,
    /// `host.<PIN>`.
    HostPin(&'a str),
    /// A circuit net.
    Net(&'a str),
}

pub fn target(s: &str) -> Target<'_> {
    if s == "$pin" {
        Target::ThisPin
    } else if let Some(p) = s.strip_prefix("host.") {
        Target::HostPin(p)
    } else {
        Target::Net(s)
    }
}

// ------------------------------------------------------------- formulas

/// A parsed formula expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
    Sqrt(Box<Expr>),
}

impl Expr {
    /// `+ - * /`, parentheses, unary minus, `sqrt(x)`, numbers and names
    /// (`[A-Za-z_][A-Za-z0-9_]*`).
    pub fn parse(s: &str) -> Result<Expr, String> {
        let tokens = tokenize(s)?;
        let mut p = Parser { t: &tokens, i: 0 };
        let e = p.sum()?;
        if p.i != tokens.len() {
            return Err(format!("unexpected {:?} in {s:?}", tokens[p.i]));
        }
        Ok(e)
    }

    /// The names it uses.
    pub fn vars(&self) -> Vec<&str> {
        let mut out = Vec::new();
        fn walk<'a>(e: &'a Expr, out: &mut Vec<&'a str>) {
            match e {
                Expr::Num(_) => {}
                Expr::Var(v) => {
                    if !out.contains(&v.as_str()) {
                        out.push(v)
                    }
                }
                Expr::Neg(a) | Expr::Sqrt(a) => walk(a, out),
                Expr::Bin(_, a, b) => {
                    walk(a, out);
                    walk(b, out)
                }
            }
        }
        walk(self, &mut out);
        out
    }

    pub fn eval(&self, var: &dyn Fn(&str) -> Option<f64>) -> Option<f64> {
        Some(match self {
            Expr::Num(n) => *n,
            Expr::Var(v) => var(v)?,
            Expr::Neg(a) => -a.eval(var)?,
            Expr::Sqrt(a) => a.eval(var)?.sqrt(),
            Expr::Bin(op, a, b) => {
                let (a, b) = (a.eval(var)?, b.eval(var)?);
                match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    _ => a / b,
                }
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Name(String),
    Op(char),
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if "+-*/()".contains(ch) {
            out.push(Tok::Op(ch));
            i += 1;
        } else if ch.is_ascii_digit() || ch == '.' {
            let start = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.' || c[i] == 'e') {
                i += 1;
            }
            let t: String = c[start..i].iter().collect();
            out.push(Tok::Num(
                t.parse().map_err(|_| format!("bad number {t:?}"))?,
            ));
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            out.push(Tok::Name(c[start..i].iter().collect()));
        } else {
            return Err(format!("unexpected {ch:?} in {s:?}"));
        }
    }
    Ok(out)
}

struct Parser<'a> {
    t: &'a [Tok],
    i: usize,
}

impl Parser<'_> {
    fn op(&mut self, ch: char) -> bool {
        if self.t.get(self.i) == Some(&Tok::Op(ch)) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn sum(&mut self) -> Result<Expr, String> {
        let mut e = self.product()?;
        loop {
            let op = if self.op('+') {
                '+'
            } else if self.op('-') {
                '-'
            } else {
                return Ok(e);
            };
            e = Expr::Bin(op, Box::new(e), Box::new(self.product()?));
        }
    }

    fn product(&mut self) -> Result<Expr, String> {
        let mut e = self.unary()?;
        loop {
            let op = if self.op('*') {
                '*'
            } else if self.op('/') {
                '/'
            } else {
                return Ok(e);
            };
            e = Expr::Bin(op, Box::new(e), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.op('-') {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        match self.t.get(self.i).cloned() {
            Some(Tok::Num(n)) => {
                self.i += 1;
                Ok(Expr::Num(n))
            }
            Some(Tok::Name(n)) => {
                self.i += 1;
                if n == "sqrt" {
                    if !self.op('(') {
                        return Err("sqrt needs (".into());
                    }
                    let e = self.sum()?;
                    if !self.op(')') {
                        return Err("missing )".into());
                    }
                    return Ok(Expr::Sqrt(Box::new(e)));
                }
                Ok(Expr::Var(n))
            }
            Some(Tok::Op('(')) => {
                self.i += 1;
                let e = self.sum()?;
                if !self.op(')') {
                    return Err("missing )".into());
                }
                Ok(e)
            }
            other => Err(format!("expected a value, found {other:?}")),
        }
    }
}

/// `v` in SI from a stated value (its `typ`, else the middle of min/max,
/// else its one bound), via the quantity of parameter `key` or, for a
/// literal, any quantity that knows the unit.
pub fn si_value(v: &ParameterValue, key: Option<&str>) -> Option<f64> {
    let n = v.typ.or(match (v.min, v.max) {
        (Some(a), Some(b)) => Some((a + b) / 2.0),
        (a, b) => a.or(b),
    })?;
    let unit = v.unit.as_deref()?;
    let factor = match key.and_then(crate::parameter_spec) {
        Some(spec) => match spec.shape {
            crate::ParameterShape::Numeric(q) => q.unit_factor(unit)?,
            _ => return None,
        },
        None => crate::parameter::Quantity::ALL
            .iter()
            .find_map(|q| q.unit_factor(unit))?,
    };
    Some(n * factor)
}

impl Circuit {
    /// The formula's value from the roles' required values and the
    /// literal inputs (SI), when every input has one.
    pub fn evaluate(&self, formula: &Formula) -> Option<f64> {
        let expr = Expr::parse(&formula.expr).ok()?;
        expr.eval(&|name| match formula.inputs.get(name)? {
            FormulaInput::Literal(v) => si_value(v, None),
            FormulaInput::Role(rp) => {
                let role = self.roles.get(&rp.role)?;
                let v = role.requires.parameters.get(&rp.param)?;
                si_value(v, Some(&rp.param))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions_parse_and_evaluate() {
        let e = Expr::parse("C1 * C2 / (C1 + C2) + Cs").unwrap();
        assert_eq!(e.vars(), ["C1", "C2", "Cs"]);
        let v = e
            .eval(&|n| match n {
                "C1" | "C2" => Some(15e-12),
                "Cs" => Some(3e-12),
                _ => None,
            })
            .unwrap();
        assert!((v - 10.5e-12).abs() < 1e-18);
        assert_eq!(
            Expr::parse("-sqrt(4) * 2").unwrap().eval(&|_| None),
            Some(-4.0)
        );
        assert!(Expr::parse("a +").is_err());
        assert!(Expr::parse("a $ b").is_err());
    }

    #[test]
    fn level_reads_bare_and_full() {
        let l: Level = serde_yaml::from_str("required").unwrap();
        assert_eq!(l.value, LevelValue::Required);
        assert_eq!(l.word, None);
        let l: Level = serde_yaml::from_str("{ value: recommended, word: should }").unwrap();
        assert_eq!(l.word.as_deref(), Some("should"));
        assert_eq!(
            serde_yaml::to_string(&l).unwrap(),
            "value: recommended\nword: should\n"
        );
        assert!(serde_yaml::from_str::<Level>("sometimes").is_err());
    }

    #[test]
    fn hosts_match_by_id_list_or_pins() {
        let one: CircuitHost = serde_yaml::from_str("raspberrypi/RP2040").unwrap();
        assert!(one.matches(
            "raspberrypi/RP2040",
            ComponentCategory::Microcontroller,
            |_| true
        ));
        assert!(!one.matches("x/Y", ComponentCategory::Microcontroller, |_| true));
        let generic: CircuitHost =
            serde_yaml::from_str("{ category: connector, has_pins: [CC1, CC2] }").unwrap();
        assert!(
            generic.matches("gct/USB4105", ComponentCategory::Connector, |p| p
                .starts_with("CC"))
        );
        assert!(!generic.matches("gct/USB4105", ComponentCategory::Connector, |p| p == "CC1"));
        assert!(!generic.matches("x/Y", ComponentCategory::Resistor, |_| true));
    }

    #[test]
    fn a_role_takes_parameters_beside_category_and_part() {
        let r: Role = serde_yaml::from_str(
            "requires: { category: capacitor, capacitance: { typ: 100, unit: nF } }\n\
             pins: { '1': $pin, '2': GND }\nlevel: recommended\n",
        )
        .unwrap();
        assert_eq!(
            r.requires.category,
            Some(Categories::One(ComponentCategory::Capacitor))
        );
        assert_eq!(r.requires.parameters["capacitance"].typ, Some(100.0));
        assert_eq!(target(&r.pins["1"]), Target::ThisPin);
        assert_eq!(target("host.XIN"), Target::HostPin("XIN"));
        assert!(serde_yaml::from_str::<Role>("requires: {}\nlevel: example\nbogus: 1\n").is_err());
    }
}
