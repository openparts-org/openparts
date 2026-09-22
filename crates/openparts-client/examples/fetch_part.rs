//! Minimal usage example: search, fetch a Part, and download its KiCad
//! symbol artifact from a running `openparts-server`.
//!
//! Run an `openparts-server` first (see its README), then:
//!
//!   cargo run -p openparts-client --example fetch_part -- \
//!     http://localhost:8080 raspberrypi RP2040

use openparts_client::{ArtifactKind, Client};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let registry_url = args
        .next()
        .unwrap_or_else(|| "http://localhost:8080".to_string());
    let manufacturer = args.next().unwrap_or_else(|| "raspberrypi".to_string());
    let mpn = args.next().unwrap_or_else(|| "RP2040".to_string());

    let client = Client::new(&registry_url);

    let results = client.search(&mpn)?;
    println!("search({mpn:?}) -> {} result(s)", results.len());

    let part = client.get_part(&manufacturer, &mpn)?;
    println!(
        "part: {} ({:?} / {:?})",
        part.mpn, part.existence.status, part.lifecycle.status
    );

    let symbol = client.get_artifact(&manufacturer, &mpn, ArtifactKind::KicadSymbol, None)?;
    println!(
        "kicad-symbol: {} bytes, sha256:{}",
        symbol.content.len(),
        symbol.hash
    );

    Ok(())
}
