// Every string the site shows, in Japanese (the default, unprefixed
// URLs) and English (under /en/).

///|
#derive(Eq)
enum Lang {
  Ja
  En
}

///|
fn Lang::code(self : Lang) -> String {
  match self {
    Ja => "ja"
    En => "en"
  }
}

///|
struct Feature {
  title : String
  body : String
}

///|
struct Copy {
  // nav / footer
  nav_search : String
  nav_kicad : String
  nav_contributing : String
  open_source : String
  // home
  home_title : String
  home_lead : String
  home_cta_search : String
  home_cta_kicad : String
  features : Array[Feature]
  // KiCad plugin
  kicad_title : String
  kicad_intro : String
  kicad_quick_install : String
  kicad_auto_detect : String
  kicad_restart : String
  kicad_good_to_know : String
  kicad_session : String
  // contributing
  contrib_title : String
  contrib_intro : String
  contrib_report_heading : String
  contrib_report_before : String
  contrib_report_link : String
  contrib_report_after : String
  contrib_pr_heading : String
  // [before, link 1, after, link 2, after 2] per step; "" = absent
  contrib_steps : Array[Array[String]]
  contrib_specs_heading : String
  // search
  search_title : String
  search_placeholder : String
  loading : String
  search_failed : String
  no_results : String
  // part
  not_found_title : String
  not_found_before : String
  not_found_link : String
  not_found_after : String
  load_failed : String
  artifacts_failed : String
  download : String
  zip_building : String
  zip_button : String
  stl_button : String
  zip_failed : String
  preview3d : String
  model_load_failed : String
  viewer_controls : String
  report_issue : String
  pin_number : String
  pin_name : String
  pin_type : String
  pin_alt : String
  revisions : String
  family : String
  lead_count : String
  pitch : String
  body_width : String
  body_length : String
  body_height : String
  footprint : String
  footprint_legend : String
  sources_heading : String
  sources_note : String
  source_cited_by : String
  sources_none : String
}

///|
let ja : Copy = {
  nav_search: "部品を探す",
  nav_kicad: "KiCadプラグイン",
  nav_contributing: "コントリビュート",
  open_source: "OpenParts is open source.",
  home_title: "オープンで、出典のたどれる電子部品データ",
  home_lead: "OpenPartsは、電子部品のCADアセット(KiCadシンボル/フットプリント、STEP/STLモデル)を既製ファイルとして配布するのではなく、検証可能な正規データから再現可能に生成するプラットフォームです。",
  home_cta_search: "部品を探す",
  home_cta_kicad: "KiCadプラグインを入れる",
  features: [
    {
      title: "オープンな部品データ",
      body: "電子部品のピン配置・パッケージ寸法・リビジョン情報を、誰でも閲覧・検証・修正できる正規YAMLデータとして公開しています。",
    },
    {
      title: "Provenance(出典)追跡",
      body: "すべてのフィールドがデータシート等の出典に紐づいています。「なぜそう書かれているか」を常にたどれます。",
    },
    {
      title: "再現可能なCAD生成",
      body: "既製のシンボル/フットプリント/3Dモデルを配布するのではなく、正規データから都度生成します。データを直せば、生成物も追従します。",
    },
  ],
  kicad_title: "KiCadプラグイン",
  kicad_intro: "KiCadのツールバーから直接OpenPartsレジストリを検索し、部品のSymbol / Footprint / STEPモデルをプロジェクトのローカルライブラリにインストールできる、小さなネイティブGUIアプリです。スキーマティックやボードへの配置は行いません。インストール後はKiCad標準のSymbol/Footprintピッカーからいつも通り使ってください。",
  kicad_quick_install: "クイックインストール",
  kicad_auto_detect: "どちらのスクリプトもKiCadのプラグインディレクトリを自動検出しようとします。検出できない場合はKiCadの「Tools > External Plugins > Open Plugin Directory」で場所を確認し、--plugin-dir(Linux)/ -PluginDir(Windows)で明示的に指定して再実行してください。",
  kicad_restart: "インストール後はKiCadを再起動するか、「Tools > External Plugins > Refresh Plugins」を実行し、PCBエディタのツールバーに現れる「OpenParts」ボタンを探してください。Plugin and Content Managerの「Installed」一覧には表示されません。",
  kicad_good_to_know: "知っておくとよいこと",
  kicad_session: "あるKiCadセッションで初めて部品をインストールした直後は、Symbol Editor / Footprint Editor / 3Dビューアのウィンドウを一度閉じて開き直してください。KiCadはこれらのウィンドウを開いた時点でライブラリテーブルを読み込み、開いている間はライブラリテーブルの変更をホットリロードしないためです(KiCad自体の仕様です)。",
  contrib_title: "コントリビュート",
  contrib_intro: "OpenPartsの部品データは openparts-data リポジトリの正規YAMLとして管理されています。ピン配置や寸法などの誤りに気づいた場合や、新しい部品を追加したい場合は、誰でもPull Requestを送ることができます。",
  contrib_report_heading: "個別データの問題を報告する",
  contrib_report_before: "自分で直す時間が無い場合は、まず",
  contrib_report_link: "部品を探し",
  contrib_report_after: "、詳細ページのピン表にある「このデータの問題を報告」リンクから openparts-data へIssueを作成してください。対象の部品・デバイスIDが自動的に入力されます。",
  contrib_pr_heading: "新規部品を追加する / 既存データを修正する(Pull Request)",
  contrib_steps: [
    [
      "openparts-data を", "フォーク", "します。検証コマンドのビルドに openparts(コアリポジトリ)が必要なので、兄弟ディレクトリとして",
      "openparts", "もcloneしてください。",
    ],
    [
      "", "Canonical Data Specification", "(特にRepository Layoutの節)に従い、parts/・devices/・packages/・manufacturers/・sources/配下にYAMLを追加/編集します。",
      "", "",
    ],
    [
      "openparts-data のリポジトリルートで ./scripts/validate.sh を実行し、Part/Device/Package/Sourcesの参照整合性とバリデーションが通ることを確認します(このスクリプトは兄弟の ../openparts をビルドして使います)。",
      "", "", "", "",
    ],
    [
      "openparts-data へPull Requestを作成します。作成後はCI上の validate ワークフローが同じ検証を自動実行します。",
      "", "", "", "",
    ],
  ],
  contrib_specs_heading: "関連する仕様書",
  search_title: "部品を探す",
  search_placeholder: "型番・メーカー・パッケージで検索(例: nRF52840, 0603, QFN)",
  loading: "読み込み中...",
  search_failed: "検索に失敗しました。",
  no_results: "該当する部品が見つかりませんでした。",
  not_found_title: "部品が見つかりません",
  not_found_before: "指定された部品は見つかりませんでした。",
  not_found_link: "検索",
  not_found_after: "からお探しください。",
  load_failed: "読み込みに失敗しました。",
  artifacts_failed: "CAD成果物の情報を取得できませんでした。",
  download: "ダウンロード",
  zip_building: "zip作成中...",
  zip_button: "KiCad用一式をダウンロード(.zip)",
  stl_button: "STLをダウンロード",
  zip_failed: "ダウンロードに失敗しました。",
  preview3d: "3Dプレビュー",
  model_load_failed: "3Dモデルを表示できませんでした。",
  viewer_controls: "ドラッグ: 回転(カーソル位置中心)　右/中ドラッグ・Shift+ドラッグ: 移動　ホイール: カーソルへズーム　ダブルクリック: その点へ寄る　F: 全体表示　1/3/7/0: 正面/右/上/等角(Ctrl で反対側)　P: 投影切替　寸法ラベルをクリック: 出典",
  report_issue: "このデータの問題を報告",
  pin_number: "ピン番号",
  pin_name: "名前",
  pin_type: "種別",
  pin_alt: "代替機能",
  revisions: "リビジョン",
  family: "ファミリー",
  lead_count: "リード数",
  pitch: "ピッチ",
  body_width: "本体幅",
  body_length: "本体長",
  body_height: "本体高さ",
  footprint: "フットプリント",
  footprint_legend: "金: 銅パッド　白: シルク　灰: 部品外形　桃: コートヤード",
  sources_heading: "出典",
  sources_note: "この部品のデータは次の文書から読み取っています。リンク先は各メーカーのサイトで、版が更新されている場合があります。",
  source_cited_by: "根拠にしている項目",
  sources_none: "出典が登録されていません。",
}

///|
let en : Copy = {
  nav_search: "Search",
  nav_kicad: "KiCad Plugin",
  nav_contributing: "Contributing",
  open_source: "OpenParts is open source.",
  home_title: "Open electronic component data, with sources you can trace",
  home_lead: "OpenParts generates CAD assets (KiCad symbols/footprints, STEP/STL models) reproducibly from verifiable canonical data, instead of distributing pre-made files.",
  home_cta_search: "Search parts",
  home_cta_kicad: "Install the KiCad plugin",
  features: [
    {
      title: "Open part data",
      body: "Pin assignments, package dimensions, and revision info are published as canonical YAML data anyone can view, verify, or fix.",
    },
    {
      title: "Provenance tracking",
      body: "Every field is tied to a datasheet or other source, so you can always trace why it's written the way it is.",
    },
    {
      title: "Reproducible CAD generation",
      body: "Symbols, footprints, and 3D models aren't distributed pre-made -- they're generated from canonical data each time. Fix the data, and the generated output follows.",
    },
  ],
  kicad_title: "KiCad Plugin",
  kicad_intro: "A small native GUI app that searches the OpenParts registry and installs a part's Symbol / Footprint / STEP model into your project's local libraries, right from KiCad's toolbar. It doesn't place anything on the schematic or board -- use KiCad's own Symbol/Footprint picker as usual after installing.",
  kicad_quick_install: "Quick install",
  kicad_auto_detect: "Both scripts try to auto-detect your KiCad plugin directory. If they can't, find it via KiCad's Tools > External Plugins > Open Plugin Directory, then re-run with --plugin-dir (Linux) / -PluginDir (Windows) set explicitly.",
  kicad_restart: "Afterwards, restart KiCad (or Tools > External Plugins > Refresh Plugins) and look for the new \"OpenParts\" button in the PCB editor toolbar -- not in the Plugin and Content Manager's own \"Installed\" list.",
  kicad_good_to_know: "Good to know",
  kicad_session: "Right after installing a part for the first time in a KiCad session, close and reopen the Symbol Editor / Footprint Editor / 3D viewer windows. KiCad reads library tables once when each window opens and doesn't hot-reload them while it's open -- this is KiCad's own behavior.",
  contrib_title: "Contributing",
  contrib_intro: "OpenParts' component data is maintained as canonical YAML in the openparts-data repository. If you spot an error in a pin assignment or dimension, or want to add a new part, anyone can send a Pull Request.",
  contrib_report_heading: "Report a problem with existing data",
  contrib_report_before: "If you don't have time to fix it yourself, ",
  contrib_report_link: "search for the part",
  contrib_report_after: ", then use the \"Report a problem with this data\" link on its pin table to open an Issue against openparts-data -- the part and device IDs are pre-filled.",
  contrib_pr_heading: "Add a new part / fix existing data (Pull Request)",
  contrib_steps: [
    [
      "", "Fork openparts-data", ". The validation command needs openparts (the core repo), so also clone ",
      "openparts", " as a sibling directory.",
    ],
    [
      "Add or edit YAML under parts/, devices/, packages/, manufacturers/, sources/, following the ",
      "Canonical Data Specification", " (especially the Repository Layout section).",
      "", "",
    ],
    [
      "Run ./scripts/validate.sh from openparts-data's repo root to confirm Part/Device/Package/Sources references resolve and validation passes (it builds the sibling ../openparts to do so).",
      "", "", "", "",
    ],
    [
      "Open a Pull Request against openparts-data. The validate workflow runs the same checks in CI once it's up.",
      "", "", "", "",
    ],
  ],
  contrib_specs_heading: "Related specifications",
  search_title: "Search parts",
  search_placeholder: "Search by MPN, manufacturer or package (e.g. nRF52840, 0603, QFN)",
  loading: "Loading...",
  search_failed: "Search failed.",
  no_results: "No matching parts found.",
  not_found_title: "Part not found",
  not_found_before: "The requested part could not be found. Try ",
  not_found_link: "searching",
  not_found_after: " instead.",
  load_failed: "Failed to load.",
  artifacts_failed: "Could not fetch CAD artifact info.",
  download: "Download",
  zip_building: "Building zip...",
  zip_button: "Download KiCad bundle (.zip)",
  stl_button: "Download STL",
  zip_failed: "Download failed.",
  preview3d: "3D preview",
  model_load_failed: "Could not display the 3D model.",
  viewer_controls: "Drag: orbit (about the point under the cursor) · Right/middle or Shift+drag: pan · Wheel: zoom to cursor · Double-click: close in on a point · F: fit · 1/3/7/0: front/right/top/iso (Ctrl: opposite) · P: projection · Click a dimension label: its source",
  report_issue: "Report a problem with this data",
  pin_number: "Pin",
  pin_name: "Name",
  pin_type: "Type",
  pin_alt: "Alternate functions",
  revisions: "Revisions",
  family: "Family",
  lead_count: "Lead count",
  pitch: "Pitch",
  body_width: "Body width",
  body_length: "Body length",
  body_height: "Body height",
  footprint: "Footprint",
  footprint_legend: "Gold: copper pads · White: silkscreen · Grey: body outline · Pink: courtyard",
  sources_heading: "Sources",
  sources_note: "This part's data was read from the documents below. Links go to each manufacturer's site, which may now serve a newer revision.",
  source_cited_by: "Used for",
  sources_none: "No sources are recorded.",
}

///|
fn Lang::copy(self : Lang) -> Copy {
  match self {
    Ja => ja
    En => en
  }
}

///|
fn pins_heading(lang : Lang, device_id : String) -> String {
  match lang {
    Ja => "ピン配置 (\{device_id})"
    En => "Pinout (\{device_id})"
  }
}

///|
fn pins_show(lang : Lang, n : Int32) -> String {
  match lang {
    Ja => "\{n}ピンを表示"
    En => "Show \{n} pins"
  }
}

///|
fn package_heading(lang : Lang, id : String) -> String {
  match lang {
    Ja => "パッケージ (\{id})"
    En => "Package (\{id})"
  }
}

///|
fn footprint_alt(lang : Lang, mpn : String) -> String {
  match lang {
    Ja => "\{mpn} のフットプリント（上面図）"
    En => "Footprint of \{mpn} (top view)"
  }
}

///|
fn source_revision(lang : Lang, rev : String, date : String) -> String {
  match lang {
    Ja => "Rev \{rev}（\{date}）"
    En => "Rev \{rev} (\{date})"
  }
}

///|
fn source_retrieved(lang : Lang, date : String) -> String {
  match lang {
    Ja => "\{date} 取得"
    En => "retrieved \{date}"
  }
}

///|
fn cited_by_label(lang : Lang, by : String) -> String {
  match (lang, by) {
    (Ja, "part") => "部品"
    (Ja, "device") => "ピン配置"
    (Ja, "package") => "パッケージ寸法"
    (En, "device") => "pinout"
    (En, "package") => "package dimensions"
    _ => by
  }
}

///|
fn category_label(lang : Lang, c : @api.ComponentCategory) -> String {
  match (lang, c) {
    (Ja, Microcontroller) => "マイコン"
    (Ja, Capacitor) => "コンデンサ"
    (Ja, Resistor) => "抵抗"
    (Ja, Inductor) => "インダクタ"
    (Ja, Diode) => "ダイオード"
    (Ja, Transistor) => "トランジスタ"
    (Ja, Led) => "LED"
    (Ja, Crystal) => "水晶発振子"
    (Ja, Fuse) => "ヒューズ"
    (Ja, Connector) => "コネクタ"
    (Ja, Other) => "その他"
    (En, Led) => "LED"
    (En, c) => {
      let w = c.wire()
      w[:1].to_owned().to_upper() + w[1:].to_owned()
    }
  }
}

// ---- catalog (faceted search, catalog.mbt) ----

///|
struct CatalogCopy {
  lead : String
  browse_all : String
  by_size : String
  by_family : String
  filters : String
  clear_all : String
  close : String
  sort : String
  ascending : String
  descending : String
  min : String
  max : String
  prev : String
  next : String
  show_less : String
  electrical_note : String
  // facet group titles
  f_category : String
  f_manufacturer : String
  f_family : String
  f_size : String
  f_pins : String
  f_pitch : String
  f_mounting : String
  f_lifecycle : String
  f_existence : String
  f_width : String
  f_length : String
  f_height : String
  // sort orders
  s_relevance : String
  s_mpn : String
  s_manufacturer : String
  s_pins : String
  s_pitch : String
  s_size : String
  s_height : String
  // table columns
  c_mpn : String
  c_manufacturer : String
  c_category : String
  c_package : String
  c_size : String
  c_pins : String
  c_pitch : String
  c_dims : String
  c_mounting : String
  c_lifecycle : String
}

///|
let catalog_ja : CatalogCopy = {
  lead: "カテゴリを選ぶか型番・キーワードで検索し、パッケージ・サイズ・ピン数・ピッチ・寸法で絞り込めます。",
  browse_all: "すべての部品",
  by_size: "チップサイズから探す",
  by_family: "パッケージから探す",
  filters: "絞り込み",
  clear_all: "すべてクリア",
  close: "閉じる",
  sort: "並び替え",
  ascending: "昇順",
  descending: "降順",
  min: "最小",
  max: "最大",
  prev: "前へ",
  next: "次へ",
  show_less: "折りたたむ",
  electrical_note: "容量・抵抗値・定格電圧などの電気的特性は、出典付きの値がある部品だけが絞り込み対象です(まだ一部の部品のみ)。範囲は 10n、0.1u、100nF、4k7 のように単位接頭辞付きで入力できます。",
  f_category: "カテゴリ",
  f_manufacturer: "メーカー",
  f_family: "パッケージ",
  f_size: "チップサイズ (インチ / ミリ)",
  f_pins: "ピン数",
  f_pitch: "ピッチ (mm)",
  f_mounting: "実装方式",
  f_lifecycle: "ライフサイクル",
  f_existence: "実在確認",
  f_width: "本体幅 W (mm)",
  f_length: "本体長 L (mm)",
  f_height: "本体高さ H (mm)",
  s_relevance: "関連度順",
  s_mpn: "型番順",
  s_manufacturer: "メーカー順",
  s_pins: "ピン数順",
  s_pitch: "ピッチ順",
  s_size: "サイズ順",
  s_height: "高さ順",
  c_mpn: "型番",
  c_manufacturer: "メーカー",
  c_category: "カテゴリ",
  c_package: "パッケージ",
  c_size: "サイズ",
  c_pins: "ピン数",
  c_pitch: "ピッチ",
  c_dims: "W×L×H (mm)",
  c_mounting: "実装",
  c_lifecycle: "ライフサイクル",
}

///|
let catalog_en : CatalogCopy = {
  lead: "Pick a category or search by part number or keyword, then narrow by package, size, pin count, pitch and dimensions.",
  browse_all: "All parts",
  by_size: "Browse by chip size",
  by_family: "Browse by package",
  filters: "Filters",
  clear_all: "Clear all",
  close: "Close",
  sort: "Sort",
  ascending: "Ascending",
  descending: "Descending",
  min: "Min",
  max: "Max",
  prev: "Previous",
  next: "Next",
  show_less: "Show less",
  electrical_note: "Electrical parameters such as capacitance, resistance and voltage rating filter only the parts with sourced values (so far a few). Ranges take unit prefixes: 10n, 0.1u, 100nF, 4k7.",
  f_category: "Category",
  f_manufacturer: "Manufacturer",
  f_family: "Package",
  f_size: "Chip size (inch / mm)",
  f_pins: "Pin count",
  f_pitch: "Pitch (mm)",
  f_mounting: "Mounting",
  f_lifecycle: "Lifecycle",
  f_existence: "Existence",
  f_width: "Body width W (mm)",
  f_length: "Body length L (mm)",
  f_height: "Body height H (mm)",
  s_relevance: "Relevance",
  s_mpn: "Part number",
  s_manufacturer: "Manufacturer",
  s_pins: "Pin count",
  s_pitch: "Pitch",
  s_size: "Size",
  s_height: "Height",
  c_mpn: "Part number",
  c_manufacturer: "Manufacturer",
  c_category: "Category",
  c_package: "Package",
  c_size: "Size",
  c_pins: "Pins",
  c_pitch: "Pitch",
  c_dims: "W×L×H (mm)",
  c_mounting: "Mounting",
  c_lifecycle: "Lifecycle",
}

///|
fn Lang::catalog(self : Lang) -> CatalogCopy {
  match self {
    Ja => catalog_ja
    En => catalog_en
  }
}

///|
fn parts_count(lang : Lang, n : Int32) -> String {
  match lang {
    Ja => "\{n}件"
    En => if n == 1 { "1 part" } else { "\{n} parts" }
  }
}

///|
fn results_range(
  lang : Lang,
  from : Int32,
  to : Int32,
  total : Int32,
) -> String {
  match lang {
    Ja => "\{total}件中 \{from}–\{to}件を表示"
    En => "Showing \{from}–\{to} of \{total}"
  }
}

///|
fn show_results(lang : Lang, n : Int32) -> String {
  match lang {
    Ja => "\{n}件を表示"
    En => "Show \{n} results"
  }
}

///|
fn show_more(lang : Lang, n : Int32) -> String {
  match lang {
    Ja => "さらに\{n}件を表示"
    En => "Show \{n} more"
  }
}

///|
fn page_of(lang : Lang, page : Int32, pages : Int32) -> String {
  match lang {
    Ja => "\{page} / \{pages}ページ"
    En => "Page \{page} of \{pages}"
  }
}

///|
/// A package family's display name; `fallback` is the server's label.
fn family_label(lang : Lang, value : String, fallback : String) -> String {
  match (lang, value) {
    (Ja, "chip") => "チップ"
    (Ja, "radial") => "ラジアルリード"
    (Ja, "axial") => "アキシャルリード"
    (Ja, "module") => "モジュール"
    (Ja, "header") => "ピンヘッダ"
    (Ja, "socket") => "ピンソケット"
    (Ja, "shrouded_header") => "ボックスヘッダ"
    (Ja, "right_angle_header") => "L型ピンヘッダ"
    (Ja, "tact_switch_smd") => "タクトスイッチ(SMD)"
    (Ja, "tact_switch_tht") => "タクトスイッチ(挿入)"
    (Ja, "sod_flat") => "SOD(フラットリード)"
    (En, "chip") => "Chip"
    (En, "radial") => "Radial lead"
    (En, "axial") => "Axial lead"
    (En, "module") => "Module"
    (En, "header") => "Pin header"
    (En, "socket") => "Pin socket"
    (En, "shrouded_header") => "Shrouded header"
    (En, "right_angle_header") => "Right-angle header"
    (En, "tact_switch_smd") => "Tact switch (SMD)"
    (En, "tact_switch_tht") => "Tact switch (THT)"
    (En, "sod_flat") => "SOD (flat lead)"
    (_, "usb_c") => "USB Type-C"
    (_, "sot223") => "SOT-223"
    (_, "do214") => "DO-214"
    (_, "to220") => "TO-220"
    (_, "to92") => "TO-92"
    (_, "d2pak") => "D2PAK"
    _ => fallback
  }
}

///|
fn mounting_label(lang : Lang, value : String) -> String {
  match (lang, value) {
    (Ja, "smd") => "表面実装 (SMD)"
    (Ja, "tht") => "挿入実装 (THT)"
    (Ja, "mixed") => "SMD+THT混在"
    (En, "smd") => "Surface mount (SMD)"
    (En, "tht") => "Through hole (THT)"
    (En, "mixed") => "SMD + THT"
    _ => value
  }
}

///|
fn mounting_short(lang : Lang, value : String) -> String {
  match (lang, value) {
    (_, "smd") => "SMD"
    (_, "tht") => "THT"
    (Ja, "mixed") => "混在"
    (En, "mixed") => "Mixed"
    _ => value
  }
}

///|
fn lifecycle_label(lang : Lang, value : String) -> String {
  match (lang, value) {
    (Ja, "pre_release") => "量産前"
    (Ja, "active") => "量産中"
    (Ja, "nrnd") => "新規設計非推奨"
    (Ja, "eol_announced") => "生産終了予告"
    (Ja, "obsolete") => "生産終了"
    (Ja, "unknown") => "不明"
    (En, "pre_release") => "Pre-release"
    (En, "active") => "Active"
    (En, "nrnd") => "NRND"
    (En, "eol_announced") => "EOL announced"
    (En, "obsolete") => "Obsolete"
    (En, "unknown") => "Unknown"
    _ => value
  }
}

///|
fn existence_label(lang : Lang, value : String) -> String {
  match (lang, value) {
    (Ja, "verified") => "実在確認済み"
    (Ja, "historical_verified") => "過去に実在確認"
    (Ja, "probable") => "実在の可能性が高い"
    (Ja, "unverified") => "未確認"
    (Ja, "rejected") => "却下"
    (En, "verified") => "Verified"
    (En, "historical_verified") => "Historically verified"
    (En, "probable") => "Probable"
    (En, "unverified") => "Unverified"
    (En, "rejected") => "Rejected"
    _ => value
  }
}

///|
fn pins_chip(lang : Lang, n : String) -> String {
  match lang {
    Ja => "\{n}ピン"
    En => "\{n} pins"
  }
}

///|
fn pitch_chip(lang : Lang, mm : String) -> String {
  match lang {
    Ja => "ピッチ \{mm} mm"
    En => "\{mm} mm pitch"
  }
}

// ---- electrical parameters (the server's vocabulary,
// openparts_core::PARAMETERS; a server test checks every key is here) ----

///|
/// (key, Japanese name, English name) of every parameter key, in the
/// vocabulary's order.
let param_names : Array[(String, String, String)] = [
  ("capacitance", "静電容量", "Capacitance"),
  ("tolerance", "許容差", "Tolerance"),
  (
    "capacitance_tolerance", "容量許容差 (絶対値)", "Capacitance tolerance (absolute)",
  ),
  ("rated_voltage", "定格電圧", "Rated voltage"),
  ("dielectric", "温度特性 (誘電体)", "Dielectric"),
  ("esr", "等価直列抵抗 (ESR)", "ESR"),
  ("resistance", "抵抗値", "Resistance"),
  ("power_rating", "定格電力", "Power rating"),
  ("temperature_coefficient", "抵抗温度係数", "Temperature coefficient"),
  ("inductance", "インダクタンス", "Inductance"),
  ("rated_current", "定格電流", "Rated current"),
  ("saturation_current", "飽和電流", "Saturation current"),
  ("dcr", "直流抵抗 (DCR)", "DC resistance (DCR)"),
  ("frequency", "周波数", "Frequency"),
  ("load_capacitance", "負荷容量", "Load capacitance"),
  ("frequency_tolerance", "周波数許容差", "Frequency tolerance"),
  ("forward_voltage", "順方向電圧 (Vf)", "Forward voltage (Vf)"),
  ("reverse_voltage", "逆方向電圧 (Vr)", "Reverse voltage (Vr)"),
  ("forward_current", "順方向電流 (If)", "Forward current (If)"),
  ("color", "発光色", "Color"),
  ("wavelength", "波長", "Wavelength"),
  ("polarity", "極性", "Polarity"),
  (
    "drain_source_voltage", "ドレイン-ソース間電圧 (Vds)", "Drain-source voltage (Vds)",
  ),
  ("drain_current", "ドレイン電流 (Id)", "Drain current (Id)"),
  ("rds_on", "オン抵抗 (Rds(on))", "On-resistance (Rds(on))"),
  (
    "gate_threshold_voltage", "ゲートしきい値電圧 (Vgs(th))", "Gate threshold voltage (Vgs(th))",
  ),
  (
    "collector_emitter_voltage", "コレクタ-エミッタ間電圧 (Vceo)", "Collector-emitter voltage (Vceo)",
  ),
  ("collector_current", "コレクタ電流 (Ic)", "Collector current (Ic)"),
  ("output_type", "出力タイプ", "Output type"),
  ("output_voltage", "出力電圧", "Output voltage"),
  ("input_voltage_max", "最大入力電圧", "Max input voltage"),
  ("output_current", "出力電流", "Output current"),
  ("dropout_voltage", "ドロップアウト電圧", "Dropout voltage"),
  ("capacity", "記憶容量", "Capacity"),
  ("actuation_force", "操作力", "Actuation force"),
  ("travel", "ストローク", "Travel"),
]

///|
/// A parameter key's display name (the key itself if unknown).
fn param_name(lang : Lang, key : String) -> String {
  for p in param_names {
    if p.0 == key {
      return match lang {
        Ja => p.1
        En => p.2
      }
    }
  }
  key
}

///|
/// An enumerated parameter value's display name; dielectric codes and
/// unknown values show as they are.
fn param_value_label(lang : Lang, key : String, value : String) -> String {
  match (lang, key, value) {
    (_, "polarity", "npn") => "NPN"
    (_, "polarity", "pnp") => "PNP"
    (Ja, "polarity", "n_channel") => "Nチャネル"
    (Ja, "polarity", "p_channel") => "Pチャネル"
    (En, "polarity", "n_channel") => "N-channel"
    (En, "polarity", "p_channel") => "P-channel"
    (Ja, "output_type", "fixed") => "固定"
    (Ja, "output_type", "adjustable") => "可変"
    (En, "output_type", "fixed") => "Fixed"
    (En, "output_type", "adjustable") => "Adjustable"
    (Ja, "color", "red") => "赤"
    (Ja, "color", "orange") => "橙"
    (Ja, "color", "amber") => "アンバー"
    (Ja, "color", "yellow") => "黄"
    (Ja, "color", "green") => "緑"
    (Ja, "color", "blue") => "青"
    (Ja, "color", "white") => "白"
    (Ja, "color", "infrared") => "赤外"
    (Ja, "color", "ultraviolet") => "紫外"
    (_, "color", "rgb") => "RGB"
    (En, "color", v) => v[:1].to_owned().to_upper() + v[1:].to_owned()
    _ => value
  }
}

///|
fn range_chip(
  lang : Lang,
  name : String,
  bound : String,
  value : String,
) -> String {
  match (lang, bound) {
    (_, "min") => "\{name} ≥ \{value}"
    _ => "\{name} ≤ \{value}"
  }
}

///|
/// The part viewer's UI strings (crates/viewer/openparts-viewer
/// src/labels.rs has the same keys; it falls back to English).
fn viewer_labels(lang : Lang) -> Json {
  match lang {
    Ja =>
      {
        "dimensions": "寸法",
        "measure": "計測",
        "reset_view": "視点リセット",
      "fit": "全体表示",
      "home": "ホームビュー",
      "perspective": "透視投影",
      "orthographic": "平行投影",
      "view_front": "正面",
      "view_back": "背面",
      "view_left": "左側面",
      "view_right": "右側面",
      "view_top": "上面",
      "view_bottom": "下面",
      "view_iso": "等角",
      "axis_hint": "軸に近いとX/Y/Zに平行にスナップ・Shift: 自由・X/Y/Zキー: 軸を固定",
      "axis_locked": "軸固定",
      "free": "自由",
        "measure_hint": "2点をクリック(頂点・辺の中点・パッド中心にスナップ)。Escでクリア。",
        "distance": "距離",
        "source": "出典",
        "page": "ページ",
        "locator": "箇所",
        "open_source": "文書を開く",
        "model_value": "データシートに記載がないため、生成モデル上で測った値です。",
        "computed": "OpenPartsが計算した値(IPC-7351Bランドパターン)で、データシートの値ではありません。",
        "close": "閉じる",
        "model_failed": "3Dモデルを読み込めませんでした。",
        "no_gpu": "3DプレビューにはWebGPUかWebGL2が必要ですが、このブラウザでは使えません。",
        "footprint_failed": "フットプリントを描画できませんでした。",
        "snap_vertex": "頂点",
        "snap_edge": "辺の中点",
        "snap_pad": "パッド中心",
        "snap_surface": "表面",
        "body_width": "本体幅",
        "body_length": "本体長",
        "body_height": "高さ",
        "overall_width": "全幅",
        "overall_length": "全長",
        "overall_height": "全高",
        "pitch": "ピッチ",
        "lead_width": "リード幅",
        "lead_length": "リード長",
        "pad_size": "パッド",
        "pad_span_x": "パッド間 X",
        "pad_span_y": "パッド間 Y",
        "courtyard_x": "コートヤード X",
        "courtyard_y": "コートヤード Y",
      }
    En =>
      {
        "dimensions": "Dimensions",
        "measure": "Measure",
        "reset_view": "Reset view",
      "fit": "Fit",
      "home": "Home view",
      "perspective": "Perspective",
      "orthographic": "Orthographic",
      "view_front": "Front",
      "view_back": "Back",
      "view_left": "Left",
      "view_right": "Right",
      "view_top": "Top",
      "view_bottom": "Bottom",
      "view_iso": "Isometric",
      "axis_hint": "Snaps parallel to X/Y/Z near an axis · Shift: free · X/Y/Z key: lock an axis",
      "axis_locked": "axis locked",
      "free": "free",
        "measure_hint": "Click two points (snaps to vertices, edge midpoints and pad centres). Esc clears.",
        "distance": "Distance",
        "source": "Source",
        "page": "Page",
        "locator": "Where",
        "open_source": "Open the document",
        "model_value": "Not stated by the datasheet: measured on the generated model.",
        "computed": "Computed by OpenParts (IPC-7351B land pattern), not a datasheet value.",
        "close": "Close",
        "model_failed": "The 3D model could not be loaded.",
        "no_gpu": "The 3D preview needs WebGPU or WebGL2, which this browser doesn't offer.",
        "footprint_failed": "The footprint could not be drawn.",
        "snap_vertex": "vertex",
        "snap_edge": "edge midpoint",
        "snap_pad": "pad centre",
        "snap_surface": "surface",
        "body_width": "Body width",
        "body_length": "Body length",
        "body_height": "Height",
        "overall_width": "Overall width",
        "overall_length": "Overall length",
        "overall_height": "Overall height",
        "pitch": "Pitch",
        "lead_width": "Lead width",
        "lead_length": "Lead length",
        "pad_size": "Pad",
        "pad_span_x": "Pad span X",
        "pad_span_y": "Pad span Y",
        "courtyard_x": "Courtyard X",
        "courtyard_y": "Courtyard Y",
      }
  }
}
