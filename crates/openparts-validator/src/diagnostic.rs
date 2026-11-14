//! What a check reports.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub rule_id: &'static str,
    pub severity: Severity,
    /// e.g. "part:ex/EX1", "device:ex/DEV1".
    pub entity: String,
    /// JSON-Pointer-shaped path to the offending field, when the
    /// diagnostic is about a single field rather than the whole entity.
    pub field_path: Option<String>,
    pub message: String,
}

impl Diagnostic {
    pub(crate) fn error(
        rule_id: &'static str,
        entity: impl Into<String>,
        field_path: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            rule_id,
            severity: Severity::Error,
            entity: entity.into(),
            field_path: field_path.map(str::to_string),
            message: message.into(),
        }
    }

    pub(crate) fn warning(
        rule_id: &'static str,
        entity: impl Into<String>,
        field_path: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            ..Diagnostic::error(rule_id, entity, field_path, message)
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}
