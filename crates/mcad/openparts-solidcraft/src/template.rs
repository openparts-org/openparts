//! Script fragments written as YAML templates (`templates/*.yaml`).
//!
//! A fragment is a list of SolidCraft commands, one `- command.id: {params}`
//! per item -- the same shape as a SolidCraft script -- with `{{var}}`
//! placeholders the Rust side fills in. Fragments have no loops or
//! conditions: the family builders decide which fragments to use, how
//! often and with which variables.
//!
//! - `"{{var}}"` as a whole value becomes the variable's value as is (an
//!   array, a number, ...); inside a longer string it is spliced in as
//!   text. YAML reads a bare `{{` as a mapping, so quote such values.
//! - A placeholder with no variable is an error, never left in a script.
//! - The pseudo-command `include` inserts another fragment: `include: name`
//!   with the same variables, or `include: {fragment: name, var: value}`
//!   with some added or overridden.
//! - The pseudo-command `params` declares design parameters in order:
//!   `name: [expression, comment]`, or `[expression, comment, unit]`
//!   (`unit: ""` for a count). A parameter declared again by a later
//!   fragment keeps its first definition, so fragments can each declare
//!   what they rely on.

use crate::Script;
use serde_json::Value;

/// A template compiled into the crate.
#[derive(Debug, Clone, Copy)]
pub struct Fragment {
    pub name: &'static str,
    pub source: &'static str,
}

/// `fragment!("gullwing_row")` embeds `templates/gullwing_row.yaml`.
macro_rules! fragment {
    ($name:literal) => {
        $crate::template::Fragment {
            name: $name,
            source: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/templates/",
                $name,
                ".yaml"
            )),
        }
    };
}
pub(crate) use fragment;

/// Every template, by name (for `include`). A test checks this lists
/// every file in `templates/`.
pub const ALL: &[Fragment] = &[
    fragment!("axial"),
    fragment!("axial_lead"),
    fragment!("bga"),
    fragment!("body_params"),
    fragment!("can_sleeve_section"),
    fragment!("chip_ceramic"),
    fragment!("chip_crystal"),
    fragment!("chip_end_caps"),
    fragment!("chip_led"),
    fragment!("chip_molded"),
    fragment!("chip_params"),
    fragment!("chip_polarity_band"),
    fragment!("chip_power_inductor"),
    fragment!("chip_resistor"),
    fragment!("diode_band"),
    fragment!("dip"),
    fragment!("dip_details"),
    fragment!("dip_leg_row"),
    fragment!("electrolytic"),
    fragment!("flat_lead"),
    fragment!("flat_lead_row"),
    fragment!("gullwing_lead"),
    fragment!("gullwing_row"),
    fragment!("hc49"),
    fragment!("header_grid"),
    fragment!("header_notch_row"),
    fragment!("header_notches"),
    fragment!("header_params"),
    fragment!("header_pin"),
    fragment!("header_ra_pin"),
    fragment!("inlay"),
    fragment!("jbend_lead"),
    fragment!("jbend_row"),
    fragment!("lead_stub"),
    fragment!("module_aux_pad"),
    fragment!("module_board"),
    fragment!("module_castellation"),
    fragment!("module_hole_row"),
    fragment!("module_mounting_holes"),
    fragment!("module_pad_row"),
    fragment!("module_via"),
    fragment!("molded_body"),
    fragment!("pattern_row"),
    fragment!("pin1_dimple"),
    fragment!("qfn"),
    fragment!("qfn_exposed_pad"),
    fragment!("qfn_pad_row"),
    fragment!("radial_led"),
    fragment!("radial_led_finish"),
    fragment!("radial_led_lead"),
    fragment!("shroud_cavity"),
    fragment!("sip"),
    fragment!("small_can"),
    fragment!("socket_cut"),
    fragment!("socket_entry"),
    fragment!("tab_plate"),
    fragment!("tact_smd_lead"),
    fragment!("tact_switch"),
    fragment!("tact_tht_pin"),
    fragment!("tht_lead_params"),
    fragment!("tht_wire"),
    fragment!("to220"),
    fragment!("to220_leg"),
    fragment!("to92"),
    fragment!("to92_formed_leg"),
    fragment!("to92_leg"),
    fragment!("usb_c"),
    fragment!("usb_c_pad"),
    fragment!("usb_c_tab"),
];

pub fn named(name: &str) -> Option<Fragment> {
    ALL.iter().copied().find(|f| f.name == name)
}

/// The variables of one fragment use.
#[derive(Debug, Clone, Default)]
pub struct Vars(Vec<(String, Value)>);

impl Vars {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(mut self, name: &str, value: impl Into<Value>) -> Self {
        self.0.retain(|(n, _)| n != name);
        self.0.push((name.to_string(), value.into()));
        self
    }
    /// Adds every variable of `other` (overriding same-named ones).
    pub fn with(mut self, other: &Vars) -> Self {
        for (n, v) in &other.0 {
            self = self.set(n, v.clone());
        }
        self
    }
    fn get(&self, name: &str) -> Option<&Value> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
}

/// `vars!(tag = "L", count = layout.count)`.
macro_rules! vars {
    ($($name:ident = $value:expr),* $(,)?) => {
        $crate::template::Vars::new()$(.set(stringify!($name), $value))*
    };
}
pub(crate) use vars;

#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("template {fragment}: not valid YAML: {message}")]
    Yaml {
        fragment: &'static str,
        message: String,
    },
    #[error("template {fragment}: {message}")]
    Shape {
        fragment: &'static str,
        message: String,
    },
    #[error("template {fragment}: no value for {{{{{var}}}}}")]
    Undefined { fragment: &'static str, var: String },
}

impl Script {
    /// Appends `fragment`'s commands with `vars` filled in.
    pub fn fragment(&mut self, fragment: Fragment, vars: &Vars) -> Result<(), TemplateError> {
        let shape = |message: &str| TemplateError::Shape {
            fragment: fragment.name,
            message: message.into(),
        };
        let doc: serde_yaml::Value =
            serde_yaml::from_str(fragment.source).map_err(|e| TemplateError::Yaml {
                fragment: fragment.name,
                message: e.to_string(),
            })?;
        let items = doc
            .as_sequence()
            .ok_or_else(|| shape("must be a list of commands"))?;
        for item in items {
            let map = item
                .as_mapping()
                .filter(|m| m.len() == 1)
                .ok_or_else(|| shape("each item must be a single `command.id: {params}` entry"))?;
            let (command, params) = map.iter().next().expect("one entry");
            let command = command
                .as_str()
                .ok_or_else(|| shape("a command id must be text"))?;
            if command == "include" {
                let (name, extra) = match params {
                    serde_yaml::Value::String(name) => (name.clone(), Vars::new()),
                    serde_yaml::Value::Mapping(_) => {
                        let Value::Object(map) = to_json(params, fragment, vars)? else {
                            unreachable!()
                        };
                        let mut extra = Vars::new();
                        let mut name = None;
                        for (k, v) in map {
                            if k == "fragment" {
                                name = v.as_str().map(str::to_string);
                            } else {
                                extra = extra.set(&k, v);
                            }
                        }
                        (
                            name.ok_or_else(|| shape("`include` needs a `fragment` name"))?,
                            extra,
                        )
                    }
                    _ => {
                        return Err(shape(
                            "`include` takes a fragment name or {fragment, vars...}",
                        ))
                    }
                };
                let included =
                    named(&name).ok_or_else(|| shape(&format!("no template named {name:?}")))?;
                self.fragment(included, &vars.clone().with(&extra))?;
            } else if command == "params" {
                let params = params
                    .as_mapping()
                    .ok_or_else(|| shape("`params` must map names to [expression, comment]"))?;
                for (name, def) in params {
                    let name = name
                        .as_str()
                        .ok_or_else(|| shape("a parameter name must be text"))?;
                    let def = to_json(def, fragment, vars)?;
                    let parts = def.as_array().filter(|a| (2..=3).contains(&a.len())).ok_or_else(|| {
                        shape(&format!("parameter {name}: expected [expression, comment] or [expression, comment, unit]"))
                    })?;
                    let text = |v: &Value| match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    self.declare(
                        name,
                        text(&parts[0]),
                        parts.get(2).map(text),
                        &text(&parts[1]),
                    );
                }
            } else {
                self.cmd(command, to_json(params, fragment, vars)?);
            }
        }
        Ok(())
    }
}

/// A YAML value as JSON, placeholders filled in.
fn to_json(
    value: &serde_yaml::Value,
    fragment: Fragment,
    vars: &Vars,
) -> Result<Value, TemplateError> {
    let json = serde_json::to_value(value).map_err(|e| TemplateError::Shape {
        fragment: fragment.name,
        message: e.to_string(),
    })?;
    fill(json, fragment, vars)
}

fn fill(value: Value, fragment: Fragment, vars: &Vars) -> Result<Value, TemplateError> {
    Ok(match value {
        Value::String(s) => fill_str(&s, fragment, vars)?,
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|v| fill(v, fragment, vars))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| Ok((k, fill(v, fragment, vars)?)))
                .collect::<Result<_, TemplateError>>()?,
        ),
        other => other,
    })
}

fn fill_str(s: &str, fragment: Fragment, vars: &Vars) -> Result<Value, TemplateError> {
    let lookup = |name: &str| {
        vars.get(name.trim())
            .ok_or_else(|| TemplateError::Undefined {
                fragment: fragment.name,
                var: name.trim().to_string(),
            })
    };
    // A whole-value placeholder keeps the variable's JSON type.
    if let Some(name) = s.strip_prefix("{{").and_then(|r| r.strip_suffix("}}")) {
        if !name.contains("{{") && !name.contains("}}") {
            return Ok(lookup(name)?.clone());
        }
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find("}}").ok_or_else(|| TemplateError::Shape {
            fragment: fragment.name,
            message: format!("unclosed placeholder in {s:?}"),
        })?;
        match lookup(&after[..end])? {
            Value::String(v) => out.push_str(v),
            other => out.push_str(&other.to_string()),
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(Value::String(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const T: Fragment = Fragment {
        name: "test",
        source: r#"
- params:
    side_w: ["{{w}}", "width of {{what}}"]
    n: ["{{n}}", count, ""]
- solid.box: {length: side_w, width: "{{w}}", height: "{{neg}}(side_w / 2)", center: "{{at}}"}
"#,
    };

    #[test]
    fn fills_placeholders_and_keeps_whole_value_types() {
        let mut s = Script::new();
        s.fragment(
            T,
            &vars!(
                w = "2 mm",
                what = "it",
                n = 4,
                neg = "-",
                at = json!([0, 1, 2])
            ),
        )
        .unwrap();
        let c = s.commands();
        assert_eq!(
            c[0]["params"],
            json!({"name": "side_w", "expression": "2 mm", "comment": "width of it"})
        );
        assert_eq!(
            c[1]["params"],
            json!({"name": "n", "expression": "4", "unit": "", "comment": "count"})
        );
        assert_eq!(
            c[2]["params"],
            json!({"length": "side_w", "width": "2 mm", "height": "-(side_w / 2)", "center": [0, 1, 2]})
        );
    }

    #[test]
    fn every_template_file_is_registered_and_is_valid_yaml() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/templates");
        let mut files: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                e.unwrap()
                    .path()
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        files.sort();
        let names: Vec<&str> = ALL.iter().map(|f| f.name).collect();
        assert_eq!(files, names);
        for f in ALL {
            let doc: serde_yaml::Value =
                serde_yaml::from_str(f.source).unwrap_or_else(|e| panic!("{}: {e}", f.name));
            assert!(doc.is_sequence(), "{} is not a list of commands", f.name);
        }
    }

    #[test]
    fn a_missing_variable_is_an_error() {
        let err = Script::new().fragment(T, &vars!(w = "2 mm")).unwrap_err();
        assert!(matches!(err, TemplateError::Undefined { .. }), "{err}");
    }
}
