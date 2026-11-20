// Calls to openparts-server, decoded with the generated types.

///|
/// One request's state. `Failed` carries the server's own message
/// (an `ErrorBody.error`) when there is one.
#derive(Eq)
enum Fetch[T] {
  Loading
  Got(T)
  Missing
  Failed(String)
}

///|
fn part_api_path(manufacturer : String, mpn : String) -> String {
  "/v1/parts/\{encode_uri_component(manufacturer)}/\{encode_uri_component(mpn)}"
}

///|
fn artifact_url(manufacturer : String, mpn : String, kind : String) -> String {
  api_base() + part_api_path(manufacturer, mpn) + "/artifacts/" + kind
}

///|
/// `kind`'s link from the part's artifact listing -- content-addressed
/// (`/v1/a/<sha256>/...`, cached for good by the browser and the CDN)
/// where the server knows the content -- else its plain link.
fn listed_url(
  artifacts : @api.ArtifactsResponse?,
  manufacturer : String,
  mpn : String,
  kind : String,
) -> String {
  let listed = match artifacts {
    None => ""
    Some(l) =>
      match kind {
        "kicad-symbol" => l.kicad_symbol
        "kicad-footprint" => l.kicad_footprint
        "footprint-svg" => l.footprint_svg
        "symbol-svg" => l.symbol_svg
        "step" => l.step
        "stl" => l.stl
        "3mf" => l.x3mf
        "pcbcraft" => l.pcbcraft
        "pcbcraft-symbol-svg" => l.pcbcraft_symbol_svg
        _ => ""
      }
  }
  if listed.has_prefix("/") {
    api_base() + listed
  } else {
    artifact_url(manufacturer, mpn, kind)
  }
}

///|
/// [listed_url] for the loaded part.
fn artifact_href(data : PartData, kind : String) -> String {
  listed_url(data.artifacts, data.part.manufacturer, data.part.mpn, kind)
}

///|
fn error_message(r : JsResponse) -> String {
  let text = r.body_text()
  let message = try {
    let body : @api.ErrorBody = @json.from_json(@json.parse(text))
    Some(body.error)
  } catch {
    _ => None
  }
  message.unwrap_or("\{r.status()} \{r.status_text()}")
}

///|
async fn[T : @json.FromJson] get_json(path : String) -> Fetch[T] noraise {
  try {
    let r = fetch_raw(api_base() + path, false).wait()
    match r.status() {
      200 => Got(@json.from_json(@json.parse(r.body_text())))
      404 => Missing
      _ => Failed(error_message(r))
    }
  } catch {
    e => Failed(e.to_string())
  }
}

///|
/// An artifact's bytes, or the reason it isn't there.
async fn get_bytes(url : String) -> Result[Bytes, String] noraise {
  try {
    let r = fetch_raw(url, true).wait()
    if r.status() == 200 {
      Ok(r.body_bytes())
    } else {
      Err(error_message(r))
    }
  } catch {
    e => Err(e.to_string())
  }
}

///|
/// A text artifact (an SVG drawing), or the reason it isn't there.
async fn get_text(url : String) -> Result[String, String] noraise {
  try {
    let r = fetch_raw(url, false).wait()
    if r.status() == 200 {
      Ok(r.body_text())
    } else {
      Err(error_message(r))
    }
  } catch {
    e => Err(e.to_string())
  }
}

///|
/// Everything the part page shows.
#derive(Eq)
struct PartData {
  part : @api.Part
  device : @api.Device
  package_ : @api.Package
  revisions : Array[@api.RevisionSummary]
  sources : Array[@api.PartSource]
  /// Why the CAD artifacts aren't available (e.g. a missing dimension),
  /// when they aren't; the rest of the page still shows.
  artifacts_error : String?
  /// Every artifact's link (`GET .../artifacts`), when listed.
  artifacts : @api.ArtifactsResponse?
  /// The schematic symbol as KiCad draws it (SVG markup).
  symbol_kicad : Result[String, String]
  /// As PCBCraft draws it; `None` when PCBCraft refuses the part.
  symbol_pcbcraft : Result[String, String]?
  /// The part's, device's and package's files and review blocks
  /// (data_review.mbt); `None` when the server didn't answer.
  documents : @api.PartDocuments?
  /// Where each value was read; `None` when the server didn't answer.
  provenance : @api.ProvenanceResponse?
  /// Reference circuits for its device (circuits.mbt); `None` when the
  /// server didn't answer.
  circuits : Array[@api.CircuitView]?
  /// The part as PCBCraft sees it: exportable, or refused with the
  /// reason (`GET /v1/pcbcraft/parts/...`).
  pcbcraft : Fetch[@api.PcbcraftPartDetails]
}

///|
async fn load_part(
  manufacturer : String,
  mpn : String,
) -> Fetch[PartData] noraise {
  let base = part_api_path(manufacturer, mpn)
  let part : @api.Part = match get_json(base) {
    Got(p) => p
    Missing => return Missing
    Failed(e) => return Failed(e)
    Loading => return Loading
  }
  // Device and package ids may contain "/" (wildcard routes): not encoded.
  let device : Fetch[@api.Device] = get_json("/v1/devices/\{part.device}")
  let package_ : Fetch[@api.Package] = get_json("/v1/packages/\{part.package_}")
  let revisions : Fetch[Array[@api.RevisionSummary]] = get_json(
    base + "/revisions",
  )
  let sources : Fetch[Array[@api.PartSource]] = get_json(base + "/sources")
  let artifacts : Fetch[@api.ArtifactsResponse] = get_json(base + "/artifacts")
  let documents : Fetch[@api.PartDocuments] = get_json(base + "/documents")
  let documents = if documents is Got(d) { Some(d) } else { None }
  let provenance : Fetch[@api.ProvenanceResponse] = get_json(
    base + "/provenance",
  )
  let provenance = if provenance is Got(p) { Some(p) } else { None }
  let circuits : Fetch[Array[@api.CircuitView]] = get_json(base + "/circuits")
  let circuits = if circuits is Got(c) { Some(c) } else { None }
  let pcbcraft : Fetch[@api.PcbcraftPartDetails] = get_json(
    "/v1/pcbcraft/parts/\{encode_uri_component(manufacturer)}/\{encode_uri_component(mpn)}",
  )
  let artifacts_error = match artifacts {
    Got(_) => None
    Failed(e) => Some(e)
    _ => Some("")
  }
  let artifacts = if artifacts is Got(l) { Some(l) } else { None }
  let symbol_kicad = get_text(
    listed_url(artifacts, manufacturer, mpn, "symbol-svg"),
  )
  let symbol_pcbcraft = match pcbcraft {
    Got(d) if !d.summary.refused =>
      Some(get_text(listed_url(artifacts, manufacturer, mpn, "pcbcraft-symbol-svg")))
    _ => None
  }
  match (device, package_, revisions, sources) {
    (Got(device), Got(package_), Got(revisions), Got(sources)) =>
      Got({
        part,
        device,
        package_,
        revisions,
        sources,
        artifacts_error,
        artifacts,
        symbol_kicad,
        symbol_pcbcraft,
        documents,
        provenance,
        circuits,
        pcbcraft,
      })
    (Failed(e), _, _, _)
    | (_, Failed(e), _, _)
    | (_, _, Failed(e), _)
    | (_, _, _, Failed(e)) => Failed(e)
    _ => Missing
  }
}

///|
/// The KiCad bundle: symbol, footprint and the STEP model the footprint
/// points at, which KiCad needs together.
async fn kicad_zip(data : PartData) -> Result[Bytes, String] noraise {
  let mpn = data.part.mpn
  let files = [
    ("kicad-symbol", "kicad_sym"),
    ("kicad-footprint", "kicad_mod"),
    ("step", "step"),
  ]
  let fetched = []
  for kind_ext in files {
    let (kind, ext) = kind_ext
    match get_bytes(artifact_href(data, kind)) {
      Ok(bytes) => fetched.push(("\{mpn}.\{ext}", bytes))
      Err(e) => return Err(e)
    }
  }
  try {
    let archive = @zip.Archive::empty()
    for named in fetched {
      let (name, bytes) = named
      let file = @file.File::deflate_of_bytes(bytes, 0, bytes.length())
      archive.add(@member.make(name, File(file)))
    }
    Ok(archive.to_bytes())
  } catch {
    e => Err(e.to_string())
  }
}
