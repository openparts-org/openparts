#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("request to {url} failed: {message}")]
    Network { url: String, message: String },
    #[error("server returned {status} for {url}: {body}")]
    ServerError {
        url: String,
        status: u16,
        body: String,
    },
    #[error("failed to parse response from {url}: {source}")]
    Parse {
        url: String,
        #[source]
        source: serde_json::Error,
    },
    /// A downloaded artifact's computed hash didn't match the server's
    /// `X-Content-Hash` header -- the response is never returned to the
    /// caller as a valid result (Testing and Quality Specification
    /// section 4: "Hash不一致のArtifactを有効な結果として返さない").
    #[error(
        "artifact hash mismatch for {url}: server claimed {claimed:?}, computed sha256:{computed}"
    )]
    HashMismatch {
        url: String,
        claimed: Option<String>,
        computed: String,
    },
    #[error("offline and no cached copy of {0} is available")]
    OfflineCacheMiss(String),
    #[error("cache I/O error at {path}: {source}")]
    CacheIo {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
