//! openparts-client: abstracts communication with an `openparts-server`
//! instance -- search, component/artifact retrieval, local cache, hash
//! validation, and offline fallback (Architecture Specification
//! section 7.4). CLI/EDA integrations are meant to use this instead of
//! each reimplementing their own HTTP handling.
//!
//! `openparts-server` is optional (section 4.8) -- this crate is the
//! only place in the workspace that depends on a server actually being
//! reachable; every other crate works entirely offline against local
//! `openparts-data` files.

mod cache;
mod error;

pub use error::ClientError;

use openparts_core::{Device, Package, Part};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    KicadSymbol,
    KicadFootprint,
    Step,
    Stl,
}

impl ArtifactKind {
    fn as_str(self) -> &'static str {
        match self {
            ArtifactKind::KicadSymbol => "kicad-symbol",
            ArtifactKind::KicadFootprint => "kicad-footprint",
            ArtifactKind::Step => "step",
            ArtifactKind::Stl => "stl",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Artifact {
    pub content: String,
    /// Hex-encoded SHA-256 digest, already verified against the
    /// server's `X-Content-Hash` header when one was fetched live.
    pub hash: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PartSummary {
    pub manufacturer: String,
    pub mpn: String,
    pub existence: openparts_core::ExistenceStatus,
    pub lifecycle: openparts_core::LifecycleStatus,
}

pub struct Client {
    pub registry_url: String,
    cache_dir: PathBuf,
    offline: bool,
}

impl Client {
    pub fn new(registry_url: impl Into<String>) -> Self {
        Self {
            registry_url: registry_url.into(),
            cache_dir: cache::default_cache_dir(),
            offline: false,
        }
    }

    pub fn with_cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = dir.into();
        self
    }

    /// When `true`, every method reads only from the local cache and
    /// never touches the network (offline use, Architecture
    /// Specification section 27).
    pub fn offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.registry_url.trim_end_matches('/'), path)
    }

    fn request_text(&self, path: &str) -> Result<(String, Option<String>), ClientError> {
        let url = self.url(path);
        match ureq::get(&url).call() {
            Ok(response) => {
                let hash = response.header("x-content-hash").map(str::to_string);
                let body = response
                    .into_string()
                    .map_err(|source| ClientError::Network {
                        url: url.clone(),
                        message: source.to_string(),
                    })?;
                Ok((body, hash))
            }
            Err(ureq::Error::Status(status, response)) => {
                let body = response.into_string().unwrap_or_default();
                Err(ClientError::ServerError { url, status, body })
            }
            Err(ureq::Error::Transport(transport)) => Err(ClientError::Network {
                url,
                message: transport.to_string(),
            }),
        }
    }

    /// Fetches `path` live, falling back to whatever is cached under
    /// `cache_key` if the network request fails (or is skipped
    /// entirely in offline mode) -- Architecture Specification
    /// section 27, Offline Use.
    fn fetch_with_cache(&self, path: &str, cache_key: &str) -> Result<String, ClientError> {
        if self.offline {
            return cache::read(&self.cache_dir, cache_key)
                .ok_or_else(|| ClientError::OfflineCacheMiss(cache_key.to_string()));
        }
        match self.request_text(path) {
            Ok((body, _hash)) => {
                let _ = cache::write(&self.cache_dir, cache_key, &body);
                Ok(body)
            }
            Err(network_err) => cache::read(&self.cache_dir, cache_key).ok_or(network_err),
        }
    }

    fn fetch_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        cache_key: &str,
    ) -> Result<T, ClientError> {
        let body = self.fetch_with_cache(path, cache_key)?;
        serde_json::from_str(&body).map_err(|source| ClientError::Parse {
            url: cache_key.to_string(),
            source,
        })
    }

    /// `GET /v1/search?q=...`
    pub fn search(&self, query: &str) -> Result<Vec<PartSummary>, ClientError> {
        let cache_key = format!("metadata/search/{}.json", sanitize_for_filename(query));
        self.fetch_json(
            &format!("/v1/search?q={}", percent_encode(query)),
            &cache_key,
        )
    }

    /// `GET /v1/parts/{manufacturer}/{mpn}`
    pub fn get_part(&self, manufacturer: &str, mpn: &str) -> Result<Part, ClientError> {
        let cache_key = cache::metadata_key("parts", &part_segments(manufacturer, mpn));
        self.fetch_json(&part_path(manufacturer, mpn), &cache_key)
    }

    /// `GET /v1/devices/{id}`
    pub fn get_device(&self, id: &str) -> Result<Device, ClientError> {
        let cache_key = cache::metadata_key("devices", id);
        self.fetch_json(&format!("/v1/devices/{id}"), &cache_key)
    }

    /// `GET /v1/packages/{id}`
    pub fn get_package(&self, id: &str) -> Result<Package, ClientError> {
        let cache_key = cache::metadata_key("packages", id);
        self.fetch_json(&format!("/v1/packages/{id}"), &cache_key)
    }

    /// `GET /v1/parts/{manufacturer}/{mpn}/artifacts/{kind}[?silicon_revision=...]`.
    /// A live fetch's content is hashed and checked against the
    /// server's `X-Content-Hash` header before being cached or
    /// returned -- a hash mismatch is always an error, never a
    /// best-effort result (Cache Integrity, Architecture Specification
    /// section 55).
    pub fn get_artifact(
        &self,
        manufacturer: &str,
        mpn: &str,
        kind: ArtifactKind,
        revision: Option<&str>,
    ) -> Result<Artifact, ClientError> {
        let pointer_key = cache::artifact_pointer_key(
            &percent_encode(manufacturer),
            &percent_encode(mpn),
            kind.as_str(),
            revision.map(percent_encode).as_deref(),
        );

        if self.offline {
            let hash = cache::read(&self.cache_dir, &pointer_key)
                .ok_or_else(|| ClientError::OfflineCacheMiss(pointer_key.clone()))?;
            let content_key = cache::artifact_content_key(&hash);
            let content = cache::read(&self.cache_dir, &content_key)
                .ok_or(ClientError::OfflineCacheMiss(content_key))?;
            return Ok(Artifact { content, hash });
        }

        let mut path = format!(
            "{}/artifacts/{}",
            part_path(manufacturer, mpn),
            kind.as_str()
        );
        if let Some(rev) = revision {
            path.push_str(&format!("?silicon_revision={}", percent_encode(rev)));
        }
        let url = self.url(&path);

        match self.request_text(&path) {
            Ok((body, claimed_hash)) => {
                let computed = sha256_hex(body.as_bytes());
                if let Some(claimed) = &claimed_hash {
                    let claimed_digest = claimed.strip_prefix("sha256:").unwrap_or(claimed);
                    if claimed_digest != computed {
                        return Err(ClientError::HashMismatch {
                            url,
                            claimed: claimed_hash,
                            computed,
                        });
                    }
                }
                let _ = cache::write(&self.cache_dir, &pointer_key, &computed);
                let _ = cache::write(
                    &self.cache_dir,
                    &cache::artifact_content_key(&computed),
                    &body,
                );
                Ok(Artifact {
                    content: body,
                    hash: computed,
                })
            }
            Err(network_err) => {
                // Cache Integrity: only ever serve a cached artifact
                // whose stored hash still matches its stored content.
                if let Some(hash) = cache::read(&self.cache_dir, &pointer_key) {
                    let content_key = cache::artifact_content_key(&hash);
                    if let Some(content) = cache::read(&self.cache_dir, &content_key) {
                        if sha256_hex(content.as_bytes()) == hash {
                            return Ok(Artifact { content, hash });
                        }
                    }
                }
                Err(network_err)
            }
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn sanitize_for_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// `/v1/parts/{manufacturer}/{mpn}` with each segment percent-encoded:
/// an MPN may contain `/` ("APT1608QBC/D"), which must reach the server
/// as `%2F` inside the one `:mpn` segment rather than as a separator.
pub fn part_path(manufacturer: &str, mpn: &str) -> String {
    format!("/v1/parts/{}", part_segments(manufacturer, mpn))
}

/// `{manufacturer}/{mpn}`, each percent-encoded (also the cache path,
/// so an MPN's `/` doesn't become a directory).
fn part_segments(manufacturer: &str, mpn: &str) -> String {
    format!("{}/{}", percent_encode(manufacturer), percent_encode(mpn))
}

/// RFC 3986 percent-encoding of everything but the unreserved
/// characters, byte by byte (UTF-8): safe for a path segment and a
/// query value.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Spins up a bare TCP listener that replies with `raw_response`
    /// (a full HTTP/1.1 response, status line + headers + body) to the
    /// next connection it receives, then stops. No mocking library
    /// dependency needed for the handful of fixed responses these
    /// tests need.
    fn mock_server_once(raw_response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(raw_response.as_bytes());
                let _ = stream.flush();
            }
        });
        format!("http://{addr}")
    }

    fn temp_cache_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "openparts-client-test-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn search_parses_server_response() {
        let body = r#"[{"manufacturer":"raspberrypi","mpn":"RP2040","existence":"verified","lifecycle":"active"}]"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let base_url = mock_server_once(Box::leak(response.into_boxed_str()));

        let client = Client::new(base_url).with_cache_dir(temp_cache_dir("search"));
        let results = client.search("RP2040").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].mpn, "RP2040");
    }

    #[test]
    fn not_found_becomes_server_error() {
        let body = r#"{"error":"no part x/y"}"#;
        let response = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let base_url = mock_server_once(Box::leak(response.into_boxed_str()));

        let client = Client::new(base_url).with_cache_dir(temp_cache_dir("404"));
        let err = client.get_part("x", "y").unwrap_err();
        assert!(matches!(err, ClientError::ServerError { status: 404, .. }));
    }

    #[test]
    fn artifact_hash_mismatch_is_rejected() {
        let body = "not the real content";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nX-Content-Hash: sha256:0000000000000000000000000000000000000000000000000000000000000\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let base_url = mock_server_once(Box::leak(response.into_boxed_str()));

        let client = Client::new(base_url).with_cache_dir(temp_cache_dir("hash-mismatch"));
        let err = client
            .get_artifact("raspberrypi", "RP2040", ArtifactKind::KicadSymbol, None)
            .unwrap_err();
        assert!(matches!(err, ClientError::HashMismatch { .. }));
    }

    #[test]
    fn part_paths_percent_encode_each_segment() {
        assert_eq!(
            part_path("raspberrypi", "RP2040"),
            "/v1/parts/raspberrypi/RP2040"
        );
        assert_eq!(
            part_path("kingbright", "APT1608QBC/D"),
            "/v1/parts/kingbright/APT1608QBC%2FD"
        );
        assert_eq!(
            part_path("everlight", "19-217/R6C-P1Q2/6T"),
            "/v1/parts/everlight/19-217%2FR6C-P1Q2%2F6T"
        );
        assert_eq!(
            part_path("ti", "LM1117MPX-3.3/NOPB"),
            "/v1/parts/ti/LM1117MPX-3.3%2FNOPB"
        );
        // Space, '#', '?', '%' and non-ASCII (UTF-8 bytes).
        assert_eq!(percent_encode("a b#?%"), "a%20b%23%3F%25");
        assert_eq!(percent_encode("Ω"), "%CE%A9");
    }

    #[test]
    fn artifact_request_sends_the_encoded_mpn() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let body = "x";
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
            request.lines().next().unwrap_or_default().to_string()
        });
        let cache_dir = temp_cache_dir("slash-mpn");
        let client = Client::new(format!("http://{addr}")).with_cache_dir(&cache_dir);
        let a = client
            .get_artifact("kingbright", "APT1608QBC/D", ArtifactKind::Step, None)
            .unwrap();
        assert_eq!(a.content, "x");
        assert_eq!(
            seen.join().unwrap(),
            "GET /v1/parts/kingbright/APT1608QBC%2FD/artifacts/step HTTP/1.1"
        );
        // The cache keeps the MPN in one file name, not a directory.
        assert!(cache_dir
            .join("metadata/artifact-pointers/kingbright/APT1608QBC%2FD/step.hash")
            .is_file());
    }

    #[test]
    fn offline_client_reads_only_from_cache() {
        let cache_dir = temp_cache_dir("offline");
        cache::write(&cache_dir, &cache::metadata_key("parts", "raspberrypi/RP2040"), r#"{"schema_version":"0.1","kind":"part","id":"raspberrypi/RP2040","manufacturer":"raspberrypi","mpn":"RP2040","device":"raspberrypi/RP2040","package":"standards/QFN56-RP2040","existence":{"status":"verified"},"lifecycle":{"status":"active","replacement":[]},"sources":[],"provenance":{}}"#).unwrap();

        // A registry URL that nothing is listening on -- offline mode
        // must never attempt to connect to it.
        let client = Client::new("http://127.0.0.1:1")
            .with_cache_dir(cache_dir)
            .offline(true);

        let part = client.get_part("raspberrypi", "RP2040").unwrap();
        assert_eq!(part.mpn, "RP2040");

        let err = client.get_part("nobody", "NOPE").unwrap_err();
        assert!(matches!(err, ClientError::OfflineCacheMiss(_)));
    }
}
