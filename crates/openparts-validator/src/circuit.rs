//! Reference circuit checks (Canonical Data Specification section 41):
//! the host and its pins exist, the roles' parameter keys and units are
//! the catalog's, named parts and included circuits exist, every pin
//! target and net is declared, formulas have their inputs, and every
//! fact cites its source.

use crate::cross::pointer_escape;
use crate::provenance::check_provenance;
use crate::Diagnostic;
use openparts_core::circuit::{target, Expr, Target};
use openparts_core::{
    parameter::plausible_for, parameter_spec, Circuit, CircuitHost, ComponentCategory, Device,
    FormulaInput, Level, LevelValue, ParameterError, ParameterValue, SourceId,
};
use std::collections::{BTreeMap, BTreeSet};

/// What a circuit is checked against: the rest of the dataset.
pub struct CircuitContext<'a> {
    pub devices: &'a BTreeMap<String, Device>,
    pub parts: &'a BTreeSet<String>,
    pub sources: &'a BTreeSet<SourceId>,
    pub circuits: &'a BTreeMap<String, Circuit>,
}

/// Every check of one circuit.
pub fn validate_circuit(c: &Circuit, ctx: &CircuitContext) -> Vec<Diagnostic> {
    let entity = format!("circuit:{}", c.id);
    let mut v = Check {
        entity: &entity,
        out: Vec::new(),
    };
    v.out
        .extend(check_provenance(&entity, &c.provenance, ctx.sources));
    v.out
        .extend(crate::review::check_review(&entity, c.review.as_ref()));
    if c.title.trim().is_empty() {
        v.error("circuit.invalid", "/title", "title is empty".into());
    }
    if c.id.split('/').count() != 2 {
        v.error(
            "circuit.invalid",
            "/id",
            format!("id \"{}\" is not <publisher>/<name>", c.id),
        );
    }

    // The host's pin names (None: any name is possible, e.g. a device
    // the dataset doesn't have -- reported).
    let host_pins = host_pins(&mut v, &c.host, ctx);
    let has_pin = |name: &str| host_pins.as_ref().is_none_or(|p| p.contains(name));

    for (name, net) in &c.nets {
        if let Some(volts) = &net.voltage {
            let at = format!("/nets/{}", pointer_escape(name));
            v.quantity("output_voltage", volts, &format!("{at}/voltage"));
            v.cited(c, &at);
        }
    }
    for (name, value) in &c.conditions {
        let at = format!("/conditions/{}", pointer_escape(name));
        if !has_pin(name) && !c.nets.contains_key(name) {
            v.error(
                "circuit.pin",
                &at,
                format!("condition on \"{name}\": neither a host pin nor a net"),
            );
        }
        v.any_quantity(value, &at);
        v.cited(c, &at);
    }
    for (pin, conn) in &c.connect {
        let at = format!("/connect/{}", pointer_escape(pin));
        if !has_pin(pin) {
            v.error(
                "circuit.pin",
                &at,
                format!("the host has no pin named \"{pin}\""),
            );
        }
        match (&conn.net, conn.no_connect) {
            (Some(net), false) => v.net(c, net, &at),
            (None, true) => {}
            _ => v.error(
                "circuit.connection",
                &at,
                "a connection takes `net` or `no_connect: true`".into(),
            ),
        }
        if let Some(l) = &conn.level {
            v.level(l, &format!("{at}/level"));
        }
        v.cited(c, &at);
    }

    for (name, role) in &c.roles {
        let at = format!("/roles/{}", pointer_escape(name));
        if let Some(p) = &role.per_pin {
            if !has_pin(p) {
                v.error(
                    "circuit.pin",
                    &format!("{at}/per_pin"),
                    format!("the host has no pin named \"{p}\""),
                );
            }
            if role.count.is_some() {
                v.error(
                    "circuit.invalid",
                    &at,
                    "`per_pin` and `count` exclude each other".into(),
                );
            }
        }
        let r = &role.requires;
        let categories = r.category.as_ref().map(|c| c.list()).unwrap_or_default();
        if r.part.is_none() && categories.is_empty() {
            v.error(
                "circuit.invalid",
                &format!("{at}/requires"),
                "`requires` names a `category` or a `part`".into(),
            );
        }
        if let Some(part) = &r.part {
            v.part(ctx, &part.0, &format!("{at}/requires/part"));
        }
        for (key, value) in &r.parameters {
            v.parameter(key, value, &format!("{at}/requires"), &categories);
        }
        v.cited(c, &format!("{at}/requires"));
        v.level(&role.level, &format!("{at}/level"));
        v.cited(c, &format!("{at}/level"));
        for (pin, to) in &role.pins {
            let p = format!("{at}/pins/{}", pointer_escape(pin));
            match target(to) {
                Target::ThisPin if role.per_pin.is_none() => {
                    v.error("circuit.connection", &p, "`$pin` needs `per_pin`".into())
                }
                Target::ThisPin => {}
                Target::HostPin(h) if !has_pin(h) => v.error(
                    "circuit.pin",
                    &p,
                    format!("the host has no pin named \"{h}\""),
                ),
                Target::HostPin(_) => {}
                Target::Net(n) => v.net(c, n, &p),
            }
        }
        if let Some(place) = &role.place {
            let p = format!("{at}/place");
            match target(&place.near) {
                Target::ThisPin if role.per_pin.is_none() => {
                    v.error("circuit.connection", &p, "`$pin` needs `per_pin`".into())
                }
                Target::HostPin(h) if !has_pin(h) => v.error(
                    "circuit.pin",
                    &p,
                    format!("the host has no pin named \"{h}\""),
                ),
                Target::Net(n) if !c.roles.contains_key(n) => v.error(
                    "circuit.connection",
                    &p,
                    format!("`near: {n}`: not `$pin`, `host.<PIN>` or a role"),
                ),
                _ => {}
            }
            v.cited(c, &p);
        }
        if let Some(ex) = &role.example {
            let p = format!("{at}/example");
            if let Some(part) = &ex.part {
                v.part(ctx, &part.0, &format!("{p}/part"));
            }
            for (key, value) in &ex.parameters {
                v.parameter(key, value, &p, &categories);
            }
            v.cited(c, &p);
        }
        if !role.expects.is_empty() {
            let p = format!("{at}/expects");
            for (key, value) in &role.expects {
                v.parameter(key, value, &p, &categories);
            }
            v.cited(c, &p);
        }
    }

    for (name, f) in &c.formulas {
        let at = format!("/formulas/{}", pointer_escape(name));
        match Expr::parse(&f.expr) {
            Err(e) => v.error("circuit.formula", &format!("{at}/expr"), e),
            Ok(expr) => {
                for var in expr.vars() {
                    if !f.inputs.contains_key(var) {
                        v.error(
                            "circuit.formula",
                            &format!("{at}/expr"),
                            format!("\"{var}\" is not one of the inputs"),
                        );
                    }
                }
                for name in f.inputs.keys() {
                    if !expr.vars().contains(&name.as_str()) {
                        v.warning(
                            "circuit.formula",
                            &format!("{at}/inputs"),
                            format!("input \"{name}\" is not used"),
                        );
                    }
                }
            }
        }
        for (input, value) in &f.inputs {
            let p = format!("{at}/inputs/{}", pointer_escape(input));
            match value {
                FormulaInput::Literal(q) => {
                    v.any_quantity(q, &p);
                    v.cited(c, &p);
                }
                FormulaInput::Role(rp) => v.role_param(c, &rp.role, &rp.param, &p),
            }
        }
        if let Some(t) = &f.target {
            v.role_param(c, &t.role, &t.param, &format!("{at}/target"));
        }
        if let Some(r) = &f.result {
            v.any_quantity(r, &format!("{at}/result"));
            v.cited(c, &format!("{at}/result"));
        }
        v.cited(c, &at);
    }

    for (i, hint) in c.placement.iter().enumerate() {
        let at = format!("/placement/{i}");
        for t in hint.keep_short.iter().chain(&hint.short_stub) {
            v.reference(c, t, &has_pin, &at);
        }
        if hint.keep_short.is_empty() && hint.short_stub.is_empty() {
            v.error(
                "circuit.invalid",
                &at,
                "a hint names what it is about".into(),
            );
        }
        if let Some(m) = &hint.max_length {
            v.length(m, &format!("{at}/max_length"));
        }
        if let Some(l) = &hint.level {
            v.level(l, &format!("{at}/level"));
        }
        v.cited(c, &at);
    }
    for (i, hint) in c.routing.iter().enumerate() {
        let at = format!("/routing/{i}");
        if hint.diff_pair.len() != 2 {
            v.error(
                "circuit.invalid",
                &format!("{at}/diff_pair"),
                "a diff pair names two nets".into(),
            );
        }
        for t in &hint.diff_pair {
            v.reference(c, t, &has_pin, &at);
        }
        if let Some(n) = &hint.reference_plane {
            v.net(c, n, &format!("{at}/reference_plane"));
        }
        if let Some(z) = &hint.impedance {
            v.quantity("resistance", z, &format!("{at}/impedance"));
        }
        v.cited(c, &at);
    }

    for (i, inc) in c.includes.iter().enumerate() {
        let at = format!("/includes/{i}");
        match ctx.circuits.get(&inc.circuit) {
            None => v.error(
                "circuit.include",
                &at,
                format!("no circuit \"{}\"", inc.circuit),
            ),
            Some(child) => {
                for (theirs, ours) in &inc.nets {
                    if !child.nets.contains_key(theirs) {
                        v.error(
                            "circuit.include",
                            &format!("{at}/with"),
                            format!("{} has no net \"{theirs}\"", inc.circuit),
                        );
                    }
                    v.net(c, ours, &format!("{at}/with"));
                }
                if inc.host.is_none() && child.host != c.host {
                    v.error(
                        "circuit.include",
                        &at,
                        format!("{} is for another host", inc.circuit),
                    );
                }
            }
        }
        if inc.circuit == c.id {
            v.error("circuit.include", &at, "a circuit includes itself".into());
        }
        if let Some(h) = &inc.host {
            if !c.roles.contains_key(&h.role) {
                v.error(
                    "circuit.include",
                    &format!("{at}/host"),
                    format!("no role \"{}\"", h.role),
                );
            }
        }
        v.cited(c, &at);
    }
    v.out
}

/// `circuits/<publisher>/<name>.yaml` for id `<publisher>/<name>`.
pub fn validate_circuit_location(id: &str, rel_path: &str) -> Vec<Diagnostic> {
    let path = rel_path.replace('\\', "/");
    if path == format!("circuits/{id}.yaml") {
        return vec![];
    }
    vec![Diagnostic::error(
        "id.location",
        format!("circuit:{id}"),
        Some("/id"),
        format!("id {id} doesn't match its file {path} (circuits/{id}.yaml)"),
    )]
}

fn host_pins(v: &mut Check, host: &CircuitHost, ctx: &CircuitContext) -> Option<BTreeSet<String>> {
    let names =
        |d: &Device| -> BTreeSet<String> { d.pins.values().map(|p| p.name.clone()).collect() };
    match host {
        CircuitHost::Device(d) => match ctx.devices.get(&d.0) {
            Some(dev) => Some(names(dev)),
            None => {
                v.error("circuit.host", "/host", format!("no device \"{d}\""));
                None
            }
        },
        CircuitHost::Match(m) => {
            if m.devices.is_empty() && m.category.is_none() && m.has_pins.is_empty() {
                v.error(
                    "circuit.host",
                    "/host",
                    "a host match names devices, a category or pins".into(),
                );
            }
            let mut common: Option<BTreeSet<String>> = None;
            for d in &m.devices {
                match ctx.devices.get(&d.0) {
                    Some(dev) => {
                        let n = names(dev);
                        common = Some(match common {
                            None => n,
                            Some(c) => c.intersection(&n).cloned().collect(),
                        });
                    }
                    None => v.error("circuit.host", "/host", format!("no device \"{d}\"")),
                }
            }
            if m.devices.is_empty() {
                // A generic host: only the pins it requires are known.
                return Some(m.has_pins.iter().cloned().collect());
            }
            common
        }
    }
}

struct Check<'a> {
    entity: &'a str,
    out: Vec<Diagnostic>,
}

impl Check<'_> {
    fn error(&mut self, rule: &'static str, at: &str, message: String) {
        self.out
            .push(Diagnostic::error(rule, self.entity, Some(at), message));
    }

    fn warning(&mut self, rule: &'static str, at: &str, message: String) {
        self.out
            .push(Diagnostic::warning(rule, self.entity, Some(at), message));
    }

    /// A fact at `at` has evidence: at its pointer or an ancestor's.
    fn cited(&mut self, c: &Circuit, at: &str) {
        let mut p = at.to_string();
        loop {
            if c.provenance.contains_key(&p) {
                return;
            }
            match p.rfind('/') {
                Some(0) | None => break,
                Some(i) => p.truncate(i),
            }
        }
        self.error(
            "provenance.missing",
            at,
            format!("{at} has no `src`: every fact of a circuit cites its source"),
        );
    }

    fn level(&mut self, l: &Level, at: &str) {
        if let Some(w) = &l.word {
            let lw = w.to_lowercase();
            if !l.value.words().contains(&lw.as_str()) {
                let allowed = l.value.words().join(", ");
                self.error(
                    "circuit.level",
                    at,
                    if l.value == LevelValue::Example {
                        format!("`example` has no modal word, found \"{w}\"")
                    } else {
                        format!(
                            "\"{w}\" is not a word for `{}` ({allowed})",
                            l.value.as_str()
                        )
                    },
                );
            }
        }
    }

    fn net(&mut self, c: &Circuit, net: &str, at: &str) {
        if !c.nets.contains_key(net) {
            self.error(
                "circuit.connection",
                at,
                format!("\"{net}\" is not a net of the circuit (declare it under `nets`)"),
            );
        }
    }

    fn part(&mut self, ctx: &CircuitContext, part: &str, at: &str) {
        if !ctx.parts.contains(part) {
            self.error(
                "circuit.part",
                at,
                format!("no part \"{part}\" in the dataset"),
            );
        }
    }

    /// A placement or routing reference: `host.<PIN>`, a net or a role.
    fn reference(&mut self, c: &Circuit, t: &str, has_pin: &dyn Fn(&str) -> bool, at: &str) {
        match target(t) {
            Target::HostPin(h) if !has_pin(h) => self.error(
                "circuit.pin",
                at,
                format!("the host has no pin named \"{h}\""),
            ),
            Target::Net(n) if !c.nets.contains_key(n) && !c.roles.contains_key(n) => self.error(
                "circuit.connection",
                at,
                format!("\"{n}\" is not a net or role of the circuit"),
            ),
            Target::ThisPin => self.error("circuit.connection", at, "`$pin` here".into()),
            _ => {}
        }
    }

    fn role_param(&mut self, c: &Circuit, role: &str, param: &str, at: &str) {
        let Some(r) = c.roles.get(role) else {
            self.error("circuit.formula", at, format!("no role \"{role}\""));
            return;
        };
        if parameter_spec(param).is_none() {
            self.error(
                "parameter.unknown_key",
                at,
                format!("\"{param}\" is not a parameter key"),
            );
            return;
        }
        let stated = r.requires.parameters.contains_key(param)
            || r.expects.contains_key(param)
            || r.requires.part.is_some();
        if !stated {
            self.error(
                "circuit.formula",
                at,
                format!(
                    "role {role} states no \"{param}\" (in `requires`/`expects`) and names no part"
                ),
            );
        }
    }

    fn parameter(
        &mut self,
        key: &str,
        value: &ParameterValue,
        at: &str,
        categories: &[ComponentCategory],
    ) {
        let p = format!("{at}/{}", pointer_escape(key));
        let Some(spec) = parameter_spec(key) else {
            self.error(
                "parameter.unknown_key",
                &p,
                format!("\"{key}\" is not a parameter key of the vocabulary (section 11.1)"),
            );
            return;
        };
        if let Err(e) = value.normalize(spec) {
            let rule = match e {
                ParameterError::MissingUnit | ParameterError::IncompatibleUnit { .. } => {
                    "parameter.unit"
                }
                ParameterError::OutOfOrder => "parameter.out_of_range",
                _ => "parameter.invalid",
            };
            self.error(rule, &p, format!("\"{key}\": {e}"));
        }
        if !categories.is_empty() && !categories.iter().any(|c| plausible_for(spec, *c)) {
            self.warning(
                "parameter.implausible",
                &p,
                format!("\"{key}\" is unusual for {categories:?}"),
            );
        }
    }

    fn length(&mut self, v: &ParameterValue, at: &str) {
        let ok = v.unit.as_deref().is_some_and(|u| {
            openparts_core::parameter::Quantity::Length
                .unit_factor(u)
                .is_some()
        });
        if !ok {
            self.error("parameter.unit", at, "a length (mm)".into());
        }
    }

    /// A value of the quantity of parameter `key`.
    fn quantity(&mut self, key: &str, v: &ParameterValue, at: &str) {
        if let Some(spec) = parameter_spec(key) {
            if let Err(e) = v.normalize(spec) {
                self.error("parameter.unit", at, format!("{e}"));
            }
        }
    }

    /// A literal of any quantity: its unit must be known.
    fn any_quantity(&mut self, v: &ParameterValue, at: &str) {
        if openparts_core::circuit::si_value(v, None).is_none() {
            self.error(
                "parameter.unit",
                at,
                "a value with a known unit (\"3 pF\")".into(),
            );
        }
    }
}
