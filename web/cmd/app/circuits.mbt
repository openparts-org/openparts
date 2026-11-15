// The part page's reference circuits (`GET .../circuits`, Canonical
// Data Specification section 41). Collapsed, one line per circuit: a
// short title, how many facts are required and recommended, the
// source and its pages, and a button that copies the board.yaml entry.
// A composite (the minimal design) lists the circuits it is made of
// beneath it. Open, a circuit shows its schematic (drawn by the server
// from the facts) or, on the "table" tab, one table of its parts and
// pin connections, with any formula on one line.

///|
#derive(Eq)
struct CircuitUi {
  /// Circuits shown open, by id.
  open : Array[String]
  /// Circuits showing the table instead of the schematic.
  table : Array[String]
  /// The circuit whose board.yaml entry was just copied.
  copied : String
}

///|
fn CircuitUi::new() -> CircuitUi {
  { open: [], table: [], copied: "" }
}

///|
enum CircuitMsg {
  CircuitToggle(String)
  CircuitTable(String, Bool)
  CircuitCopy(String)
  CircuitCopied(String)
}

///|
fn toggled(list : Array[String], id : String, on : Bool) -> Array[String] {
  let out = list.filter(x => x != id)
  if on {
    out.push(id)
  }
  out
}

///|
fn circuit_update(
  model : Model,
  msg : CircuitMsg,
  emit : Emit[Msg],
) -> (Model, @rabbita.Cmd) {
  let s = model.circuits
  match msg {
    CircuitToggle(id) => {
      let open = toggled(s.open, id, !s.open.contains(id))
      ({ ..model, circuits: { ..s, open, }, }, @rabbita.none)
    }
    CircuitTable(id, on) => {
      let table = toggled(s.table, id, on)
      ({ ..model, circuits: { ..s, table, }, }, @rabbita.none)
    }
    CircuitCopy(id) => {
      let line = "- { circuit: \{id}, host: <REF> }"
      let cmd = @clipboard.copy(
        Text(line),
        copied=emit(Circuit(CircuitCopied(id))),
      )
      (model, cmd)
    }
    CircuitCopied(id) =>
      ({ ..model, circuits: { ..s, copied: id, }, }, @rabbita.none)
  }
}

///|
struct CircuitCopy {
  heading : String
  required : String
  recommended : String
  example : String
  optional : String
  unreviewed : String
  unreviewed_help : String
  part : String
  connection : String
  level : String
  page : String
  diagram : String
  table : String
  source : String
  target : String
  copy : String
  copied : String
  legend_help : String
  datasheet : String
  reference_design : String
  application_note : String
  standard : String
  document : String
}

///|
let circuit_ja : CircuitCopy = {
  heading: "参考回路",
  required: "必須",
  recommended: "推奨",
  example: "例",
  optional: "任意",
  unreviewed: "未レビュー",
  unreviewed_help: "AI が出典から抽出した候補。人のレビュー前です。",
  part: "部品",
  connection: "接続",
  level: "レベル",
  page: "頁",
  diagram: "回路図",
  table: "表",
  source: "出典",
  target: "目標",
  copy: "board.yaml に追加 (コピー)",
  copied: "コピーしました",
  legend_help: "回路図は出典の事実から自動生成したもので、出典の図の転載ではありません。",
  datasheet: "データシート",
  reference_design: "リファレンス設計",
  application_note: "アプリノート",
  standard: "規格",
  document: "資料",
}

///|
let circuit_en : CircuitCopy = {
  heading: "Reference circuits",
  required: "required",
  recommended: "recommended",
  example: "example",
  optional: "optional",
  unreviewed: "unreviewed",
  unreviewed_help: "AI-extracted from the source; not yet reviewed by a person.",
  part: "Part",
  connection: "Connection",
  level: "Level",
  page: "p.",
  diagram: "Schematic",
  table: "Table",
  source: "source",
  target: "target",
  copy: "Copy board.yaml entry",
  copied: "Copied",
  legend_help: "Drawn from the source's facts, not copied from its figure.",
  datasheet: "Datasheet",
  reference_design: "Reference design",
  application_note: "App note",
  standard: "Standard",
  document: "Document",
}

///|
fn Lang::circuit(self : Lang) -> CircuitCopy {
  match self {
    Ja => circuit_ja
    En => circuit_en
  }
}

///|
/// Short titles (ja, en); other circuits show their data title.
let circuit_titles : Map[String, (String, String)] = {
  "raspberrypi/rp2040-power": ("電源デカップリング", "Power decoupling"),
  "raspberrypi/rp2040-xosc-12mhz": ("12 MHz 水晶", "12 MHz crystal"),
  "raspberrypi/rp2040-usb-device": ("USB", "USB"),
  "raspberrypi/rp2040-qspi-boot": ("QSPI フラッシュ", "QSPI flash"),
  "raspberrypi/rp2040-debug-reset": ("SWD・RUN", "SWD, RUN"),
  "raspberrypi/rp2040-minimal": ("最小構成", "Minimal design"),
  "raspberrypi/pico-powerchain": ("Pico 電源", "Pico power chain"),
  "usb-if/typec-usb2-sink": ("USB-C シンク (CC)", "USB-C sink (CC)"),
  "onsemi/ncp1117-fixed": ("入出力コンデンサ", "Input/output capacitors"),
}

///|
fn circuit_title(lang : Lang, v : @api.CircuitView) -> String {
  match circuit_titles.get(v.id) {
    Some((ja, en)) =>
      match lang {
        Ja => ja
        En => en
      }
    None => v.title
  }
}

///|
fn level_label(c : CircuitCopy, l : @api.LevelValue) -> String {
  match l {
    Required => c.required
    Recommended => c.recommended
    Example => c.example
    Optional => c.optional
  }
}

///|
fn level_rank(l : @api.LevelValue) -> Int {
  match l {
    Required => 0
    Recommended => 1
    Example => 2
    Optional => 3
  }
}

///|
/// A dot and the level's word; the source's own word in the tooltip.
fn level_view(c : CircuitCopy, l : @api.LevelValue, word : String?) -> Html {
  let label = level_label(c, l)
  let title = match word {
    Some(w) => "\{label}: \"\{w}\""
    None => label
  }
  span(class="lv lv-\{l.wire()}", title~, [
    span(class="lv-dot", nodes([])),
    text(label),
  ])
}

///|
/// `p.152–157`; at most three pages of a long list.
fn pages_label(pages : String) -> String {
  let bare = if pages.has_prefix("p") { pages[1:].to_owned() } else { pages }
  let items = []
  for item in bare.split(", ") {
    items.push(item.to_owned())
  }
  if items.length() > 3 {
    "p.\{items[0]}, \{items[1]}, \{items[2]} …"
  } else {
    "p.\{items.join(", ")}"
  }
}

///|
fn document_label(c : CircuitCopy, t : @api.DocumentType?) -> String {
  match t {
    Some(Datasheet) => c.datasheet
    Some(ReferenceDesign) => c.reference_design
    Some(ApplicationNote) => c.application_note
    Some(Standard) => c.standard
    _ => c.document
  }
}

///|
/// The source: its kind and pages, one link.
fn source_link(c : CircuitCopy, v : @api.CircuitView) -> Html {
  guard v.source is Some(s) else { text("") }
  let label = "\{document_label(c, v.document_type)} \{pages_label(v.pages)}"
  match s.url {
    Some(u) =>
      a(
        class="circuit-source",
        href=u,
        target=Blank,
        rel="noopener noreferrer",
        title=s.label,
        label,
        escape=true,
      )
    None => span(class="circuit-source", title=s.label, label)
  }
}

///|
/// `● 必須 2  ● 推奨 4`
fn counts_view(c : CircuitCopy, v : @api.CircuitView) -> Html {
  let out : Array[Html] = []
  for pair in [
    (v.required, @api.LevelValue::Required),
    (v.recommended, Recommended),
    (v.example, Example),
    (v.optional, Optional),
  ] {
    if pair.0 > 0 {
      out.push(
        span(class="lv lv-\{pair.1.wire()}", [
          span(class="lv-dot", nodes([])),
          text("\{level_label(c, pair.1)} \{pair.0}"),
        ]),
      )
    }
  }
  span(class="circuit-counts", out)
}

///|
/// `C_IOVDD` -> `C`, `X1` -> `X`, `SW_BOOT` -> `SW`.
fn role_prefix(role : String) -> String {
  let sb = StringBuilder()
  for ch in role {
    if (ch >= 'A' && ch <= 'Z') || (ch >= 'a' && ch <= 'z') {
      sb.write_char(ch)
    } else {
      break
    }
  }
  sb.to_string()
}

///|
/// A named part's MPN (`abracon/ABM8-272-T3` -> `ABM8-272-T3`).
fn short_value(value : String) -> String {
  let mut last = value
  for piece in value.split("/") {
    last = piece.to_owned()
  }
  if value.contains(" / ") {
    value
  } else {
    last
  }
}

///|
/// One row of the unified table.
priv struct CircuitRow {
  part : String
  part_tip : String
  connection : String
  level : @api.LevelValue
  word : String?
  cite : @api.CircuitCite?
  /// Every page the row's facts cite.
  pages : Array[Int]
}

///|
/// Parts with the same kind, value, level and far end become one row
/// (`C 100 nF ×10`, `IOVDD×6, DVDD×2, USB_VDD, ADC_AVDD – GND`); host
/// pins tied to the same net at the same level, too. Required first.
fn circuit_rows(v : @api.CircuitView) -> Array[CircuitRow] {
  let keys : Array[String] = []
  let groups : Map[String, Array[@api.CircuitRoleRow]] = Map([])
  for r in v.roles {
    let key = if r.connects.length() == 2 {
      "\{role_prefix(r.role)}|\{r.value}|\{r.level.wire()}|\{r.connects[1]}"
    } else {
      "role|" + r.role
    }
    match groups.get(key) {
      Some(g) => g.push(r)
      None => {
        keys.push(key)
        groups[key] = [r]
      }
    }
  }
  let rows : Array[CircuitRow] = []
  for key in keys {
    let g = groups[key]
    let first = g[0]
    let total = g.fold(init=0, (n, r) => n + r.count)
    let value = short_value(first.value)
    let prefix = role_prefix(first.role)
    let part = if total > 1 {
      "\{prefix} \{value} ×\{total}"
    } else {
      "\{prefix} \{value}"
    }
    let connection = if first.connects.length() == 2 {
      let pins = g.map(r => if r.count > 1 {
        "\{r.connects[0]}×\{r.count}"
      } else {
        r.connects[0]
      })
      "\{pins.join(", ")} – \{first.connects[1]}"
    } else {
      first.connects.join(", ")
    }
    let pages = []
    for r in g {
      if r.cite is Some({ page: Some(p), .. }) && !pages.contains(p) {
        pages.push(p)
      }
    }
    rows.push({
      part,
      part_tip: g.map(r => r.role).join(", "),
      connection,
      level: first.level,
      word: first.word,
      cite: first.cite,
      pages,
    })
  }
  // Host pins tied to a net.
  let ckeys : Array[String] = []
  let cgroups : Map[String, Array[@api.CircuitConnectRow]] = Map([])
  for k in v.connects {
    let key = "\{k.net}|\{k.level.wire()}|\{k.word.unwrap_or("")}"
    match cgroups.get(key) {
      Some(g) => g.push(k)
      None => {
        ckeys.push(key)
        cgroups[key] = [k]
      }
    }
  }
  for key in ckeys {
    let g = cgroups[key]
    let first = g[0]
    let pages = []
    for k in g {
      if k.cite is Some({ page: Some(p), .. }) && !pages.contains(p) {
        pages.push(p)
      }
    }
    rows.push({
      part: "",
      part_tip: "",
      connection: "\{g.map(k => k.pin).join(", ")} → \{first.net}",
      level: first.level,
      word: first.word,
      cite: first.cite,
      pages,
    })
  }
  rows.sort_by((a, b) => level_rank(a.level) - level_rank(b.level))
  rows
}

///|
/// The page most rows cite (shown once, in the source link).
fn usual_page(rows : Array[CircuitRow]) -> Int? {
  let counts : Map[Int, Int] = Map([])
  for r in rows {
    for p in r.pages {
      counts[p] = counts.get(p).unwrap_or(0) + 1
    }
  }
  let mut best : (Int, Int)? = None
  for p, n in counts {
    match best {
      Some((_, m)) if m >= n => ()
      _ => best = Some((p, n))
    }
  }
  best.map(b => b.0)
}

///|
/// The row's page when it is not the usual one, as a small link.
fn page_view(r : CircuitRow, usual : Int?) -> Html {
  guard r.pages.length() > 0 else { text("") }
  if r.pages.length() == 1 && Some(r.pages[0]) == usual {
    return text("")
  }
  let sorted = r.pages.copy()
  sorted.sort()
  let all = sorted.map(p => p.to_string()).join(", ")
  let lo = sorted[0]
  let hi = sorted[sorted.length() - 1]
  let label = if lo == hi { lo.to_string() } else { "\{lo}–\{hi}" }
  let title = match r.cite {
    Some(ci) =>
      match ci.locator {
        Some(l) => "\{ci.label} p\{all} \{l}"
        None => "\{ci.label} p\{all}"
      }
    None => label
  }
  match r.cite.bind(ci => ci.url) {
    Some(u) =>
      a(
        class="circuit-page",
        href=u,
        target=Blank,
        rel="noopener noreferrer",
        title~,
        "p" + label,
        escape=true,
      )
    None => span(class="circuit-page", title~, "p" + label)
  }
}

///|
fn circuit_table(c : CircuitCopy, v : @api.CircuitView) -> Html {
  let rows = circuit_rows(v)
  let usual = usual_page(rows)
  table(class="circuit-table", [
    colgroup([
      col(class="ct-part"),
      col(class="ct-conn"),
      col(class="ct-level"),
      col(class="ct-page"),
    ]),
    thead([
      tr([
        th(scope=Col, c.part),
        th(scope=Col, c.connection),
        th(scope=Col, c.level),
        th(scope=Col, [span(class="visually-hidden", c.page)]),
      ]),
    ]),
    tbody(
      rows.map(r => {
        tr(class=if r.part.is_empty() { "ct-pin" } else { "" }, [
          td(class="ct-part", title=r.part_tip, r.part),
          td(class="ct-conn", [code(r.connection)]),
          td(class="ct-level", [level_view(c, r.level, r.word)]),
          td(class="ct-page", [page_view(r, usual)]),
        ])
      }),
    ),
  ])
}

///|
/// `load_capacitance` with `C_XIN * C_XOUT / (C_XIN + C_XOUT) + C_stray`
/// -> `CL = 15·15/(15 + 15) + 3 = 10.5 pF (source 10.5 pF · target 10 pF)`.
fn formula_view(c : CircuitCopy, f : @api.CircuitFormulaRow) -> Html {
  let values : Map[String, String] = Map([])
  for input in f.inputs {
    let parts = []
    for piece in input.split(" = ") {
      parts.push(piece.to_owned())
    }
    if parts.length() >= 2 {
      let mut number = parts[1]
      for piece in parts[1].split(" ") {
        number = piece.to_owned()
        break
      }
      values[parts[0]] = number
    }
  }
  let sb = StringBuilder()
  let word = StringBuilder()
  let flush = () => {
    let w = word.to_string()
    if !w.is_empty() {
      sb.write_string(values.get(w).unwrap_or(w))
      word.reset()
    }
  }
  for ch in f.expr {
    if (ch >= 'A' && ch <= 'Z') ||
      (ch >= 'a' && ch <= 'z') ||
      (ch >= '0' && ch <= '9') ||
      ch == '_' ||
      ch == '.' {
      word.write_char(ch)
      continue
    }
    flush()
    match ch {
      ' ' => ()
      '*' => sb.write_string("·")
      '+' => sb.write_string(" + ")
      '-' => sb.write_string(" − ")
      _ => sb.write_char(ch)
    }
  }
  flush()
  let name = match f.name {
    "load_capacitance" => "CL"
    n => n.replace_all(old="_", new=" ")
  }
  let notes = []
  if f.stated is Some(x) {
    notes.push("\{c.source} \{x}")
  }
  if f.target is Some(t) {
    // `X1 load_capacitance 10 pF`: the value after role and parameter.
    let pieces = []
    for piece in t.split(" ") {
      pieces.push(piece.to_owned())
    }
    if pieces.length() > 2 {
      notes.push("\{c.target} \{pieces[2:].to_owned().join(" ")}")
    }
  }
  let out : Array[Html] = [code("\{name} = \{sb.to_string()} = ")]
  out.push(strong(f.computed.unwrap_or("?")))
  if !notes.is_empty() {
    out.push(span(class="text-muted", " (\{notes.join(" · ")})"))
  }
  p(class="circuit-formula", out)
}

///|
/// The schematic's legend: line colour and style per level.
fn legend_view(c : CircuitCopy) -> Html {
  let item = (l : @api.LevelValue) => span(class="lg lg-\{l.wire()}", [
    span(class="lg-line", nodes([])),
    text(level_label(c, l)),
  ])
  div(class="circuit-legend", title=c.legend_help, [
    item(Required),
    item(Recommended),
    item(Example),
    item(Optional),
  ])
}

///|
/// The schematic inlined (so it takes the page's colours). It is our
/// own server's SVG, every text in it escaped there.
#warnings("-alert_xss_vulnerable")
fn svg_markup(svg : String) -> @html.Attrs {
  @html.Attrs::build().inner_html(svg)
}

///|
fn circuit_body(
  c : CircuitCopy,
  ui : CircuitUi,
  v : @api.CircuitView,
  emit : Emit[Msg],
) -> Html {
  let body : Array[Html] = []
  let show_table = v.diagram is None || ui.table.contains(v.id)
  if v.diagram is Some(_) {
    let tab = (label : String, on : Bool) => button(
      type_="button",
      class="seg" + (if on == show_table { " seg-on" } else { "" }),
      attrs=@html.Attrs::build().aria_pressed((on == show_table).to_string()),
      on_click=emit(Circuit(CircuitTable(v.id, on))),
      label,
    )
    body.push(
      div(class="circuit-tabs", [tab(c.diagram, false), tab(c.table, true)]),
    )
  }
  match (show_table, v.diagram) {
    (false, Some(svg)) => {
      body.push(
        div(class="circuit-diagram", attrs=svg_markup(svg), nodes([])),
      )
      body.push(legend_view(c))
    }
    _ =>
      if !v.roles.is_empty() || !v.connects.is_empty() {
        body.push(circuit_table(c, v))
      }
  }
  for f in v.formulas {
    body.push(formula_view(c, f))
  }
  div(class="circuit-body", body)
}

///|
fn circuit_item(
  lang : Lang,
  ui : CircuitUi,
  v : @api.CircuitView,
  badge : Bool,
  children : Array[Html],
  emit : Emit[Msg],
) -> Html {
  let c = lang.circuit()
  let open = ui.open.contains(v.id)
  let copied = ui.copied == v.id
  let head = div(class="circuit-row", [
    button(
      type_="button",
      class="circuit-toggle",
      attrs=@html.Attrs::build().aria_expanded(open.to_string()),
      on_click=emit(Circuit(CircuitToggle(v.id))),
      [
        span(class="chevron", nodes([])),
        span(class="circuit-title", title=v.title, circuit_title(lang, v)),
        counts_view(c, v),
      ],
    ),
    if badge && !v.reviewed {
      span(
        class="badge badge-warn badge-mini",
        title=c.unreviewed_help,
        c.unreviewed,
      )
    } else {
      text("")
    },
    source_link(c, v),
    button(
      type_="button",
      class="circuit-copy" + (if copied { " is-copied" } else { "" }),
      title=if copied { c.copied } else { c.copy },
      attrs=@html.Attrs::build().aria_label(c.copy),
      on_click=emit(Circuit(CircuitCopy(v.id))),
      nodes([]),
    ),
  ])
  let inner : Array[Html] = [head]
  if open {
    inner.push(circuit_body(c, ui, v, emit))
  }
  if !children.is_empty() {
    inner.push(ul(class="circuit-children", children))
  }
  li(class="circuit" + (if open { " is-open" } else { "" }), inner)
}

///|
/// The section, or nothing when the part has no circuits. Circuits a
/// composite on this page includes sit under it.
fn circuits_section(
  lang : Lang,
  ui : CircuitUi,
  data : PartData,
  emit : Emit[Msg],
) -> Html {
  guard data.circuits is Some(list) && !list.is_empty() else { text("") }
  let c = lang.circuit()
  let all_unreviewed = list.iter().all(v => !v.reviewed)
  let ids = list.map(v => v.id)
  let nested : Array[String] = []
  for v in list {
    for id in v.includes {
      if ids.contains(id) && id != v.id && !nested.contains(id) {
        nested.push(id)
      }
    }
  }
  let badge = !all_unreviewed
  let items : Array[Html] = []
  // Composites (with what they include) first, then the rest.
  for v in list {
    if v.includes.is_empty() || nested.contains(v.id) {
      continue
    }
    // In the composite's own order.
    let children : Array[Html] = []
    for id in v.includes {
      if list.iter().find_first(k => k.id == id && id != v.id) is Some(k) {
        children.push(circuit_item(lang, ui, k, badge, [], emit))
      }
    }
    items.push(circuit_item(lang, ui, v, badge, children, emit))
  }
  for v in list {
    if v.includes.is_empty() && !nested.contains(v.id) {
      items.push(circuit_item(lang, ui, v, badge, [], emit))
    }
  }
  section(class="section circuits", [
    div(class="circuits-head", [
      h3(c.heading),
      if all_unreviewed {
        span(
          class="badge badge-warn badge-mini",
          title=c.unreviewed_help,
          c.unreviewed,
        )
      } else {
        text("")
      },
    ]),
    ul(class="circuit-list", items),
  ])
}
