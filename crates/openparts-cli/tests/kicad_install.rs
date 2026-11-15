//! `openparts kicad-install` against a registry, for an MPN containing
//! `/` (Kingbright "APT1608QBC/D", in the real data): the CLI asks for
//! `/v1/parts/kingbright/APT1608QBC%2FD/artifacts/<kind>` -- one
//! percent-encoded segment, what the server's `:mpn` route matches.
//!
//! The registry here is a small HTTP stand-in serving the part's real
//! KiCad symbol and footprint (built from ../openparts-data) and a
//! placeholder STEP, so no SolidCraft is needed.

use openparts_artifacts::{ArtifactKind, PartBuild};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const MPN: &str = "APT1608QBC/D";
const ENCODED: &str = "/v1/parts/kingbright/APT1608QBC%2FD/artifacts";

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../openparts-data")
}

/// Serves `files` (path -> body, with its `X-Content-Hash`) until the
/// test ends; every request line's path is recorded.
fn registry(files: HashMap<String, Vec<u8>>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            // Drain the headers.
            let mut h = String::new();
            while reader.read_line(&mut h).is_ok() && h != "\r\n" && !h.is_empty() {
                h.clear();
            }
            let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
            log.lock().unwrap().push(path.clone());
            let response = match files.get(&path) {
                Some(body) => {
                    let hash: String = Sha256::digest(body)
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect();
                    let mut r = format!(
                        "HTTP/1.1 200 OK\r\nX-Content-Hash: sha256:{hash}\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    r.extend_from_slice(body);
                    r
                }
                None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_vec(),
            };
            let _ = stream.write_all(&response);
        }
    });
    (format!("http://{addr}"), seen)
}

#[test]
fn kicad_install_fetches_an_mpn_with_a_slash() {
    let dataset = openparts_data::Dataset::load(data_dir()).unwrap();
    let part = dataset
        .part("kingbright", MPN)
        .expect("APT1608QBC/D in the data");
    let build = PartBuild::from_dataset(&dataset, part, None).unwrap();
    let files = HashMap::from([
        (
            format!("{ENCODED}/kicad-symbol"),
            build.render(ArtifactKind::KicadSymbol).unwrap(),
        ),
        (
            format!("{ENCODED}/kicad-footprint"),
            build.render(ArtifactKind::KicadFootprint).unwrap(),
        ),
        (
            format!("{ENCODED}/step"),
            b"ISO-10303-21;\nEND-ISO-10303-21;\n".to_vec(),
        ),
    ]);
    let (url, seen) = registry(files);

    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("kicad-install-slash");
    let _ = std::fs::remove_dir_all(&tmp);
    let project = tmp.join("project");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_openparts"))
        .args(["--registry", &url, "kicad-install", "--project"])
        .arg(&project)
        .arg(format!("kingbright/{MPN}"))
        // The client's cache (~/.cache/openparts) goes here.
        .env("HOME", &tmp)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        *seen.lock().unwrap(),
        [
            format!("{ENCODED}/kicad-symbol"),
            format!("{ENCODED}/kicad-footprint"),
            format!("{ENCODED}/step"),
        ]
    );
    let symbols = std::fs::read_to_string(project.join("openparts.kicad_sym")).unwrap();
    assert!(symbols.contains(MPN), "{symbols}");
    assert!(std::fs::read_dir(project.join("openparts.pretty"))
        .unwrap()
        .any(|e| e
            .unwrap()
            .path()
            .extension()
            .is_some_and(|x| x == "kicad_mod")));
    let _ = std::fs::remove_dir_all(&tmp);
}
