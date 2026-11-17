//! Faceted (parametric) part search for `GET /v1/catalog`, the way a
//! component catalog narrows parts: free text (through the
//! [`SearchIndex`]) plus multi-select filters on category, manufacturer,
//! package family, chip size code, pin count, pitch, mounting,
//! lifecycle and existence, the package's structure (pin rows x pins
//! per row, connector gender and orientation, shroud, exposed pad:
//! `Package::structure`), and min-max ranges on the body's width,
//! length and height.
//!
//! Electrical parameters (Canonical Data Specification section 11.1)
//! add one facet per vocabulary key present in the results: numeric
//! keys list their distinct values (engineering-formatted, `100 nF`)
//! and take `<key>_min`/`<key>_max` ranges typed with prefixes (`10n`,
//! `0.1u`, `100nF`); enumerated keys are checkboxes.
//!
//! Every filterable fact is read from the Part, its Device and its
//! Package as the dataset states them -- nothing is parsed out of an
//! MPN. A dimension counts as stated when it has a nominal value, else
//! the middle of its min/max, else its one limit (the generators' rule).
//!
//! Facet counts follow the usual rule: the counts of one facet apply
//! every *other* active filter, so picking an option doesn't hide its
//! siblings. Range facets report the min/max over the parts that pass
//! every other filter, likewise.

use crate::search::{chip_codes, CHIP_SIZES};
use openparts_core::{
    conditions_label, effective_parameters, parameter_spec, parse_value, CardMechanism,
    ComponentCategory, ConnectorInterface, ContactSide, Dimension, ExistenceStatus, Gender,
    LifecycleStatus, Normalized, Orientation, Package, PackageFamily, ParameterShape, PARAMETERS,
};
use openparts_data::Dataset;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// How a package is soldered, from its generated leads (and lands).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mounting {
    /// Surface-mount terminals only.
    Smd,
    /// Plated through-hole leads only.
    Tht,
    /// Both (e.g. a USB-C receptacle with through-hole shell legs).
    Mixed,
}

impl Mounting {
    fn wire(self) -> &'static str {
        match self {
            Mounting::Smd => "smd",
            Mounting::Tht => "tht",
            Mounting::Mixed => "mixed",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Mounting::Smd => "SMD",
            Mounting::Tht => "THT",
            Mounting::Mixed => "SMD + THT",
        }
    }
}

/// One row of the catalog: the Part with the Device and Package facts
/// a parametric table shows. Dimensions are in mm; `None` where the
/// dataset doesn't state the value.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct CatalogItem {
    pub manufacturer: String,
    /// The manufacturer's display name (its id when it has no document).
    pub manufacturer_name: String,
    pub mpn: String,
    pub category: Option<ComponentCategory>,
    pub device: String,
    pub package: String,
    pub package_family: Option<PackageFamily>,
    pub lead_count: Option<u32>,
    pub pitch: Option<f64>,
    /// Body size along X (mm).
    pub body_width: Option<f64>,
    /// Body size along Y (mm).
    pub body_length: Option<f64>,
    pub body_height: Option<f64>,
    /// EIA (imperial) chip size code, e.g. `0603`, for `chip` packages
    /// of a standard size.
    pub size_code: Option<String>,
    /// The matching metric code, e.g. `1608`.
    pub size_code_metric: Option<String>,
    pub mounting: Option<Mounting>,
    /// Pin rows x pins per row (`1x20`): headers, sockets, DIP, SIP.
    pub rows: Option<String>,
    /// Connectors: male (header, plug) or female (socket, receptacle).
    pub gender: Option<Gender>,
    /// Connectors: vertical or right angle.
    pub orientation: Option<Orientation>,
    /// Pin headers and sockets: a shroud (box header) or not.
    pub shrouded: Option<bool>,
    /// IC packages: an exposed (thermal) pad or not.
    pub exposed_pad: Option<bool>,
    /// Card sockets: push-push, push-pull, hinged.
    pub mechanism: Option<CardMechanism>,
    /// FPC connectors: top, bottom or dual contact.
    pub contact_side: Option<ContactSide>,
    /// Card sockets and RF connectors: the mating interface.
    pub interface: Option<ConnectorInterface>,
    pub lifecycle: LifecycleStatus,
    pub existence: ExistenceStatus,
    /// Which of the part, its device and its package are reviewed.
    pub review: crate::documents::ReviewSummary,
    /// The Part's electrical parameters (its Device's family-wide ones
    /// included), by vocabulary key.
    pub parameters: BTreeMap<String, CatalogParameter>,
}

/// One parameter of a catalog row.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CatalogParameter {
    /// The value in SI units (a tolerance's larger magnitude); `None`
    /// for an enumerated key.
    pub value: Option<f64>,
    /// Display form of the primary value with its conditions: `100 nF`,
    /// `±10%`, `X7R`, `18 mΩ @ VGS 10 V`.
    pub label: String,
}

/// Whether a parameter facet lists numbers or words.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParameterKind {
    /// Discrete values plus a `<key>_min`/`<key>_max` range.
    Numeric,
    /// Checkboxes only.
    Enumerated,
}

/// One electrical-parameter facet: present only when some part under
/// the other filters states the key (or the key is filtered on).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ParameterFacet {
    /// The vocabulary key, also the filter parameter (`capacitance`).
    pub key: String,
    pub kind: ParameterKind,
    /// The SI unit `min`/`max` and item values are in (`F`, `Ω`).
    pub unit: String,
    /// Distinct values (numeric: smallest first; enumerated: in
    /// vocabulary order). `value` is what the filter parameter takes.
    pub values: Vec<FacetValue>,
    /// Span of the values under every other filter (SI), numeric keys.
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// `min`/`max` formatted (`10 pF`).
    pub min_label: Option<String>,
    pub max_label: Option<String>,
}

/// One option of a facet. `value` is what the filter parameter takes
/// (stable, lowercase); `label` is an English display form, which
/// clients may localise by `value`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FacetValue {
    pub value: String,
    pub label: String,
    /// Parts matching this option under every other active filter.
    pub count: usize,
    pub selected: bool,
}

/// One node of the category tree facet.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CategoryNode {
    /// The category value (`power`, `linear_regulator`).
    pub value: String,
    /// English display name; clients may localise by `value`.
    pub label: String,
    /// Parts in this node or its subcategories under every other filter.
    pub count: usize,
    pub selected: bool,
    /// Subcategories with parts (or picked), in catalog order.
    pub children: Vec<CategoryNode>,
}

/// The span of a dimension over the parts that pass every other
/// filter (`None` when none of them states it).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct RangeFacet {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct CatalogFacets {
    /// Each part counted under its own category (the most specific
    /// node its Device states).
    pub category: Vec<FacetValue>,
    /// The category tree: top-level nodes in catalog order, each
    /// counting its whole subtree, with its subcategories. A picked
    /// group (`category=power`) selects its subcategories too.
    pub category_tree: Vec<CategoryNode>,
    pub manufacturer: Vec<FacetValue>,
    pub package_family: Vec<FacetValue>,
    /// Chip size codes, smallest first; `value` is the imperial code.
    pub size: Vec<FacetValue>,
    pub pins: Vec<FacetValue>,
    pub pitch: Vec<FacetValue>,
    pub mounting: Vec<FacetValue>,
    pub lifecycle: Vec<FacetValue>,
    pub existence: Vec<FacetValue>,
    /// `reviewed` (part, device and package), `partly`, `unreviewed`.
    pub review: Vec<FacetValue>,
    /// Pin rows x pins per row (`1x20`), fewest rows then pins first.
    pub rows: Vec<FacetValue>,
    /// `male`, `female`.
    pub gender: Vec<FacetValue>,
    /// `vertical`, `right_angle`.
    pub orientation: Vec<FacetValue>,
    /// `yes`, `no`.
    pub shrouded: Vec<FacetValue>,
    /// `yes`, `no`.
    pub exposed_pad: Vec<FacetValue>,
    /// `push_push`, `push_pull`, `hinged`.
    pub mechanism: Vec<FacetValue>,
    /// `top`, `bottom`, `dual`.
    pub contact_side: Vec<FacetValue>,
    /// `micro_sd`, `sd`, `sma`, `mcx`, ...
    pub interface: Vec<FacetValue>,
    pub body_width: RangeFacet,
    pub body_length: RangeFacet,
    pub body_height: RangeFacet,
    /// Electrical-parameter facets, in vocabulary order.
    pub parameters: Vec<ParameterFacet>,
}

/// `GET /v1/catalog`'s answer: `total` matching parts, the
/// `offset..offset+limit` page of them, and the facets.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct CatalogResponse {
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub items: Vec<CatalogItem>,
    pub facets: CatalogFacets,
}

/// The multi-select facets, in response order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Category,
    Manufacturer,
    Family,
    Size,
    Pins,
    Pitch,
    Mounting,
    Lifecycle,
    Existence,
    /// Whether the part, its device and its package are reviewed
    /// (`crate::documents::ReviewState`).
    Review,
    /// The package's structure (`Package::structure`).
    Rows,
    Gender,
    Orientation,
    Shrouded,
    ExposedPad,
    /// Connector facts (`Package::connector`).
    Mechanism,
    ContactSide,
    Interface,
}

const GROUP_COUNT: usize = 18;

const GROUPS: [Group; GROUP_COUNT] = [
    Group::Category,
    Group::Manufacturer,
    Group::Family,
    Group::Size,
    Group::Pins,
    Group::Pitch,
    Group::Mounting,
    Group::Lifecycle,
    Group::Existence,
    Group::Review,
    Group::Rows,
    Group::Gender,
    Group::Orientation,
    Group::Shrouded,
    Group::ExposedPad,
    Group::Mechanism,
    Group::ContactSide,
    Group::Interface,
];

impl Group {
    /// The query parameter (and accepted aliases).
    fn params(self) -> &'static [&'static str] {
        match self {
            Group::Category => &["category"],
            Group::Manufacturer => &["manufacturer"],
            Group::Family => &["family", "package_family"],
            Group::Size => &["size"],
            Group::Pins => &["pins", "lead_count"],
            Group::Pitch => &["pitch"],
            Group::Mounting => &["mounting"],
            Group::Lifecycle => &["lifecycle"],
            Group::Existence => &["existence"],
            Group::Review => &["review"],
            Group::Rows => &["rows", "layout"],
            Group::Gender => &["gender"],
            Group::Orientation => &["orientation"],
            Group::Shrouded => &["shrouded"],
            Group::ExposedPad => &["exposed_pad"],
            Group::Mechanism => &["mechanism"],
            Group::ContactSide => &["contact_side", "contacts"],
            Group::Interface => &["interface"],
        }
    }

    /// Options listed by count (most first) rather than by `order`.
    fn by_count(self) -> bool {
        matches!(self, Group::Category | Group::Manufacturer | Group::Family)
    }

    /// A filter value as the facet keys spell it, or `None` if it can't
    /// be one (then it matches nothing).
    fn normalize(self, v: &str) -> Option<String> {
        let v = v.trim();
        if v.is_empty() {
            return None;
        }
        let lower = v.to_ascii_lowercase();
        Some(match self {
            Group::Family => PackageFamily::parse(v).map_or(lower, |f| f.name().to_string()),
            Group::Size => size_imperial(&lower).unwrap_or(lower),
            Group::Pins => v.parse::<u32>().ok()?.to_string(),
            Group::Pitch => fmt_mm(lower.trim_end_matches("mm").trim().parse::<f64>().ok()?),
            Group::Mounting if lower == "through_hole" => "tht".into(),
            Group::Rows => rows_key(&lower)?,
            Group::Gender => match lower.as_str() {
                "male" | "plug" | "header" | "pin" | "オス" => "male".into(),
                "female" | "socket" | "receptacle" | "メス" => "female".into(),
                _ => lower,
            },
            Group::Orientation => match lower.replace(['-', ' '], "_").as_str() {
                "vertical" | "straight" => "vertical".into(),
                "right_angle" | "rightangle" | "horizontal" | "angled" => "right_angle".into(),
                _ => lower,
            },
            Group::Mechanism | Group::ContactSide | Group::Interface => {
                lower.replace(['-', ' ', '.'], "_")
            }
            Group::Shrouded | Group::ExposedPad => match lower.as_str() {
                "yes" | "true" | "1" => "yes".into(),
                "no" | "false" | "0" => "no".into(),
                _ => lower,
            },
            _ => lower,
        })
    }
}

/// The body dimensions with range filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Width,
    Length,
    Height,
}

const AXES: [Axis; 3] = [Axis::Width, Axis::Length, Axis::Height];

impl Axis {
    fn param(self) -> &'static str {
        match self {
            Axis::Width => "width",
            Axis::Length => "length",
            Axis::Height => "height",
        }
    }
}

/// Result order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    /// Best text match first (manufacturer, MPN without a query).
    #[default]
    Relevance,
    Mpn,
    Manufacturer,
    Pins,
    Pitch,
    /// Body footprint: the longer side, then the shorter.
    Size,
    Height,
    /// A numeric parameter's value (`sort=capacitance`).
    Parameter(&'static str),
}

impl Sort {
    fn parse(s: &str) -> Option<Sort> {
        if let Some(spec) = parameter_spec(s) {
            if matches!(spec.shape, ParameterShape::Numeric(_)) {
                return Some(Sort::Parameter(spec.key));
            }
        }
        Some(match s {
            "" | "relevance" => Sort::Relevance,
            "mpn" => Sort::Mpn,
            "manufacturer" => Sort::Manufacturer,
            "pins" | "lead_count" => Sort::Pins,
            "pitch" => Sort::Pitch,
            "size" => Sort::Size,
            "height" => Sort::Height,
            _ => return None,
        })
    }
}

pub const DEFAULT_LIMIT: usize = 50;
pub const MAX_LIMIT: usize = 500;

/// A parsed `/v1/catalog` query.
#[derive(Debug, Clone, Default)]
pub struct CatalogQuery {
    pub q: String,
    /// Selected values per [`GROUPS`] entry (normalised).
    filters: [BTreeSet<String>; GROUP_COUNT],
    /// (min, max) per [`AXES`] entry, mm.
    ranges: [(Option<f64>, Option<f64>); 3],
    /// Parameter filters by vocabulary key.
    params: BTreeMap<&'static str, ParamFilter>,
    pub sort: Sort,
    pub descending: bool,
    pub offset: usize,
    pub limit: usize,
}

impl CatalogQuery {
    /// Parses `key=value` pairs (URL-decoded). Multi-select filters
    /// take comma-separated values, repeated parameters, or both.
    pub fn from_pairs<'a>(
        pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<CatalogQuery, String> {
        let mut out = CatalogQuery {
            limit: DEFAULT_LIMIT,
            ..CatalogQuery::default()
        };
        for (key, value) in pairs {
            if key == "q" {
                if !out.q.is_empty() {
                    out.q.push(' ');
                }
                out.q.push_str(value);
                continue;
            }
            if let Some(spec) = parameter_spec(key) {
                let f = out.params.entry(spec.key).or_default();
                for v in value.split(',').map(str::trim).filter(|v| !v.is_empty()) {
                    match spec.shape {
                        ParameterShape::Numeric(_) => {
                            f.values.insert(v.replace(' ', ""));
                            if let Some(n) = parse_value(v, spec) {
                                f.numbers.push(n);
                            }
                        }
                        ParameterShape::Enumerated(words) => {
                            let w = words.iter().find(|w| w.eq_ignore_ascii_case(v));
                            f.values
                                .insert(w.map_or_else(|| format!("\u{0}{v}"), |w| w.to_string()));
                        }
                    }
                }
                continue;
            }
            if let Some((name, bound)) = key.rsplit_once('_') {
                if let Some(spec) =
                    parameter_spec(name).filter(|s| matches!(s.shape, ParameterShape::Numeric(_)))
                {
                    let parsed = if value.trim().is_empty() {
                        None
                    } else {
                        Some(parse_value(value, spec).ok_or_else(|| {
                            format!("{key}: not a {name} value: {value:?} (e.g. 100nF, 10k, 0.1u)")
                        })?)
                    };
                    let f = out.params.entry(spec.key).or_default();
                    match bound {
                        "min" => f.min = parsed,
                        "max" => f.max = parsed,
                        _ => return Err(format!("unknown parameter {key:?}")),
                    }
                    continue;
                }
            }
            if let Some(i) = GROUPS.iter().position(|g| g.params().contains(&key)) {
                for v in value.split(',') {
                    if let Some(v) = GROUPS[i].normalize(v) {
                        out.filters[i].insert(v);
                    } else if !v.trim().is_empty() {
                        // Unparseable (e.g. pins=abc): matches nothing.
                        out.filters[i].insert(format!("\u{0}{v}"));
                    }
                }
                continue;
            }
            if let Some((axis, bound)) = key.rsplit_once('_') {
                if let Some(i) = AXES.iter().position(|a| a.param() == axis) {
                    let parsed = if value.trim().is_empty() {
                        None
                    } else {
                        Some(
                            value
                                .trim()
                                .parse::<f64>()
                                .ok()
                                .filter(|v| v.is_finite())
                                .ok_or_else(|| format!("{key}: not a number: {value:?}"))?,
                        )
                    };
                    match bound {
                        "min" => out.ranges[i].0 = parsed,
                        "max" => out.ranges[i].1 = parsed,
                        _ => return Err(format!("unknown parameter {key:?}")),
                    }
                    continue;
                }
            }
            match key {
                "sort" => {
                    out.sort = Sort::parse(value).ok_or_else(|| {
                        format!(
                            "sort: unknown order {value:?} (relevance, mpn, manufacturer, pins, pitch, size, height)"
                        )
                    })?
                }
                "order" => {
                    out.descending = match value {
                        "" | "asc" => false,
                        "desc" => true,
                        _ => return Err(format!("order: expected asc or desc, got {value:?}")),
                    }
                }
                "offset" => {
                    out.offset = value
                        .parse()
                        .map_err(|_| format!("offset: not a count: {value:?}"))?
                }
                "limit" => {
                    let n: usize = value
                        .parse()
                        .map_err(|_| format!("limit: not a count: {value:?}"))?;
                    out.limit = n.min(MAX_LIMIT);
                }
                // Unknown parameters are ignored, like the rest of the API.
                _ => {}
            }
        }
        Ok(out)
    }
}

/// The filter on one parameter key: picked values (and, for numeric
/// keys, those values parsed to SI) and an SI range.
#[derive(Debug, Clone, Default)]
struct ParamFilter {
    values: BTreeSet<String>,
    numbers: Vec<f64>,
    min: Option<f64>,
    max: Option<f64>,
}

impl ParamFilter {
    fn active(&self) -> bool {
        !self.values.is_empty() || self.min.is_some() || self.max.is_some()
    }

    fn matches(&self, fact: Option<&ParamFact>) -> bool {
        if !self.active() {
            return true;
        }
        let Some(f) = fact else {
            return false;
        };
        let near = |a: f64, b: f64| (a - b).abs() <= 1e-9 * a.abs().max(b.abs());
        let in_values = self.values.is_empty()
            || self.values.contains(&f.key)
            || f.stated
                .is_some_and(|v| self.numbers.iter().any(|&n| near(n, v)));
        let ranged = self.min.is_some() || self.max.is_some();
        let in_range = !ranged
            || match f.span {
                // An operating range must contain the asked span: a
                // part for 3.3 V has min <= 3.3 V and max >= 3.3 V.
                Some((lo, hi)) => {
                    self.min
                        .is_none_or(|m| lo.is_some_and(|lo| lo <= m || near(lo, m)))
                        && self
                            .max
                            .is_none_or(|m| hi.is_some_and(|hi| hi >= m || near(hi, m)))
                }
                None => f.stated.is_some_and(|v| {
                    self.min.is_none_or(|m| v >= m || near(v, m))
                        && self.max.is_none_or(|m| v <= m || near(v, m))
                }),
            };
        in_values && in_range
    }
}

/// One Part's value of one parameter.
#[derive(Debug, Clone)]
struct ParamFact {
    /// The facet option key (the label without spaces, `100nF`).
    key: String,
    /// SI value of a numeric key.
    stated: Option<f64>,
    /// Option order: the SI value, or the vocabulary index.
    order: f64,
    /// An operating-range key's (`ParameterSpec::span`) lower and upper
    /// end in SI (a lone `typ` is both).
    span: Option<(Option<f64>, Option<f64>)>,
}

/// One Part's filterable facts.
#[derive(Debug)]
struct Facts {
    /// The facet key per [`GROUPS`] entry (`None`: not stated).
    keys: [Option<String>; GROUP_COUNT],
    dims: [Option<f64>; 3],
    params: BTreeMap<&'static str, ParamFact>,
    item: CatalogItem,
}

/// An option's display label and its place in a value-ordered facet.
#[derive(Debug, Clone)]
struct OptionInfo {
    label: String,
    order: f64,
}

/// The facts of every Part of a dataset, in `Dataset::parts` order
/// (the [`crate::search::SearchIndex`] order).
#[derive(Debug, Default)]
pub struct CatalogIndex {
    facts: Vec<Facts>,
    options: [BTreeMap<String, OptionInfo>; GROUP_COUNT],
    /// Display labels of parameter options, by key then option key.
    param_labels: BTreeMap<&'static str, BTreeMap<String, String>>,
}

impl CatalogIndex {
    pub fn build(ds: &Dataset) -> CatalogIndex {
        let mut index = CatalogIndex::default();
        let mut mountings: BTreeMap<String, Option<Mounting>> = BTreeMap::new();
        for p in &ds.parts {
            let device = ds.devices.get(&p.device.0);
            let pkg = ds.packages.get(&p.package.0);
            let manufacturer_name = ds
                .manufacturers
                .get(&p.manufacturer.0)
                .map_or_else(|| p.manufacturer.0.clone(), |m| m.name.clone());
            let category = device.map(|d| d.category);
            let pitch = pkg.and_then(|k| stated(&k.pitch));
            let width = pkg.and_then(|k| stated(&k.dimensions.body_width));
            let length = pkg.and_then(|k| stated(&k.dimensions.body_length));
            let height = pkg.and_then(|k| k.dimensions.body_height.as_ref().and_then(stated));
            let codes = match (pkg, width, length) {
                (Some(k), Some(w), Some(l)) if k.family == PackageFamily::Chip => {
                    chip_codes(l.max(w), l.min(w))
                }
                _ => &[],
            };
            let mounting = pkg.and_then(|k| {
                *mountings
                    .entry(k.id.0.clone())
                    .or_insert_with(|| mounting(k))
            });
            let structure = pkg.map(|k| k.structure()).unwrap_or_default();
            let no_params = Default::default();
            let device_params = device.map_or(&no_params, |d| &d.parameters);
            let mut params = BTreeMap::new();
            let mut item_params = BTreeMap::new();
            for (key, (values, _)) in effective_parameters(&p.parameters, device_params) {
                let Some(spec) = parameter_spec(key) else {
                    continue;
                };
                // The primary value (the first condition variant). Invalid
                // values are the validator's to report; the catalog only
                // lists what normalises.
                let value = values.primary();
                let Ok(n) = value.normalize(spec) else {
                    continue;
                };
                let at = conditions_label(&value.at);
                // A value under other conditions is another option
                // (like-for-like): `18 mΩ @ VGS 10 V`.
                let label = if at.is_empty() {
                    n.label(spec)
                } else {
                    format!("{} @ {at}", n.label(spec))
                };
                let stated = n.stated(spec);
                let order = match (&n, spec.shape) {
                    (Normalized::Text(t), ParameterShape::Enumerated(words)) => {
                        words.iter().position(|w| w == t).unwrap_or(0) as f64
                    }
                    _ => stated.unwrap_or(f64::MAX),
                };
                let fact_key = label.replace(' ', "");
                index
                    .param_labels
                    .entry(spec.key)
                    .or_default()
                    .entry(fact_key.clone())
                    .or_insert_with(|| label.clone());
                item_params.insert(
                    spec.key.to_string(),
                    CatalogParameter {
                        value: stated,
                        label,
                    },
                );
                let span = match n {
                    Normalized::Numeric { typ, min, max } if spec.span => {
                        Some((min.or(typ), max.or(typ)))
                    }
                    _ => None,
                };
                params.insert(
                    spec.key,
                    ParamFact {
                        key: fact_key,
                        stated,
                        order,
                        span,
                    },
                );
            }
            let item = CatalogItem {
                manufacturer: p.manufacturer.0.clone(),
                manufacturer_name: manufacturer_name.clone(),
                mpn: p.mpn.clone(),
                category,
                device: p.device.0.clone(),
                package: p.package.0.clone(),
                package_family: pkg.map(|k| k.family),
                lead_count: pkg.map(|k| k.lead_count),
                pitch,
                body_width: width,
                body_length: length,
                body_height: height,
                size_code: codes.first().map(|s| s.to_string()),
                size_code_metric: codes.get(1).map(|s| s.trim_end_matches('m').to_string()),
                mounting,
                rows: structure.rows.map(|(r, n)| format!("{r}x{n}")),
                gender: structure.gender,
                orientation: structure.orientation,
                shrouded: structure.shrouded,
                exposed_pad: structure.exposed_pad,
                mechanism: structure.mechanism,
                contact_side: structure.contact_side,
                interface: structure.interface,
                lifecycle: p.lifecycle.status,
                existence: p.existence.status,
                review: crate::documents::ReviewSummary::of(p, device, pkg),
                parameters: item_params,
            };
            let mut keys: [Option<String>; GROUP_COUNT] = Default::default();
            for (i, g) in GROUPS.iter().enumerate() {
                let opt = match g {
                    Group::Category => {
                        category.map(|c| (c.name().to_string(), category_label(c), 0.0))
                    }
                    Group::Manufacturer => {
                        Some((p.manufacturer.0.clone(), manufacturer_name.clone(), 0.0))
                    }
                    Group::Family => {
                        pkg.map(|k| (k.family.name().to_string(), family_label(k.family), 0.0))
                    }
                    Group::Size => item.size_code.as_ref().map(|c| {
                        let metric = item.size_code_metric.clone().unwrap_or_default();
                        let order = CHIP_SIZES
                            .iter()
                            .position(|s| s.2[0] == c)
                            .unwrap_or(usize::MAX);
                        (c.clone(), format!("{c} ({metric})"), order as f64)
                    }),
                    Group::Pins => item
                        .lead_count
                        .map(|n| (n.to_string(), n.to_string(), n as f64)),
                    Group::Pitch => pitch.map(|v| (fmt_mm(v), format!("{} mm", fmt_mm(v)), v)),
                    Group::Mounting => mounting
                        .map(|m| (m.wire().to_string(), m.label().to_string(), m as u8 as f64)),
                    Group::Lifecycle => {
                        let s = p.lifecycle.status;
                        Some((wire(&s), lifecycle_label(s).to_string(), s as u8 as f64))
                    }
                    Group::Existence => {
                        let s = p.existence.status;
                        Some((wire(&s), title_case(&wire(&s)), s as u8 as f64))
                    }
                    Group::Review => {
                        let s = item.review.state;
                        let label = match s {
                            crate::documents::ReviewState::Reviewed => "Reviewed",
                            crate::documents::ReviewState::Partly => "Partly reviewed",
                            crate::documents::ReviewState::Unreviewed => "Unreviewed",
                        };
                        Some((s.wire().to_string(), label.to_string(), s as u8 as f64))
                    }
                    Group::Rows => structure.rows.map(|(r, n)| {
                        (
                            format!("{r}x{n}"),
                            format!("{r}\u{d7}{n}"),
                            f64::from(r) * 1000.0 + f64::from(n),
                        )
                    }),
                    Group::Gender => structure.gender.map(|g| {
                        let label = match g {
                            Gender::Male => "Male (header, plug)",
                            Gender::Female => "Female (socket, receptacle)",
                        };
                        (wire(&g), label.to_string(), g as u8 as f64)
                    }),
                    Group::Orientation => structure.orientation.map(|o| {
                        let label = match o {
                            Orientation::Vertical => "Vertical",
                            Orientation::RightAngle => "Right angle",
                        };
                        (wire(&o), label.to_string(), o as u8 as f64)
                    }),
                    Group::Shrouded => structure
                        .shrouded
                        .map(|b| yes_no(b, "Shrouded", "Unshrouded")),
                    Group::ExposedPad => structure
                        .exposed_pad
                        .map(|b| yes_no(b, "Exposed pad", "No exposed pad")),
                    Group::Mechanism => structure.mechanism.map(|m| {
                        let label = match m {
                            CardMechanism::PushPush => "Push-push",
                            CardMechanism::PushPull => "Push-pull",
                            CardMechanism::Hinged => "Hinged",
                        };
                        (wire(&m), label.to_string(), m as u8 as f64)
                    }),
                    Group::ContactSide => structure.contact_side.map(|c| {
                        let label = match c {
                            ContactSide::Top => "Top contact",
                            ContactSide::Bottom => "Bottom contact",
                            ContactSide::Dual => "Dual contact",
                        };
                        (wire(&c), label.to_string(), c as u8 as f64)
                    }),
                    Group::Interface => structure
                        .interface
                        .map(|i| (wire(&i), interface_label(i).to_string(), i as u8 as f64)),
                };
                if let Some((key, label, order)) = opt {
                    index.options[i]
                        .entry(key.clone())
                        .or_insert(OptionInfo { label, order });
                    keys[i] = Some(key);
                }
            }
            index.facts.push(Facts {
                keys,
                dims: [width, length, height],
                params,
                item,
            });
        }
        index
    }

    /// Answers `q` over `hits` (indices into `Dataset::parts` with
    /// their text scores, as [`crate::search::SearchIndex::search`]
    /// returns them).
    pub fn query(&self, hits: &[(usize, u32)], q: &CatalogQuery) -> CatalogResponse {
        // The constraints each hit is checked against: GROUPS, then
        // AXES, then one per filtered parameter key.
        let active: Vec<(&'static str, &ParamFilter)> = q
            .params
            .iter()
            .filter(|(_, f)| f.active())
            .map(|(k, f)| (*k, f))
            .collect();
        let base = GROUPS.len() + AXES.len();
        let n = base + active.len();
        let param_slot = |key: &str| active.iter().position(|(k, _)| *k == key).map(|i| base + i);
        let passes: Vec<(usize, u32, Vec<bool>)> = hits
            .iter()
            .filter(|(i, _)| *i < self.facts.len())
            .map(|&(i, score)| {
                let f = &self.facts[i];
                let mut pass = vec![true; n];
                for g in 0..GROUPS.len() {
                    let wanted = &q.filters[g];
                    pass[g] = wanted.is_empty()
                        || if GROUPS[g] == Group::Category {
                            // A picked group takes in its subcategories.
                            f.item.category.is_some_and(|c| {
                                wanted.contains(c.name())
                                    || c.parent().is_some_and(|p| wanted.contains(p.name()))
                            })
                        } else {
                            f.keys[g].as_ref().is_some_and(|k| wanted.contains(k))
                        };
                }
                for a in 0..AXES.len() {
                    let (min, max) = q.ranges[a];
                    pass[GROUPS.len() + a] = (min.is_none() && max.is_none())
                        || f.dims[a].is_some_and(|v| {
                            min.is_none_or(|m| v >= m - 1e-9) && max.is_none_or(|m| v <= m + 1e-9)
                        });
                }
                for (j, (key, filter)) in active.iter().enumerate() {
                    pass[base + j] = filter.matches(f.params.get(key));
                }
                (i, score, pass)
            })
            .collect();
        let all_but = |pass: &[bool], skip: Option<usize>| {
            pass.iter()
                .enumerate()
                .all(|(j, &ok)| ok || Some(j) == skip)
        };

        let mut groups: Vec<Vec<FacetValue>> = Vec::with_capacity(GROUPS.len());
        for (g, group) in GROUPS.iter().enumerate() {
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for (i, _, pass) in &passes {
                if all_but(pass, Some(g)) {
                    if let Some(k) = &self.facts[*i].keys[g] {
                        *counts.entry(k.as_str()).or_default() += 1;
                    }
                }
            }
            let mut values: Vec<(f64, FacetValue)> = counts
                .iter()
                .map(|(k, n)| (k.to_string(), *n))
                .chain(
                    q.filters[g]
                        .iter()
                        .filter(|k| !counts.contains_key(k.as_str()) && !k.starts_with('\u{0}'))
                        .map(|k| (k.clone(), 0)),
                )
                .map(|(value, count)| {
                    let info = self.options[g].get(&value);
                    let order = info.map_or(f64::MAX, |o| o.order);
                    let label = info.map_or_else(|| value.clone(), |o| o.label.clone());
                    let selected = q.filters[g].contains(&value);
                    (
                        order,
                        FacetValue {
                            value,
                            label,
                            count,
                            selected,
                        },
                    )
                })
                .collect();
            if group.by_count() {
                values.sort_by(|a, b| {
                    b.1.count
                        .cmp(&a.1.count)
                        .then_with(|| a.1.label.to_lowercase().cmp(&b.1.label.to_lowercase()))
                });
            } else {
                values.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.value.cmp(&b.1.value)));
            }
            groups.push(values.into_iter().map(|(_, v)| v).collect());
        }
        let ranges: Vec<RangeFacet> = (0..AXES.len())
            .map(|a| {
                let mut r = RangeFacet::default();
                let values = passes
                    .iter()
                    .filter(|(_, _, pass)| all_but(pass, Some(GROUPS.len() + a)))
                    .filter_map(|(i, _, _)| self.facts[*i].dims[a]);
                for v in values {
                    r.min = Some(r.min.map_or(v, |m: f64| m.min(v)));
                    r.max = Some(r.max.map_or(v, |m: f64| m.max(v)));
                }
                r
            })
            .collect();
        let category_tree = {
            let g = GROUPS
                .iter()
                .position(|g| *g == Group::Category)
                .unwrap_or(0);
            let mut own: BTreeMap<ComponentCategory, usize> = BTreeMap::new();
            for (i, _, pass) in &passes {
                if all_but(pass, Some(g)) {
                    if let Some(c) = self.facts[*i].item.category {
                        *own.entry(c).or_default() += 1;
                    }
                }
            }
            let picked = |c: ComponentCategory| q.filters[g].contains(c.name());
            let node = |c: ComponentCategory, children: Vec<CategoryNode>| CategoryNode {
                value: c.name().to_string(),
                label: category_label(c),
                count: own.get(&c).copied().unwrap_or(0)
                    + children.iter().map(|n| n.count).sum::<usize>(),
                selected: picked(c),
                children,
            };
            let shown = |n: &CategoryNode| {
                n.count > 0 || n.selected || n.children.iter().any(|c| c.selected)
            };
            ComponentCategory::ALL
                .iter()
                .filter(|c| c.parent().is_none())
                .map(|&root| {
                    let children = root
                        .children()
                        .map(|c| node(c, vec![]))
                        .filter(shown)
                        .collect();
                    node(root, children)
                })
                .filter(shown)
                .collect()
        };
        let parameters = PARAMETERS
            .iter()
            .filter_map(|spec| {
                let filter = q.params.get(spec.key).filter(|f| f.active());
                let skip = param_slot(spec.key);
                // (option key) -> (order, count)
                let mut counts: BTreeMap<&str, (f64, usize)> = BTreeMap::new();
                let mut span: Option<(f64, f64)> = None;
                for (i, _, pass) in &passes {
                    if !all_but(pass, skip) {
                        continue;
                    }
                    let Some(fact) = self.facts[*i].params.get(spec.key) else {
                        continue;
                    };
                    counts.entry(fact.key.as_str()).or_insert((fact.order, 0)).1 += 1;
                    // An operating range spans its ends, not its middle.
                    let (lo, hi) = fact.span.map_or((fact.stated, fact.stated), |(lo, hi)| {
                        (lo.or(fact.stated), hi.or(fact.stated))
                    });
                    for v in [lo, hi].into_iter().flatten() {
                        span = Some(span.map_or((v, v), |(a, b)| (a.min(v), b.max(v))));
                    }
                }
                if counts.is_empty() && filter.is_none() {
                    return None;
                }
                let labels = self.param_labels.get(spec.key);
                let mut values: Vec<(f64, FacetValue)> = counts
                    .iter()
                    .map(|(k, (order, count))| {
                        let fact_like = ParamFact {
                            key: k.to_string(),
                            stated: (*order != f64::MAX
                                && matches!(spec.shape, ParameterShape::Numeric(_)))
                            .then_some(*order),
                            order: *order,
                            span: None,
                        };
                        let selected = filter.is_some_and(|f| {
                            !f.values.is_empty()
                                && ParamFilter {
                                    min: None,
                                    max: None,
                                    ..(*f).clone()
                                }
                                .matches(Some(&fact_like))
                        });
                        (
                            *order,
                            FacetValue {
                                value: k.to_string(),
                                label: labels
                                    .and_then(|l| l.get(*k))
                                    .cloned()
                                    .unwrap_or_else(|| k.to_string()),
                                count: *count,
                                selected,
                            },
                        )
                    })
                    .collect();
                // Picked values no part under the other filters has.
                if let Some(f) = filter {
                    for v in &f.values {
                        let parsed = parse_value(v, spec);
                        let covered = values.iter().any(|(order, x)| {
                            x.value == *v
                                || (x.selected
                                    && parsed.is_some_and(|n| {
                                        (n - order).abs() <= 1e-9 * n.abs().max(order.abs())
                                    }))
                        });
                        if !v.starts_with('\u{0}') && !covered {
                            values.push((
                                parsed.unwrap_or(f64::MAX),
                                FacetValue {
                                    value: v.clone(),
                                    label: parsed.map_or_else(
                                        || v.clone(),
                                        |n| openparts_core::format_value(n, spec),
                                    ),
                                    count: 0,
                                    selected: true,
                                },
                            ));
                        }
                    }
                }
                values.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.value.cmp(&b.1.value)));
                let (kind, unit) = match spec.shape {
                    ParameterShape::Numeric(q) => (ParameterKind::Numeric, q.si_unit()),
                    ParameterShape::Enumerated(_) => (ParameterKind::Enumerated, ""),
                };
                let fmt = |v: f64| openparts_core::format_value(v, spec);
                Some(ParameterFacet {
                    key: spec.key.to_string(),
                    kind,
                    unit: unit.to_string(),
                    values: values.into_iter().map(|(_, v)| v).collect(),
                    min: span.map(|s| s.0),
                    max: span.map(|s| s.1),
                    min_label: span.map(|s| fmt(s.0)),
                    max_label: span.map(|s| fmt(s.1)),
                })
            })
            .collect();

        let mut matched: Vec<(usize, u32)> = passes
            .iter()
            .filter(|(_, _, pass)| pass.iter().all(|&ok| ok))
            .map(|(i, s, _)| (*i, *s))
            .collect();
        self.sort(&mut matched, q);
        let total = matched.len();
        let items = matched
            .iter()
            .skip(q.offset)
            .take(q.limit)
            .map(|(i, _)| self.facts[*i].item.clone())
            .collect();
        let mut groups = groups.into_iter();
        let mut next = || groups.next().unwrap_or_default();
        let mut ranges = ranges.into_iter();
        let mut range = || ranges.next().unwrap_or_default();
        CatalogResponse {
            total,
            offset: q.offset,
            limit: q.limit,
            items,
            facets: CatalogFacets {
                category: next(),
                category_tree,
                manufacturer: next(),
                package_family: next(),
                size: next(),
                pins: next(),
                pitch: next(),
                mounting: next(),
                lifecycle: next(),
                existence: next(),
                review: next(),
                rows: next(),
                gender: next(),
                orientation: next(),
                shrouded: next(),
                exposed_pad: next(),
                mechanism: next(),
                contact_side: next(),
                interface: next(),
                body_width: range(),
                body_length: range(),
                body_height: range(),
                parameters,
            },
        }
    }

    fn sort(&self, hits: &mut [(usize, u32)], q: &CatalogQuery) {
        use std::cmp::Ordering;
        let name = |i: usize| {
            let it = &self.facts[i].item;
            (it.manufacturer.as_str(), it.mpn.as_str())
        };
        // A value-keyed order; parts without the value go last either way.
        let by_value = |a: Option<(f64, f64)>, b: Option<(f64, f64)>| match (a, b) {
            (Some(x), Some(y)) => {
                let o = x.0.total_cmp(&y.0).then(x.1.total_cmp(&y.1));
                if q.descending {
                    o.reverse()
                } else {
                    o
                }
            }
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        let key = |i: usize| -> Option<(f64, f64)> {
            let it = &self.facts[i].item;
            match q.sort {
                Sort::Pins => it.lead_count.map(|n| (n as f64, 0.0)),
                Sort::Pitch => it.pitch.map(|v| (v, 0.0)),
                Sort::Height => it.body_height.map(|v| (v, 0.0)),
                Sort::Parameter(k) => self.facts[i]
                    .params
                    .get(k)
                    .and_then(|f| f.stated)
                    .map(|v| (v, 0.0)),
                Sort::Size => match (it.body_width, it.body_length) {
                    (Some(w), Some(l)) => Some((w.max(l), w.min(l))),
                    _ => None,
                },
                _ => None,
            }
        };
        let flip = |o: Ordering| if q.descending { o.reverse() } else { o };
        hits.sort_by(|&(a, sa), &(b, sb)| {
            let primary = match q.sort {
                Sort::Relevance if q.q.trim().is_empty() => Ordering::Equal,
                Sort::Relevance => flip(sb.cmp(&sa)),
                Sort::Mpn => flip(
                    self.facts[a]
                        .item
                        .mpn
                        .to_lowercase()
                        .cmp(&self.facts[b].item.mpn.to_lowercase()),
                ),
                Sort::Manufacturer => flip(
                    self.facts[a]
                        .item
                        .manufacturer_name
                        .to_lowercase()
                        .cmp(&self.facts[b].item.manufacturer_name.to_lowercase()),
                ),
                _ => by_value(key(a), key(b)),
            };
            primary.then_with(|| name(a).cmp(&name(b)))
        });
    }
}

/// How `pkg` mounts, from its generated leads and lands; by family when
/// the geometry can't be generated (a missing dimension).
fn mounting(pkg: &Package) -> Option<Mounting> {
    use openparts_mcad::Mounting as M;
    let (mut smd, mut tht) = (false, false);
    match openparts_mcad::generate(pkg) {
        Ok(g) => {
            for lead in g.leads.iter().chain(&g.lands) {
                match lead.mounting {
                    M::Smd => smd = true,
                    M::ThroughHole => tht = true,
                    M::NonPlated => {}
                }
            }
        }
        Err(_) => {
            use PackageFamily as F;
            match pkg.family {
                F::Radial
                | F::Axial
                | F::To92
                | F::To220
                | F::Dip
                | F::Sip
                | F::Header
                | F::Socket
                | F::ShroudedHeader
                | F::RightAngleHeader
                | F::TactSwitchTht => tht = true,
                F::UsbC
                | F::Module
                | F::CardSocket
                | F::BarrelJack
                | F::TerminalBlock
                | F::FpcConnector
                | F::RfCoax
                | F::MicroUsbB
                | F::MiniUsbB
                | F::UsbB => return None,
                _ => smd = true,
            }
        }
    }
    match (smd, tht) {
        (true, false) => Some(Mounting::Smd),
        (false, true) => Some(Mounting::Tht),
        (true, true) => Some(Mounting::Mixed),
        (false, false) => None,
    }
}

/// A dimension's value as the generators read it: nominal, else the
/// middle of min/max, else the one stated limit.
fn stated(d: &Dimension) -> Option<f64> {
    d.typ.or(match (d.min, d.max) {
        (Some(min), Some(max)) => Some((min + max) / 2.0),
        (min, max) => min.or(max),
    })
}

/// `1x20` from `1x20`, `1×20`, `1*20` (rows x pins per row).
pub(crate) fn rows_key(v: &str) -> Option<String> {
    let v = v.replace(['\u{d7}', '*', 'X'], "x");
    let (r, n) = v.split_once('x')?;
    let (r, n) = (r.trim().parse::<u32>().ok()?, n.trim().parse::<u32>().ok()?);
    (r > 0 && n > 0).then(|| format!("{r}x{n}"))
}

/// A yes/no facet option: (value, label, order).
fn yes_no(b: bool, yes: &str, no: &str) -> (String, String, f64) {
    if b {
        ("yes".into(), yes.into(), 0.0)
    } else {
        ("no".into(), no.into(), 1.0)
    }
}

/// mm with at most three decimals and no trailing zeros (`2.54`, `0.5`, `1`).
fn fmt_mm(v: f64) -> String {
    let s = format!("{:.3}", (v * 1000.0).round() / 1000.0);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// An imperial chip code for an imperial or metric one (`1608` and
/// `0603` both give `0603`; `0603m` is metric 0603, imperial `0201`).
fn size_imperial(code: &str) -> Option<String> {
    CHIP_SIZES.iter().find_map(|(_, _, codes)| {
        let metric = codes[1].trim_end_matches('m');
        let is =
            codes[0] == code || codes[1] == code || (metric == code && !codes[1].ends_with('m'));
        is.then(|| codes[0].to_string())
    })
}

/// An enum's serde (wire) name.
fn wire<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|j| j.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn title_case(s: &str) -> String {
    let s = s.replace('_', " ");
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// A category's English display name.
fn category_label(c: ComponentCategory) -> String {
    use ComponentCategory as C;
    match c {
        C::Microcontroller => "Microcontroller".into(),
        C::Led => "LED".into(),
        C::Imu => "IMU / accelerometer".into(),
        C::Sram => "SRAM".into(),
        C::Eeprom => "EEPROM".into(),
        C::Adc => "ADC".into(),
        C::Dac => "DAC".into(),
        C::Rtc => "RTC".into(),
        C::OpAmp => "Op amp".into(),
        C::IoExpander => "I/O expander".into(),
        C::CanController => "CAN controller".into(),
        C::CanTransceiver => "CAN transceiver".into(),
        C::Rs485Transceiver => "RS-485 transceiver".into(),
        C::Rs232Transceiver => "RS-232 transceiver".into(),
        other => title_case(other.name()),
    }
}

fn family_label(f: PackageFamily) -> String {
    use PackageFamily as F;
    match f {
        F::Chip => "Chip".into(),
        F::Radial => "Radial".into(),
        F::Axial => "Axial".into(),
        F::Module => "Module".into(),
        F::Header => "Pin header".into(),
        F::Socket => "Pin socket".into(),
        F::ShroudedHeader => "Shrouded header".into(),
        F::RightAngleHeader => "Right-angle header".into(),
        F::TactSwitchSmd => "Tact switch (SMD)".into(),
        F::TactSwitchTht => "Tact switch (THT)".into(),
        F::UsbC => "USB-C".into(),
        F::CardSocket => "Card socket".into(),
        F::BarrelJack => "DC barrel jack".into(),
        F::TerminalBlock => "Terminal block".into(),
        F::FpcConnector => "FPC/FFC connector".into(),
        F::RfCoax => "RF coaxial".into(),
        F::MicroUsbB => "Micro-USB B".into(),
        F::MiniUsbB => "Mini-USB B".into(),
        F::UsbB => "USB-B".into(),
        F::Melf => "MELF".into(),
        F::SmdLed => "SMD LED".into(),
        F::SmdElectrolytic => "SMD electrolytic".into(),
        F::SodFlat => "SOD (flat)".into(),
        F::Sot223 => "SOT-223".into(),
        F::Do214 => "DO-214".into(),
        F::To220 => "TO-220".into(),
        F::To92 => "TO-92".into(),
        F::D2pak => "D2PAK".into(),
        other => other.name().to_uppercase(),
    }
}

fn interface_label(i: ConnectorInterface) -> &'static str {
    use ConnectorInterface as I;
    match i {
        I::MicroSd => "microSD",
        I::Sd => "SD",
        I::Sma => "SMA",
        I::RpSma => "RP-SMA",
        I::Smb => "SMB",
        I::Mcx => "MCX",
        I::Mmcx => "MMCX",
        I::UFl => "U.FL",
        I::Bnc => "BNC",
    }
}

fn lifecycle_label(s: LifecycleStatus) -> &'static str {
    match s {
        LifecycleStatus::PreRelease => "Pre-release",
        LifecycleStatus::Active => "Active",
        LifecycleStatus::Nrnd => "NRND",
        LifecycleStatus::EolAnnounced => "EOL announced",
        LifecycleStatus::Obsolete => "Obsolete",
        LifecycleStatus::Unknown => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(pairs: &[(&str, &str)]) -> CatalogQuery {
        CatalogQuery::from_pairs(pairs.iter().copied()).unwrap()
    }

    #[test]
    fn size_codes_normalise_to_imperial() {
        assert_eq!(size_imperial("0603").as_deref(), Some("0603"));
        assert_eq!(size_imperial("1608").as_deref(), Some("0603"));
        assert_eq!(size_imperial("0603m").as_deref(), Some("0201"));
        assert_eq!(size_imperial("1005").as_deref(), Some("0402"));
        assert_eq!(size_imperial("9999"), None);
    }

    #[test]
    fn filter_values_normalise() {
        let q = parse(&[
            ("category", "Capacitor,resistor"),
            ("category", "led"),
            ("family", "TQFP"),
            ("size", "1608"),
            ("pitch", "2.540"),
            ("pins", "08"),
            ("height_max", "1.2"),
            ("sort", "pitch"),
            ("order", "desc"),
            ("limit", "100000"),
        ]);
        let set = |g: usize| q.filters[g].iter().cloned().collect::<Vec<_>>();
        assert_eq!(set(0), ["capacitor", "led", "resistor"]);
        assert_eq!(set(2), ["lqfp"]);
        assert_eq!(set(3), ["0603"]);
        assert_eq!(set(4), ["8"]);
        assert_eq!(set(5), ["2.54"]);
        assert_eq!(q.ranges[2], (None, Some(1.2)));
        assert_eq!(
            (q.sort, q.descending, q.limit),
            (Sort::Pitch, true, MAX_LIMIT)
        );
    }

    #[test]
    fn bad_numbers_are_errors() {
        assert!(CatalogQuery::from_pairs([("width_min", "wide")]).is_err());
        assert!(CatalogQuery::from_pairs([("sort", "price")]).is_err());
        assert!(CatalogQuery::from_pairs([("limit", "-1")]).is_err());
    }

    #[test]
    fn parameter_filters_parse_typed_values() {
        let q = parse(&[
            ("capacitance", "100nF,0.1u"),
            ("capacitance_min", "10n"),
            ("capacitance_max", "1µF"),
            ("dielectric", "x7r,C0G,Z9Z"),
            ("sort", "capacitance"),
        ]);
        let c = &q.params["capacitance"];
        assert_eq!(c.min, Some(1e-8));
        assert!(c.max.is_some_and(|m| (m - 1e-6).abs() < 1e-18));
        assert_eq!(c.numbers.len(), 2);
        assert!(c.numbers.iter().all(|n| (n - 1e-7).abs() < 1e-18));
        let d: Vec<_> = q.params["dielectric"].values.iter().cloned().collect();
        assert_eq!(d, ["\u{0}Z9Z", "C0G", "X7R"]);
        assert_eq!(q.sort, Sort::Parameter("capacitance"));
        let fact = |v: f64| ParamFact {
            key: String::new(),
            stated: Some(v),
            order: v,
            span: None,
        };
        assert!(c.matches(Some(&fact(1e-7))));
        assert!(!c.matches(Some(&fact(2.2e-7))));
        assert!(!c.matches(None));
        assert!(CatalogQuery::from_pairs([("capacitance_min", "ten")]).is_err());
        assert!(CatalogQuery::from_pairs([("resistance_max", "10V")]).is_err());
        assert!(CatalogQuery::from_pairs([("resistance_max", "4k7")]).is_ok());
        // An enumerated key takes no range.
        assert!(CatalogQuery::from_pairs([("dielectric_min", "1")]).is_ok());
        // Zener and optocoupler keys filter in their display units.
        let q = parse(&[
            ("zener_voltage", "5.1V"),
            ("current_transfer_ratio_min", "100"),
            ("isolation_voltage_min", "3.75k"),
        ]);
        assert!(q.params["zener_voltage"]
            .numbers
            .iter()
            .all(|n| (n - 5.1).abs() < 1e-12));
        let ctr = q.params["current_transfer_ratio"].min.unwrap();
        assert!((ctr - 1.0).abs() < 1e-12);
        assert_eq!(q.params["isolation_voltage"].min, Some(3750.0));
    }

    /// An operating range (supply voltage, operating temperature) is
    /// filtered by containment: `supply_voltage_min=3.3&max=3.3` finds
    /// a 1.8-5.5 V part, not a 4.5-5.5 V one.
    #[test]
    fn operating_ranges_filter_by_containment() {
        let q = parse(&[("supply_voltage_min", "3.3"), ("supply_voltage_max", "3.3")]);
        let f = &q.params["supply_voltage"];
        let part = |lo: f64, hi: f64| ParamFact {
            key: String::new(),
            stated: Some((lo + hi) / 2.0),
            order: (lo + hi) / 2.0,
            span: Some((Some(lo), Some(hi))),
        };
        assert!(f.matches(Some(&part(1.8, 5.5))));
        assert!(!f.matches(Some(&part(4.5, 5.5))));
        assert!(!f.matches(Some(&part(1.65, 3.0))));
        let q = parse(&[("operating_temperature_min", "-40")]);
        let t = &q.params["operating_temperature"];
        assert!(t.matches(Some(&part(-40.0, 85.0))));
        assert!(!t.matches(Some(&part(-20.0, 70.0))));
    }

    /// The web catalog names every parameter key in both languages and
    /// knows which keys are enumerated.
    #[test]
    fn the_web_labels_every_parameter_key() {
        let i18n = include_str!("../../web/cmd/app/i18n.mbt");
        let catalog = include_str!("../../web/cmd/app/catalog.mbt");
        for spec in PARAMETERS {
            assert!(
                i18n.contains(&format!("\"{}\", \"", spec.key)),
                "web/cmd/app/i18n.mbt param_names lacks {}",
                spec.key
            );
            let enumerated = matches!(spec.shape, ParameterShape::Enumerated(_));
            let listed = catalog
                .split("let enum_param_keys : Array[String] = [")
                .nth(1)
                .and_then(|s| s.split(']').next())
                .is_some_and(|s| s.contains(&format!("\"{}\"", spec.key)));
            assert_eq!(enumerated, listed, "enum_param_keys and {}", spec.key);
        }
    }

    /// Connector facet values normalise to their wire names, and the web
    /// labels the connector families and every connector facet value.
    #[test]
    fn connector_facets_normalise_and_the_web_labels_them() {
        let q = parse(&[
            ("mechanism", "Push-Push"),
            ("contacts", "bottom"),
            ("interface", "micro-SD"),
        ]);
        let set = |g: Group| {
            let i = GROUPS.iter().position(|x| *x == g).unwrap();
            q.filters[i].iter().cloned().collect::<Vec<_>>()
        };
        assert_eq!(set(Group::Mechanism), ["push_push"]);
        assert_eq!(set(Group::ContactSide), ["bottom"]);
        assert_eq!(set(Group::Interface), ["micro_sd"]);
        let i18n = include_str!("../../web/cmd/app/i18n.mbt");
        let catalog = include_str!("../../web/cmd/app/catalog.mbt");
        for f in PackageFamily::ALL
            .iter()
            .filter(|f| openparts_core::is_connector_family(**f))
        {
            assert!(i18n.contains(&format!("(Ja, \"{}\")", f.name())), "{f}");
            assert!(i18n.contains(&format!("(En, \"{}\")", f.name())), "{f}");
        }
        let pairs = [
            ("mechanism", wire(&CardMechanism::PushPush)),
            ("mechanism", wire(&CardMechanism::PushPull)),
            ("mechanism", wire(&CardMechanism::Hinged)),
            ("contact_side", wire(&ContactSide::Top)),
            ("contact_side", wire(&ContactSide::Bottom)),
            ("contact_side", wire(&ContactSide::Dual)),
        ];
        use ConnectorInterface as I;
        let interfaces = [
            I::MicroSd,
            I::Sd,
            I::Sma,
            I::RpSma,
            I::Smb,
            I::Mcx,
            I::Mmcx,
            I::UFl,
            I::Bnc,
        ];
        let interfaces = interfaces.iter().map(|i| ("interface", wire(i)));
        for (key, v) in pairs.into_iter().chain(interfaces) {
            assert!(catalog.contains(&format!(r#""{key}""#)), "{key}");
            let option = format!(r#""{key}", "{v}")"#);
            assert!(i18n.contains(&option), "{option}");
        }
    }

    #[test]
    fn category_names_are_the_wire_names() {
        for c in ComponentCategory::ALL {
            assert_eq!(wire(c), c.name());
        }
    }

    #[test]
    fn mm_formatting() {
        assert_eq!(fmt_mm(2.54), "2.54");
        assert_eq!(fmt_mm(0.5000001), "0.5");
        assert_eq!(fmt_mm(1.0), "1");
        assert_eq!(fmt_mm(1.27), "1.27");
    }
}
