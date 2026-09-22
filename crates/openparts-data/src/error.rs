use openparts_core::Kind;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DataError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse YAML in {path}: {source}")]
    Yaml {
        path: String,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("{path}: expected kind \"{expected}\" but document declares kind \"{found}\"")]
    KindMismatch {
        path: String,
        expected: Kind,
        found: Kind,
    },
}
