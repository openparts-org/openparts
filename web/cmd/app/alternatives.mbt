// The part page's alternatives section: `GET .../alternatives`, in
// tabs per class (drop-in, footprint-compatible with another pinout,
// functional equivalent, near miss), each candidate with its parameter
// comparison and pad-by-pad pin diff.

///|
#derive(Eq)
struct AltState {
  /// (manufacturer, mpn) loaded or loading.
  key : (String, String)
  data : Fetch[@api.AlternativesResponse]
  /// The class tab shown ("" = the first with candidates).
  tab : String
}

///|
fn AltState::new() -> AltState {
  { key: ("", ""), data: Loading, tab: "", }
}

///|
enum AltMsg {
  AltGot((String, String), Fetch[@api.AlternativesResponse])
  AltTab(String)
}

///|
let alt_classes : Array[@api.AlternativeClass] = [
  DropIn,
  FootprintCompatible,
  Functional,
  NearMiss,
  Listed,
]

///|
/// Starts loading the alternatives of (m, p) unless they are loaded.
fn alt_enter(
  model : Model,
  m : String,
  p : String,
  emit : Emit[Msg],
) -> (Model, @rabbita.Cmd) {
  let key = (m, p)
  if model.alts.key == key && !(model.alts.data is Failed(_)) {
    return (model, @rabbita.none)
  }
  let path = part_api_path(m, p) + "/alternatives"
  let cmd = @rabbita.perform(r => emit(Alt(AltGot(key, r))), async fn() noraise {
    get_json(path)
  })
  ({ ..model, alts: { key, data: Loading, tab: "", }, }, cmd)
}

///|
fn alt_update(model : Model, msg : AltMsg) -> (Model, @rabbita.Cmd) {
  let s = model.alts
  match msg {
    AltGot(key, data) => {
      guard key == s.key else { (model, @rabbita.none) }
      ({ ..model, alts: { ..s, data, }, }, @rabbita.none)
    }
    AltTab(tab) => ({ ..model, alts: { ..s, tab, }, }, @rabbita.none)
  }
}

///|
fn alt_class_badge(c : @api.AlternativeClass) -> String {
  match c {
    DropIn => "badge badge-good"
    FootprintCompatible => "badge badge-bad"
    Functional => "badge badge-neutral"
    NearMiss => "badge badge-warn"
    Listed => "badge badge-neutral"
  }
}

///|
fn verdict_class(v : @api.Verdict) -> String {
  match v {
    Meets => "badge badge-good"
    Unverified => "badge badge-neutral"
    Worse => "badge badge-warn"
  }
}

///|
fn check_class(s : @api.CheckStatus) -> String {
  match s {
    Pass | Better => "alt-pass"
    Info => "alt-info"
    Unverified => "alt-unverified"
    Fail => "alt-fail"
  }
}

///|
fn pad_class(s : @api.PadStatus) -> String {
  match s {
    Same | Equivalent => "alt-pass"
    Note => "alt-unverified"
    Differs => "alt-fail"
  }
}

///|
/// One line on the pinout: compatible, or which pads differ.
fn pins_summary(lang : Lang, a : @api.Alternative) -> Html {
  let c = lang.bom()
  match a.pins {
    Some(pins) if pins.compatible =>
      span(class="badge badge-good", c.alt_pins_ok)
    Some(pins) =>
      span(
        class="badge badge-bad",
        "\{c.alt_pins_bad}: \{c.alt_pad} \{pins.differing.join(", ")}",
      )
    None => span(class="badge badge-neutral", c.alt_footprint_diff)
  }
}

///|
fn alternatives_section(model : Model, emit : Emit[Msg]) -> Html {
  let lang = model.lang
  let c = lang.bom()
  let body : Array[Html] = [h3(c.alt_title)]
  match model.alts.data {
    Loading => body.push(p(class="text-muted", c.alt_loading))
    Missing => body.push(p(class="text-muted", c.alt_none))
    Failed(e) => body.push(p(class="text-muted", e))
    Got(r) => body.push(alternatives_view(lang, model.alts.tab, r, emit))
  }
  section(class="section alternatives", body)
}

///|
fn alternatives_view(
  lang : Lang,
  tab : String,
  r : @api.AlternativesResponse,
  emit : Emit[Msg],
) -> Html {
  let c = lang.bom()
  let count = (cl : @api.AlternativeClass) => {
    r.alternatives.iter().filter(a => a.class == cl).count()
  }
  if r.alternatives.is_empty() {
    let out = [p(class="text-muted", c.alt_none)]
    if r.rejected > 0 {
      out.push(p(class="text-muted", alt_rejected_note(lang, r.rejected)))
    }
    return div(out)
  }
  let shown = if tab.is_empty() {
    match alt_classes.iter().find_first(cl => count(cl) > 0) {
      Some(cl) => cl.wire()
      None => ""
    }
  } else {
    tab
  }
  let tabs = alt_classes
    .filter(cl => count(cl) > 0 || cl != Listed)
    .map(cl => {
      let n = count(cl)
      button(
        type_="button",
        class="alt-tab" +
          (if cl.wire() == shown { " alt-tab-active" } else { "" }),
        disabled=n == 0,
        title=alt_class_help(lang, cl),
        attrs=@html.Attrs::build()
          .attribute("role", "tab")
          .aria_selected((cl.wire() == shown).to_string()),
        on_click=emit(Alt(AltTab(cl.wire()))),
        [
          text(alt_class_label(lang, cl) + " "),
          span(class="chip-count", n.to_string()),
        ],
      )
    })
  let items : Array[Html] = []
  for a in r.alternatives {
    if a.class.wire() != shown {
      continue
    }
    items.push(alternative_card(lang, r.part, a))
  }
  let out : Array[Html] = [
    div(
      class="alt-tabs",
      attrs=@html.Attrs::build().attribute("role", "tablist"),
      tabs,
    ),
    div(class="alt-list", items),
  ]
  if r.rejected > 0 {
    out.push(
      p(class="text-muted alt-rejected", alt_rejected_note(lang, r.rejected)),
    )
  }
  div(out)
}

///|
fn param_display(lang : Lang, key : String, v : String?) -> String {
  match v {
    Some(v) => param_value_label(lang, key, v)
    None => "?"
  }
}

///|
fn alternative_card(
  lang : Lang,
  orig : @api.PartBrief,
  a : @api.Alternative,
) -> Html {
  let c = lang.bom()
  let p = a.part
  let pkg = match p.size_code {
    Some(code) => code
    None =>
      match (p.package_family, p.lead_count) {
        (Some(f), Some(n)) =>
          "\{family_label(lang, f.wire(), f.wire().to_upper())} \{n}"
        _ => package_name(p.package_)
      }
  }
  let head : Array[Html] = [
    a_link(lang, p),
    text(" "),
    span(class=alt_class_badge(a.class), alt_class_label(lang, a.class)),
  ]
  if a.listed_replacement {
    head.push(text(" "))
    head.push(span(class="badge badge-good", c.alt_replacement))
  }
  let badges = [
    span(class=verdict_class(a.verdict), verdict_label(lang, a.verdict)),
    span(
      class=if a.footprint.same_land_pattern {
        "badge badge-good"
      } else {
        "badge badge-neutral"
      },
      if a.footprint.same_land_pattern {
        c.alt_footprint_same
      } else {
        c.alt_footprint_diff
      },
    ),
    pins_summary(lang, a),
    span(
      class=lifecycle_class(p.lifecycle),
      lifecycle_label(lang, p.lifecycle.wire()),
    ),
  ]
  // Keys that fail or are unverified, on the card itself.
  let flagged = a.parameters
    .filter(x => x.status is (Fail | Unverified))
    .map(x => {
      span(
        class=check_class(x.status),
        "\{param_name(lang, x.key)}: \{param_display(lang, x.key, x.original)} → \{param_display(lang, x.key, x.candidate)}",
      )
    })
  // The comparison tables.
  let rows = a.parameters.map(x => {
    tr(class=check_class(x.status), [
      td(param_name(lang, x.key)),
      td(class="nowrap", rule_label(lang, x.rule)),
      td(class="num", param_display(lang, x.key, x.original)),
      td(class="num", param_display(lang, x.key, x.candidate)),
      td(class="nowrap", check_status_label(lang, x.status)),
    ])
  })
  let tables : Array[Html] = []
  if !rows.is_empty() {
    tables.push(h3(class="alt-sub", c.alt_parameters))
    tables.push(
      div(class="bom-table-wrap", [
        table(class="alt-table", [
          thead([
            tr([
              th(scope=Col, c.alt_key),
              th(scope=Col, c.alt_rule),
              th(scope=Col, class="num", "\{c.alt_original} (\{orig.mpn})"),
              th(scope=Col, class="num", "\{c.alt_candidate} (\{p.mpn})"),
              th(scope=Col, c.alt_result),
            ]),
          ]),
          tbody(rows),
        ]),
      ]),
    )
  }
  if a.pins is Some(pins) {
    let pin = (b : @api.PinBrief?) => {
      match b {
        Some(b) =>
          [strong(b.name), span(class="text-muted", " \{b.type_.wire()}")]
        None => [span(class="text-muted", "—")]
      }
    }
    let pad_rows = pins.pads.map(d => {
      let reason = match d.reason {
        Some(r) => pad_reason_label(lang, r)
        None => ""
      }
      tr(class=pad_class(d.status), [
        td(
          class="num",
          if d.pad == d.candidate_pad {
            d.pad
          } else {
            "\{d.pad} → \{d.candidate_pad}"
          },
        ),
        td(pin(d.original)),
        td(pin(d.candidate)),
        td(class="nowrap", pad_status_label(lang, d.status)),
        td(class="text-muted", reason),
      ])
    })
    tables.push(h3(class="alt-sub", c.alt_pins))
    tables.push(
      div(class="bom-table-wrap", [
        table(class="alt-table alt-pins", [
          thead([
            tr([
              th(scope=Col, class="num", c.alt_pad),
              th(scope=Col, "\{c.alt_original} (\{orig.mpn})"),
              th(scope=Col, "\{c.alt_candidate} (\{p.mpn})"),
              th(scope=Col, c.alt_status),
              th(scope=Col, ""),
            ]),
          ]),
          tbody(pad_rows),
        ]),
      ]),
    )
  }
  // A different pinout is the point of its class: shown open.
  let open = a.class is FootprintCompatible
  div(class="card alt-card", [
    div(class="alt-head", head),
    div(
      class="alt-badges",
      [
        ..badges,
        span(
          class="text-muted",
          " \{c.alt_package}: \{pkg} · \{p.manufacturer_name}",
        ),
      ],
    ),
    if flagged.is_empty() {
      text("")
    } else {
      div(class="alt-flagged", flagged)
    },
    if open {
      details(class="alt-details", open=true, [
        summary(c.alt_compare),
        div(tables),
      ])
    } else {
      details(class="alt-details", [summary(c.alt_compare), div(tables)])
    },
  ])
}

///|
fn a_link(lang : Lang, p : @api.PartBrief) -> Html {
  a(href=href(lang, part_path(p.manufacturer, p.mpn)), [strong(p.mpn)])
}
