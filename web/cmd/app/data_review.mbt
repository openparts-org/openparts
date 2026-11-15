// The part page's data review: whether the part, its device and its
// package are reviewed (their `review` blocks, Canonical Data
// Specification section 26.1), "Edit on GitHub" links to their YAML
// files, a prefilled "Report an error" issue, and values linked to the
// page their provenance cites (src/datasheet.js opens it, or the
// original PDF at #page=N). Review itself happens in pull requests to
// openparts-data.

///|
let data_repo : String = "https://github.com/openparts-org/openparts-data"

///|
fn edit_url(path : String) -> String {
  "\{data_repo}/edit/main/\{path}"
}

///|
fn pr_url(n : Int32) -> String {
  "\{data_repo}/pull/\{n}"
}

///|
fn contributing_url() -> String {
  "\{data_repo}/blob/main/CONTRIBUTING.md"
}

///|
/// `template` with `{by}` and `{pr}` replaced by `by` and `pr`.
fn fill_html(
  template : String,
  by : Array[Html],
  pr : Array[Html],
) -> Array[Html] {
  let out : Array[Html] = []
  let mut rest = template
  while true {
    let i_by = rest.find("{by}")
    let i_pr = rest.find("{pr}")
    let next = match (i_by, i_pr) {
      (Some(a), Some(b)) =>
        if a < b {
          Some((a, true))
        } else {
          Some((b, false))
        }
      (Some(a), None) => Some((a, true))
      (None, Some(b)) => Some((b, false))
      (None, None) => None
    }
    match next {
      None => {
        if !rest.is_empty() {
          out.push(text(rest))
        }
        break
      }
      Some((i, is_by)) => {
        if i > 0 {
          out.push(text(rest[:i].to_owned()))
        }
        let pieces = if is_by { by } else { pr }
        for h in pieces {
          out.push(h)
        }
        rest = rest[i + 4:].to_owned()
      }
    }
  }
  out
}

///|
/// "Reviewed by @a, @b in PR #12", with links.
fn reviewed_by_view(lang : Lang, r : @api.DocumentReview) -> Array[Html] {
  let by : Array[Html] = []
  for i, login in r.by {
    if i > 0 {
      by.push(text(", "))
    }
    by.push(ext_link("https://github.com/\{login}", "@" + login))
  }
  let pr : Array[Html] = match r.pr {
    Some(n) => [ext_link(pr_url(n), n.to_string())]
    None => [text("?")]
  }
  fill_html(lang.data_review().reviewed_by, by, pr)
}

///|
/// The same reviewers and pull request on every document: one line
/// can say it.
fn common_review(docs : @api.PartDocuments) -> @api.DocumentReview? {
  guard docs.part.review is Some(r) else { None }
  for d in [docs.device, docs.package_] {
    guard d.review is Some(o) && o.pr == r.pr && o.by == r.by else {
      return None
    }
  }
  Some(r)
}

///|
/// The badge beside existence and lifecycle; a candidate's links to the
/// review steps.
fn review_badge(lang : Lang, docs : @api.PartDocuments?) -> Html {
  let r = lang.data_review()
  guard docs is Some(d) else { text("") }
  let candidate = (label : String) => {
    a(
      href=contributing_url(),
      target=Blank,
      rel="noopener noreferrer",
      class="badge badge-warn review-badge",
      title=r.candidate_tip,
      label,
      escape=true,
    )
  }
  match d.review {
    Reviewed =>
      match common_review(d) {
        Some(one) =>
          span(
            class="badge badge-good review-badge",
            [text("✓ "), ..reviewed_by_view(lang, one)],
          )
        None => span(class="badge badge-good review-badge", "✓ " + r.reviewed)
      }
    Partly => candidate(r.partly)
    Unreviewed => candidate(r.ai_candidate)
  }
}

///|
/// GitHub's new-issue URLs stop working at about 8 KB; a prefill that
/// would pass this is left out.
let issue_url_limit : Int32 = 7500

///|
/// The "Report an error" issue: openparts-data's data-error issue form
/// (.github/ISSUE_TEMPLATE/data-error.yml) with the part, the page and
/// the files filled in by field id, and for one value also its pointer
/// and the value shown. In English, like the data repository.
fn report_error_url(
  data : PartData,
  field? : String,
  current? : String,
) -> String {
  let part = data.part
  let files : Array[String] = []
  if data.documents is Some(docs) {
    for
      item in [
        ("part", docs.part),
        ("device", docs.device),
        ("package", docs.package_),
      ] {
      files.push("\{item.0}: \{item.1.path.unwrap_or(item.1.id)}")
    }
  }
  let title = match field {
    Some(f) => "Data error: \{part.manufacturer}/\{part.mpn} \{f}"
    None => "Data error: \{part.manufacturer}/\{part.mpn}"
  }
  let params = [
    ("template", "data-error.yml"),
    ("title", title),
    ("part", part.id),
    ("page", current_href()),
    ("files", files.join("\n")),
  ]
  if field is Some(f) {
    params.push(("field", f))
  }
  if current is Some(v) {
    params.push(("current", v))
  }
  let mut url = "\{data_repo}/issues/new"
  for i, kv in params {
    let next = url +
      (if i == 0 { "?" } else { "&" }) +
      kv.0 +
      "=" +
      encode_uri_component(kv.1)
    if next.length() <= issue_url_limit {
      url = next
    }
  }
  url
}

///|
/// A small flag beside a cited value, shown on hover: reports `kind`'s
/// `pointer` (shown as `value`) as wrong.
fn report_value_link(
  lang : Lang,
  data : PartData,
  kind : String,
  pointer : String,
  value : String,
) -> Html {
  let label = lang.data_review().report_value
  a(
    href=report_error_url(data, field="\{kind} \{pointer}", current=value),
    target=Blank,
    rel="noopener noreferrer",
    class="report-value",
    title=label,
    attrs=@html.Attrs::build().aria_label(label),
    "⚑",
    escape=true,
  )
}

///|
/// The part page's review section: each document's status and file.
fn review_section(lang : Lang, data : PartData) -> Html {
  let r = lang.data_review()
  let rows : Array[Html] = []
  if data.documents is Some(docs) {
    for
      item in [
        (r.doc_part, docs.part),
        (r.doc_device, docs.device),
        (r.doc_package, docs.package_),
      ] {
      let (label, d) = item
      let status : Html = match d.review {
        Some(rv) =>
          span(
            class="review-status review-status-ok",
            reviewed_by_view(lang, rv),
          )
        None =>
          span(class="review-status review-status-candidate", r.not_reviewed)
      }
      let edit : Html = match d.path {
        Some(p) =>
          ext_link(edit_url(p), r.edit_on_github, class="button button-small")
        None => text("")
      }
      rows.push(
        tr([
          th(label),
          td([code(d.path.unwrap_or(d.id))]),
          td([status]),
          td(class="review-edit", [edit]),
        ]),
      )
    }
  }
  section(class="section review-section", [
    h3(r.heading),
    if rows.is_empty() {
      text("")
    } else {
      table(class="review-docs", [tbody(rows)])
    },
    div(class="review-actions", [
      ext_link(report_error_url(data), r.report_error, class="button"),
      ext_link(contributing_url(), r.how_to_review, class="button"),
    ]),
  ])
}

///|
/// The evidence for `pointers` (the first that has any, else the
/// nearest covering provenance key) in `kind`'s provenance.
fn evidence_of(
  data : PartData,
  kind : String,
  pointers : Array[String],
) -> @api.ProvenanceEntry? {
  guard data.provenance is Some(pr) else { None }
  let map = match kind {
    "part" => pr.part
    "device" => pr.device
    _ => pr.package_
  }
  for p in pointers {
    if map.get(p) is Some(list) && list.get(0) is Some(e) {
      return Some(e)
    }
  }
  // A covering key: /pins for /pins/12/name.
  guard pointers.get(0) is Some(first) else { None }
  let mut p = first
  while p.rev_find("/") is Some(i) && i > 0 {
    p = p[:i].to_owned()
    if map.get(p) is Some(list) && list.get(0) is Some(e) {
      return Some(e)
    }
  }
  None
}

///|
fn source_of(data : PartData, id : String) -> @api.Source? {
  for ps in data.sources {
    if ps.source.id == id {
      return Some(ps.source)
    }
  }
  None
}

///|
/// `value` linked to the page its provenance cites: the datasheet
/// panel opens it (when this server serves documents), else the
/// original PDF opens at `#page=N`. Plain text without evidence.
fn cited_value(
  lang : Lang,
  data : PartData,
  kind : String,
  pointers : Array[String],
  label : String,
  value : String,
) -> Html {
  guard evidence_of(data, kind, pointers) is Some(e) &&
    source_of(data, e.source) is Some(s) else {
    text(value)
  }
  let href = match e.page {
    Some(n) => "\{s.url}#page=\{n}"
    None => s.url
  }
  let title = "\{s.publisher.to_upper()} \{s.document_id}"
  let mut attrs = @html.Attrs::build()
    .data_set("op-source", s.id)
    .data_set("op-url", s.url)
    .data_set("op-title", title)
    .data_set("op-revision", s.revision)
    .data_set("op-locator", e.locator.unwrap_or(""))
    .data_set("op-label", "\{label}: \{value}")
    .data_set("op-value", value)
  if e.page is Some(n) {
    attrs = attrs.data_set("op-page", n.to_string())
  }
  let tip = match (e.page, e.locator) {
    (Some(n), Some(l)) => "\{title} \{lang.data_review().page_short}\{n}: \{l}"
    (Some(n), None) => "\{title} \{lang.data_review().page_short}\{n}"
    (None, Some(l)) => "\{title}: \{l}"
    (None, None) => title
  }
  a(
    href~,
    target=Blank,
    rel="noopener noreferrer",
    class="fact-link",
    title=tip,
    attrs~,
    if value.is_empty() {
      "-"
    } else {
      value
    },
    escape=true,
  )
}

///|
/// Where a value was read: "p.4 · “Package outline, E1”".
fn cited_where(
  lang : Lang,
  data : PartData,
  kind : String,
  pointers : Array[String],
) -> Html {
  guard evidence_of(data, kind, pointers) is Some(e) else { text("") }
  let page = match e.page {
    Some(n) => "\{lang.data_review().page_short}\{n}"
    None => ""
  }
  let locator = match e.locator {
    Some(l) if l.has_prefix("\"") || l.has_prefix("“") => l
    Some(l) => "“\{l}”"
    None => ""
  }
  let sep = if page.is_empty() || locator.is_empty() { "" } else { " · " }
  span(class="text-muted cited-where", page + sep + locator)
}
