//! Reference circuits (Canonical Data Specification section 41):
//!
//! - `GET /v1/circuits`: every circuit (id, title, host, review state);
//! - `GET /v1/circuits/{publisher}/{name}`: one circuit document as
//!   stored (in-memory shape: quantities `typ`/`unit`, a `provenance`
//!   map) -- what the board tool expands;
//! - `GET /v1/parts/{manufacturer}/{mpn}/circuits`: the circuits whose
//!   host is the part's device, as display rows ([`CircuitView`]) for
//!   the part page.

use crate::error::ApiError;
use crate::store::Store;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use openparts_core::circuit::{target, Target};
use openparts_core::{
    parameter_spec, Circuit, CircuitHost, DocumentType, FormulaInput, Level, LevelValue,
    ParameterValue, ProvenanceEntry, Requirement,
};
use openparts_data::Dataset;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn router() -> Router<Arc<Store>> {
    Router::new()
        .route("/v1/circuits", get(list_circuits))
        .route("/v1/circuits/:publisher/:name", get(get_circuit))
        .route("/v1/parts/:manufacturer/:mpn/circuits", get(part_circuits))
}

/// One line of `GET /v1/circuits`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct CircuitSummary {
    pub id: String,
    pub title: String,
    /// The host device id, or a description of a generic host.
    pub host: String,
    pub reviewed: bool,
}

/// Where a fact is cited.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CircuitCite {
    pub source: String,
    /// `RP-008279-DS rev 2`.
    pub label: String,
    pub page: Option<u32>,
    pub locator: Option<String>,
    /// The document's URL (with `#page=N` when a page is cited).
    pub url: Option<String>,
}

/// One role of a circuit, as the part page lists it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct CircuitRoleRow {
    pub role: String,
    /// Instances on this host (one per pin for `per_pin`).
    pub count: u32,
    /// `100 nF`, `5.1 kΩ, tolerance ≤ +20%`, or a part id.
    pub value: String,
    /// What the role's pins connect to: host pin names and nets.
    pub connects: Vec<String>,
    pub level: LevelValue,
    pub word: Option<String>,
    pub cite: Option<CircuitCite>,
}

/// One host pin connection.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct CircuitConnectRow {
    pub pin: String,
    /// The net, or `NC` (left open).
    pub net: String,
    pub level: LevelValue,
    pub word: Option<String>,
    pub cite: Option<CircuitCite>,
}

/// A formula with its value from the required inputs.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct CircuitFormulaRow {
    pub name: String,
    pub expr: String,
    /// The inputs: `C_XIN = 15 pF`.
    pub inputs: Vec<String>,
    /// `expr` evaluated from the inputs, in `unit`.
    pub computed: Option<String>,
    /// The source's own result.
    pub stated: Option<String>,
    /// What it is compared with (`X1 load_capacitance`); never pass/fail.
    pub target: Option<String>,
    pub cite: Option<CircuitCite>,
}

/// A circuit as the part page shows it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct CircuitView {
    pub id: String,
    pub title: String,
    /// The most cited source and the pages cited from it.
    pub source: Option<CircuitCite>,
    pub pages: String,
    pub document_type: Option<DocumentType>,
    pub reviewed: bool,
    /// Roles and connections by level.
    pub required: u32,
    pub recommended: u32,
    pub example: u32,
    pub optional: u32,
    pub roles: Vec<CircuitRoleRow>,
    pub connects: Vec<CircuitConnectRow>,
    pub formulas: Vec<CircuitFormulaRow>,
    /// Circuits this one is made of.
    pub includes: Vec<String>,
    /// The board.yaml `use:` entry (`host:` to fill in).
    pub use_line: String,
}

fn host_label(h: &CircuitHost) -> String {
    match h {
        CircuitHost::Device(d) => d.0.clone(),
        CircuitHost::Match(m) => {
            let mut parts: Vec<String> = m.devices.iter().map(|d| d.0.clone()).collect();
            if let Some(c) = m.category {
                parts.push(c.name().to_string());
            }
            if !m.has_pins.is_empty() {
                parts.push(format!("pins {}", m.has_pins.join(", ")));
            }
            parts.join("; ")
        }
    }
}

/// `GET /v1/circuits`
async fn list_circuits(State(store): State<Arc<Store>>) -> Json<Vec<CircuitSummary>> {
    Json(
        store
            .dataset
            .circuits
            .values()
            .map(|c| CircuitSummary {
                id: c.id.clone(),
                title: c.title.clone(),
                host: host_label(&c.host),
                reviewed: c.review.is_some(),
            })
            .collect(),
    )
}

/// `GET /v1/circuits/{publisher}/{name}`
async fn get_circuit(
    State(store): State<Arc<Store>>,
    Path((publisher, name)): Path<(String, String)>,
) -> Result<Json<Circuit>, ApiError> {
    let id = format!("{publisher}/{name}");
    store
        .dataset
        .circuits
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::NotFound(format!("no circuit {id}")))
}

/// `GET /v1/parts/{manufacturer}/{mpn}/circuits`
async fn part_circuits(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<Vec<CircuitView>>, ApiError> {
    let ds = &store.dataset;
    let part = ds
        .part(&manufacturer, &mpn)
        .ok_or_else(|| ApiError::NotFound(format!("no part {manufacturer}/{mpn}")))?;
    let device = ds
        .devices
        .get(&part.device.0)
        .ok_or_else(|| ApiError::NotFound(format!("no device {}", part.device)))?;
    let mut circuits = ds.circuits_for(device);
    // Building blocks first, composites after.
    circuits.sort_by_key(|c| (!c.includes.is_empty(), c.id.clone()));
    Ok(Json(
        circuits
            .into_iter()
            .map(|c| {
                view(ds, c, |name| {
                    device.pins.values().filter(|p| p.name == name).count()
                })
            })
            .collect(),
    ))
}

fn cite_of(ds: &Dataset, e: &ProvenanceEntry) -> CircuitCite {
    let source = ds.sources.get(&e.source.0);
    let label = source.map_or_else(
        || e.source.0.clone(),
        |s| format!("{} rev {}", s.document_id, s.revision),
    );
    let url = source.map(|s| match e.page {
        Some(p) if !s.url.ends_with(".zip") => format!("{}#page={p}", s.url),
        _ => s.url.clone(),
    });
    CircuitCite {
        source: e.source.0.clone(),
        label,
        page: e.page,
        locator: e.locator.clone(),
        url,
    }
}

/// The first citation at `ptr` or the nearest ancestor.
fn cite_at(ds: &Dataset, c: &Circuit, ptr: &str) -> Option<CircuitCite> {
    let mut p = ptr.to_string();
    loop {
        if let Some(e) = c.provenance.get(&p).and_then(|l| l.first()) {
            return Some(cite_of(ds, e));
        }
        match p.rfind('/') {
            Some(0) | None => return None,
            Some(i) => p.truncate(i),
        }
    }
}

fn esc(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn value_label(key: &str, v: &ParameterValue) -> String {
    match parameter_spec(key).and_then(|spec| v.normalize(spec).ok().map(|n| n.label(spec))) {
        Some(l) => l,
        None => v.value.clone().unwrap_or_default(),
    }
}

/// `100 nF`; `5.1 kΩ, tolerance ≤ +20%`; a part id.
fn requirement_label(r: &Requirement) -> String {
    if let Some(p) = &r.part {
        return p.0.clone();
    }
    const PRIMARY: &[&str] = &[
        "capacitance",
        "resistance",
        "inductance",
        "capacity",
        "frequency",
    ];
    let mut out: Vec<String> = Vec::new();
    for (k, v) in &r.parameters {
        let label = value_label(k, v);
        if PRIMARY.contains(&k.as_str()) {
            out.insert(0, label);
        } else {
            out.push(format!("{} {label}", k.replace('_', " ")));
        }
    }
    if out.is_empty() {
        if let Some(c) = &r.category {
            return c
                .list()
                .iter()
                .map(|c| c.name())
                .collect::<Vec<_>>()
                .join(" / ");
        }
    }
    out.join(", ")
}

fn level_of(l: &Level) -> (LevelValue, Option<String>) {
    (l.value, l.word.clone())
}

/// Displays an SI value in `unit` (`10.5 pF`).
fn in_unit(si: f64, unit: Option<&str>) -> String {
    let factor = unit.and_then(|u| {
        openparts_core::parameter::Quantity::ALL
            .iter()
            .find_map(|q| q.unit_factor(u))
    });
    match (unit, factor) {
        (Some(u), Some(f)) => format!("{} {u}", openparts_core::parameter::plain_number(si / f)),
        _ => openparts_core::parameter::plain_number(si),
    }
}

fn literal_label(v: &ParameterValue) -> String {
    let n = v.typ.or(v.max).or(v.min).unwrap_or_default();
    format!(
        "{} {}",
        openparts_core::parameter::plain_number(n),
        v.unit.as_deref().unwrap_or("")
    )
    .trim()
    .to_string()
}

pub fn view(ds: &Dataset, c: &Circuit, pins_named: impl Fn(&str) -> usize) -> CircuitView {
    let mut counts = [0u32; 4];
    let mut count = |l: LevelValue| {
        counts[match l {
            LevelValue::Required => 0,
            LevelValue::Recommended => 1,
            LevelValue::Example => 2,
            LevelValue::Optional => 3,
        }] += 1;
    };
    let roles: Vec<CircuitRoleRow> = c
        .roles
        .iter()
        .map(|(name, r)| {
            count(r.level.value);
            let (level, word) = level_of(&r.level);
            let connects = r
                .pins
                .values()
                .map(|t| match target(t) {
                    Target::ThisPin => r.per_pin.clone().unwrap_or_default(),
                    Target::HostPin(p) => p.to_string(),
                    Target::Net(n) => n.to_string(),
                })
                .collect();
            let at = format!("/roles/{}", esc(name));
            CircuitRoleRow {
                role: name.clone(),
                count: r.instances(&pins_named) as u32,
                value: requirement_label(&r.requires),
                connects,
                level,
                word,
                cite: cite_at(ds, c, &format!("{at}/level"))
                    .or_else(|| cite_at(ds, c, &format!("{at}/requires"))),
            }
        })
        .collect();
    let connects: Vec<CircuitConnectRow> = c
        .connect
        .iter()
        .map(|(pin, k)| {
            count(k.level());
            CircuitConnectRow {
                pin: pin.clone(),
                net: if k.no_connect {
                    "NC".into()
                } else {
                    k.net.clone().unwrap_or_default()
                },
                level: k.level(),
                word: k.level.as_ref().and_then(|l| l.word.clone()),
                cite: cite_at(ds, c, &format!("/connect/{}", esc(pin))),
            }
        })
        .collect();
    let formulas = c
        .formulas
        .iter()
        .map(|(name, f)| {
            let unit = f
                .unit
                .as_deref()
                .or(f.result.as_ref().and_then(|r| r.unit.as_deref()));
            let inputs = f
                .inputs
                .iter()
                .map(|(k, i)| match i {
                    FormulaInput::Literal(v) => format!("{k} = {}", literal_label(v)),
                    FormulaInput::Role(rp) => {
                        let v = c
                            .roles
                            .get(&rp.role)
                            .and_then(|r| r.requires.parameters.get(&rp.param))
                            .map(|v| value_label(&rp.param, v))
                            .unwrap_or_else(|| "?".into());
                        if *k == rp.role {
                            format!("{k} = {v}")
                        } else {
                            format!("{k} = {v} ({} {})", rp.role, rp.param)
                        }
                    }
                })
                .collect();
            CircuitFormulaRow {
                name: name.clone(),
                expr: f.expr.clone(),
                inputs,
                computed: c.evaluate(f).map(|v| in_unit(v, unit)),
                stated: f.result.as_ref().map(literal_label),
                target: f.target.as_ref().map(|t| {
                    let expected = c
                        .roles
                        .get(&t.role)
                        .and_then(|r| r.expects.get(&t.param))
                        .map(|v| format!(" {}", value_label(&t.param, v)))
                        .unwrap_or_default();
                    format!("{} {}{expected}", t.role, t.param)
                }),
                cite: cite_at(ds, c, &format!("/formulas/{}", esc(name))),
            }
        })
        .collect();

    // The most cited source and the pages cited from it.
    let mut by_source: BTreeMap<&str, (usize, Vec<u32>, &ProvenanceEntry)> = BTreeMap::new();
    for e in c.provenance.values().flatten() {
        let s = by_source.entry(&e.source.0).or_insert((0, Vec::new(), e));
        s.0 += 1;
        if let Some(p) = e.page {
            s.1.push(p);
        }
    }
    let main = by_source
        .iter()
        .max_by(|a, b| a.1 .0.cmp(&b.1 .0).then(b.0.cmp(a.0)))
        .map(|(_, v)| v);
    // `p8–14`, or the pages themselves when they are far apart.
    let pages = main
        .map(|(_, pages, _)| {
            let mut pages = pages.clone();
            pages.sort_unstable();
            pages.dedup();
            match (pages.first(), pages.last()) {
                (Some(a), Some(b)) if a == b => format!("p{a}"),
                (Some(a), Some(b)) if b - a <= 10 => format!("p{a}–{b}"),
                _ => format!(
                    "p{}",
                    pages
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
        })
        .unwrap_or_default();
    let source = main.map(|(_, _, e)| {
        let mut cite = cite_of(
            ds,
            &ProvenanceEntry {
                source: e.source.clone(),
                page: None,
                locator: None,
            },
        );
        cite.page = None;
        cite
    });
    let document_type =
        main.and_then(|(_, _, e)| ds.sources.get(&e.source.0).map(|s| s.document_type));
    CircuitView {
        id: c.id.clone(),
        title: c.title.clone(),
        source,
        pages,
        document_type,
        reviewed: c.review.is_some(),
        required: counts[0],
        recommended: counts[1],
        example: counts[2],
        optional: counts[3],
        roles,
        connects,
        formulas,
        includes: c.includes.iter().map(|i| i.circuit.clone()).collect(),
        use_line: format!("{{ circuit: {}, host: <REF> }}", c.id),
    }
}

#[cfg(feature = "schema")]
pub fn subschemas(gen: &mut schemars::SchemaGenerator) {
    gen.subschema_for::<CircuitView>();
    gen.subschema_for::<CircuitSummary>();
}
