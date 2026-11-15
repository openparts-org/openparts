// The review workflow's strings (review.mbt, the part page's review
// badges, and the datasheet viewer src/datasheet.js), in Japanese and
// English.

///|
struct ReviewCopy {
  nav_review : String
  // part page
  facts_heading : String
  facts_note : String
  review_this_part : String
  col_field : String
  col_value : String
  col_source : String
  col_status : String
  unreviewed : String
  confirmed : String
  disputed : String
  pending : String
  // review page
  title : String
  intro : String
  read_only : String
  reviewer_label : String
  token_label : String
  creds_note : String
  save : String
  change : String
  filter_category : String
  filter_manufacturer : String
  filter_status : String
  all : String
  any_category : String
  apply : String
  value : String
  where_ : String
  source : String
  confirm : String
  dispute : String
  skip : String
  back : String
  submit_dispute : String
  cancel : String
  comment_label : String
  corrected_label : String
  shortcuts : String
  queue_empty : String
  need_creds : String
  no_evidence : String
  previous_reviews : String
  group_part : String
  group_device : String
  group_package : String
}

///|
let review_ja : ReviewCopy = {
  nav_review: "レビュー",
  facts_heading: "記録された事実とレビュー",
  facts_note: "以下の値はそれぞれ出典(データシートのページと場所)付きで記録されており、多くはAIエージェントが抽出した候補です。人が出典と照合した値には「確認済み」が付きます。値をクリックすると、データシートの該当ページを開いて場所をハイライトします。",
  review_this_part: "この部品をレビューする",
  col_field: "項目",
  col_value: "値",
  col_source: "出典",
  col_status: "レビュー",
  unreviewed: "未レビュー",
  confirmed: "確認済み",
  disputed: "異議あり",
  pending: "反映待ち",
  title: "レビューキュー",
  intro: "記録された事実を1件ずつ、出典が示すデータシートの場所と照合します。データシートと一致すれば「確認」、違えば「異議」(データシートに何と書かれているか)を送ってください。レビューはこのサーバーに保存され、メンテナがopenparts-dataへコミットします。",
  read_only: "このサーバーはレビューを受け付けていません(レビュートークンが未設定)。キューの閲覧はできます。",
  reviewer_label: "レビュアー名",
  token_label: "レビュートークン",
  creds_note: "このブラウザ(localStorage)にのみ保存されます。",
  save: "保存",
  change: "変更",
  filter_category: "カテゴリ",
  filter_manufacturer: "メーカー",
  filter_status: "状態",
  all: "すべて",
  any_category: "すべてのカテゴリ",
  apply: "絞り込む",
  value: "値",
  where_: "場所",
  source: "出典",
  confirm: "確認",
  dispute: "異議",
  skip: "スキップ",
  back: "戻る",
  submit_dispute: "異議を送信",
  cancel: "キャンセル",
  comment_label: "データシートには何と書かれていますか?(必須)",
  corrected_label: "正しい値(任意)",
  shortcuts: "キー: C 確認 · D 異議 · S / → スキップ · ← 戻る",
  queue_empty: "この条件でレビュー待ちの事実はありません。",
  need_creds: "先にレビュアー名とレビュートークンを入力してください。",
  no_evidence: "出典の記録がありません。",
  previous_reviews: "これまでのレビュー",
  group_part: "部品",
  group_device: "デバイス",
  group_package: "パッケージ",
}

///|
let review_en : ReviewCopy = {
  nav_review: "Review",
  facts_heading: "Recorded facts and review",
  facts_note: "Every value below was recorded with its source (the datasheet page and place), most of them as candidates extracted by AI agents. A value a person has checked against its source is marked confirmed. Click a value to open the datasheet at that page with the place highlighted.",
  review_this_part: "Review this part",
  col_field: "Field",
  col_value: "Value",
  col_source: "Source",
  col_status: "Review",
  unreviewed: "Unreviewed",
  confirmed: "Confirmed",
  disputed: "Disputed",
  pending: "pending",
  title: "Review queue",
  intro: "Check one recorded fact at a time against the place in the datasheet it cites. Confirm it if the datasheet says the same; dispute it, saying what the datasheet says, if not. Reviews are stored on this server and committed to openparts-data by a maintainer.",
  read_only: "This server doesn't accept reviews (no review token is configured). You can still browse the queue.",
  reviewer_label: "Reviewer name",
  token_label: "Review token",
  creds_note: "Kept in this browser only (localStorage).",
  save: "Save",
  change: "Change",
  filter_category: "Category",
  filter_manufacturer: "Manufacturer",
  filter_status: "Status",
  all: "All",
  any_category: "Any category",
  apply: "Filter",
  value: "Value",
  where_: "Where",
  source: "Source",
  confirm: "Confirm",
  dispute: "Dispute",
  skip: "Skip",
  back: "Back",
  submit_dispute: "Submit dispute",
  cancel: "Cancel",
  comment_label: "What does the datasheet say? (required)",
  corrected_label: "Corrected value (optional)",
  shortcuts: "Keys: C confirm · D dispute · S / → skip · ← back",
  queue_empty: "Nothing left to review with these filters.",
  need_creds: "Enter your reviewer name and the review token first.",
  no_evidence: "No source recorded.",
  previous_reviews: "Reviews so far",
  group_part: "Part",
  group_device: "Device",
  group_package: "Package",
}

///|
fn Lang::review(self : Lang) -> ReviewCopy {
  match self {
    Ja => review_ja
    En => review_en
  }
}

///|
fn fact_status_label(lang : Lang, s : @api.FactReviewStatus) -> String {
  let r = lang.review()
  match s {
    Unreviewed => r.unreviewed
    Confirmed => r.confirmed
    Disputed => r.disputed
  }
}

///|
/// "12 of 40 facts confirmed by a person · 1 disputed".
fn review_summary(lang : Lang, r : @api.PartReviews) -> String {
  let mut s = match lang {
    Ja => "\{r.total}件中\{r.confirmed}件を人が確認済み"
    En => "\{r.confirmed} of \{r.total} facts confirmed by a person"
  }
  if r.disputed > 0 {
    s = s +
      (match lang {
        Ja => " · \{r.disputed}件に異議"
        En => " · \{r.disputed} disputed"
      })
  }
  if r.pending > 0 {
    s = s +
      (match lang {
        Ja => "(うち\{r.pending}件は反映待ち)"
        En => " (\{r.pending} awaiting commit)"
      })
  }
  s
}

///|
fn stale_label(lang : Lang, n : Int32) -> String {
  match lang {
    Ja => "古い値へのレビュー\{n}件"
    En =>
      if n == 1 {
        "1 review of an older value"
      } else {
        "\{n} reviews of older values"
      }
  }
}

///|
fn page_short(lang : Lang, page : Int32) -> String {
  match lang {
    Ja => "p.\{page}"
    En => "p. \{page}"
  }
}

///|
fn queue_remaining(lang : Lang, n : Int32) -> String {
  match lang {
    Ja => "残り\{n}件"
    En => if n == 1 { "1 fact left" } else { "\{n} facts left" }
  }
}

///|
fn shared_by(
  lang : Lang,
  kind : @api.ReviewTargetKind,
  target : String,
) -> String {
  match (lang, kind) {
    (_, Part) => ""
    (Ja, Device) =>
      "デバイス \{target} の事実(このデバイスを使う全部品で共有)"
    (En, Device) => "A fact of device \{target}, shared by every part using it"
    (Ja, Package) =>
      "パッケージ \{target} の事実(このパッケージを使う全部品で共有)"
    (En, Package) =>
      "A fact of package \{target}, shared by every part using it"
  }
}

///|
fn submitted_label(lang : Lang, s : @api.FactReviewStatus) -> String {
  let st = fact_status_label(lang, s)
  match lang {
    Ja => "記録しました: \{st}"
    En => "Recorded: \{st}"
  }
}

///|
fn reviewing_as(lang : Lang, name : String) -> String {
  match lang {
    Ja => "\{name} としてレビュー中"
    En => "Reviewing as \{name}"
  }
}

///|
/// A provenance key as a field name: `/parameters/capacitance` ->
/// "Capacitance", `/dimensions/body_width` -> "Body width", `/pins/3/name`
/// -> "Pin 3 name".
fn pointer_label(lang : Lang, pointer : String) -> String {
  let c = lang.copy()
  let segs = pointer.split("/").map(s => s.to_owned()).collect()
  let segs = segs.filter(s => !s.is_empty())
  let words = (s : String) => {
    let t = s.replace_all(old="_", new=" ")
    if t.is_empty() {
      t
    } else {
      t[:1].to_owned().to_upper() + t[1:].to_owned()
    }
  }
  match segs {
    ["parameters", key, ..] => param_name(lang, key)
    ["dimensions", "body_width"] => c.body_width
    ["dimensions", "body_length"] => c.body_length
    ["dimensions", "body_height"] => c.body_height
    ["dimensions", .. rest] => rest.map(words).join(" / ")
    ["pitch"] => c.pitch
    ["lead_count"] => c.lead_count
    ["family"] => c.family
    ["existence", ..] =>
      match lang {
        Ja => "存在確認"
        En => "Existence"
      }
    ["lifecycle", ..] =>
      match lang {
        Ja => "ライフサイクル"
        En => "Lifecycle"
      }
    ["package"] =>
      match lang {
        Ja => "パッケージ割り当て"
        En => "Package assignment"
      }
    ["pins"] =>
      match lang {
        Ja => "ピン配置(全体)"
        En => "Pinout (all pins)"
      }
    ["pins", n] =>
      match lang {
        Ja => "ピン \{n}"
        En => "Pin \{n}"
      }
    ["pins", n, "name"] =>
      match lang {
        Ja => "ピン \{n} の名前"
        En => "Pin \{n} name"
      }
    ["pins", n, "type"] =>
      match lang {
        Ja => "ピン \{n} の種類"
        En => "Pin \{n} type"
      }
    ["pins", n, field] => "\{c.pin_number} \{n} \{words(field)}"
    [] => pointer
    _ => segs.map(words).join(" / ")
  }
}

///|
/// The datasheet viewer's strings (src/datasheet.js `setLabels`).
fn datasheet_labels(lang : Lang) -> String {
  let j : Json = match lang {
    Ja =>
      {
        "close": "閉じる",
        "where": "場所",
        "page": "ページ",
        "revision": "リビジョン",
        "open_original": "元のPDFを開く({page}ページ)",
        "loading": "データシートを読み込み中…",
        "found": "{page}ページの「{fragment}」をハイライトしました。",
        "not_found": "{page}ページに場所の記述が見つかりませんでした。出典のページを表示しています。",
        "no_locator": "場所の記録がありません。出典のページを表示しています。",
        "no_page": "ページの記録がなく、文書内にも場所の記述が見つかりませんでした。",
        "searching": "ページの記録がないため、文書内を検索しています({page} / {pages}ページ)…",
        "found_search": "ページの記録はありませんが、{page}ページに「{fragment}」が見つかりました。",
        "failed": "このサイト上では文書を表示できませんでした: {error}",
        "prev": "前のページ",
        "next": "次のページ",
        "zoom_in": "拡大",
        "zoom_out": "縮小",
        "panel": "データシート",
      }
    En =>
      {
        "close": "Close",
        "where": "Where",
        "page": "Page",
        "revision": "Revision",
        "open_original": "Open the original PDF (page {page})",
        "loading": "Loading the datasheet…",
        "found": "Highlighted “{fragment}” on page {page}.",
        "not_found": "The locator text was not found on page {page}; the cited page is shown.",
        "no_locator": "No locator recorded; the cited page is shown.",
        "no_page": "No page recorded, and the locator text was not found in the document.",
        "searching": "No page recorded: searching the document for the locator (page {page} of {pages})…",
        "found_search": "No page recorded: found “{fragment}” on page {page}.",
        "failed": "The document could not be shown here: {error}",
        "prev": "Previous page",
        "next": "Next page",
        "zoom_in": "Zoom in",
        "zoom_out": "Zoom out",
        "panel": "Datasheet",
      }
  }
  j.stringify()
}
