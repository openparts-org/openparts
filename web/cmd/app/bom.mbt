// The BOM check page at /bom: a BOM file (dropped, picked or pasted)
// goes to `POST /v1/bom/check`; the results table groups the lines by
// status, filters them, expands a line to its match details and
// alternatives, and exports the annotated BOM as CSV (built here, in
// the browser).

///|
const BOM_DROP_ID : String = "bom-drop"

///|
const BOM_FILE_ID : String = "bom-file"

///|
/// The server's limit (openparts-bom MAX_BYTES).
const BOM_MAX_BYTES : Int32 = 2097152

///|
#derive(Eq)
enum BomPhase {
  Idle
  Checking
  Done(@api.BomReport)
  BomFailed(String)
}

///|
#derive(Eq)
struct BomState {
  /// The BOM text (pasted, or the loaded file's).
  text : String
  /// The loaded file's name ("" when pasted).
  file_name : String
  /// Bumped per request, so an old response can't replace a newer one.
  seq : Int32
  phase : BomPhase
  /// The status shown ("" for all): a `LineStatus` wire name.
  filter : String
  /// Expanded lines (indices into the report's lines).
  open : Array[Int32]
  /// Waiting for a dropped or picked file.
  armed : Bool
}

///|
fn BomState::new() -> BomState {
  {
    text: "",
    file_name: "",
    seq: 0,
    phase: Idle,
    filter: "",
    open: [],
    armed: false,
  }
}

///|
enum BomMsg {
  BomText(String)
  /// A dropped or picked file: name, text, size in bytes.
  BomFile(String, String, Int32)
  BomCheck
  BomGot(Int32, Result[@api.BomReport, String])
  BomFilter(String)
  BomToggle(Int32)
  BomExport
  BomClear
  BomSample
  BomSampleGot(Result[String, String])
}

// ---- browser calls ----

///|
#external
type JsFile

///|
extern "js" fn JsFile::name(self : JsFile) -> String =
  #|(f) => f.name

///|
extern "js" fn JsFile::text(self : JsFile) -> String =
  #|(f) => f.text

///|
extern "js" fn JsFile::size(self : JsFile) -> Int32 =
  #|(f) => f.size

///|
/// Resolves with the next file dropped on the element `zone` or picked
/// with the file input `input`, read as text (UTF-8, or UTF-16 with a
/// byte-order mark). One listener for the page; each call replaces the
/// pending one.
extern "js" fn bom_next_file(
  zone : String,
  input : String,
) -> @js_async.Promise[JsFile] =
  #|(zone, input) => new Promise((resolve) => {
  #|  const g = (globalThis.__openpartsBom ??= { resolve: null, installed: false });
  #|  g.resolve = resolve;
  #|  if (g.installed) return;
  #|  g.installed = true;
  #|  const read = async (file) => {
  #|    if (!file) return;
  #|    const buf = new Uint8Array(await file.arrayBuffer());
  #|    let enc = "utf-8";
  #|    if (buf[0] === 0xff && buf[1] === 0xfe) enc = "utf-16le";
  #|    else if (buf[0] === 0xfe && buf[1] === 0xff) enc = "utf-16be";
  #|    const text = new TextDecoder(enc).decode(buf).replace(/^﻿/, "");
  #|    const r = g.resolve;
  #|    g.resolve = null;
  #|    if (r) r({ name: file.name, text, size: file.size });
  #|  };
  #|  const zoneOf = (e) => e.target && e.target.closest ? e.target.closest("#" + zone) : null;
  #|  document.addEventListener("dragover", (e) => {
  #|    const z = zoneOf(e);
  #|    if (z) { e.preventDefault(); z.classList.add("bom-drop-active"); }
  #|  });
  #|  document.addEventListener("dragleave", (e) => {
  #|    const z = zoneOf(e);
  #|    if (z) z.classList.remove("bom-drop-active");
  #|  });
  #|  document.addEventListener("drop", (e) => {
  #|    const z = zoneOf(e);
  #|    if (!z) return;
  #|    e.preventDefault();
  #|    z.classList.remove("bom-drop-active");
  #|    read(e.dataTransfer && e.dataTransfer.files[0]);
  #|  });
  #|  document.addEventListener("change", (e) => {
  #|    if (!e.target || e.target.id !== input) return;
  #|    read(e.target.files && e.target.files[0]);
  #|    e.target.value = "";
  #|  });
  #|})

///|
/// `POST url` with a text body; the same response shape as `fetch_raw`.
extern "js" fn post_text(
  url : String,
  body : String,
) -> @js_async.Promise[JsResponse] =
  #|async (url, body) => {
  #|  const r = await fetch(url, { method: "POST", headers: { "content-type": "text/plain; charset=utf-8" }, body });
  #|  return { status: r.status, statusText: r.statusText, body_text: await r.text() };
  #|}

///|
/// Hands `text` (UTF-8, with a byte-order mark so spreadsheets read it
/// as UTF-8) to the browser as a file download.
extern "js" fn save_text(
  text : String,
  filename : String,
  mime : String,
) -> Unit =
  #|(text, name, mime) => {
  #|  const url = URL.createObjectURL(new Blob(["﻿" + text], { type: mime }));
  #|  const a = document.createElement("a");
  #|  a.href = url;
  #|  a.download = name;
  #|  a.click();
  #|  setTimeout(() => URL.revokeObjectURL(url), 0);
  #|}

///|
extern "js" fn utf8_length(s : String) -> Int32 =
  #|(s) => new Blob([s]).size

// ---- update ----

///|
fn bom_arm(emit : Emit[Msg]) -> @rabbita.Cmd {
  @rabbita.perform(
    (f : JsFile?) => {
      match f {
        Some(f) => emit(Bom(BomFile(f.name(), f.text(), f.size())))
        None => @rabbita.none
      }
    },
    async fn() noraise {
      Some(bom_next_file(BOM_DROP_ID, BOM_FILE_ID).wait()) catch {
        _ => None
      }
    },
  )
}

///|
/// The URL moved to /bom: start listening for files.
fn bom_enter(model : Model, emit : Emit[Msg]) -> (Model, @rabbita.Cmd) {
  if model.bom.armed {
    return (model, @rabbita.none)
  }
  ({ ..model, bom: { ..model.bom, armed: true, }, }, bom_arm(emit))
}

///|
async fn post_bom(text : String) -> Result[@api.BomReport, String] noraise {
  try {
    let r = post_text(api_base() + "/v1/bom/check", text).wait()
    if r.status() == 200 {
      Ok(@json.from_json(@json.parse(r.body_text())))
    } else {
      Err(error_message(r))
    }
  } catch {
    e => Err(e.to_string())
  }
}

///|
fn bom_check_cmd(s : BomState, emit : Emit[Msg]) -> (BomState, @rabbita.Cmd) {
  if s.text.trim().is_empty() {
    return (s, @rabbita.none)
  }
  if utf8_length(s.text) > BOM_MAX_BYTES {
    return ({ ..s, phase: BomFailed(""), }, @rabbita.none)
  }
  let seq = s.seq + 1
  let text = s.text
  let cmd = @rabbita.perform(r => emit(Bom(BomGot(seq, r))), async fn() noraise {
    post_bom(text)
  })
  ({ ..s, seq, phase: Checking, open: [], }, cmd)
}

///|
fn bom_update(
  model : Model,
  msg : BomMsg,
  emit : Emit[Msg],
) -> (Model, @rabbita.Cmd) {
  let s = model.bom
  let set = (s : BomState) => { ..model, bom: s, }
  match msg {
    BomText(text) => (set({ ..s, text, file_name: "", }), @rabbita.none)
    BomFile(name, text, size) => {
      let s = { ..s, text, file_name: name, }
      if size > BOM_MAX_BYTES {
        return (set({ ..s, phase: BomFailed(""), }), bom_arm(emit))
      }
      let (s, cmd) = bom_check_cmd(s, emit)
      (set(s), @rabbita.batch([cmd, bom_arm(emit)]))
    }
    BomCheck => {
      let (s, cmd) = bom_check_cmd(s, emit)
      (set(s), cmd)
    }
    BomGot(seq, result) => {
      guard seq == s.seq else { (model, @rabbita.none) }
      let phase = match result {
        Ok(r) => Done(r)
        Err(e) => BomFailed(e)
      }
      (set({ ..s, phase, filter: "", }), @rabbita.none)
    }
    BomFilter(f) => (set({ ..s, filter: f, }), @rabbita.none)
    BomToggle(i) => {
      let open = if s.open.contains(i) {
        s.open.filter(x => x != i)
      } else {
        let more = s.open.copy()
        more.push(i)
        more
      }
      (set({ ..s, open, }), @rabbita.none)
    }
    BomExport => {
      if s.phase is Done(r) {
        let name = if s.file_name.is_empty() {
          "bom"
        } else {
          strip_extension(s.file_name)
        }
        save_text(annotated_csv(r), "\{name}-openparts.csv", "text/csv")
      }
      (model, @rabbita.none)
    }
    BomClear =>
      (
        set({
          ..s,
          text: "",
          file_name: "",
          phase: Idle,
          open: [],
          seq: s.seq + 1,
        }),
        @rabbita.none,
      )
    BomSample => {
      let cmd = @rabbita.perform(r => emit(Bom(BomSampleGot(r))), async fn() noraise {
        try {
          let r = fetch_raw(base_path() + "samples/pico-kicad-bom.csv", false).wait()
          if r.status() == 200 {
            Ok(r.body_text())
          } else {
            Err(error_message(r))
          }
        } catch {
          e => Err(e.to_string())
        }
      })
      (model, cmd)
    }
    BomSampleGot(Ok(text)) => {
      let (s, cmd) = bom_check_cmd(
        { ..s, text, file_name: "pico-kicad-bom.csv", },
        emit,
      )
      (set(s), cmd)
    }
    BomSampleGot(Err(e)) => (set({ ..s, phase: BomFailed(e), }), @rabbita.none)
  }
}

///|
fn strip_extension(name : String) -> String {
  match name.rev_find(".") {
    Some(i) if i > 0 => name[:i].to_owned()
    _ => name
  }
}

// ---- CSV export ----

///|
fn csv_cell(s : String) -> String {
  if s.contains(",") || s.contains("\"") || s.contains("\n") || s.contains("\r") {
    "\"" + s.replace_all(old="\"", new="\"\"") + "\""
  } else {
    s
  }
}

///|
/// The BOM as checked: each line's own fields, then what OpenParts
/// found (wire names, so the file reads the same in every language).
fn annotated_csv(r : @api.BomReport) -> String {
  let rows = [
    [
      "References", "Qty", "Value", "Footprint", "MPN", "Manufacturer", "OpenParts status",
      "Match", "Confidence", "OpenParts manufacturer", "OpenParts MPN", "Package",
      "Lifecycle", "Existence", "Issues", "Candidates", "Alternatives",
    ],
  ]
  for l in r.lines {
    let best = l.matches.get(0)
    let issues = l.issues.map(i => {
      match i.detail {
        Some(d) => "\{i.code.wire()} (\{d})"
        None => i.code.wire()
      }
    })
    let candidates = l.matches.map(m => "\{m.part.manufacturer}/\{m.part.mpn}")
    let alternatives = l.alternatives.map(a => {
      "\{a.part.manufacturer}/\{a.part.mpn} (\{a.class.wire()})"
    })
    let confidence = (l.confidence * 100.0).round() / 100.0
    rows.push([
      l.references.join(" "),
      l.quantity.to_string(),
      l.value,
      l.footprint,
      l.mpn,
      l.manufacturer,
      l.status.wire(),
      l.kind.wire(),
      confidence.to_string(),
      match best {
        Some(m) => m.part.manufacturer_name
        None => ""
      },
      match best {
        Some(m) => m.part.mpn
        None => ""
      },
      match best {
        Some(m) => m.part.package_
        None => ""
      },
      match best {
        Some(m) => m.part.lifecycle.wire()
        None => ""
      },
      match best {
        Some(m) => m.part.existence.wire()
        None => ""
      },
      issues.join("; "),
      candidates.join(" "),
      alternatives.join("; "),
    ])
  }
  rows.map(r => r.map(csv_cell).join(",")).join("\r\n") + "\r\n"
}

// ---- views ----

///|
let status_order : Array[@api.LineStatus] = [NotFound, Warning, Review, Ok]

///|
fn line_status_class(s : @api.LineStatus) -> String {
  match s {
    Ok => "badge badge-good"
    Review => "badge badge-neutral"
    Warning => "badge badge-warn"
    NotFound => "badge badge-bad"
  }
}

///|
fn summary_count(sum : @api.BomSummary, s : @api.LineStatus) -> Int32 {
  match s {
    Ok => sum.ok
    Review => sum.review
    Warning => sum.warning
    NotFound => sum.not_found
  }
}

///|
fn bom_view(model : Model, emit : Emit[Msg]) -> Html {
  let lang = model.lang
  let c = lang.bom()
  let s = model.bom
  let checking = s.phase is Checking
  let drop : Array[Html] = [
    p([strong(c.drop_here)]),
    @html.label(class="button", for_=BOM_FILE_ID, [
      text(c.choose_file),
      input(
        input_type=File,
        id=BOM_FILE_ID,
        class="visually-hidden",
        accept=".csv,.tsv,.txt,.net,text/csv,text/plain",
      ),
    ]),
    p(class="text-muted bom-formats", c.formats),
  ]
  if !s.file_name.is_empty() {
    drop.push(
      p(class="bom-file-name", [text(c.file_label + ": "), code(s.file_name)]),
    )
  }
  let input_section = section(class="section bom-input", [
    div(id=BOM_DROP_ID, class="bom-drop", drop),
    div(class="bom-paste", [
      @html.label(for_="bom-text", c.paste_label),
      @html.textarea(
        id="bom-text",
        class="bom-textarea",
        rows=7,
        value=s.text,
        placeholder=c.paste_placeholder,
        on_input=emit.map(t => Bom(BomText(t))),
        nodes([]),
      ),
      div(class="bom-actions", [
        button(
          type_="button",
          class="button button-primary",
          disabled=checking || s.text.trim().is_empty(),
          on_click=emit(Bom(BomCheck)),
          if checking {
            c.checking
          } else {
            c.check
          },
        ),
        button(
          type_="button",
          class="button",
          on_click=emit(Bom(BomSample)),
          c.sample,
        ),
        button(
          type_="button",
          class="button",
          disabled=s.text.is_empty() && s.phase is Idle,
          on_click=emit(Bom(BomClear)),
          c.clear,
        ),
      ]),
    ]),
  ])
  let body : Array[Html] = [h1(c.title), input_section]
  match s.phase {
    Idle => ()
    Checking => body.push(p(class="text-muted", c.checking))
    BomFailed(e) =>
      body.push(
        p(
          class="bom-error",
          if e.is_empty() {
            c.too_large
          } else {
            "\{c.failed}: \{e}"
          },
        ),
      )
    Done(r) => body.push(bom_results(lang, s, r, emit))
  }
  div(class="bom-page", body)
}

///|
fn bom_results(
  lang : Lang,
  s : BomState,
  r : @api.BomReport,
  emit : Emit[Msg],
) -> Html {
  let c = lang.bom()
  let sum = r.summary
  let chips : Array[Html] = [
    chip("\{c.all} \{sum.lines}", s.filter.is_empty(), emit(Bom(BomFilter("")))),
  ]
  for st in status_order {
    let n = summary_count(sum, st)
    chips.push(
      button(
        type_="button",
        class="chip bom-chip bom-chip-\{st.wire()}" +
          (if s.filter == st.wire() { " chip-active" } else { "" }),
        disabled=n == 0,
        on_click=emit(Bom(BomFilter(st.wire()))),
        [
          text(line_status_label(lang, st) + " "),
          span(class="chip-count", n.to_string()),
        ],
      ),
    )
  }
  let columns = r.columns
    .map(col => "\{column_role_label(lang, col.role)} ← \"\{col.header}\"")
    .join(" · ")
  let head = div(class="bom-summary", [
    div(class="bom-summary-line", [
      strong(format_label(lang, r.format)),
      span(
        class="text-muted",
        " · \{sum.lines} \{c.summary_lines} · \{sum.components} \{c.summary_components}",
      ),
      button(
        type_="button",
        class="button bom-export",
        on_click=emit(Bom(BomExport)),
        c.export_csv,
      ),
    ]),
    p(class="text-muted bom-columns", "\{c.detected_columns}: \{columns}"),
    div(class="chip-row", chips),
  ])
  // Grouped by status, problems first.
  let groups : Array[Html] = []
  for st in status_order {
    if !s.filter.is_empty() && s.filter != st.wire() {
      continue
    }
    let rows : Array[Html] = []
    for i, l in r.lines {
      if l.status != st {
        continue
      }
      rows.push(bom_row(lang, i, l, s.open.contains(i), emit))
      if s.open.contains(i) {
        rows.push(bom_detail_row(lang, l))
      }
    }
    if rows.is_empty() {
      continue
    }
    groups.push(
      section(class="bom-group", [
        h3([
          span(class=line_status_class(st), line_status_label(lang, st)),
          text(" "),
          span(class="text-muted", alt_count_note(lang, summary_count(sum, st))),
        ]),
        div(class="bom-table-wrap", [
          table(class="bom-table", [
            thead([
              tr([
                th(scope=Col, c.c_refs),
                th(scope=Col, class="num", c.c_qty),
                th(scope=Col, c.c_bom),
                th(scope=Col, c.c_match),
                th(scope=Col, c.c_how),
                th(scope=Col, c.c_issues),
                th(scope=Col, ""),
              ]),
            ]),
            tbody(rows),
          ]),
        ]),
      ]),
    )
  }
  div(class="section bom-results", [head, ..groups])
}

///|
fn part_link(lang : Lang, part : @api.PartBrief) -> Html {
  a(href=href(lang, part_path(part.manufacturer, part.mpn)), [
    strong(part.mpn),
    span(class="text-muted", " / \{part.manufacturer_name}"),
  ])
}

///|
fn issue_chip(lang : Lang, i : @api.BomIssue) -> Html {
  let class = match i.severity {
    Warning => "badge badge-warn"
    Info => "badge badge-neutral"
  }
  let label = match (i.code, i.detail) {
    (Ambiguous, Some(n)) => "\{issue_label(lang, i.code)} (\{n})"
    (FootprintMismatch, Some(d)) => "\{issue_label(lang, i.code)}: \{d}"
    (ManufacturerMismatch, Some(d)) => "\{issue_label(lang, i.code)}: \{d}"
    _ => issue_label(lang, i.code)
  }
  span(class="\{class} bom-issue", label)
}

///|
fn bom_row(
  lang : Lang,
  i : Int32,
  l : @api.BomLineResult,
  open : Bool,
  emit : Emit[Msg],
) -> Html {
  let c = lang.bom()
  let refs = if l.references.length() > 6 {
    l.references[:5].iter().collect().join(", ") +
    " … " +
    l.references[l.references.length() - 1]
  } else {
    l.references.join(", ")
  }
  let entry : Array[Html] = []
  if !l.mpn.is_empty() {
    entry.push(div([strong(l.mpn)]))
    if !l.manufacturer.is_empty() {
      entry.push(div(class="text-muted", l.manufacturer))
    }
  }
  if !l.value.is_empty() && l.value != l.mpn {
    entry.push(div(l.value))
  }
  if !l.footprint.is_empty() {
    entry.push(div(class="text-muted bom-fp", l.footprint))
  }
  let matched : Array[Html] = match l.matches.get(0) {
    Some(m) => {
      let out = [part_link(lang, m.part)]
      if l.matches.length() > 1 {
        out.push(
          div(class="text-muted", more_candidates(lang, l.matches.length() - 1)),
        )
      }
      out
    }
    None => [span(class="text-muted", "—")]
  }
  let how = [
    div(match_kind_label(lang, l.kind)),
    div(
      class="text-muted",
      if l.matches.is_empty() {
        ""
      } else {
        "\{c.confidence} \{((l.confidence * 100.0).round()).to_int()}%"
      },
    ),
  ]
  let issues = l.issues.map(x => issue_chip(lang, x))
  tr(class=if open { "bom-row bom-row-open" } else { "bom-row" }, [
    td(class="bom-refs", title=l.references.join(", "), refs),
    td(class="num", l.quantity.to_string()),
    td(entry),
    td(matched),
    td(class="nowrap", how),
    td(class="bom-issues", issues),
    td([
      button(
        type_="button",
        class="button bom-toggle",
        attrs=@html.Attrs::build().aria_expanded(open.to_string()),
        on_click=emit(Bom(BomToggle(i))),
        if open {
          c.hide
        } else {
          c.details
        },
      ),
    ]),
  ])
}

///|
fn bom_detail_row(lang : Lang, l : @api.BomLineResult) -> Html {
  let c = lang.bom()
  let blocks : Array[Html] = []
  if l.issues.is_empty() {
    blocks.push(p(class="text-muted", c.no_issues))
  }
  if l.matches.is_empty() {
    blocks.push(p(class="text-muted", issue_label(lang, NotFound)))
  } else {
    let items = l.matches.map(m => {
      let cov = m.coverage
      let fp_class = match m.footprint.status {
        Match => "badge badge-good"
        Mismatch => "badge badge-bad"
        Unknown => "badge badge-neutral"
      }
      let fp_text = match m.footprint.bom {
        Some(b) =>
          "\{c.bom_footprint} \{b} / \{c.part_package} \{m.footprint.part}"
        None => "\{c.part_package} \{m.footprint.part}"
      }
      let params = m.part.parameters
        .iter()
        .map(kv => {
          "\{param_name(lang, kv.0)} \{param_value_label(lang, kv.0, kv.1)}"
        })
        .collect()
        .join(" · ")
      li(class="bom-candidate", [
        div([
          part_link(lang, m.part),
          text(" "),
          span(
            class=lifecycle_class(m.part.lifecycle),
            lifecycle_label(lang, m.part.lifecycle.wire()),
          ),
          text(" "),
          span(
            class="badge badge-neutral",
            existence_label(lang, m.part.existence.wire()),
          ),
          text(" "),
          span(
            class="text-muted",
            "\{c.confidence} \{((m.confidence * 100.0).round()).to_int()}%",
          ),
        ]),
        div(class="text-muted", [
          text("\{c.footprint_check}: "),
          span(class=fp_class, fp_text),
          text(
            " · \{c.coverage}: \{coverage_label(lang, cov.expected_present, cov.expected)} · \{sources_count(lang, cov.sources)}",
          ),
        ]),
        if params.is_empty() {
          text("")
        } else {
          div(class="bom-params", params)
        },
      ])
    })
    blocks.push(h3(c.candidates))
    blocks.push(ul(class="bom-candidates", items))
  }
  if !l.alternatives.is_empty() {
    blocks.push(h3(c.alternatives))
    blocks.push(
      ul(
        class="bom-alternatives",
        l.alternatives.map(alt => {
          li([
            span(
              class=alt_class_badge(alt.class),
              alt_class_label(lang, alt.class),
            ),
            text(" "),
            part_link(lang, alt.part),
            text(" "),
            span(
              class=verdict_class(alt.verdict),
              verdict_label(lang, alt.verdict),
            ),
            text(" "),
            pins_summary(lang, alt),
          ])
        }),
      ),
    )
  } else if l.issues
    .iter()
    .any(i => i.code is (LifecycleEol | LifecycleNrnd | LifecycleObsolete)) {
    blocks.push(p(class="text-muted", c.alternatives_none))
  }
  tr(class="bom-detail", [td(colspan=7, [div(class="bom-detail-body", blocks)])])
}
