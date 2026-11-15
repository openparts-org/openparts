// Human review of recorded facts: the /review queue (one fact at a
// time beside its datasheet page), and the part page's review badges
// and fact table. The server keeps submissions (POST /v1/reviews, with
// the reviewer's token); a maintainer commits them to openparts-data.

///|
const DATASHEET_ID : String = "review-datasheet"

///|
const COMMENT_ID : String = "review-comment"

///|
/// The queue's filters, in the /review URL.
#derive(Eq)
struct ReviewQuery {
  category : String
  manufacturer : String
  mpn : String
  /// unreviewed (default), disputed, confirmed, all
  status : String
}

///|
fn review_query_of(params : Map[String, String]) -> ReviewQuery {
  {
    category: params.get("category").unwrap_or(""),
    manufacturer: params.get("manufacturer").unwrap_or(""),
    mpn: params.get("mpn").unwrap_or(""),
    status: params.get("status").unwrap_or(""),
  }
}

///|
fn ReviewQuery::params(self : ReviewQuery) -> Array[String] {
  let out = []
  for
    kv in [
      ("category", self.category),
      ("manufacturer", self.manufacturer),
      ("mpn", self.mpn),
      ("status", self.status),
    ] {
    if !kv.1.is_empty() {
      out.push("\{kv.0}=\{encode_uri_component(kv.1)}")
    }
  }
  out
}

///|
fn review_path(q : ReviewQuery) -> String {
  let params = q.params()
  if params.is_empty() {
    "review"
  } else {
    "review?" + params.join("&")
  }
}

///|
#derive(Eq)
struct ReviewState {
  query : ReviewQuery
  /// Bumped per queue request.
  seq : Int32
  config : Fetch[@api.ReviewConfig]
  queue : Fetch[@api.ReviewQueue]
  /// The batch being worked through, and the fact shown.
  items : Array[@api.QueueItem]
  index : Int32
  /// Facts passed that stay in the queue (skipped): the next batch's
  /// offset.
  passed : Int32
  reviewer : String
  token : String
  name_input : String
  token_input : String
  editing : Bool
  disputing : Bool
  comment : String
  corrected : String
  submitting : Bool
  notice : String
  error : String
  /// The filter form, ahead of `query`.
  form : ReviewQuery
}

///|
fn ReviewState::new() -> ReviewState {
  let empty = { category: "", manufacturer: "", mpn: "", status: "", }
  {
    query: empty,
    seq: 0,
    config: Loading,
    queue: Loading,
    items: [],
    index: 0,
    passed: 0,
    reviewer: "",
    token: "",
    name_input: "",
    token_input: "",
    editing: false,
    disputing: false,
    comment: "",
    corrected: "",
    submitting: false,
    notice: "",
    error: "",
    form: empty,
  }
}

///|
enum ReviewMsg {
  RevGotConfig(Fetch[@api.ReviewConfig])
  RevGotQueue(Int32, Fetch[@api.ReviewQueue])
  RevNameInput(String)
  RevTokenInput(String)
  RevSaveCreds
  RevEditCreds
  RevConfirm
  RevDisputeOpen
  RevDisputeCancel
  RevComment(String)
  RevCorrected(String)
  RevSubmitDispute
  RevSubmitted(Result[@api.SubmitResponse, String])
  RevSkip
  RevBack
  RevKey(String)
  RevForm(String, String)
  RevApply
}

///|
fn ReviewState::current(self : ReviewState) -> @api.QueueItem? {
  self.items.get(self.index)
}

///|
fn review_fetch(s : ReviewState, emit : Emit[Msg]) -> @rabbita.Cmd {
  let seq = s.seq
  let params = s.query.params()
  params.push("offset=\{s.passed}")
  params.push("limit=20")
  let path = "/v1/reviews/queue?" + params.join("&")
  @rabbita.perform(r => emit(Review(RevGotQueue(seq, r))), async fn() noraise {
    get_json(path)
  })
}

///|
/// Shows the current fact's first source in the inline datasheet view.
fn show_datasheet_cmd(lang : Lang, s : ReviewState) -> @rabbita.Cmd {
  guard s.current() is Some(item) else { @rabbita.none }
  guard item.fact.evidence.get(0) is Some(e) else { @rabbita.none }
  let options = datasheet_options(lang, item.fact, e)
  @rabbita.effect(fn() { datasheet_inline(DATASHEET_ID, options) })
}

///|
/// The datasheet viewer's options for one evidence entry of `fact`.
fn datasheet_options(
  lang : Lang,
  fact : @api.FactView,
  e : @api.FactEvidence,
) -> String {
  let j : Json = {
    "source": Json::string(e.source),
    "page": match e.page {
      Some(n) => Json::number(n.to_double())
      None => Json::null()
    },
    "locator": Json::string(e.locator.unwrap_or("")),
    "url": Json::string(e.url),
    "title": Json::string(e.title),
    "revision": Json::string(e.revision),
    "date": Json::string(e.date),
    "label": Json::string("\{pointer_label(lang, fact.pointer)}: \{fact.value}"),
    "value": Json::string(fact.value),
  }
  j.stringify()
}

///|
fn review_enter(
  model : Model,
  q : ReviewQuery,
  emit : Emit[Msg],
) -> (Model, @rabbita.Cmd) {
  let s = model.review
  datasheet_close()
  let mut s = s
  if s.seq == 0 {
    // First visit: the saved reviewer and token.
    let reviewer = local_get("openparts.reviewer")
    let token = local_get("openparts.review_token")
    s = {
      ..s,
      reviewer,
      token,
      name_input: reviewer,
      token_input: token,
      editing: reviewer.is_empty() || token.is_empty(),
    }
  }
  let config_cmd = if s.config is Loading || s.config is Failed(_) {
    @rabbita.perform(r => emit(Review(RevGotConfig(r))), async fn() noraise {
      get_json("/v1/reviews/config")
    })
  } else {
    @rabbita.none
  }
  if s.seq != 0 && q == s.query && s.queue is Got(_) {
    // Back on the same queue: the datasheet view has to be drawn again.
    return (
      { ..model, review: s, },
      @rabbita.batch([config_cmd, show_datasheet_cmd(model.lang, s)]),
    )
  }
  let s = {
    ..s,
    query: q,
    form: q,
    seq: s.seq + 1,
    queue: Loading,
    items: [],
    index: 0,
    passed: 0,
    disputing: false,
    notice: "",
    error: "",
  }
  ({ ..model, review: s, }, @rabbita.batch([config_cmd, review_fetch(s, emit)]))
}

///|
fn submit_cmd(
  s : ReviewState,
  item : @api.QueueItem,
  status : String,
  emit : Emit[Msg],
) -> @rabbita.Cmd {
  let f = item.fact
  let body : Json = {
    "target_kind": Json::string(f.target_kind.wire()),
    "target": Json::string(f.target),
    "pointer": Json::string(f.pointer),
    "status": Json::string(status),
    "reviewer": Json::string(s.reviewer),
    "value_hash": Json::string(f.value_hash),
    "comment": if s.comment.is_empty() {
      Json::null()
    } else {
      Json::string(s.comment)
    },
    "corrected_value": if s.corrected.is_empty() {
      Json::null()
    } else {
      Json::string(s.corrected)
    },
  }
  let token = s.token
  let text = body.stringify()
  @rabbita.perform(r => emit(Review(RevSubmitted(r))), async fn() noraise {
    try {
      let r = post_json(api_base() + "/v1/reviews", token, text).wait()
      if r.status() == 201 {
        let reply : @api.SubmitResponse = @json.from_json(
          @json.parse(r.body_text()),
        )
        Ok(reply)
      } else {
        Err(error_message(r))
      }
    } catch {
      e => Err(e.to_string())
    }
  })
}

///|
/// Moves to the next fact (fetching the next batch at the end).
fn advance(
  model : Model,
  s : ReviewState,
  emit : Emit[Msg],
) -> (Model, @rabbita.Cmd) {
  let s = {
    ..s,
    index: s.index + 1,
    disputing: false,
    comment: "",
    corrected: "",
  }
  if s.index >= s.items.length() {
    let s = { ..s, seq: s.seq + 1, queue: Loading, items: [], index: 0, }
    return ({ ..model, review: s, }, review_fetch(s, emit))
  }
  ({ ..model, review: s, }, show_datasheet_cmd(model.lang, s))
}

///|
fn review_update(
  model : Model,
  msg : ReviewMsg,
  emit : Emit[Msg],
) -> (Model, @rabbita.Cmd) {
  let s = model.review
  let lang = model.lang
  let set = (s : ReviewState) => ({ ..model, review: s, }, @rabbita.none)
  let can_submit = s.config is Got(c) &&
    c.writes_enabled &&
    !s.reviewer.is_empty() &&
    !s.token.is_empty() &&
    !s.submitting
  match msg {
    RevGotConfig(c) => set({ ..s, config: c, })
    RevGotQueue(seq, r) => {
      guard seq == s.seq else { (model, @rabbita.none) }
      let items = match r {
        Got(q) => q.items
        _ => []
      }
      let s = { ..s, queue: r, items, index: 0, }
      ({ ..model, review: s, }, show_datasheet_cmd(lang, s))
    }
    RevNameInput(v) => set({ ..s, name_input: v, })
    RevTokenInput(v) => set({ ..s, token_input: v, })
    RevSaveCreds => {
      let reviewer = s.name_input.trim().to_owned()
      let token = s.token_input.trim().to_owned()
      local_set("openparts.reviewer", reviewer)
      local_set("openparts.review_token", token)
      set({
        ..s,
        reviewer,
        token,
        editing: reviewer.is_empty() || token.is_empty(),
        error: "",
      })
    }
    RevEditCreds => set({ ..s, editing: true, })
    RevConfirm => {
      guard s.current() is Some(item) else { (model, @rabbita.none) }
      guard can_submit else {
        set({
          ..s,
          error: lang.review().need_creds,
          editing: s.reviewer.is_empty() || s.token.is_empty(),
        })
      }
      (
        {
          ..model,
          review: {
            ..s,
            submitting: true,
            comment: "",
            corrected: "",
            error: "",
          },
        },
        submit_cmd(
          { ..s, comment: "", corrected: "", },
          item,
          "confirmed",
          emit,
        ),
      )
    }
    RevDisputeOpen => {
      guard s.current() is Some(_) else { (model, @rabbita.none) }
      (
        { ..model, review: { ..s, disputing: true, }, },
        @rabbita.effect(fn() { focus_element(COMMENT_ID) }),
      )
    }
    RevDisputeCancel => set({ ..s, disputing: false, })
    RevComment(v) => set({ ..s, comment: v, })
    RevCorrected(v) => set({ ..s, corrected: v, })
    RevSubmitDispute => {
      guard s.current() is Some(item) else { (model, @rabbita.none) }
      guard can_submit else {
        set({
          ..s,
          error: lang.review().need_creds,
          editing: s.reviewer.is_empty() || s.token.is_empty(),
        })
      }
      (
        { ..model, review: { ..s, submitting: true, error: "", }, },
        submit_cmd(s, item, "disputed", emit),
      )
    }
    RevSubmitted(Ok(reply)) => {
      let status = reply.fact.status
      let stays = match (s.query.status, status) {
        ("all", _) => true
        ("confirmed", Confirmed) => true
        ("disputed", Disputed) => true
        _ => false
      }
      let passed = if stays { s.passed + 1 } else { s.passed }
      let notice = "\{submitted_label(lang, status)} — \{pointer_label(lang, reply.fact.pointer)} (\{reply.fact.target})"
      advance(
        model,
        { ..s, submitting: false, passed, notice, error: "", },
        emit,
      )
    }
    RevSubmitted(Err(e)) => set({ ..s, submitting: false, error: e, })
    RevSkip => {
      guard s.current() is Some(_) else { (model, @rabbita.none) }
      advance(
        model,
        { ..s, passed: s.passed + 1, notice: "", error: "", },
        emit,
      )
    }
    RevBack => {
      guard s.index > 0 else { (model, @rabbita.none) }
      let s = {
        ..s,
        index: s.index - 1,
        passed: s.passed - 1,
        disputing: false,
        notice: "",
        error: "",
      }
      ({ ..model, review: s, }, show_datasheet_cmd(lang, s))
    }
    RevKey(key) => {
      if key == "Escape" {
        return set({ ..s, disputing: false, })
      }
      guard !typing_in_field() && !s.disputing else { (model, @rabbita.none) }
      match key {
        "c" | "C" => review_update(model, RevConfirm, emit)
        "d" | "D" => review_update(model, RevDisputeOpen, emit)
        "s" | "S" | "ArrowRight" => review_update(model, RevSkip, emit)
        "ArrowLeft" => review_update(model, RevBack, emit)
        _ => (model, @rabbita.none)
      }
    }
    RevForm(key, v) => {
      let f = s.form
      let form = match key {
        "category" => { ..f, category: v, }
        "manufacturer" => { ..f, manufacturer: v.trim().to_owned(), mpn: "", }
        "status" => { ..f, status: v, }
        _ => f
      }
      set({ ..s, form, })
    }
    RevApply => (model, @nav.push_url(href(lang, review_path(s.form))))
  }
}

// ---- views ----

///|
fn status_class(s : @api.FactReviewStatus) -> String {
  match s {
    Unreviewed => "rv-badge rv-unreviewed"
    Confirmed => "rv-badge rv-confirmed"
    Disputed => "rv-badge rv-disputed"
  }
}

///|
/// Who reviewed, for a badge's tooltip.
fn reviewers_title(lang : Lang, f : @api.FactView) -> String {
  let lines = f.reviews.map(r => {
    let mut l = "\{r.reviewer} · \{r.date} · \{r.status.wire()}"
    if r.pending {
      l = l + " (" + lang.review().pending + ")"
    }
    if r.comment is Some(c) {
      l = l + ": " + c
    }
    l
  })
  if f.stale > 0 {
    lines.push(stale_label(lang, f.stale))
  }
  lines.join("\n")
}

///|
/// A fact's review badge.
fn fact_badge(lang : Lang, f : @api.FactView) -> Html {
  let r = lang.review()
  let mut label = fact_status_label(lang, f.status)
  if f.pending {
    label = label + " · " + r.pending
  }
  span(class=status_class(f.status), title=reviewers_title(lang, f), label)
}

///|
/// The small dot badge beside an existing table value.
fn fact_dot(lang : Lang, f : @api.FactView?) -> Html {
  guard f is Some(f) else { text("") }
  let cls = status_class(f.status) + " rv-dot"
  let title = fact_status_label(lang, f.status) +
    (if f.reviews.is_empty() { "" } else { "\n" + reviewers_title(lang, f) })
  span(class=cls, title~, "")
}

///|
/// `value` as a link opening the fact's (first) source at the cited
/// page in the datasheet viewer; plain text without a source.
fn fact_value(lang : Lang, f : @api.FactView, value : String) -> Html {
  guard f.evidence.get(0) is Some(e) else { text(value) }
  let href = if e.page is Some(n) { "\{e.url}#page=\{n}" } else { e.url }
  let mut attrs = @html.Attrs::build()
    .data_set("op-source", e.source)
    .data_set("op-url", e.url)
    .data_set("op-title", e.title)
    .data_set("op-revision", e.revision)
    .data_set("op-locator", e.locator.unwrap_or(""))
    .data_set("op-label", "\{pointer_label(lang, f.pointer)}: \{f.value}")
    .data_set("op-value", f.value)
  if e.page is Some(n) {
    attrs = attrs.data_set("op-page", n.to_string())
  }
  a(
    href~,
    target=Blank,
    rel="noopener noreferrer",
    class="fact-link",
    title=e.locator.unwrap_or(e.title),
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
fn find_fact(
  reviews : @api.PartReviews?,
  kind : @api.ReviewTargetKind,
  pointers : Array[String],
) -> @api.FactView? {
  guard reviews is Some(r) else { None }
  for p in pointers {
    for f in r.facts {
      if f.target_kind == kind && f.pointer == p {
        return Some(f)
      }
    }
  }
  None
}

///|
/// A table value with its review dot, a link to its source when it has
/// one.
fn reviewed_cell(
  lang : Lang,
  reviews : @api.PartReviews?,
  kind : @api.ReviewTargetKind,
  pointers : Array[String],
  value : String,
) -> Array[Html] {
  match find_fact(reviews, kind, pointers) {
    Some(f) => [fact_value(lang, f, value), fact_dot(lang, Some(f))]
    None => [text(value)]
  }
}

///|
/// The part page's summary line under the badges.
fn review_summary_view(lang : Lang, data : PartData) -> Html {
  guard data.reviews is Some(r) else { text("") }
  let q = {
    category: "",
    manufacturer: data.part.manufacturer,
    mpn: data.part.mpn,
    status: "",
  }
  let cls = if r.disputed > 0 {
    "review-summary review-summary-disputed"
  } else if r.total > 0 && r.confirmed == r.total {
    "review-summary review-summary-done"
  } else {
    "review-summary"
  }
  div(class=cls, [
    span(class="review-summary-count", review_summary(lang, r)),
    a(href=href(lang, review_path(q)), lang.review().review_this_part),
  ])
}

///|
/// Every fact of the part, its device and its package, with its source
/// and review status.
fn facts_section(lang : Lang, data : PartData) -> Html {
  guard data.reviews is Some(r) else { text("") }
  let rc = lang.review()
  let groups : Array[Html] = []
  for kind in [@api.ReviewTargetKind::Part, Device, Package] {
    let facts = r.facts.filter(f => f.target_kind == kind)
    if facts.is_empty() {
      continue
    }
    let target = facts[0].target
    let rows = facts.map(f => {
      let src = match f.evidence.get(0) {
        Some(e) => {
          let page = match e.page {
            Some(n) => " " + page_short(lang, n)
            None => ""
          }
          "\{e.title}\{page}"
        }
        None => rc.no_evidence
      }
      tr([
        td(pointer_label(lang, f.pointer)),
        td(class="fact-value", [fact_value(lang, f, f.value)]),
        td(class="text-muted", src),
        td([fact_badge(lang, f)]),
      ])
    })
    groups.push(
      div(class="facts-group", [
        @html.h4([
          text(
            (match kind {
              Part => rc.group_part
              Device => rc.group_device
              Package => rc.group_package
            }) +
            " ",
          ),
          span(class="text-muted", target),
        ]),
        table(class="facts-table", [
          thead([
            tr([
              th(rc.col_field),
              th(rc.col_value),
              th(rc.col_source),
              th(rc.col_status),
            ]),
          ]),
          tbody(rows),
        ]),
      ]),
    )
  }
  section(
    class="section",
    [
      h3(rc.facts_heading),
      review_summary_view(lang, data),
      p(class="text-muted", rc.facts_note),
      ..groups,
    ],
  )
}

///|
fn review_view(model : Model, emit : Emit[Msg]) -> Html {
  let lang = model.lang
  let rc = lang.review()
  let s = model.review
  let writes = s.config is Got(c) && c.writes_enabled
  let parts : Array[Html] = [
    h2(class="review-title", rc.title),
    p(class="text-muted", rc.intro),
  ]
  if s.config is Got(c) && !c.writes_enabled {
    parts.push(p(class="review-readonly", rc.read_only))
  }
  if writes {
    parts.push(creds_view(lang, s, emit))
  }
  parts.push(filters_view(lang, s, emit))
  let body = match s.queue {
    Loading => p(class="text-muted", lang.copy().loading)
    Missing => p(class="text-muted", lang.copy().load_failed)
    Failed(e) =>
      p(
        class="text-muted",
        if e.is_empty() {
          lang.copy().load_failed
        } else {
          e
        },
      )
    Got(q) =>
      match s.current() {
        None => p(class="review-empty", rc.queue_empty)
        Some(item) => fact_view(lang, s, q, item, writes, emit)
      }
  }
  parts.push(body)
  div(class="review-page", parts)
}

///|
fn creds_view(lang : Lang, s : ReviewState, emit : Emit[Msg]) -> Html {
  let rc = lang.review()
  if !s.editing {
    return div(class="review-creds", [
      span(reviewing_as(lang, s.reviewer)),
      button(
        type_="button",
        class="button button-small",
        on_click=emit(Review(RevEditCreds)),
        rc.change,
      ),
    ])
  }
  div(class="review-creds review-creds-form", [
    @html.label([
      span(rc.reviewer_label),
      input(
        value=s.name_input,
        maxlength=64,
        auto_complete="nickname",
        on_input=emit.map(v => Review(RevNameInput(v))),
      ),
    ]),
    @html.label([
      span(rc.token_label),
      input(
        input_type=Password,
        value=s.token_input,
        auto_complete="off",
        on_input=emit.map(v => Review(RevTokenInput(v))),
      ),
    ]),
    button(
      type_="button",
      class="button button-primary",
      on_click=emit(Review(RevSaveCreds)),
      rc.save,
    ),
    span(class="text-muted review-creds-note", rc.creds_note),
  ])
}

///|
fn filters_view(lang : Lang, s : ReviewState, emit : Emit[Msg]) -> Html {
  let rc = lang.review()
  let f = s.form
  let categories = [
    @html.option(value="", selected=f.category.is_empty(), rc.any_category),
  ]
  for c in @api.ComponentCategory::all() {
    categories.push(
      @html.option(
        value=c.wire(),
        selected=f.category == c.wire(),
        category_label(lang, c),
      ),
    )
  }
  let statuses = [
    ("", rc.unreviewed),
    ("disputed", rc.disputed),
    ("confirmed", rc.confirmed),
    ("all", rc.all),
  ].map(st => @html.option(value=st.0, selected=f.status == st.0, st.1))
  let mut maker = f.manufacturer
  if !f.mpn.is_empty() {
    maker = maker + " / " + f.mpn
  }
  div(class="review-filters", [
    @html.label([
      span(rc.filter_category),
      @html.select(
        on_change=emit.map(v => Review(RevForm("category", v))),
        categories,
      ),
    ]),
    @html.label([
      span(rc.filter_manufacturer),
      input(
        value=maker,
        placeholder="raspberrypi",
        on_input=emit.map(v => Review(RevForm("manufacturer", v))),
      ),
    ]),
    @html.label([
      span(rc.filter_status),
      @html.select(
        on_change=emit.map(v => Review(RevForm("status", v))),
        statuses,
      ),
    ]),
    button(
      type_="button",
      class="button",
      on_click=emit(Review(RevApply)),
      rc.apply,
    ),
  ])
}

///|
fn fact_view(
  lang : Lang,
  s : ReviewState,
  q : @api.ReviewQueue,
  item : @api.QueueItem,
  writes : Bool,
  emit : Emit[Msg],
) -> Html {
  let rc = lang.review()
  let f = item.fact
  let left_count = q.total - s.passed - s.index
  let context : Array[Html] = [
    span(class="text-muted", queue_remaining(lang, left_count)),
    text(" · "),
    a(
      href=href(lang, part_path(item.manufacturer, item.mpn)),
      "\{item.manufacturer}/\{item.mpn}",
    ),
  ]
  if item.category is Some(c) {
    context.push(text(" · " + category_label(lang, c)))
  }
  let shared = shared_by(lang, f.target_kind, f.target)
  let evidence = if f.evidence.is_empty() {
    [p(class="text-muted", rc.no_evidence)]
  } else {
    f.evidence.map(e => {
      let page = match e.page {
        Some(n) => " · " + page_short(lang, n)
        None => ""
      }
      div(class="review-evidence", [
        div([
          span(class="review-key", rc.source),
          text(" "),
          strong(e.title),
          span(
            class="text-muted",
            " " + source_revision(lang, e.revision, e.date) + page,
          ),
        ]),
        div([
          span(class="review-key", rc.where_),
          text(" "),
          span(class="review-locator", e.locator.unwrap_or("-")),
        ]),
      ])
    })
  }
  let actions : Array[Html] = []
  if writes {
    actions.push(
      button(
        type_="button",
        class="button button-confirm",
        disabled=s.submitting,
        on_click=emit(Review(RevConfirm)),
        [text(rc.confirm), span(class="kbd", "C")],
      ),
    )
    actions.push(
      button(
        type_="button",
        class="button button-dispute",
        disabled=s.submitting,
        on_click=emit(Review(RevDisputeOpen)),
        [text(rc.dispute), span(class="kbd", "D")],
      ),
    )
  }
  actions.push(
    button(type_="button", class="button", on_click=emit(Review(RevSkip)), [
      text(rc.skip),
      span(class="kbd", "S"),
    ]),
  )
  actions.push(
    button(
      type_="button",
      class="button",
      disabled=s.index == 0,
      on_click=emit(Review(RevBack)),
      [text(rc.back), span(class="kbd", "←")],
    ),
  )
  let panel : Array[Html] = [
    div(class="review-context", context),
    h3(class="review-field", pointer_label(lang, f.pointer)),
  ]
  if !shared.is_empty() {
    panel.push(p(class="text-muted review-shared", shared))
  }
  panel.push(
    div(class="review-value", [
      span(class="review-key", rc.value),
      strong(if f.value.is_empty() { "-" } else { f.value }),
      fact_badge(lang, f),
    ]),
  )
  panel.push(div(evidence))
  if !f.reviews.is_empty() || f.stale > 0 {
    let items = f.reviews.map(r => {
      let mut l = "\{r.reviewer} · \{r.date} · \{fact_status_label(lang, if r.status == Confirmed { Confirmed } else { Disputed })}"
      if r.pending {
        l = l + " (" + rc.pending + ")"
      }
      if r.comment is Some(c) {
        l = l + " — " + c
      }
      if r.corrected_value is Some(v) {
        l = l + " → " + v
      }
      li(l)
    })
    if f.stale > 0 {
      items.push(li(class="text-muted", stale_label(lang, f.stale)))
    }
    panel.push(
      div(class="review-history", [@html.h4(rc.previous_reviews), ul(items)]),
    )
  }
  panel.push(div(class="review-actions", actions))
  if s.disputing {
    panel.push(
      div(class="review-dispute", [
        @html.label([
          span(rc.comment_label),
          @html.textarea(
            id=COMMENT_ID,
            rows=3,
            value=s.comment,
            on_input=emit.map(v => Review(RevComment(v))),
            "",
          ),
        ]),
        @html.label([
          span(rc.corrected_label),
          input(
            value=s.corrected,
            maxlength=500,
            on_input=emit.map(v => Review(RevCorrected(v))),
          ),
        ]),
        div(class="review-actions", [
          button(
            type_="button",
            class="button button-dispute",
            disabled=s.submitting || s.comment.trim().is_empty(),
            on_click=emit(Review(RevSubmitDispute)),
            rc.submit_dispute,
          ),
          button(
            type_="button",
            class="button",
            on_click=emit(Review(RevDisputeCancel)),
            rc.cancel,
          ),
        ]),
      ]),
    )
  }
  if !s.error.is_empty() {
    panel.push(p(class="review-error", s.error))
  }
  if !s.notice.is_empty() {
    panel.push(p(class="review-notice", s.notice))
  }
  panel.push(p(class="text-muted review-keys", rc.shortcuts))
  div(class="review-grid", [
    div(class="review-panel", panel),
    div(id=DATASHEET_ID, class="review-datasheet", nodes([])),
  ])
}
