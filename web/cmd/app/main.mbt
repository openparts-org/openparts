// The OpenParts web site: a single-page rabbita app. The URL is the
// state's source of truth (`route_of`); data comes from the API.

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
  catalog : CatalogState
  /// (manufacturer, mpn) the part page has loaded or is loading.
  part_key : (String, String)
  part : Fetch[PartData]
  zip : ZipState
  viewer_error : String?
  /// The BOM check page (bom.mbt).
  bom : BomState
  /// The part page's alternatives (alternatives.mbt).
  alts : AltState
  review : ReviewState
}

///|
enum Msg {
  UrlChanged(@url.Url)
  UrlRequest(@url.UrlRequest)
  Catalog(CatalogMsg)
  Review(ReviewMsg)
  PartGot((String, String), Fetch[PartData])
  ViewerDone(String?)
  ZipClicked
  ZipDone(String?)
  Bom(BomMsg)
  Alt(AltMsg)
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
    Bom => lang.bom().title
    Reviews(_) => lang.review().title
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
  datasheet_set_labels(datasheet_labels(lang))
  let model = { ..model, lang, route, }
  if !(route is Part(_, _)) {
    // Off the part page: the viewer's views (and listeners) go too.
    (stop_viewer.val)()
  }
  match route {
    Search(q) => catalog_enter(model, q, emit)
    Bom => bom_enter(model, emit)
    Reviews(q) => review_enter(model, q, emit)
    Part(m, p) => {
      let key = (m, p)
      let (model, alt_cmd) = alt_enter(model, m, p, emit)
      if key == model.part_key && !(model.part is Failed(_)) {
        // Back to the same part: the 3D view has to be drawn again.
        if model.part is Got(_) {
          return (
            model,
            @rabbita.batch([viewer_cmd(lang, m, p, emit), alt_cmd]),
          )
        }
        return (model, alt_cmd)
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
        @rabbita.batch([cmd, alt_cmd]),
      )
    }
    _ => (model, @rabbita.none)
  }
}

///|
fn viewer_cmd(
  lang : Lang,
  m : String,
  p : String,
  emit : @rabbita.Emit[Msg],
) -> @rabbita.Cmd {
  @rabbita.perform(e => emit(ViewerDone(e)), async fn() noraise {
    show_model(lang, m, p)
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
      // Refining the catalog (a filter, the sort) keeps the scroll.
      if !(model.route is Search(_) && route_of(url).1 is Search(_)) {
        scroll_top()
      }
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
    Catalog(m) => catalog_update(model, m, emit)
    Review(m) => review_update(model, m, emit)
    PartGot(key, result) => {
      guard key == model.part_key else { (model, @rabbita.none) }
      let model = { ..model, part: result, }
      match result {
        Got(_) => (model, viewer_cmd(model.lang, key.0, key.1, emit))
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
    Bom(m) => bom_update(model, m, emit)
    Alt(m) => alt_update(model, m)
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
          catalog: CatalogState::new(),
          part_key: ("", ""),
          part: Loading,
          zip: Idle,
          viewer_error: None,
          bom: BomState::new(),
          alts: AltState::new(),
          review: ReviewState::new(),
        }
        let url = try! @url.parse(current_href())
        enter(model, url, emit)
      },
      update~,
      subscriptions=(model, emit) => {
        let subs = [
          @sub.on_url_changed(emit.map(u => UrlChanged(u))),
          @sub.on_url_request(emit.map(r => UrlRequest(r))),
        ]
        // The review queue's keyboard shortcuts.
        if model.route is Reviews(_) {
          subs.push(@sub.on_key_down(emit.map(k => Review(RevKey(k.key())))))
        }
        @sub.batch(subs)
      },
    )
    model.view(m => view(m, emit))
  }).mount("app")
}
