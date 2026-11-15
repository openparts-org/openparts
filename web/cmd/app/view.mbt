// Page views.

///|
using @html {
  a,
  button,
  code,
  details,
  div,
  figcaption,
  figure,
  footer,
  h1,
  h2,
  h3,
  header,
  img,
  input,
  li,
  main_,
  nav,
  ol,
  p,
  pre,
  section,
  span,
  strong,
  summary,
  table,
  tbody,
  td,
  text,
  th,
  thead,
  tr,
  ul,
}

///|
using @rabbita {type Html, type Emit}

///|
fn nodes(children : Array[Html]) -> Array[Html] {
  children
}

///|
/// A link out of the site (or into the API): left to the browser.
fn ext_link(url : String, label : String, class? : String) -> Html {
  a(
    href=url,
    target=Blank,
    rel="noopener noreferrer",
    class?,
    label,
    escape=true,
  )
}

///|
fn view(model : Model, emit : Emit[Msg]) -> Html {
  let page = match model.route {
    Home => home_view(model.lang)
    Search(_) => catalog_view(model, emit)
    Part(_, _) => part_view(model, emit)
    KicadPlugin => kicad_view(model.lang)
    Contributing => contributing_view(model.lang)
    Bom => bom_view(model, emit)
    NotFound => not_found_view(model.lang)
  }
  div([
    nav_view(model),
    main_(class="container section", [page]),
    footer_view(model.lang),
  ])
}

///|
fn nav_view(model : Model) -> Html {
  let lang = model.lang
  let c = lang.copy()
  let links = [
    ("search", c.nav_search),
    ("bom", lang.bom().nav),
    ("kicad-plugin", c.nav_kicad),
    ("contributing", c.nav_contributing),
  ].map(l => li([a(href=href(lang, l.0), l.1)]))
  let path = route_path(model.route)
  for l in [Ja, En] {
    let active = l == lang
    links.push(
      li([
        a(
          href=href(l, path),
          class=if active { "lang-active" } else { "" },
          attrs=if active {
            @html.Attrs::build().aria_current("page")
          } else {
            @html.Attrs::build()
          },
          l.code().to_upper(),
        ),
      ]),
    )
  }
  header(class="nav", [
    div(class="container nav-inner", [
      a(class="nav-brand", href=href(lang, ""), "OpenParts"),
      nav([ul(class="nav-links", links)]),
    ]),
  ])
}

///|
fn footer_view(lang : Lang) -> Html {
  let repos = [
    ("https://github.com/openparts-org/openparts", "openparts"),
    ("https://github.com/openparts-org/openparts-data", "openparts-data"),
    ("https://github.com/openparts-org/openparts/tree/main/server", "server"),
    (
      "https://github.com/openparts-org/openparts/tree/main/kicad-plugin", "kicad-plugin",
    ),
  ]
  footer(class="footer", [
    div(class="container footer-inner", [
      span(class="text-muted", lang.copy().open_source),
      ul(class="footer-links", repos.map(r => li([ext_link(r.0, r.1)]))),
    ]),
  ])
}

///|
fn home_view(lang : Lang) -> Html {
  let c = lang.copy()
  div([
    section(class="hero", [
      h1(c.home_title),
      p(class="text-muted hero-lead", c.home_lead),
      div(class="hero-actions", [
        a(
          class="button button-primary",
          href=href(lang, "search"),
          c.home_cta_search,
        ),
        a(class="button", href=href(lang, "kicad-plugin"), c.home_cta_kicad),
      ]),
    ]),
    section(
      class="section features",
      c.features.map(f => {
        div(class="card", [h3(f.title), p(class="text-muted", f.body)])
      }),
    ),
  ])
}

///|
fn kicad_view(lang : Lang) -> Html {
  let c = lang.copy()
  let raw = "https://raw.githubusercontent.com/openparts-org/openparts/main/kicad-plugin"
  div([
    h1(c.kicad_title),
    p(class="text-muted", c.kicad_intro),
    section(class="section", [
      h2(c.kicad_quick_install),
      h3("Linux"),
      pre([code("curl -fsSL \{raw}/install.sh | bash")]),
      h3("Windows (PowerShell)"),
      pre([code("irm \{raw}/install.ps1 | iex")]),
      p(class="text-muted", c.kicad_auto_detect),
      p(class="text-muted", c.kicad_restart),
    ]),
    section(class="section", [
      h2(c.kicad_good_to_know),
      p(class="text-muted", c.kicad_session),
    ]),
  ])
}

///|
fn contributing_view(lang : Lang) -> Html {
  let c = lang.copy()
  let spec = "https://github.com/openparts-org/openparts/blob/main/docs/spec/"
  let step_links = [
    [
      "https://github.com/openparts-org/openparts-data/fork", "https://github.com/openparts-org/openparts",
    ],
    [spec + "canonical-data.md", ""],
    ["", ""],
    ["", ""],
  ]
  let steps = c.contrib_steps.mapi((i, s) => {
    let parts : Array[Html] = [text(s[0])]
    if s[1] != "" {
      parts.push(ext_link(step_links[i][0], s[1]))
      parts.push(text(s[2]))
    }
    if s[3] != "" {
      parts.push(ext_link(step_links[i][1], s[3]))
      parts.push(text(s[4]))
    }
    li(parts)
  })
  let specs = [
    ("architecture.md", "Architecture Specification"),
    ("canonical-data.md", "Canonical Data Specification"),
    ("testing-and-quality.md", "Testing and Quality Specification"),
  ]
  div([
    h1(c.contrib_title),
    p(class="text-muted", c.contrib_intro),
    section(class="section", [
      h2(c.contrib_report_heading),
      p(class="text-muted", [
        text(c.contrib_report_before),
        a(href=href(lang, "search"), c.contrib_report_link),
        text(c.contrib_report_after),
      ]),
    ]),
    section(class="section", [
      h2(c.contrib_review_heading),
      p(class="text-muted", [
        text(c.contrib_review_body),
        ext_link(contributing_url(), c.contrib_review_link),
        text(c.contrib_review_after),
      ]),
    ]),
    section(class="section", [h2(c.contrib_pr_heading), ol(steps)]),
    section(class="section", [
      h2(c.contrib_specs_heading),
      ul(specs.map(s => li([ext_link(spec + s.0, s.1)]))),
    ]),
  ])
}

///|
fn not_found_view(lang : Lang) -> Html {
  let c = lang.copy()
  div([
    h1(c.not_found_title),
    p(class="text-muted", [
      text(c.not_found_before),
      a(href=href(lang, "search"), c.not_found_link),
      text(c.not_found_after),
    ]),
  ])
}

///|
fn existence_badge(s : @api.ExistenceStatus) -> Html {
  let class = match s {
    Verified | HistoricalVerified => "badge badge-good"
    Probable => "badge badge-warn"
    Rejected => "badge badge-bad"
    Unverified => "badge badge-neutral"
  }
  span(class~, s.wire())
}

///|
fn lifecycle_badge(s : @api.LifecycleStatus) -> Html {
  let class = match s {
    Active | PreRelease => "badge badge-good"
    Nrnd | EolAnnounced => "badge badge-warn"
    Obsolete => "badge badge-bad"
    Unknown => "badge badge-neutral"
  }
  span(class~, s.wire())
}

///|
fn chip(label : String, active : Bool, on_click : @rabbita.Cmd) -> Html {
  button(
    type_="button",
    class=if active { "chip chip-active" } else { "chip" },
    on_click~,
    label,
  )
}

///|
fn format_number(v : Double) -> String {
  // 0.5 not 0.5000000001, 3 not 3.0
  let r = (v * 1000.0).round() / 1000.0
  if r == r.floor() {
    r.to_int().to_string()
  } else {
    r.to_string()
  }
}

///|
fn format_dimension(d : @api.Dimension?) -> String {
  guard d is Some(d) else { "-" }
  let u = d.unit
  match (d.nominal, d.min, d.max) {
    (Some(n), _, _) => "\{format_number(n)} \{u}"
    (None, Some(lo), Some(hi)) =>
      "\{format_number(lo)}–\{format_number(hi)} \{u}"
    (None, Some(lo), None) => "≥\{format_number(lo)} \{u}"
    (None, None, Some(hi)) => "≤\{format_number(hi)} \{u}"
    _ => "-"
  }
}

///|
/// Pin numbers in their natural order: 2 before 10, A1 before A12.
fn pin_order(a : String, b : String) -> Int32 {
  fn split(s : String) -> (String, Int32) {
    let mut i = s.length()
    while i > 0 && s[i - 1] >= '0' && s[i - 1] <= '9' {
      i -= 1
    }
    let mut n = if i < s.length() { 0 } else { -1 }
    for j in i..<s.length() {
      n = n * 10 + (s[j].to_int() - '0'.to_int())
    }
    (s[:i].to_owned(), n)
  }
  let (pa, na) = split(a)
  let (pb, nb) = split(b)
  if pa != pb {
    pa.compare(pb)
  } else if na != nb {
    na.compare(nb)
  } else {
    a.compare(b)
  }
}

///|
fn part_view(model : Model, emit : Emit[Msg]) -> Html {
  let lang = model.lang
  let c = lang.copy()
  match model.part {
    Loading => p(class="text-muted", c.loading)
    Missing => not_found_view(lang)
    Failed(e) =>
      p(class="text-muted", if e.is_empty() { c.load_failed } else { e })
    Got(data) => part_loaded_view(model, data, emit)
  }
}

///|
fn part_loaded_view(model : Model, data : PartData, emit : Emit[Msg]) -> Html {
  let lang = model.lang
  let c = lang.copy()
  let part = data.part
  let (m, mpn) = (part.manufacturer, part.mpn)
  // Downloads
  let zip_label = if model.zip is Building {
    c.zip_building
  } else {
    c.zip_button
  }
  let downloads : Array[Html] = [
    h3(c.download),
    div(class="artifact-buttons", [
      button(
        type_="button",
        class="button button-primary",
        disabled=model.zip is Building,
        on_click=emit(ZipClicked),
        zip_label,
      ),
      a(
        class="button",
        href=artifact_url(m, mpn, "stl"),
        download="\{mpn}.stl",
        c.stl_button,
        escape=true,
      ),
    ]),
  ]
  if model.zip is ZipFailed(e) {
    downloads.push(
      p(class="text-muted", if e.is_empty() { c.zip_failed } else { e }),
    )
  }
  if data.artifacts_error is Some(e) {
    downloads.push(
      p(class="text-muted", if e.is_empty() { c.artifacts_failed } else { e }),
    )
  }
  // 3D and footprint: the wasm viewer fills the empty #model-viewer and
  // #footprint-viewer (viewer.mbt). Without it, the footprint artifact's
  // plain SVG.
  let viewer : Array[Html] = [
    div(id=VIEWER_ID, class="model-viewer-canvas", nodes([])),
    p(class="text-muted viewer-controls", c.viewer_controls),
  ]
  if model.viewer_error is Some(e) {
    viewer.push(
      p(class="text-muted", if e.is_empty() { c.model_load_failed } else { e }),
    )
  }
  let footprint_view = if model.viewer_error is Some(_) {
    img(src=artifact_url(m, mpn, "footprint-svg"), alt=footprint_alt(lang, mpn))
  } else {
    div(id=FOOTPRINT_ID, class="footprint-viewer", nodes([]))
  }
  // Pins
  let device = data.device
  let numbers = device.pins.keys().collect()
  numbers.sort_by(pin_order)
  let pin_rows = numbers.map(n => {
    let pin = device.pins[n]
    let alt = pin.alternate_functions.join(", ")
    tr([
      td(n),
      td([
        cited_value(
          lang,
          data,
          "device",
          ["/pins/\{n}/name", "/pins/\{n}", "/pins"],
          "\{c.pin_number} \{n}",
          pin.name,
        ),
      ]),
      td(pin.type_.wire()),
      td(if alt.is_empty() { "-" } else { alt }),
    ])
  })
  let sections : Array[Html] = [
    h1([text(part.mpn + " "), span(class="text-muted", "/ \{m}")]),
    div(class="result-badges", [
      existence_badge(part.existence.status),
      lifecycle_badge(part.lifecycle.status),
      review_badge(lang, data.documents),
    ]),
    section(class="section", downloads),
    section(class="section", [h3(c.preview3d), ..viewer]),
    section(class="section", [
      h3(c.footprint),
      figure(class="footprint-preview", [
        footprint_view,
        figcaption(class="text-muted", c.footprint_legend),
      ]),
    ]),
    section(class="section", [
      div(class="pin-table-header", [
        h3(pins_heading(lang, device.id)),
        ext_link(report_error_url(data), c.report_issue, class="button"),
      ]),
      details(class="pin-table-details", [
        summary(pins_show(lang, numbers.length())),
        table([
          thead([
            tr([th(c.pin_number), th(c.pin_name), th(c.pin_type), th(c.pin_alt)]),
          ]),
          tbody(pin_rows),
        ]),
      ]),
    ]),
  ]
  if !data.revisions.is_empty() {
    sections.push(
      section(class="section", [
        h3(c.revisions),
        ul(data.revisions.map(r => li("\{r.id} (\{r.manufacturer_revision})"))),
      ]),
    )
  }
  let pkg = data.package_
  let dims = pkg.dimensions
  let rows = [
    (c.family, pkg.family.wire(), "/family"),
    (c.lead_count, pkg.lead_count.to_string(), "/lead_count"),
    (c.pitch, format_dimension(Some(pkg.pitch)), "/pitch"),
    (
      c.body_width,
      format_dimension(Some(dims.body_width)),
      "/dimensions/body_width",
    ),
    (
      c.body_length,
      format_dimension(Some(dims.body_length)),
      "/dimensions/body_length",
    ),
    (
      c.body_height,
      format_dimension(dims.body_height),
      "/dimensions/body_height",
    ),
  ]
  sections.push(
    section(class="section", [
      h3(package_heading(lang, pkg.id)),
      table(class="cited-table", [
        tbody(
          rows.map(r => {
            tr([
              th(r.0),
              td([cited_value(lang, data, "package", [r.2], r.0, r.1)]),
              td([cited_where(lang, data, "package", [r.2])]),
            ])
          }),
        ),
      ]),
    ]),
  )
  sections.push(review_section(lang, data))
  sections.push(alternatives_section(model, emit))
  sections.push(
    section(class="section", [
      h3(c.sources_heading),
      sources_view(lang, data.sources),
    ]),
  )
  div(sections)
}

///|
fn sources_view(lang : Lang, sources : Array[@api.PartSource]) -> Html {
  let c = lang.copy()
  if sources.is_empty() {
    return p(class="text-muted", c.sources_none)
  }
  div([
    p(class="text-muted", c.sources_note),
    ul(
      class="source-list",
      sources.map(ps => {
        let s = ps.source
        let mut meta = "\{s.document_type.wire()} · \{source_revision(lang, s.revision, s.date)}"
        if s.retrieved is Some(r) {
          meta = meta + " · " + source_retrieved(lang, r)
        }
        let cited = ps.cited_by.map(b => cited_by_label(lang, b)).join(", ")
        meta = meta + " · \{c.source_cited_by}: \{cited}"
        li([
          ext_link(s.url, "\{s.publisher.to_upper()} \{s.document_id}"),
          text(" "),
          span(class="text-muted", meta),
        ])
      }),
    ),
  ])
}
