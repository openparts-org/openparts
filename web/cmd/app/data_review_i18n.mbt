// Strings of the part page's review status, "Edit on GitHub" and
// "Report an error" (data_review.mbt), the catalog's review facet, and
// the datasheet panel src/datasheet.js, in Japanese and English.

///|
struct DataReviewCopy {
  ai_candidate : String
  reviewed : String
  partly : String
  /// "{by}" and "{pr}" are replaced.
  reviewed_by : String
  not_reviewed : String
  candidate_note : String
  reviewed_note : String
  doc_part : String
  doc_device : String
  doc_package : String
  edit_on_github : String
  edit_note : String
  report_error : String
  how_to_review : String
  heading : String
  page_short : String
  reviewed_option : String
  partly_option : String
  unreviewed_option : String
  reviewed_mark : String
}

///|
let data_review_ja : DataReviewCopy = {
  ai_candidate: "AI候補 — 要レビュー",
  reviewed: "レビュー済み",
  partly: "一部レビュー済み",
  reviewed_by: "{by} が PR #{pr} でレビュー",
  not_reviewed: "AI候補(未レビュー)",
  candidate_note: "この部品のデータはAIエージェントがデータシートから抽出した候補で、まだ人が確認していません。各値のリンクから出典のページを開いて照合できます。",
  reviewed_note: "部品・デバイス・パッケージのすべてのデータを、人がデータシートと照合済みです。",
  doc_part: "部品",
  doc_device: "デバイス",
  doc_package: "パッケージ",
  edit_on_github: "GitHubで編集",
  edit_note: "値の誤りを見つけたら、GitHubで該当ファイルを編集してください。GitHubがフォークとPull Requestの作成を案内します。PRには変更した値と出典ページの一覧が自動でコメントされます。",
  report_error: "誤りを報告",
  how_to_review: "レビューの手順",
  heading: "データのレビュー",
  page_short: "p.",
  reviewed_option: "レビュー済み",
  partly_option: "一部レビュー済み",
  unreviewed_option: "AI候補(未レビュー)",
  reviewed_mark: "レビュー済み",
}

///|
let data_review_en : DataReviewCopy = {
  ai_candidate: "AI candidate — needs review",
  reviewed: "Reviewed",
  partly: "Partly reviewed",
  reviewed_by: "Reviewed by {by} in PR #{pr}",
  not_reviewed: "AI candidate (not reviewed)",
  candidate_note: "This part's data was extracted from its datasheets by AI agents and no person has checked it yet. Each value links to the page it was read from, so you can check it.",
  reviewed_note: "A person has checked all of this part's, device's and package's data against the datasheets.",
  doc_part: "Part",
  doc_device: "Device",
  doc_package: "Package",
  edit_on_github: "Edit on GitHub",
  edit_note: "Found a wrong value? Edit the file on GitHub: it guides you through forking and opening a pull request, which gets a comment listing every changed value with its source page.",
  report_error: "Report an error",
  how_to_review: "How to review",
  heading: "Data review",
  page_short: "p.",
  reviewed_option: "Reviewed",
  partly_option: "Partly reviewed",
  unreviewed_option: "AI candidate (unreviewed)",
  reviewed_mark: "Reviewed",
}

///|
fn Lang::data_review(self : Lang) -> DataReviewCopy {
  match self {
    Ja => data_review_ja
    En => data_review_en
  }
}

///|
/// The catalog's review facet option label.
fn review_option_label(lang : Lang, value : String) -> String {
  let r = lang.data_review()
  match value {
    "reviewed" => r.reviewed_option
    "partly" => r.partly_option
    "unreviewed" => r.unreviewed_option
    _ => value
  }
}

///|
/// The datasheet panel's strings (src/datasheet.js `setLabels`).
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
