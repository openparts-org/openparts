// The OpenParts web site: a single-page rabbita app. The URL is the
// state's source of truth (`route_of`); data comes from the API.

///|
#derive(Eq)
struct SearchState {
  query : String
  category : @api.ComponentCategory?
  /// Bumped per request, so a slow old response can't replace a newer one.
  seq : Int32
  results : Fetch[Array[@api.PartSummary]]
}

///|
#derive(Eq)
enum ZipState {
  Idle
  Building
  ZipFailed(String)
}

///|
#derive(Eq)
struct Model {
  lang : Lang
  route : Route
  search : SearchState
  /// (manufacturer, mpn) the part page has loaded or is loading.
  part_key : (String, String)
  part : Fetch[PartData]
  zip : ZipState
  viewer_error : String?
}

///|
enum Msg {
  UrlChanged(@url.Url)
  UrlRequest(@url.UrlRequest)
  SearchInput(String)
  SearchCategory(@api.ComponentCategory?)
  SearchGot(Int32, Fetch[Array[@api.PartSummary]])
  PartGot((String, String), Fetch[PartData])
  ViewerDone(String?)
  ZipClicked
  ZipDone(String?)
}

///|
fn search_cmd(s : SearchState, emit : @rabbita.Emit[Msg]) -> @rabbita.Cmd {
  let seq = s.seq
  let mut path = "/v1/search?q=\{encode_uri_component(s.query)}"
  if s.category is Some(c) {
    path = path + "&category=" + c.wire()
  }
  @rabbita.perform(r => emit(SearchGot(seq, r)), async fn() noraise {
    get_json(path)
  })
}

///|
fn title_of(lang : Lang, route : Route) -> String {
  let c = lang.copy()
  let page = match route {
    Home => ""
    Search(_) => c.search_title
    Part(_, mpn) => mpn
    KicadPlugin => c.kicad_title
    Contributing => c.contrib_title
    NotFound => c.not_found_title
  }
  if page.is_empty() {
    "OpenParts"
  } else {
    "\{page} | OpenParts"
  }
}

///|
/// Moves to `url`: the page's state follows, and its data is fetched.
fn enter(
  model : Model,
  url : @url.Url,
  emit : @rabbita.Emit[Msg],
) -> (Model, @rabbita.Cmd) {
  let (lang, route) = route_of(url)
  set_document_lang(lang.code())
  set_document_title(title_of(lang, route))
  let model = { ..model, lang, route, }
  match route {
    Search(q) => {
      // Fetched already (typing replaces the URL, which lands here too)?
      let stale = model.search.seq == 0 ||
        q != model.search.query ||
        model.search.results is Failed(_)
      if stale {
        let search = {
          ..model.search,
          query: q,
          seq: model.search.seq + 1,
          results: Loading,
        }
        ({ ..model, search, }, search_cmd(search, emit))
      } else {
        (model, @rabbita.none)
      }
    }
    Part(m, p) => {
      let key = (m, p)
      if key == model.part_key && !(model.part is Failed(_)) {
        // Back to the same part: the 3D view has to be drawn again.
        if model.part is Got(_) {
          return (model, viewer_cmd(m, p, emit))
        }
        return (model, @rabbita.none)
      }
      let cmd = @rabbita.perform(r => emit(PartGot(key, r)), async fn() noraise {
        load_part(m, p)
      })
      (
        {
          ..model,
          part_key: key,
          part: Loading,
          zip: Idle,
          viewer_error: None,
        },
        cmd,
      )
    }
    _ => (model, @rabbita.none)
  }
}

///|
fn viewer_cmd(
  m : String,
  p : String,
  emit : @rabbita.Emit[Msg],
) -> @rabbita.Cmd {
  @rabbita.perform(e => emit(ViewerDone(e)), async fn() noraise {
    show_model(m, p)
  })
}

///|
fn update(
  model : Model,
  msg : Msg,
  emit : @rabbita.Emit[Msg],
) -> (Model, @rabbita.Cmd) {
  match msg {
    UrlChanged(url) => {
      scroll_top()
      enter(model, url, emit)
    }
    UrlRequest(Internal(url)) => {
      let path = "/" + strip_slashes(url.path)
      // Links into the API (downloads) are left to the browser.
      if path.has_prefix(api_base() + "/") {
        (model, @nav.load(url.to_string()))
      } else {
        (model, @nav.push_url(url.to_string()))
      }
    }
    UrlRequest(External(href)) => (model, @nav.load(href))
    SearchInput(q) => {
      let search = {
        ..model.search,
        query: q,
        seq: model.search.seq + 1,
        results: Loading,
      }
      let path = route_path(Search(q))
      (
        { ..model, search, route: Search(q), },
        @rabbita.batch([
          search_cmd(search, emit),
          @nav.replace_url(href(model.lang, path)),
        ]),
      )
    }
    SearchCategory(category) => {
      let search = {
        ..model.search,
        category,
        seq: model.search.seq + 1,
        results: Loading,
      }
      ({ ..model, search, }, search_cmd(search, emit))
    }
    SearchGot(seq, results) =>
      if seq == model.search.seq {
        ({ ..model, search: { ..model.search, results, }, }, @rabbita.none)
      } else {
        (model, @rabbita.none)
      }
    PartGot(key, result) => {
      guard key == model.part_key else { (model, @rabbita.none) }
      let model = { ..model, part: result, }
      match result {
        Got(_) => (model, viewer_cmd(key.0, key.1, emit))
        _ => (model, @rabbita.none)
      }
    }
    ViewerDone(e) => ({ ..model, viewer_error: e, }, @rabbita.none)
    ZipClicked => {
      let (m, p) = model.part_key
      let cmd = @rabbita.perform(r => emit(ZipDone(r)), async fn() noraise {
        match kicad_zip(m, p) {
          Ok(bytes) => {
            save_file(bytes, "\{p}-kicad.zip", "application/zip")
            None
          }
          Err(e) => Some(e)
        }
      })
      ({ ..model, zip: Building, }, cmd)
    }
    ZipDone(None) => ({ ..model, zip: Idle, }, @rabbita.none)
    ZipDone(Some(e)) => ({ ..model, zip: ZipFailed(e), }, @rabbita.none)
  }
}

///|
fn main {
  @rabbita.new(fn() {
    let (model, emit) = @rabbita.create_state_with_init(
      init=emit => {
        let model : Model = {
          lang: Ja,
          route: Home,
          search: { query: "", category: None, seq: 0, results: Loading, },
          part_key: ("", ""),
          part: Loading,
          zip: Idle,
          viewer_error: None,
        }
        let url = try! @url.parse(current_href())
        enter(model, url, emit)
      },
      update~,
      subscriptions=(_, emit) => {
        @sub.batch([
          @sub.on_url_changed(emit.map(u => UrlChanged(u))),
          @sub.on_url_request(emit.map(r => UrlRequest(r))),
        ])
      },
    )
    model.view(m => view(m, emit))
  }).mount("app")
}
