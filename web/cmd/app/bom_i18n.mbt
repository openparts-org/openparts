// Strings of the BOM check page (bom.mbt) and the part page's
// alternatives section (alternatives.mbt), in Japanese and English.

///|
struct BomCopy {
  nav : String
  title : String
  drop_here : String
  choose_file : String
  formats : String
  paste_label : String
  paste_placeholder : String
  check : String
  checking : String
  clear : String
  sample : String
  too_large : String
  failed : String
  export_csv : String
  all : String
  summary_lines : String
  summary_components : String
  // columns
  c_refs : String
  c_qty : String
  c_bom : String
  c_match : String
  c_how : String
  c_issues : String
  // details
  details : String
  hide : String
  candidates : String
  coverage : String
  footprint_check : String
  bom_footprint : String
  part_package : String
  no_issues : String
  alternatives : String
  alternatives_none : String
  confidence : String
  detected_columns : String
  file_label : String
  // alternatives section (part page)
  alt_title : String
  alt_loading : String
  alt_none : String
  alt_replacement : String
  alt_parameters : String
  alt_pins : String
  alt_key : String
  alt_rule : String
  alt_original : String
  alt_candidate : String
  alt_result : String
  alt_pad : String
  alt_status : String
  alt_footprint_same : String
  alt_footprint_diff : String
  alt_pins_ok : String
  alt_pins_bad : String
  alt_compare : String
  alt_package : String
}

///|
let bom_ja : BomCopy = {
  nav: "BOMチェック",
  title: "BOMチェック",
  drop_here: "BOMファイルをここにドロップ",
  choose_file: "ファイルを選択",
  formats: "KiCad BOM CSV · ネットリスト (.net) · MPN列のあるCSV(2 MB・5000行まで)",
  paste_label: "またはBOMを貼り付け",
  paste_placeholder: "Reference,Value,Footprint,MPN\nC1,100nF,Capacitor_SMD:C_0603_1608Metric,\nU1,RP2040,,RP2040",
  check: "チェック",
  checking: "チェック中…",
  clear: "クリア",
  sample: "Picoボードの例で試す",
  too_large: "ファイルが大きすぎます(最大2 MB)",
  failed: "BOMを読めませんでした",
  export_csv: "注釈付きBOMをCSVで保存",
  all: "すべて",
  summary_lines: "行",
  summary_components: "部品",
  c_refs: "リファレンス",
  c_qty: "数量",
  c_bom: "BOMの記載",
  c_match: "照合した部品",
  c_how: "照合方法",
  c_issues: "確認事項",
  details: "詳細",
  hide: "閉じる",
  candidates: "候補",
  coverage: "データ充足度",
  footprint_check: "フットプリント",
  bom_footprint: "BOM",
  part_package: "部品",
  no_issues: "確認事項はありません",
  alternatives: "代替候補",
  alternatives_none: "代替候補は見つかりませんでした",
  confidence: "確度",
  detected_columns: "検出した列",
  file_label: "ファイル",
  alt_title: "代替部品",
  alt_loading: "代替部品を検索中…",
  alt_none: "代替候補はありません",
  alt_replacement: "データに記載の後継品",
  alt_parameters: "電気的特性の比較",
  alt_pins: "ピン配置の比較(パッドごと)",
  alt_key: "項目",
  alt_rule: "基準",
  alt_original: "元の部品",
  alt_candidate: "候補",
  alt_result: "結果",
  alt_pad: "パッド",
  alt_status: "判定",
  alt_footprint_same: "同じランドパターン",
  alt_footprint_diff: "別パッケージ",
  alt_pins_ok: "ピン互換",
  alt_pins_bad: "ピン配置が異なる",
  alt_compare: "比較を表示",
  alt_package: "パッケージ",
}

///|
let bom_en : BomCopy = {
  nav: "BOM check",
  title: "BOM check",
  drop_here: "Drop a BOM file here",
  choose_file: "Choose a file",
  formats: "KiCad BOM CSV · netlist (.net) · CSV with an MPN column (up to 2 MB, 5000 rows)",
  paste_label: "Or paste a BOM",
  paste_placeholder: "Reference,Value,Footprint,MPN\nC1,100nF,Capacitor_SMD:C_0603_1608Metric,\nU1,RP2040,,RP2040",
  check: "Check",
  checking: "Checking…",
  clear: "Clear",
  sample: "Try the Pico board example",
  too_large: "The file is too large (2 MB at most)",
  failed: "The BOM could not be read",
  export_csv: "Save the annotated BOM (CSV)",
  all: "All",
  summary_lines: "lines",
  summary_components: "components",
  c_refs: "References",
  c_qty: "Qty",
  c_bom: "BOM entry",
  c_match: "Matched part",
  c_how: "Match",
  c_issues: "Notes",
  details: "Details",
  hide: "Hide",
  candidates: "Candidates",
  coverage: "Data coverage",
  footprint_check: "Footprint",
  bom_footprint: "BOM",
  part_package: "Part",
  no_issues: "Nothing to note",
  alternatives: "Alternatives",
  alternatives_none: "No alternatives found",
  confidence: "Confidence",
  detected_columns: "Columns found",
  file_label: "File",
  alt_title: "Alternatives",
  alt_loading: "Looking for alternatives…",
  alt_none: "No alternatives",
  alt_replacement: "Replacement named in the data",
  alt_parameters: "Parameters",
  alt_pins: "Pinout, pad by pad",
  alt_key: "Parameter",
  alt_rule: "Rule",
  alt_original: "Original",
  alt_candidate: "Candidate",
  alt_result: "Result",
  alt_pad: "Pad",
  alt_status: "Result",
  alt_footprint_same: "Same land pattern",
  alt_footprint_diff: "Other package",
  alt_pins_ok: "Pin-compatible",
  alt_pins_bad: "Pinout differs",
  alt_compare: "Show comparison",
  alt_package: "Package",
}

///|
fn Lang::bom(self : Lang) -> BomCopy {
  match self {
    Ja => bom_ja
    En => bom_en
  }
}

///|
fn line_status_label(lang : Lang, s : @api.LineStatus) -> String {
  match (lang, s) {
    (Ja, @api.LineStatus::Ok) => "OK"
    (Ja, Review) => "要確認"
    (Ja, Warning) => "警告"
    (Ja, NotFound) => "見つからない"
    (En, @api.LineStatus::Ok) => "OK"
    (En, Review) => "Review"
    (En, Warning) => "Warning"
    (En, NotFound) => "Not found"
  }
}

///|
fn match_kind_label(lang : Lang, k : @api.MatchKind) -> String {
  match (lang, k) {
    (Ja, Exact) => "MPN完全一致"
    (Ja, Normalized) => "MPN正規化一致"
    (Ja, Variant) => "MPNの派生型番"
    (Ja, ValueFootprint) => "値+フットプリント"
    (Ja, @api.MatchKind::None) => "—"
    (En, Exact) => "Exact MPN"
    (En, Normalized) => "Normalized MPN"
    (En, Variant) => "MPN variant"
    (En, ValueFootprint) => "Value + footprint"
    (En, @api.MatchKind::None) => "—"
  }
}

///|
fn issue_label(lang : Lang, code : @api.IssueCode) -> String {
  match (lang, code) {
    (Ja, NotFound) => "OpenPartsに該当部品なし"
    (Ja, VariantMatch) =>
      "型番の派生(梱包・グレード違い)で照合"
    (Ja, ValueMatch) => "値とサイズから候補を推定"
    (Ja, Ambiguous) => "候補が複数"
    (Ja, ManufacturerMismatch) => "メーカー名が一致しない"
    (Ja, LifecycleObsolete) => "生産終了"
    (Ja, LifecycleEol) => "生産終了予告(EOL)"
    (Ja, LifecycleNrnd) => "新規設計非推奨(NRND)"
    (Ja, LifecycleUnknown) => "ライフサイクル不明"
    (Ja, ExistenceRejected) => "誤型番として却下済み"
    (Ja, ExistenceUnverified) => "実在が未確認"
    (Ja, NoParameters) => "電気的特性のデータなし"
    (Ja, FootprintMismatch) =>
      "フットプリントとパッケージが不一致"
    (Ja, FootprintUnchecked) => "フットプリント未確認"
    (Ja, Dnp) => "実装しない(DNP)"
    (En, NotFound) => "Not in OpenParts"
    (En, VariantMatch) => "Matched as an MPN variant (packaging, grade)"
    (En, ValueMatch) => "Candidates from value and size"
    (En, Ambiguous) => "Several candidates"
    (En, ManufacturerMismatch) => "Manufacturer differs"
    (En, LifecycleObsolete) => "Obsolete"
    (En, LifecycleEol) => "End of life announced"
    (En, LifecycleNrnd) => "Not recommended for new designs"
    (En, LifecycleUnknown) => "Lifecycle unknown"
    (En, ExistenceRejected) => "Rejected as a wrong MPN"
    (En, ExistenceUnverified) => "Existence not verified"
    (En, NoParameters) => "No electrical parameters in the data"
    (En, FootprintMismatch) => "Footprint and package differ"
    (En, FootprintUnchecked) => "Footprint not checked"
    (En, Dnp) => "Do not populate"
  }
}

///|
fn alt_class_label(lang : Lang, c : @api.AlternativeClass) -> String {
  match (lang, c) {
    (Ja, DropIn) => "ドロップイン"
    (Ja, FootprintCompatible) =>
      "フットプリント互換(ピン配置違い)"
    (Ja, Functional) => "機能互換(別パッケージ)"
    (Ja, NearMiss) => "一部仕様が劣る"
    (Ja, Listed) => "記載の後継品"
    (En, DropIn) => "Drop-in"
    (En, FootprintCompatible) => "Same footprint, other pinout"
    (En, Functional) => "Functional equivalent"
    (En, NearMiss) => "Near miss"
    (En, Listed) => "Listed replacement"
  }
}

///|
fn alt_class_help(lang : Lang, c : @api.AlternativeClass) -> String {
  match (lang, c) {
    (Ja, DropIn) =>
      "同じランドパターン・ピン互換で、電気的特性が同等以上(記載のない値は未確認)。"
    (Ja, FootprintCompatible) =>
      "同じランドパターンに載りますが、ピン配置が異なります。異なるパッドを確認してください。"
    (Ja, Functional) =>
      "同じカテゴリ・同じ識別値で特性が同等以上ですが、パッケージが異なります。"
    (Ja, NearMiss) =>
      "同じランドパターン・ピン互換ですが、一部の特性が元の部品より劣ります。"
    (Ja, Listed) =>
      "データに後継品として記載されていますが、互換性は上の比較のとおりです。"
    (En, DropIn) =>
      "Same land pattern, pin-compatible, parameters meeting or exceeding the original (unstated values are unverified)."
    (En, FootprintCompatible) =>
      "Fits the same land pattern, but the pinout differs: check the pads that differ."
    (En, Functional) =>
      "Same category and identity values, parameters meeting the original, in another package."
    (En, NearMiss) =>
      "Same land pattern and pin-compatible, but some parameter is worse than the original's."
    (En, Listed) =>
      "Named as a replacement in the data; its compatibility is as compared."
  }
}

///|
fn verdict_label(lang : Lang, v : @api.Verdict) -> String {
  match (lang, v) {
    (Ja, Meets) => "特性: 同等以上"
    (Ja, Unverified) => "特性: 一部未確認"
    (Ja, Worse) => "特性: 劣る項目あり"
    (En, Meets) => "Parameters meet"
    (En, Unverified) => "Parameters partly unverified"
    (En, Worse) => "Some parameter worse"
  }
}

///|
fn check_status_label(lang : Lang, s : @api.CheckStatus) -> String {
  match (lang, s) {
    (Ja, Pass) => "同等"
    (Ja, Better) => "優れる"
    (Ja, Info) => "参考"
    (Ja, Unverified) => "未確認"
    (Ja, Fail) => "劣る/異なる"
    (En, Pass) => "Meets"
    (En, Better) => "Better"
    (En, Info) => "Info"
    (En, Unverified) => "Unverified"
    (En, Fail) => "Worse / differs"
  }
}

///|
fn rule_label(lang : Lang, r : @api.RuleKind) -> String {
  match (lang, r) {
    (Ja, Equal) => "一致"
    (Ja, AtLeast) => "以上"
    (Ja, AtMost) => "以下"
    (Ja, Ranked) => "等級以上"
    (Ja, Informational) => "参考"
    (Ja, Covers) => "範囲を包含"
    (En, Equal) => "equal"
    (En, AtLeast) => "≥ original"
    (En, AtMost) => "≤ original"
    (En, Ranked) => "grade ≥"
    (En, Informational) => "info"
    (En, Covers) => "range ⊇ original"
  }
}

///|
fn pad_status_label(lang : Lang, s : @api.PadStatus) -> String {
  match (lang, s) {
    (Ja, Same) => "同じ"
    (Ja, Equivalent) => "同等(別名)"
    (Ja, Note) => "要確認"
    (Ja, Differs) => "異なる"
    (En, Same) => "Same"
    (En, Equivalent) => "Equivalent"
    (En, Note) => "Check"
    (En, Differs) => "Differs"
  }
}

///|
fn pad_reason_label(lang : Lang, r : @api.PadReason) -> String {
  match (lang, r) {
    (Ja, FunctionDiffers) => "機能が異なる"
    (Ja, GroundMismatch) => "片方だけGND"
    (Ja, PolarityDiffers) => "イネーブルの極性が逆"
    (Ja, PolarityUnverified) =>
      "イネーブルの極性を名前から判断できない"
    (Ja, CandidateUsesNcPad) =>
      "元はNC、候補はバイパス等に使用(未接続でも動作)"
    (Ja, CandidateNeedsNcPad) =>
      "元はNC、候補はこのピンの接続が必要"
    (Ja, UnusedOnCandidate) => "候補ではNC(基板側の接続は無効)"
    (Ja, MissingPin) => "対応するピンがない"
    (En, FunctionDiffers) => "Different function"
    (En, GroundMismatch) => "Ground on one side only"
    (En, PolarityDiffers) => "Enable of opposite polarity"
    (En, PolarityUnverified) => "Enable polarity not stated in the name"
    (En, CandidateUsesNcPad) =>
      "NC on the original; the candidate uses it (bypass), works unconnected"
    (En, CandidateNeedsNcPad) =>
      "NC on the original; the candidate needs it connected"
    (En, UnusedOnCandidate) =>
      "NC on the candidate: the board's connection goes nowhere"
    (En, MissingPin) => "No matching pin"
  }
}

///|
fn alt_count_note(lang : Lang, n : Int32) -> String {
  match lang {
    Ja => "\{n}件"
    En => if n == 1 { "1 part" } else { "\{n} parts" }
  }
}

///|
/// The candidates left out for another (or an unknown) identity value.
fn alt_rejected_note(lang : Lang, n : Int32) -> String {
  match lang {
    Ja => "\{n}件を除外(識別値が異なる・不明)"
    En =>
      "\{alt_count_note(lang, n)} left out (identity value differs or unknown)"
  }
}

///|
fn coverage_label(lang : Lang, have : Int32, want : Int32) -> String {
  match lang {
    Ja => "カテゴリの主要パラメータ \{have}/\{want}"
    En => "\{have} of \{want} key parameters"
  }
}

///|
fn sources_count(lang : Lang, n : Int32) -> String {
  match lang {
    Ja => "出典 \{n}件"
    En => if n == 1 { "1 source" } else { "\{n} sources" }
  }
}

///|
fn more_candidates(lang : Lang, n : Int32) -> String {
  match lang {
    Ja => "ほか\{n}件"
    En => "+\{n} more"
  }
}

///|
fn format_label(lang : Lang, f : @api.BomFormat) -> String {
  match (lang, f) {
    (Ja, KicadCsv) => "KiCad BOM CSV"
    (Ja, KicadNetlist) => "KiCad ネットリスト"
    (Ja, GenericCsv) => "汎用CSV"
    (En, KicadCsv) => "KiCad BOM CSV"
    (En, KicadNetlist) => "KiCad netlist"
    (En, GenericCsv) => "Generic CSV"
  }
}

///|
fn column_role_label(lang : Lang, r : @api.ColumnRole) -> String {
  match (lang, r) {
    (Ja, Reference) => "リファレンス"
    (Ja, Value) => "値"
    (Ja, Footprint) => "フットプリント"
    (Ja, Datasheet) => "データシート"
    (Ja, Quantity) => "数量"
    (Ja, Mpn) => "MPN"
    (Ja, Manufacturer) => "メーカー"
    (Ja, Description) => "説明"
    (Ja, Dnp) => "DNP"
    (En, Reference) => "Reference"
    (En, Value) => "Value"
    (En, Footprint) => "Footprint"
    (En, Datasheet) => "Datasheet"
    (En, Quantity) => "Quantity"
    (En, Mpn) => "MPN"
    (En, Manufacturer) => "Manufacturer"
    (En, Description) => "Description"
    (En, Dnp) => "DNP"
  }
}
