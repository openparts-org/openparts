//! Reference circuits (Canonical Data Specification section 41):
//!
//! - `GET /v1/circuits`: every circuit (id, title, host, review state);
//! - `GET /v1/circuits/{publisher}/{name}`: one circuit document as
//!   stored (in-memory shape: quantities `typ`/`unit`, a `provenance`
//!   map) -- what the board tool expands;
//! - `GET /v1/parts/{manufacturer}/{mpn}/circuits`: the circuits whose
//!   host is the part's device, as display rows ([`CircuitView`]) for
//!   the part page, each with its schematic ([`crate::circuit_diagram`]);
//! - `GET /v1/circuits/{publisher}/{name}/diagram.svg`: the schematic
//!   on its own (on its host device, or on the pin names a generic
//!   host must have).

use crate::circuit_diagram::{self, Connection, Element, End, Scene, Sym};
use crate::error::ApiError;
use crate::store::Store;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use openparts_core::circuit::{target, NetKind, Role, Target};
use openparts_core::{
    parameter_spec, Circuit, CircuitHost, ComponentCategory, Device, DocumentType, FormulaInput,
    Level, LevelValue, ParameterValue, ProvenanceEntry, Requirement,
};
use openparts_data::format::pin_order;
use openparts_data::Dataset;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn router() -> Router<Arc<Store>> {
    Router::new()
        .route("/v1/circuits", get(list_circuits))
        .route("/v1/circuits/:publisher/:name", get(get_circuit))
        .route(
            "/v1/circuits/:publisher/:name/diagram.svg",
            get(circuit_diagram),
        )
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
    /// The schematic (SVG, styled by the page it is inlined in);
    /// `None` when there is nothing to draw.
    pub diagram: Option<String>,
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
            .map(|c| view(ds, c, device, &part.mpn))
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

/// `GET /v1/circuits/{publisher}/{name}/diagram.svg`
async fn circuit_diagram(
    State(store): State<Arc<Store>>,
    Path((publisher, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let id = format!("{publisher}/{name}");
    let ds = &store.dataset;
    let c = ds
        .circuits
        .get(&id)
        .ok_or_else(|| ApiError::NotFound(format!("no circuit {id}")))?;
    let device = match &c.host {
        CircuitHost::Device(d) => ds.devices.get(&d.0),
        CircuitHost::Match(m) => m.devices.first().and_then(|d| ds.devices.get(&d.0)),
    };
    let scene = match device {
        Some(device) => {
            let label = device.id.0.rsplit('/').next().unwrap_or_default();
            scene(ds, c, device, label)
        }
        // A generic host: the pin names it must have, without numbers.
        None => {
            let CircuitHost::Match(m) = &c.host else {
                return Err(ApiError::NotFound(format!("no host device for {id}")));
            };
            let pins: serde_json::Map<String, serde_json::Value> = m
                .has_pins
                .iter()
                .enumerate()
                .map(|(k, p)| {
                    (
                        format!("{k:03}"),
                        serde_json::json!({ "name": p, "type": "passive" }),
                    )
                })
                .collect();
            let device: Device = serde_json::from_value(serde_json::json!({
                "schema_version": "0.2",
                "kind": "device",
                "id": "generic/host",
                "manufacturer": "generic",
                "category": "other",
                "pins": pins,
            }))
            .map_err(|e| ApiError::GenerationFailed(e.to_string()))?;
            let label = m.category.map_or("host", |c| c.name()).replace('_', " ");
            let mut s = scene(ds, c, &device, &label);
            for p in &mut s.pins {
                p.0.clear();
            }
            s
        }
    };
    let svg = circuit_diagram::render(&scene, true);
    if svg.is_empty() {
        return Err(ApiError::NotFound(format!("nothing to draw for {id}")));
    }
    let mut res = svg.into_response();
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("image/svg+xml; charset=utf-8"),
    );
    Ok(res)
}

/// The symbol a role is drawn with: from its category (or its named
/// part's device category); parts with other than two pins are boxes.
fn symbol_of(ds: &Dataset, r: &Role) -> Sym {
    let mut cats: Vec<ComponentCategory> = r
        .requires
        .category
        .as_ref()
        .map(|c| c.list())
        .unwrap_or_default();
    if let Some((m, mpn)) = r.requires.part.as_ref().and_then(|p| p.0.split_once('/')) {
        if let Some(dev) = ds.part(m, mpn).and_then(|p| ds.devices.get(&p.device.0)) {
            cats.push(dev.category);
        }
    }
    let Some(c) = cats.first().copied() else {
        return Sym::Block;
    };
    let sym = if c.is_within(ComponentCategory::Crystal) {
        return Sym::Crystal;
    } else if c.is_within(ComponentCategory::Capacitor) {
        Sym::Capacitor
    } else if c.is_within(ComponentCategory::Resistor) {
        Sym::Resistor
    } else if c.is_within(ComponentCategory::Inductor) {
        Sym::Inductor
    } else if c.is_within(ComponentCategory::Diode) || c == ComponentCategory::Led {
        Sym::Diode
    } else if c.is_within(ComponentCategory::Switch) || c == ComponentCategory::Connector {
        Sym::Switch
    } else {
        Sym::Block
    };
    if r.pins.len() == 2 {
        sym
    } else {
        Sym::Block
    }
}

/// `Switching regulator`
fn category_title(r: &Role) -> Option<String> {
    let c = r.requires.category.as_ref()?.list().into_iter().next()?;
    let name = c.name().replace('_', " ");
    let mut chars = name.chars();
    Some(chars.next()?.to_uppercase().collect::<String>() + chars.as_str())
}

/// What a role's symbol is labelled with: its value, a named part's
/// MPN (and a crystal's frequency), a box's category.
fn value_lines(r: &Role, sym: Sym) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(p) = &r.requires.part {
        out.push(p.0.rsplit('/').next().unwrap_or(&p.0).to_string());
        if let Some(f) = r.expects.get("frequency") {
            out.push(value_label("frequency", f));
        }
        return out;
    }
    if sym == Sym::Block {
        out.extend(category_title(r));
    }
    if !r.requires.parameters.is_empty() {
        // One line per parameter: `≥ 4.7 µF` / `ESR 33 mΩ–2.2 Ω`.
        for line in requirement_label(&r.requires).split(", ") {
            out.push(match line.strip_prefix("esr ") {
                Some(rest) => format!("ESR {rest}"),
                None => line.to_string(),
            });
        }
    }
    out
}

/// `C_IOVDD · 100 nF ×6` and `should · p152` on the next line.
fn tip_of(ds: &Dataset, c: &Circuit, name: &str, r: &Role, count: usize) -> String {
    let at = format!("/roles/{}", esc(name));
    let cite = cite_at(ds, c, &format!("{at}/level"))
        .or_else(|| cite_at(ds, c, &format!("{at}/requires")));
    let mut value = requirement_label(&r.requires);
    if count > 1 {
        value.push_str(&format!(" ×{count}"));
    }
    let word = r
        .level
        .word
        .clone()
        .unwrap_or_else(|| r.level.value.as_str().to_string());
    let page = cite
        .and_then(|c| c.page)
        .map(|p| format!(" · p{p}"))
        .unwrap_or_default();
    format!("{name} · {value}\n{word}{page}")
}

/// Adds circuit `c`'s nets, connections and roles (but `skip`) on host
/// `device`, its nets renamed by `rename`.
fn add_to_scene(
    s: &mut Scene,
    ds: &Dataset,
    c: &Circuit,
    device: &Device,
    rename: &BTreeMap<String, String>,
    skip: Option<&str>,
) {
    let net = |n: &str| rename.get(n).cloned().unwrap_or_else(|| n.to_string());
    for (name, k) in &c.nets {
        match k.kind {
            NetKind::Ground => {
                s.ground.insert(net(name));
            }
            NetKind::Power => {
                s.power.insert(net(name));
            }
            NetKind::Local => {
                s.local.insert(net(name));
            }
            NetKind::Signal => {}
        }
    }
    for (pin, k) in &c.connect {
        let to = if k.no_connect {
            None
        } else {
            k.net.as_deref().map(net)
        };
        let word = k
            .level
            .as_ref()
            .and_then(|l| l.word.clone())
            .unwrap_or_else(|| k.level().as_str().to_string());
        let page = cite_at(ds, c, &format!("/connect/{}", esc(pin)))
            .and_then(|c| c.page)
            .map(|p| format!(" · p{p}"))
            .unwrap_or_default();
        s.connects.push(Connection {
            pin: pin.clone(),
            tip: format!("{pin} → {}\n{word}{page}", to.as_deref().unwrap_or("NC")),
            net: to,
            level: k.level(),
        });
    }
    let pins_named = |name: &str| device.pins.values().filter(|p| p.name == name).count();
    for (name, r) in &c.roles {
        if Some(name.as_str()) == skip {
            continue;
        }
        let sym = symbol_of(ds, r);
        let count = r.instances(pins_named);
        let terms = r
            .pins
            .iter()
            .map(|(pin, t)| {
                let end = match target(t) {
                    Target::ThisPin => End::Pin(r.per_pin.clone().unwrap_or_default()),
                    Target::HostPin(p) => End::Pin(p.to_string()),
                    Target::Net(n) => End::Net(net(n)),
                };
                (pin.clone(), end)
            })
            .collect();
        s.elements.push(Element {
            role: name.clone(),
            sym,
            value: value_lines(r, sym),
            count,
            level: r.level.value,
            tip: tip_of(ds, c, name, r, count),
            terms,
        });
    }
}

/// The drawing of `c` on `device` (titled `label`). A composite with a
/// part of its own hosting an included circuit (the minimal design's
/// LDO) is drawn around that part; its other includes have drawings
/// of their own.
pub fn scene(ds: &Dataset, c: &Circuit, device: &Device, label: &str) -> Scene {
    let mut s = Scene::default();
    let hosted = c.includes.iter().find_map(|inc| {
        let role = &inc.host.as_ref()?.role;
        let part = c.roles.get(role)?.requires.part.as_ref()?;
        let (m, mpn) = part.0.split_once('/')?;
        let part = ds.part(m, mpn)?;
        let dev = ds.devices.get(&part.device.0)?;
        let child = ds.circuits.get(&inc.circuit)?;
        Some((role, part.mpn.clone(), dev, child, &inc.nets))
    });
    let host = match hosted {
        Some((role, mpn, dev, child, rename)) => {
            add_to_scene(&mut s, ds, child, dev, rename, None);
            add_to_scene(&mut s, ds, c, device, &BTreeMap::new(), Some(role));
            s.host = mpn;
            dev
        }
        None => {
            add_to_scene(&mut s, ds, c, device, &BTreeMap::new(), None);
            s.host = label.to_string();
            device
        }
    };
    let mut pins: Vec<(String, String)> = host
        .pins
        .iter()
        .map(|(num, p)| (num.clone(), p.name.clone()))
        .collect();
    pins.sort_by(|a, b| pin_order(&a.0, &b.0));
    s.pins = pins;
    s
}

pub fn view(ds: &Dataset, c: &Circuit, device: &Device, host_label: &str) -> CircuitView {
    let pins_named = |name: &str| device.pins.values().filter(|p| p.name == name).count();
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
                count: r.instances(pins_named) as u32,
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
        diagram: {
            let svg = circuit_diagram::render(&scene(ds, c, device, host_label), false);
            (!svg.is_empty()).then_some(svg)
        },
    }
}

#[cfg(feature = "schema")]
pub fn subschemas(gen: &mut schemars::SchemaGenerator) {
    gen.subschema_for::<CircuitView>();
    gen.subschema_for::<CircuitSummary>();
}
