// URLs <-> pages. Paths are relative to `base_path()`; English pages
// sit under `en/`.

///|
#derive(Eq)
enum Route {
  Home
  Search(CatalogQuery) // ?q=&category=&size=... (catalog.mbt)
  Part(String, String) // ?manufacturer=&mpn=
  KicadPlugin
  Contributing
  NotFound
}

///|
fn query_params(query : String?) -> Map[String, String] {
  let out : Map[String, String] = Map([])
  guard query is Some(q) else { return out }
  for pair in q.split("&") {
    let pair = pair.to_owned()
    if pair.is_empty() {
      continue
    }
    match pair.find("=") {
      Some(i) =>
        out[decode_uri_component(pair[:i].to_owned())] = decode_uri_component(
          pair[i + 1:].to_owned(),
        )
      None => out[decode_uri_component(pair)] = ""
    }
  }
  out
}

///|
fn strip_slashes(s : String) -> String {
  let mut v = s
  while v.has_prefix("/") {
    v = v[1:].to_owned()
  }
  while v.has_suffix("/") {
    v = v[:v.length() - 1].to_owned()
  }
  v
}

///|
fn route_of(url : @url.Url) -> (Lang, Route) {
  let base = strip_slashes(base_path())
  let mut path = strip_slashes(url.path)
  if !base.is_empty() {
    if path == base {
      path = ""
    } else if path.has_prefix(base + "/") {
      path = path[base.length() + 1:].to_owned()
    }
  }
  path = path.replace(old="index.html", new="") |> strip_slashes
  let (lang, rest) = if path == "en" {
    (En, "")
  } else if path.has_prefix("en/") {
    (En, path[3:].to_owned())
  } else {
    (Ja, path)
  }
  let params = query_params(url.query)
  let route = match rest {
    "" => Home
    "search" => Search(catalog_query_of(params))
    "parts" =>
      match (params.get("manufacturer"), params.get("mpn")) {
        (Some(m), Some(p)) if !m.is_empty() && !p.is_empty() => Part(m, p)
        _ => NotFound
      }
    "kicad-plugin" => KicadPlugin
    "contributing" => Contributing
    _ => NotFound
  }
  (lang, route)
}

///|
/// The URL of `path` (no leading slash, may carry a query) in `lang`.
fn href(lang : Lang, path : String) -> String {
  base_path() +
  (match lang {
    En => "en/"
    Ja => ""
  }) +
  path
}

///|
fn part_path(manufacturer : String, mpn : String) -> String {
  "parts?manufacturer=\{encode_uri_component(manufacturer)}&mpn=\{encode_uri_component(mpn)}"
}

///|
/// `route`'s path, for the language switch.
fn route_path(route : Route) -> String {
  match route {
    Home | NotFound => ""
    Search(q) => catalog_path(q)
    Part(m, p) => part_path(m, p)
    KicadPlugin => "kicad-plugin"
    Contributing => "contributing"
  }
}
