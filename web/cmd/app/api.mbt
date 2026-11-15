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
  /// Every fact's review status (review.mbt); `None` when the server
  /// didn't answer (the page shows without it).
  reviews : @api.PartReviews?
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
  let reviews : Fetch[@api.PartReviews] = get_json(base + "/reviews")
  let reviews = match reviews {
    Got(r) => Some(r)
    _ => None
  }
  match (device, package_, revisions, sources) {
    (Got(device), Got(package_), Got(revisions), Got(sources)) => {
      let artifacts_error = match artifacts {
        Got(_) => None
        Failed(e) => Some(e)
        _ => Some("")
      }
      Got({
        part,
        device,
        package_,
        revisions,
        sources,
        artifacts_error,
        reviews,
      })
    }
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
async fn kicad_zip(
  manufacturer : String,
  mpn : String,
) -> Result[Bytes, String] noraise {
  let files = [
    ("kicad-symbol", "kicad_sym"),
    ("kicad-footprint", "kicad_mod"),
    ("step", "step"),
  ]
  let fetched = []
  for kind_ext in files {
    let (kind, ext) = kind_ext
    match get_bytes(artifact_url(manufacturer, mpn, kind)) {
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
