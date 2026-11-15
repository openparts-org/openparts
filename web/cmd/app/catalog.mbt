// The parts catalog at /search: a category entry screen, then a
// parametric results screen (facet sidebar, active-filter chips, a
// sortable, paged table) over `GET /v1/catalog`. The filters live in
// the URL query, so a filtered view can be shared and back/forward
// steps through it.

///|
/// The multi-select facets (URL and API parameter names), in sidebar order.
let facet_keys : Array[String] = [
  "category", "manufacturer", "family", "size", "pins", "pitch", "mounting", "lifecycle",
  "existence", "review", "rows", "gender", "orientation", "shrouded", "exposed_pad",
]

///|
/// Every parameter a pick can have: the facets, the range bounds, then
/// each electrical parameter with its bounds.
let pick_keys : Array[String] = build_pick_keys()

///|
fn build_pick_keys() -> Array[String] {
  let out = [
    "category", "manufacturer", "family", "size", "pins", "pitch", "mounting", "lifecycle",
    "existence", "review", "rows", "gender", "orientation", "shrouded", "exposed_pad",
    "width_min", "width_max", "length_min", "length_max", "height_min", "height_max",
  ]
  for p in param_names {
    out.push(p.0)
    out.push(p.0 + "_min")
    out.push(p.0 + "_max")
  }
  out
}

///|
let sort_keys : Array[String] = [
  "", "mpn", "manufacturer", "pins", "pitch", "size", "height",
]

///|
/// The enumerated parameter keys (checkboxes only, not sortable).
let enum_param_keys : Array[String] = [
  "dielectric", "color", "polarity", "output_type",
]

///|
fn is_param_key(key : String) -> Bool {
  param_names.iter().any(p => p.0 == key)
}

///|
/// A parameter range bound (`capacitance_min`) as (key, "min"/"max").
fn param_bound(pick : String) -> (String, String)? {
  for p in param_names {
    if pick == p.0 + "_min" {
      return Some((p.0, "min"))
    }
    if pick == p.0 + "_max" {
      return Some((p.0, "max"))
    }
  }
  None
}

///|
/// The top-level category of `value` in the tree (itself if top-level).
fn category_root(tree : Array[@api.CategoryNode], value : String) -> String {
  for root in tree {
    if root.value == value {
      return value
    }
    for child in root.children {
      if child.value == value {
        return root.value
      }
    }
  }
  value
}

///|
/// The parameter columns of the results table for one picked category
/// (`root` is its top-level category).
fn param_columns(category : String, root~ : String = category) -> Array[String] {
  match category {
    "linear_regulator" =>
      ["output_type", "output_voltage", "output_current", "dropout_voltage"]
    _ => param_columns_of_root(if root.is_empty() { category } else { root })
  }
}

///|
fn param_columns_of_root(category : String) -> Array[String] {
  match category {
    "power" =>
      ["output_type", "output_voltage", "output_current", "input_voltage_max"]
    "memory" => ["capacity"]
    "switch" => ["actuation_force", "travel", "rated_voltage", "rated_current"]
    "capacitor" => ["capacitance", "rated_voltage", "dielectric", "tolerance"]
    "resistor" =>
      ["resistance", "tolerance", "power_rating", "temperature_coefficient"]
    "inductor" => ["inductance", "rated_current", "saturation_current", "dcr"]
    "crystal" => ["frequency", "load_capacitance", "frequency_tolerance", "esr"]
    "diode" =>
      ["reverse_voltage", "forward_current", "forward_voltage", "zener_voltage"]
    "optocoupler" =>
      [
        "current_transfer_ratio", "isolation_voltage", "collector_emitter_voltage",
        "forward_voltage",
      ]
    "led" => ["color", "wavelength", "forward_voltage"]
    "transistor" =>
      ["polarity", "drain_source_voltage", "drain_current", "rds_on"]
    "fuse" => ["rated_current", "rated_voltage"]
    _ => []
  }
}

///|
let page_size : Int32 = 50

///|
/// What the catalog shows: the text query, the picked filter values,
/// order and page. `all` keeps the results screen with nothing picked.
#derive(Eq)
struct CatalogQuery {
  q : String
  /// (parameter, value) of every picked option and range bound, in
  /// `pick_keys` order.
  picks : Array[(String, String)]
  sort : String
  desc : Bool
  offset : Int32
  all : Bool
}

///|
fn CatalogQuery::empty() -> CatalogQuery {
  { q: "", picks: [], sort: "", desc: false, offset: 0, all: false, }
}

///|
/// The category screen: nothing searched or picked yet.
fn CatalogQuery::is_entry(self : CatalogQuery) -> Bool {
  self.q.is_empty() && self.picks.is_empty() && !self.all
}

///|
fn CatalogQuery::values(self : CatalogQuery, key : String) -> Array[String] {
  let out = []
  for p in self.picks {
    if p.0 == key {
      out.push(p.1)
    }
  }
  out
}

///|
fn CatalogQuery::value(self : CatalogQuery, key : String) -> String {
  for p in self.picks {
    if p.0 == key {
      return p.1
    }
  }
  ""
}

///|
fn normalize_picks(picks : Array[(String, String)]) -> Array[(String, String)] {
  let out = []
  for k in pick_keys {
    for p in picks {
      if p.0 == k && !out.contains(p) {
        out.push(p)
      }
    }
  }
  out
}

///|
/// Picks or unpicks one facet option (back to the first page).
fn CatalogQuery::toggle(
  self : CatalogQuery,
  key : String,
  value : String,
) -> CatalogQuery {
  let pick = (key, value)
  let picks = if self.picks.contains(pick) {
    self.picks.filter(p => p != pick)
  } else {
    let more = self.picks.copy()
    more.push(pick)
    more
  }
  { ..self, picks: normalize_picks(picks), offset: 0, all: true, }
}

///|
/// Sets a range bound (`width_min`...); empty clears it.
fn CatalogQuery::set_range(
  self : CatalogQuery,
  key : String,
  value : String,
) -> CatalogQuery {
  let picks = self.picks.filter(p => p.0 != key)
  let value = value.trim().to_owned()
  if !value.is_empty() {
    picks.push((key, value))
  }
  { ..self, picks: normalize_picks(picks), offset: 0, all: true, }
}

///|
/// The query string, without `?`: for the site's URL, or (`api`) for
/// `/v1/catalog`.
fn CatalogQuery::params(self : CatalogQuery, api~ : Bool) -> Array[String] {
  let out = []
  if !self.q.is_empty() {
    out.push("q=" + encode_uri_component(self.q))
  }
  for k in pick_keys {
    let vs = self.values(k)
    if !vs.is_empty() {
      out.push(k + "=" + vs.map(v => encode_uri_component(v)).join(","))
    }
  }
  if !self.sort.is_empty() {
    out.push("sort=" + self.sort)
  }
  if self.desc {
    out.push("order=desc")
  }
  if self.offset > 0 {
    out.push("offset=\{self.offset}")
  }
  if !api && self.all && self.q.is_empty() && self.picks.is_empty() {
    out.push("all=1")
  }
  out
}

///|
/// The site path of `q` ("search?..."), for links and the URL bar.
fn catalog_path(q : CatalogQuery) -> String {
  let params = q.params(api=false)
  if params.is_empty() {
    "search"
  } else {
    "search?" + params.join("&")
  }
}

///|
/// The catalog query in a /search URL's parameters (old `?q=` links
/// included). Values of one facet are comma-separated.
fn catalog_query_of(params : Map[String, String]) -> CatalogQuery {
  let picks = []
  for k in pick_keys {
    if params.get(k) is Some(v) {
      for part in v.split(",") {
        let part = part.trim().to_owned()
        if !part.is_empty() {
          picks.push((k, part))
        }
      }
    }
  }
  let sort = params.get("sort").unwrap_or("")
  let offset = parse_count(params.get("offset").unwrap_or("0"))
  {
    q: params.get("q").unwrap_or(""),
    picks: normalize_picks(picks),
    sort: if sort_keys.contains(sort) ||
      (is_param_key(sort) && !enum_param_keys.contains(sort)) {
      sort
    } else {
      ""
    },
    desc: params.get("order") == Some("desc"),
    offset: offset - offset % page_size,
    all: params.get("all") is Some(_),
  }
}

///|
/// A non-negative integer, or 0.
extern "js" fn parse_count(s : String) -> Int32 =
  #|(s) => { const n = parseInt(s, 10); return Number.isFinite(n) && n > 0 ? n : 0; }

///|
extern "js" fn timeout(ms : Int32) -> @js_async.Promise[Bool] =
  #|(ms) => new Promise((resolve) => setTimeout(() => resolve(true), ms))

///|
async fn delay(ms : Int32) -> Unit noraise {
  ignore(timeout(ms).wait()) catch {
    _ => ()
  }
}

///|
#derive(Eq)
struct CatalogState {
  /// What is shown (or being fetched); follows the URL.
  query : CatalogQuery
  /// The search box, ahead of `query.q` while typing.
  text : String
  /// Bumped per keystroke: only the last one's pause commits the text.
  typing : Int32
  /// Bumped per request, so a slow old response can't replace a newer one.
  seq : Int32
  loading : Bool
  results : Fetch[@api.CatalogResponse]
  /// Facet groups the user folded, and `more:<group>` for long lists
  /// shown in full.
  folded : Array[String]
  /// The filter drawer on phone-width screens.
  drawer : Bool
}

///|
fn CatalogState::new() -> CatalogState {
  {
    query: CatalogQuery::empty(),
    text: "",
    typing: 0,
    seq: 0,
    loading: false,
    results: Loading,
    folded: [],
    drawer: false,
  }
}

///|
enum CatalogMsg {
  CatInput(String)
  CatDebounced(Int32)
  CatToggle(String, String)
  CatRange(String, String)
  CatClear
  CatSort(String)
  CatSortSelect(String)
  CatOrder
  CatPage(Int32)
  CatFold(String)
  CatDrawer(Bool)
  CatGot(Int32, Fetch[@api.CatalogResponse])
}

///|
fn catalog_fetch(s : CatalogState, emit : Emit[Msg]) -> @rabbita.Cmd {
  let seq = s.seq
  let params = s.query.params(api=true)
  params.push(
    "limit=" + (if s.query.is_entry() { "0" } else { page_size.to_string() }),
  )
  let path = "/v1/catalog?" + params.join("&")
  @rabbita.perform(r => emit(Catalog(CatGot(seq, r))), async fn() noraise {
    get_json(path)
  })
}

///|
/// The URL moved to the catalog with `q`: fetch unless that's loaded.
fn catalog_enter(
  model : Model,
  q : CatalogQuery,
  emit : Emit[Msg],
) -> (Model, @rabbita.Cmd) {
  let s = model.catalog
  let stale = s.seq == 0 || q != s.query || s.results is Failed(_)
  guard stale else { (model, @rabbita.none) }
  // A query from elsewhere (back/forward, a link) replaces the box.
  let text = if s.seq == 0 || (q.q != s.query.q && q.q != s.text) {
    q.q
  } else {
    s.text
  }
  let s = { ..s, query: q, text, seq: s.seq + 1, loading: true, }
  ({ ..model, catalog: s, }, catalog_fetch(s, emit))
}

///|
fn catalog_update(
  model : Model,
  msg : CatalogMsg,
  emit : Emit[Msg],
) -> (Model, @rabbita.Cmd) {
  let s = model.catalog
  let go = (q : CatalogQuery) => {
    (model, @nav.push_url(href(model.lang, catalog_path(q))))
  }
  match msg {
    CatInput(text) => {
      let typing = s.typing + 1
      let wait = @rabbita.perform(n => emit(Catalog(CatDebounced(n))), async fn() noraise {
        delay(250)
        typing
      })
      ({ ..model, catalog: { ..s, text, typing, }, }, wait)
    }
    CatDebounced(n) => {
      guard n == s.typing && s.text != s.query.q else { (model, @rabbita.none) }
      let q = { ..s.query, q: s.text, offset: 0, all: true, }
      (model, @nav.replace_url(href(model.lang, catalog_path(q))))
    }
    CatToggle(key, value) => go(s.query.toggle(key, value))
    CatRange(key, value) => go(s.query.set_range(key, value))
    CatClear => go({ ..s.query, picks: [], offset: 0, all: true, })
    CatSort(key) =>
      go({
        ..s.query,
        sort: key,
        desc: s.query.sort == key && !s.query.desc,
        offset: 0,
        all: true,
      })
    CatSortSelect(key) =>
      go({ ..s.query, sort: key, desc: false, offset: 0, all: true, })
    CatOrder => go({ ..s.query, desc: !s.query.desc, offset: 0, all: true, })
    CatPage(offset) => {
      scroll_top()
      go({ ..s.query, offset, all: true, })
    }
    CatFold(key) => {
      let folded = if s.folded.contains(key) {
        s.folded.filter(k => k != key)
      } else {
        let more = s.folded.copy()
        more.push(key)
        more
      }
      ({ ..model, catalog: { ..s, folded, }, }, @rabbita.none)
    }
    CatDrawer(open) =>
      ({ ..model, catalog: { ..s, drawer: open, }, }, @rabbita.none)
    CatGot(seq, results) =>
      if seq == s.seq {
        (
          { ..model, catalog: { ..s, results, loading: false, }, },
          @rabbita.none,
        )
      } else {
        (model, @rabbita.none)
      }
  }
}

// ---- labels ----

///|
fn category_of(value : String) -> @api.ComponentCategory? {
  for c in @api.ComponentCategory::all() {
    if c.wire() == value {
      return Some(c)
    }
  }
  None
}

///|
/// A facet option's label in `lang`.
fn option_label(
  lang : Lang,
  key : String,
  value : String,
  fallback : String,
) -> String {
  match key {
    "category" =>
      match category_of(value) {
        Some(c) => category_label(lang, c)
        None => fallback
      }
    "family" => family_label(lang, value, fallback)
    "mounting" => mounting_label(lang, value)
    "lifecycle" => lifecycle_label(lang, value)
    "existence" => existence_label(lang, value)
    "review" => review_option_label(lang, value)
    "gender" | "orientation" | "shrouded" | "exposed_pad" =>
      structure_label(lang, key, value).unwrap_or(fallback)
    k if enum_param_keys.contains(k) => param_value_label(lang, k, value)
    _ => fallback
  }
}

///|
fn group_title(k : CatalogCopy, key : String) -> String {
  match key {
    "category" => k.f_category
    "manufacturer" => k.f_manufacturer
    "family" => k.f_family
    "size" => k.f_size
    "pins" => k.f_pins
    "pitch" => k.f_pitch
    "mounting" => k.f_mounting
    "lifecycle" => k.f_lifecycle
    "existence" => k.f_existence
    "review" => k.f_review
    "rows" => k.f_rows
    "gender" => k.f_gender
    "orientation" => k.f_orientation
    "shrouded" => k.f_shrouded
    "exposed_pad" => k.f_exposed_pad
    "width" => k.f_width
    "length" => k.f_length
    _ => k.f_height
  }
}

///|
fn facet_values(f : @api.CatalogFacets, key : String) -> Array[@api.FacetValue] {
  match key {
    "category" => f.category
    "manufacturer" => f.manufacturer
    "family" => f.package_family
    "size" => f.size
    "pins" => f.pins
    "pitch" => f.pitch
    "mounting" => f.mounting
    "lifecycle" => f.lifecycle
    "review" => f.review
    "rows" => f.rows
    "gender" => f.gender
    "orientation" => f.orientation
    "shrouded" => f.shrouded
    "exposed_pad" => f.exposed_pad
    _ => f.existence
  }
}

///|
/// The short text of an active-filter chip.
fn pick_label(
  lang : Lang,
  f : @api.CatalogFacets?,
  key : String,
  value : String,
) -> String {
  let mut fallback = value
  if f is Some(f) && facet_keys.contains(key) {
    for v in facet_values(f, key) {
      if v.value == value {
        fallback = v.label
      }
    }
  }
  if f is Some(f) && is_param_key(key) {
    for pf in f.parameters {
      if pf.key == key {
        for v in pf.values {
          if v.value == value {
            fallback = v.label
          }
        }
      }
    }
  }
  if is_param_key(key) {
    return param_name(lang, key) +
      ": " +
      option_label(lang, key, value, fallback)
  }
  if param_bound(key) is Some((k, bound)) {
    return range_chip(lang, param_name(lang, k), bound, value)
  }
  match key {
    "pins" => pins_chip(lang, value)
    "pitch" => pitch_chip(lang, value)
    "width_min" => "W ≥ \{value} mm"
    "width_max" => "W ≤ \{value} mm"
    "length_min" => "L ≥ \{value} mm"
    "length_max" => "L ≤ \{value} mm"
    "height_min" => "H ≥ \{value} mm"
    "height_max" => "H ≤ \{value} mm"
    _ => option_label(lang, key, value, fallback)
  }
}

///|
fn mm(v : Double?) -> String {
  match v {
    Some(v) => format_number(v)
    None => "—"
  }
}

///|
/// The part of a package id after its last `/` (`QFN56-RP2040`).
fn package_name(id : String) -> String {
  let mut out = id
  for part in id.split("/") {
    out = part.to_owned()
  }
  out
}

// ---- views ----

///|
/// The short mark on a top-level category tile (a reference designator
/// where there is one).
fn category_glyph(c : @api.ComponentCategory) -> String {
  match c {
    Microcontroller => "U"
    Capacitor => "C"
    Resistor => "R"
    Connector => "J"
    Crystal => "Y"
    Diode => "D"
    Transistor => "Q"
    Led => "LED"
    Inductor => "L"
    Fuse => "F"
    Optocoupler => "OPT"
    Switch | TactSwitch => "SW"
    Power
    | LinearRegulator
    | SwitchingRegulator
    | BatteryCharger
    | VoltageReference
    | PowerSwitch => "PWR"
    Memory | Flash | Eeprom | Sram => "MEM"
    Sensor | PressureSensor | Imu | TemperatureSensor => "SNS"
    Analog
    | OpAmp
    | Comparator
    | CurrentSenseAmplifier
    | PowerMonitor
    | AnalogSwitch
    | Adc
    | Dac => "AMP"
    Interface
    | IoExpander
    | CanController
    | CanTransceiver
    | Rs485Transceiver
    | Rs232Transceiver
    | LevelShifter => "I/F"
    Logic => "74"
    Timing | Timer | Rtc => "TMR"
    Driver | GateDriver | DarlingtonArray => "DRV"
    Other => "IC"
  }
}

///|
/// A category value's label in `lang` (the server's if unknown here).
fn category_value_label(
  lang : Lang,
  value : String,
  fallback : String,
) -> String {
  match category_of(value) {
    Some(c) => category_label(lang, c)
    None => fallback
  }
}

///|
fn catalog_view(model : Model, emit : Emit[Msg]) -> Html {
  let lang = model.lang
  let s = model.catalog
  let box = input(
    input_type=Search,
    class="search-input",
    placeholder=lang.copy().search_placeholder,
    value=s.text,
    autofocus=true,
    on_input=emit.map(t => Catalog(CatInput(t))),
  )
  if s.query.is_entry() {
    catalog_entry_view(model, box)
  } else {
    catalog_results_view(model, emit, box)
  }
}

///|
fn fetch_message(lang : Lang, r : Fetch[@api.CatalogResponse]) -> Html {
  let c = lang.copy()
  match r {
    Loading | Got(_) => p(class="text-muted", c.loading)
    Failed(e) =>
      p(class="text-muted", if e.is_empty() { c.search_failed } else { e })
    Missing => p(class="text-muted", c.search_failed)
  }
}

///|
fn catalog_entry_view(model : Model, box : Html) -> Html {
  let lang = model.lang
  let c = lang.copy()
  let k = lang.catalog()
  let head = [h1(c.search_title), box]
  guard model.catalog.results is Got(r) else {
    div(
      class="catalog catalog-entry",
      head + [fetch_message(lang, model.catalog.results)],
    )
  }
  let tile = (path : String, glyph : String, name : String, n : Int32) => {
    a(class="catalog-tile", href=href(lang, path), [
      span(class="catalog-tile-glyph", glyph),
      span(class="catalog-tile-name", name),
      span(class="catalog-tile-count", parts_count(lang, n)),
    ])
  }
  let pick_path = (value : String) => {
    catalog_path({ ..CatalogQuery::empty(), picks: [("category", value)], })
  }
  // One tile per top-level category; a group lists its subcategories.
  // Plain categories first, then the (taller) groups.
  let tiles = []
  let groups = []
  for node in r.facets.category_tree {
    guard node.count > 0 else { continue }
    let glyph = match category_of(node.value) {
      Some(c) => category_glyph(c)
      None => "IC"
    }
    let main = tile(
      pick_path(node.value),
      glyph,
      category_value_label(lang, node.value, node.label),
      node.count,
    )
    if node.children.is_empty() {
      tiles.push(main)
    } else {
      let subs = node.children.map(child => {
        a(class="catalog-tile-sub", href=href(lang, pick_path(child.value)), [
          text(category_value_label(lang, child.value, child.label)),
          span(class="chip-count", child.count.to_string()),
        ])
      })
      groups.push(
        div(class="catalog-tile-group", [
          main,
          div(class="catalog-tile-subs", subs),
        ]),
      )
    }
  }
  tiles.append(groups)
  tiles.push(
    tile(
      catalog_path({ ..CatalogQuery::empty(), all: true, }),
      "*",
      k.browse_all,
      r.total,
    ),
  )
  let link_chips = (
    key : String,
    values : Array[@api.FacetValue],
    limit : Int32,
  ) => {
    let out = []
    for v in values {
      if out.length() >= limit {
        break
      }
      out.push(
        a(
          class="chip catalog-link-chip",
          href=href(
            lang,
            catalog_path({ ..CatalogQuery::empty(), picks: [(key, v.value)], }),
          ),
          [
            text(option_label(lang, key, v.value, v.label)),
            span(class="chip-count", v.count.to_string()),
          ],
        ),
      )
    }
    div(class="catalog-link-chips", out)
  }
  div(
    class="catalog catalog-entry",
    head +
    [
      div(class="catalog-tiles", tiles),
      h2(class="catalog-subhead", k.by_size),
      link_chips("size", r.facets.size, 20),
      h2(class="catalog-subhead", k.by_family),
      link_chips("family", r.facets.package_family, 24),
    ],
  )
}

///|
fn facet_group(
  s : CatalogState,
  emit : Emit[Msg],
  key : String,
  title : String,
  body : Array[Html],
) -> Html {
  let open = !s.folded.contains(key)
  let head = button(
    type_="button",
    class="facet-head",
    on_click=emit(Catalog(CatFold(key))),
    attrs=@html.Attrs::build().aria_expanded(open.to_string()),
    [span(title), span(class="facet-caret", if open { "−" } else { "+" })],
  )
  div(
    class="facet",
    if open {
      [head, div(class="facet-body", body)]
    } else {
      [head]
    },
  )
}

///|
/// A checkbox per option; long lists show `limit` (and every picked
/// one) until expanded.
fn facet_checks(
  lang : Lang,
  s : CatalogState,
  emit : Emit[Msg],
  key : String,
  values : Array[@api.FacetValue],
  limit : Int32,
) -> Array[Html] {
  let expanded = s.folded.contains("more:" + key)
  let out = []
  let mut hidden = 0
  for i, v in values {
    if !expanded && i >= limit && !v.selected {
      hidden += 1
      continue
    }
    out.push(
      @html.label(
        class=if v.count == 0 {
          "facet-option facet-empty"
        } else {
          "facet-option"
        },
        [
          input(
            input_type=Checkbox,
            checked=v.selected,
            on_change=emit.map(_ => Catalog(CatToggle(key, v.value))),
          ),
          span(class="facet-label", option_label(lang, key, v.value, v.label)),
          span(class="facet-count", v.count.to_string()),
        ],
      ),
    )
  }
  if hidden > 0 || (expanded && values.length() > limit) {
    out.push(
      button(
        type_="button",
        class="facet-more",
        on_click=emit(Catalog(CatFold("more:" + key))),
        if expanded {
          lang.catalog().show_less
        } else {
          show_more(lang, hidden)
        },
      ),
    )
  }
  out
}

///|
fn facet_chips(
  emit : Emit[Msg],
  key : String,
  values : Array[@api.FacetValue],
) -> Array[Html] {
  [
    div(
      class="facet-chips",
      values.map(v => {
        button(
          type_="button",
          class=if v.selected { "chip chip-active" } else { "chip" },
          on_click=emit(Catalog(CatToggle(key, v.value))),
          attrs=@html.Attrs::build().aria_pressed(v.selected.to_string()),
          [text(v.label), span(class="chip-count", v.count.to_string())],
        )
      }),
    ),
  ]
}

///|
fn facet_range(
  lang : Lang,
  s : CatalogState,
  emit : Emit[Msg],
  axis : String,
  span_ : @api.RangeFacet,
) -> Array[Html] {
  let k = lang.catalog()
  let bound = (which : String, hint : Double?, title : String) => {
    let key = axis + "_" + which
    input(
      input_type=Number,
      class="range-input",
      value=s.query.value(key),
      placeholder=match hint {
        Some(v) => format_number(v)
        None => title
      },
      title~,
      attrs=@html.Attrs::build()
        .attribute("step", "any")
        .attribute("min", "0")
        .aria_label(group_title(k, axis) + " " + title),
      on_change=emit.map(v => Catalog(CatRange(key, v))),
    )
  }
  [
    div(class="facet-range", [
      bound("min", span_.min, k.min),
      span(class="text-muted", "–"),
      bound("max", span_.max, k.max),
    ]),
  ]
}

///|
/// The category facet as a tree: top-level categories, each with its
/// subcategories while it or one of them is picked (or it is the only
/// one with parts).
fn category_tree_checks(
  lang : Lang,
  s : CatalogState,
  emit : Emit[Msg],
  f : @api.CatalogFacets,
) -> Array[Html] {
  let k = lang.catalog()
  let expanded = s.folded.contains("more:category")
  let limit = 14
  let row = (node : @api.CategoryNode, child : Bool) => {
    @html.label(
      class=match (child, node.count) {
        (true, 0) => "facet-option facet-child facet-empty"
        (true, _) => "facet-option facet-child"
        (false, 0) => "facet-option facet-empty"
        _ => "facet-option"
      },
      [
        input(
          input_type=Checkbox,
          checked=node.selected,
          on_change=emit.map(_ => Catalog(CatToggle("category", node.value))),
        ),
        span(
          class="facet-label",
          category_value_label(lang, node.value, node.label),
        ),
        span(class="facet-count", node.count.to_string()),
      ],
    )
  }
  let with_parts = f.category_tree.filter(n => n.count > 0).length()
  let out = []
  let mut hidden = 0
  for i, node in f.category_tree {
    let open = node.selected ||
      node.children.iter().any(c => c.selected) ||
      with_parts == 1
    if !expanded && i >= limit && !open {
      hidden += 1
      continue
    }
    out.push(row(node, false))
    if open {
      for child in node.children {
        out.push(row(child, true))
      }
    }
  }
  if hidden > 0 || (expanded && f.category_tree.length() > limit) {
    out.push(
      button(
        type_="button",
        class="facet-more",
        on_click=emit(Catalog(CatFold("more:category"))),
        if expanded {
          k.show_less
        } else {
          show_more(lang, hidden)
        },
      ),
    )
  }
  out
}

///|
/// A parameter's range: text inputs, since values are typed with unit
/// prefixes (`10n`, `0.1u`, `4k7`).
fn param_range(
  lang : Lang,
  s : CatalogState,
  emit : Emit[Msg],
  pf : @api.ParameterFacet,
) -> Html {
  let k = lang.catalog()
  let bound = (which : String, hint : String?, title : String) => {
    let key = pf.key + "_" + which
    input(
      input_type=Text,
      class="range-input",
      value=s.query.value(key),
      placeholder=hint.unwrap_or(title),
      title="\{title} (\{k.unit_hint})",
      attrs=@html.Attrs::build()
        .attribute("inputmode", "text")
        .attribute("spellcheck", "false")
        .aria_label(param_name(lang, pf.key) + " " + title),
      on_change=emit.map(v => Catalog(CatRange(key, v))),
    )
  }
  div(class="facet-range", [
    bound("min", pf.min_label, k.min),
    span(class="text-muted", "–"),
    bound("max", pf.max_label, k.max),
  ])
}

///|
/// One electrical-parameter facet: its values as checkboxes and, for a
/// numeric key, a range.
fn param_facet(
  lang : Lang,
  s : CatalogState,
  emit : Emit[Msg],
  pf : @api.ParameterFacet,
) -> Html {
  let body = facet_checks(lang, s, emit, pf.key, pf.values, 8)
  if pf.kind == @api.ParameterKind::Numeric {
    body.push(param_range(lang, s, emit, pf))
  }
  facet_group(s, emit, pf.key, param_name(lang, pf.key), body)
}

///|
fn sidebar(
  lang : Lang,
  s : CatalogState,
  emit : Emit[Msg],
  r : @api.CatalogResponse,
) -> Array[Html] {
  let k = lang.catalog()
  let f = r.facets
  let group = (key : String, body : Array[Html]) => {
    facet_group(s, emit, key, group_title(k, key), body)
  }
  let out = []
  out.push(group("category", category_tree_checks(lang, s, emit, f)))
  // Cheap to check and what a reviewer looks for first.
  out.push(group("review", facet_checks(lang, s, emit, "review", f.review, 8)))
  for pf in f.parameters {
    out.push(param_facet(lang, s, emit, pf))
  }
  if !f.size.is_empty() {
    out.push(group("size", facet_chips(emit, "size", f.size)))
  }
  out.push(
    group("family", facet_checks(lang, s, emit, "family", f.package_family, 8)),
  )
  if !f.pins.is_empty() {
    out.push(group("pins", facet_chips(emit, "pins", f.pins)))
  }
  if !f.pitch.is_empty() {
    out.push(group("pitch", facet_chips(emit, "pitch", f.pitch)))
  }
  // The package's structure, where the results have it.
  if !f.rows.is_empty() {
    out.push(group("rows", facet_chips(emit, "rows", f.rows)))
  }
  for key in ["gender", "orientation", "shrouded", "exposed_pad"] {
    let values = facet_values(f, key)
    if !values.is_empty() {
      out.push(group(key, facet_checks(lang, s, emit, key, values, 8)))
    }
  }
  out.push(group("width", facet_range(lang, s, emit, "width", f.body_width)))
  out.push(group("length", facet_range(lang, s, emit, "length", f.body_length)))
  out.push(group("height", facet_range(lang, s, emit, "height", f.body_height)))
  out.push(
    group("mounting", facet_checks(lang, s, emit, "mounting", f.mounting, 8)),
  )
  out.push(
    group(
      "manufacturer",
      facet_checks(lang, s, emit, "manufacturer", f.manufacturer, 8),
    ),
  )
  out.push(
    group("lifecycle", facet_checks(lang, s, emit, "lifecycle", f.lifecycle, 8)),
  )
  out.push(
    group("existence", facet_checks(lang, s, emit, "existence", f.existence, 8)),
  )
  out
}

///|
fn lifecycle_class(s : @api.LifecycleStatus) -> String {
  match s {
    Active | PreRelease => "badge badge-good"
    Nrnd | EolAnnounced => "badge badge-warn"
    Obsolete => "badge badge-bad"
    Unknown => "badge badge-neutral"
  }
}

///|
/// One table row; `category` is false when one category is picked
/// (the column would repeat it).
fn result_row(
  lang : Lang,
  it : @api.CatalogItem,
  category~ : Bool,
  params~ : Array[String],
) -> Html {
  let k = lang.catalog()
  let size = match (it.size_code, it.size_code_metric) {
    (Some(i), Some(m)) => "\{i} / \{m}"
    (Some(i), None) => i
    _ => "—"
  }
  let family = match it.package_family {
    Some(f) => family_label(lang, f.wire(), f.wire().to_upper())
    None => "—"
  }
  // `data-label` names the cell in the phone layout (rows as cards).
  let cell = (label : String, class : String, body : Array[Html]) => {
    td(class~, attrs=@html.Attrs::build().attribute("data-label", label), body)
  }
  let cells = [
    cell(k.c_mpn, "col-mpn", [
      a(href=href(lang, part_path(it.manufacturer, it.mpn)), [strong(it.mpn)]),
      if it.review.state is Reviewed {
        span(
          class="badge badge-good badge-mini",
          "✓ " + lang.data_review().reviewed_mark,
        )
      } else {
        text("")
      },
    ]),
    td(
      class="col-maker",
      title=it.manufacturer_name,
      attrs=@html.Attrs::build().attribute("data-label", k.c_manufacturer),
      it.manufacturer_name,
    ),
  ]
  if category {
    let name = match it.category {
      Some(c) => category_label(lang, c)
      None => "—"
    }
    cells.push(cell(k.c_category, "nowrap", [text(name)]))
  }
  for key in params {
    let value = match it.parameters.get(key) {
      Some(p) => option_label(lang, key, p.label, p.label)
      None => "—"
    }
    cells.push(cell(param_name(lang, key), "num nowrap", [text(value)]))
  }
  cells.push(
    cell(k.c_package, "col-package", [
      span(family),
      span(
        class="text-muted col-sub",
        title=it.package_,
        package_name(it.package_),
      ),
    ]),
  )
  cells.push(cell(k.c_size, "num", [text(size)]))
  let pins = match it.lead_count {
    Some(n) => n.to_string()
    None => "—"
  }
  cells.push(cell(k.c_pins, "num", [text(pins)]))
  cells.push(cell(k.c_pitch, "num", [text(mm(it.pitch))]))
  cells.push(
    cell(k.c_dims, "num", [
      text(
        "\{mm(it.body_width)} × \{mm(it.body_length)} × \{mm(it.body_height)}",
      ),
    ]),
  )
  let mounting = match it.mounting {
    Some(m) => mounting_short(lang, m.wire())
    None => "—"
  }
  cells.push(cell(k.c_mounting, "nowrap", [text(mounting)]))
  cells.push(
    cell(k.c_lifecycle, "nowrap", [
      span(
        class=lifecycle_class(it.lifecycle),
        lifecycle_label(lang, it.lifecycle.wire()),
      ),
    ]),
  )
  tr(cells)
}

///|
fn results_table(
  lang : Lang,
  q : CatalogQuery,
  emit : Emit[Msg],
  r : @api.CatalogResponse,
) -> Html {
  let items = r.items
  let k = lang.catalog()
  // One picked category: its column would only repeat it (unless it
  // is a group, whose rows show their subcategories).
  let category = match q.values("category") {
    [one] =>
      r.facets.category_tree
      .iter()
      .any(n => n.value == one && !n.children.is_empty())
    _ => true
  }
  let head = (label : String, sort : String) => {
    if sort.is_empty() {
      return th(scope=Col, label)
    }
    let active = q.sort == sort
    let arrow = if !active { "↕" } else if q.desc { "↓" } else { "↑" }
    th(
      scope=Col,
      class=if active { "sort-active" } else { "" },
      attrs=@html.Attrs::build().aria_sort(
        if !active {
          "none"
        } else if q.desc {
          "descending"
        } else {
          "ascending"
        },
      ),
      [
        button(
          type_="button",
          class="sort-head",
          on_click=emit(Catalog(CatSort(sort))),
          [text(label), span(class="sort-arrow", arrow)],
        ),
      ],
    )
  }
  let heads = [head(k.c_mpn, "mpn"), head(k.c_manufacturer, "manufacturer")]
  if category {
    heads.push(head(k.c_category, ""))
  }
  // One picked category: its key parameters as columns.
  // ...those some row on this page states.
  let params = match q.values("category") {
    [one] =>
      param_columns(one, root=category_root(r.facets.category_tree, one)).filter(key => {
          items.iter().any(it => it.parameters.contains(key))
        },
      )
    _ => []
  }
  for key in params {
    heads.push(
      head(
        param_name(lang, key),
        if enum_param_keys.contains(key) {
          ""
        } else {
          key
        },
      ),
    )
  }
  heads.push(head(k.c_package, ""))
  heads.push(head(k.c_size, "size"))
  heads.push(head(k.c_pins, "pins"))
  heads.push(head(k.c_pitch, "pitch"))
  heads.push(head(k.c_dims, "height"))
  heads.push(head(k.c_mounting, ""))
  heads.push(head(k.c_lifecycle, ""))
  div(class="catalog-table-wrap", [
    table(class="catalog-table", [
      thead([tr(heads)]),
      tbody(items.map(it => result_row(lang, it, category~, params~))),
    ]),
  ])
}

///|
fn pager(lang : Lang, emit : Emit[Msg], r : @api.CatalogResponse) -> Html {
  let k = lang.catalog()
  let pages = (r.total + page_size - 1) / page_size
  let page = r.offset / page_size + 1
  div(class="catalog-pager", [
    button(
      type_="button",
      class="button",
      disabled=page <= 1,
      on_click=emit(Catalog(CatPage(r.offset - page_size))),
      "← " + k.prev,
    ),
    span(class="text-muted", page_of(lang, page, pages)),
    button(
      type_="button",
      class="button",
      disabled=page >= pages,
      on_click=emit(Catalog(CatPage(r.offset + page_size))),
      k.next + " →",
    ),
  ])
}

///|
fn catalog_results_view(model : Model, emit : Emit[Msg], box : Html) -> Html {
  let lang = model.lang
  let c = lang.copy()
  let k = lang.catalog()
  let s = model.catalog
  let q = s.query
  let response = match s.results {
    Got(r) => Some(r)
    _ => None
  }
  let facets = response.map(r => r.facets)
  // Toolbar: filter drawer toggle (phones), count, sort.
  let sorts = [
    ("", k.s_relevance),
    ("mpn", k.s_mpn),
    ("manufacturer", k.s_manufacturer),
    ("pins", k.s_pins),
    ("pitch", k.s_pitch),
    ("size", k.s_size),
    ("height", k.s_height),
  ]
  // The picked category's numeric parameters sort too.
  if q.values("category") is [one] {
    let root = match response {
      Some(r) => category_root(r.facets.category_tree, one)
      None => one
    }
    for key in param_columns(one, root~) {
      if !enum_param_keys.contains(key) {
        sorts.push((key, param_name(lang, key)))
      }
    }
  }
  let count = match response {
    Some(r) if r.total == 0 => c.no_results
    Some(r) =>
      results_range(
        lang,
        r.offset + 1,
        if r.offset + page_size < r.total {
          r.offset + page_size
        } else {
          r.total
        },
        r.total,
      )
    None => ""
  }
  let n_picks = q.picks.length()
  let toolbar = div(class="catalog-toolbar", [
    button(
      type_="button",
      class="button catalog-filter-toggle",
      on_click=emit(Catalog(CatDrawer(true))),
      if n_picks > 0 {
        "\{k.filters} (\{n_picks})"
      } else {
        k.filters
      },
    ),
    span(class="catalog-count", count),
    div(class="catalog-sort", [
      @html.label(class="text-muted", for_="catalog-sort", k.sort),
      @html.select(
        id="catalog-sort",
        class="catalog-sort-select",
        on_change=emit.map(v => Catalog(CatSortSelect(v))),
        sorts.map(o => @html.option(value=o.0, selected=q.sort == o.0, o.1)),
      ),
      button(
        type_="button",
        class="button catalog-order",
        on_click=emit(Catalog(CatOrder)),
        if q.desc {
          k.descending + " ↓"
        } else {
          k.ascending + " ↑"
        },
      ),
    ]),
  ])
  // Active filters.
  let chips = q.picks.map(pk => {
    chip(
      pick_label(lang, facets, pk.0, pk.1) + " ×",
      true,
      if facet_keys.contains(pk.0) || is_param_key(pk.0) {
        emit(Catalog(CatToggle(pk.0, pk.1)))
      } else {
        emit(Catalog(CatRange(pk.0, "")))
      },
    )
  })
  if n_picks > 0 {
    chips.push(
      button(
        type_="button",
        class="catalog-clear",
        on_click=emit(Catalog(CatClear)),
        k.clear_all,
      ),
    )
  }
  let side = match response {
    Some(r) => sidebar(lang, s, emit, r)
    None => []
  }
  let drawer_class = if s.drawer {
    "catalog-sidebar catalog-sidebar-open"
  } else {
    "catalog-sidebar"
  }
  let total = response.map(r => r.total).unwrap_or(0)
  let close = emit(Catalog(CatDrawer(false)))
  let aside_ = @html.aside(
    class=drawer_class,
    attrs=@html.Attrs::build().aria_label(k.filters),
    [
      div(class="catalog-drawer-head", [
        strong(k.filters),
        button(type_="button", class="button", on_click=close, k.close),
      ]),
      div(class="catalog-facets", side),
      div(class="catalog-drawer-foot", [
        button(
          type_="button",
          class="button button-primary",
          on_click=close,
          show_results(lang, total),
        ),
      ]),
    ],
  )
  let main_body = match s.results {
    Got(r) if r.items.is_empty() => [p(class="text-muted", c.no_results)]
    Got(r) => [results_table(lang, q, emit, r), pager(lang, emit, r)]
    other => [fetch_message(lang, other)]
  }
  div(class=if s.loading { "catalog catalog-loading" } else { "catalog" }, [
    div(class="catalog-head", [
      h1([a(class="catalog-home", href=href(lang, "search"), c.search_title)]),
      box,
    ]),
    toolbar,
    div(class="catalog-active", chips),
    div(class="catalog-layout", [
      aside_,
      div(
        class=if s.drawer {
          "catalog-backdrop catalog-backdrop-open"
        } else {
          "catalog-backdrop"
        },
        on_click=close,
        ([] : Array[Html]),
      ),
      div(class="catalog-main", main_body),
    ]),
  ])
}
