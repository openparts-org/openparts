//! Parameter checks (Canonical Data Specification section 11.1): every
//! key is in the vocabulary, each value has its key's shape and a unit
//! of its key's quantity, `min <= nominal <= max`, every value is cited,
//! and the key suits the device's category.

use crate::Diagnostic;
use openparts_core::{
    parameter::plausible_for, parameter_spec, ComponentCategory, ParameterError, Parameters,
    ProvenanceMap,
};

pub(crate) fn check_parameters(
    entity: &str,
    parameters: &Parameters,
    provenance: &ProvenanceMap,
    category: ComponentCategory,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (key, value) in parameters {
        let path = format!("/parameters/{}", crate::cross::pointer_escape(key));
        let cited = provenance
            .keys()
            .any(|p| *p == path || p.starts_with(&format!("{path}/")));
        if !cited {
            out.push(Diagnostic::error(
                "provenance.missing",
                entity,
                Some(&path),
                format!("parameter \"{key}\" has no provenance entry at {path}"),
            ));
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
        if let Err(e) = value.normalize(spec) {
            let message = format!("parameter \"{key}\": {e}");
            out.push(match e {
                ParameterError::MissingUnit | ParameterError::IncompatibleUnit { .. } => {
                    Diagnostic::error("parameter.unit", entity, Some(&path), message)
                }
                ParameterError::OutOfOrder => {
                    Diagnostic::error("parameter.out_of_range", entity, Some(&path), message)
                }
                _ => Diagnostic::error("parameter.invalid", entity, Some(&path), message),
            });
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
