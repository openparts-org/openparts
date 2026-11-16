//! Parameter checks (Canonical Data Specification sections 11.1 and
//! 11.2): every key is in the vocabulary, each value has its key's
//! shape and a unit of its key's quantity, `min <= typ <= max`, every
//! value is cited, its conditions use known symbols with fitting units,
//! condition variants of one key differ, the key suits the device's
//! category, and each number lies in the key's plausible range.

use crate::Diagnostic;
use openparts_core::{
    condition::comparable, format_value, parameter::plausible_for, parameter_spec,
    ComponentCategory, Normalized, ParameterError, Parameters, ProvenanceMap,
};

pub(crate) fn check_parameters(
    entity: &str,
    parameters: &Parameters,
    provenance: &ProvenanceMap,
    category: ComponentCategory,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (key, values) in parameters {
        let path = format!("/parameters/{}", crate::cross::pointer_escape(key));
        let covers = |p: &str| provenance.contains_key(p);
        let many = values.len() > 1;
        for i in 0..values.len() {
            let entry = format!("{path}/{i}");
            // One value: cited at the key (or inside it). Several: each
            // at its index, or all at the key.
            let cited = covers(&path)
                || covers(&entry)
                || (!many
                    && provenance
                        .keys()
                        .any(|p| p.starts_with(&format!("{path}/"))));
            if !cited {
                let at = if many { &entry } else { &path };
                out.push(Diagnostic::error(
                    "provenance.missing",
                    entity,
                    Some(at),
                    format!("parameter \"{key}\" has no provenance entry at {at}"),
                ));
            }
        }
        let Some(spec) = parameter_spec(key) else {
            out.push(Diagnostic::error(
                "parameter.unknown_key",
                entity,
                Some(&path),
                format!("\"{key}\" is not a parameter key of the vocabulary"),
            ));
            continue;
        };
        let mut seen = Vec::new();
        for (i, value) in values.iter().enumerate() {
            let at = if many {
                format!("{path}/{i}")
            } else {
                path.clone()
            };
            let normalized = value.normalize(spec);
            if let (Ok(Normalized::Numeric { typ, min, max }), Some((lo, hi))) =
                (&normalized, spec.range)
            {
                if let Some(v) = [*min, *typ, *max]
                    .into_iter()
                    .flatten()
                    .find(|v| !spec.in_range(*v))
                {
                    out.push(Diagnostic::warning(
                        "parameter.implausible_value",
                        entity,
                        Some(&at),
                        format!(
                            "parameter \"{key}\": {} is outside the plausible {}..{} (a unit slip?)",
                            format_value(v, spec),
                            format_value(lo, spec),
                            format_value(hi, spec),
                        ),
                    ));
                }
            }
            if let Err(e) = normalized {
                let message = format!("parameter \"{key}\": {e}");
                out.push(match e {
                    ParameterError::MissingUnit | ParameterError::IncompatibleUnit { .. } => {
                        Diagnostic::error("parameter.unit", entity, Some(&at), message)
                    }
                    ParameterError::OutOfOrder => {
                        Diagnostic::error("parameter.out_of_range", entity, Some(&at), message)
                    }
                    _ => Diagnostic::error("parameter.invalid", entity, Some(&at), message),
                });
            }
            for (symbol, c) in &value.at {
                if let Err(e) = c.normalize(symbol) {
                    out.push(Diagnostic::error(
                        "parameter.condition",
                        entity,
                        Some(&format!("{at}/at/{}", crate::cross::pointer_escape(symbol))),
                        format!("parameter \"{key}\": {e}"),
                    ));
                }
            }
            if let Some(c) = comparable(&value.at) {
                if seen.contains(&c) {
                    out.push(Diagnostic::error(
                        "parameter.condition",
                        entity,
                        Some(&at),
                        format!("parameter \"{key}\": two values under the same conditions"),
                    ));
                }
                seen.push(c);
            }
        }
        if !plausible_for(spec, category) {
            out.push(Diagnostic::warning(
                "parameter.implausible",
                entity,
                Some(&path),
                format!("parameter \"{key}\" is unusual for a device of category {category:?}"),
            ));
        }
    }
    out
}
