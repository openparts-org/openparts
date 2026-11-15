// The part page's reference circuits (`GET .../circuits`, Canonical
// Data Specification section 41): one line per circuit -- source and
// pages, how many facts are required and recommended, the review
// state -- that opens to its roles, connections and formulas. Facts
// only: no figures, no prose.

///|
struct CircuitCopy {
  heading : String
  required : String
  recommended : String
  example : String
  optional : String
  unreviewed : String
  reviewed : String
  role : String
  value : String
  connects : String
  level : String
  source : String
  pin : String
  net : String
  formulas : String
  computed : String
  stated : String
  target : String
  includes : String
  use_line : String
}

///|
let circuit_ja : CircuitCopy = {
  heading: "参考回路",
  required: "必須",
  recommended: "推奨",
  example: "例",
  optional: "任意",
  unreviewed: "未レビュー",
  reviewed: "レビュー済み",
  role: "役割",
  value: "値",
  connects: "接続",
  level: "レベル",
  source: "出典",
  pin: "ピン",
  net: "ネット",
  formulas: "計算",
  computed: "計算値",
  stated: "出典の値",
  target: "目標",
  includes: "構成",
  use_line: "board.yaml",
}

///|
let circuit_en : CircuitCopy = {
  heading: "Reference circuits",
  required: "required",
  recommended: "rec.",
  example: "example",
  optional: "optional",
  unreviewed: "unreviewed",
  reviewed: "reviewed",
  role: "Role",
  value: "Value",
  connects: "Connects",
  level: "Level",
  source: "Source",
  pin: "Pin",
  net: "Net",
  formulas: "Formulas",
  computed: "computed",
  stated: "source",
  target: "target",
  includes: "Includes",
  use_line: "board.yaml",
}

///|
fn Lang::circuit(self : Lang) -> CircuitCopy {
  match self {
    Ja => circuit_ja
    En => circuit_en
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
fn level_class(l : @api.LevelValue) -> String {
  match l {
    Required => "badge badge-warn badge-mini"
    Recommended => "badge badge-neutral badge-mini"
    _ => "badge badge-mini"
  }
}

///|
/// `必須 (must)`: the level and the source's own word.
fn level_view(c : CircuitCopy, l : @api.LevelValue, word : String?) -> Html {
  let label = match word {
    Some(w) => "\{level_label(c, l)} (\{w})"
    None => level_label(c, l)
  }
  span(class=level_class(l), label)
}

///|
fn cite_view(cite : @api.CircuitCite?) -> Html {
  guard cite is Some(ci) else { text("") }
  let label = match ci.page {
    Some(p) => "p\{p}"
    None => ci.label
  }
  let title = match ci.locator {
    Some(l) => "\{ci.label} \{l}"
    None => ci.label
  }
  match ci.url {
    Some(u) =>
      a(
        href=u,
        target=Blank,
        rel="noopener noreferrer",
        title~,
        label,
        escape=true,
      )
    None => span(title~, label)
  }
}

///|
/// `2 必須 · 4 推奨 · 1 例`
fn counts_label(c : CircuitCopy, v : @api.CircuitView) -> String {
  let out = []
  for pair in [
    (v.required, c.required),
    (v.recommended, c.recommended),
    (v.example, c.example),
    (v.optional, c.optional),
  ] {
    if pair.0 > 0 {
      out.push("\{pair.0} \{pair.1}")
    }
  }
  out.join(" · ")
}

///|
fn circuit_view(lang : Lang, v : @api.CircuitView) -> Html {
  let c = lang.circuit()
  let source = match v.source {
    Some(s) => "\{s.label} \{v.pages}"
    None => v.pages
  }
  let head = summary(class="circuit-head", [
    strong(v.title),
    span(class="text-muted", " \{source} "),
    span(class="circuit-counts", counts_label(c, v)),
    text(" "),
    if v.reviewed {
      span(class="badge badge-good badge-mini", c.reviewed)
    } else {
      span(class="badge badge-warn badge-mini", c.unreviewed)
    },
  ])
  let body : Array[Html] = []
  if !v.roles.is_empty() {
    body.push(
      table(class="circuit-table", [
        thead([
          tr([
            th(scope=Col, c.role),
            th(scope=Col, c.value),
            th(scope=Col, c.connects),
            th(scope=Col, c.level),
            th(scope=Col, c.source),
          ]),
        ]),
        tbody(
          v.roles.map(r => {
            let role = if r.count > 1 {
              "\{r.role} ×\{r.count}"
            } else {
              r.role
            }
            tr([
              td([code(role)]),
              td(r.value),
              td(r.connects.join(" – ")),
              td([level_view(c, r.level, r.word)]),
              td([cite_view(r.cite)]),
            ])
          }),
        ),
      ]),
    )
  }
  if !v.connects.is_empty() {
    body.push(
      table(class="circuit-table", [
        thead([
          tr([
            th(scope=Col, c.pin),
            th(scope=Col, c.net),
            th(scope=Col, c.level),
            th(scope=Col, c.source),
          ]),
        ]),
        tbody(
          v.connects.map(k => {
            tr([
              td([code(k.pin)]),
              td(k.net),
              td([level_view(c, k.level, k.word)]),
              td([cite_view(k.cite)]),
            ])
          }),
        ),
      ]),
    )
  }
  for f in v.formulas {
    let parts = [
      code("\{f.name} = \{f.expr}"),
      span(class="text-muted", " (\{f.inputs.join(", ")})"),
    ]
    let results = []
    if f.computed is Some(x) {
      results.push("\{c.computed} \{x}")
    }
    if f.stated is Some(x) {
      results.push("\{c.stated} \{x}")
    }
    if f.target is Some(x) {
      results.push("\{c.target} \{x}")
    }
    body.push(
      p(class="circuit-formula", [
        strong(c.formulas + ": "),
        ..parts,
        text(" → " + results.join(" · ") + " "),
        cite_view(f.cite),
      ]),
    )
  }
  if !v.includes.is_empty() {
    body.push(
      p(class="text-muted", "\{c.includes}: \{v.includes.join(", ")}"),
    )
  }
  body.push(p([span(class="text-muted", c.use_line + " "), code(v.use_line)]))
  details(class="circuit", [head, div(body)])
}

///|
/// The section, or nothing when the part has no circuits.
fn circuits_section(lang : Lang, data : PartData) -> Html {
  guard data.circuits is Some(list) && !list.is_empty() else { text("") }
  section(class="section", [
    h3(lang.circuit().heading),
    div(list.map(v => circuit_view(lang, v))),
  ])
}
