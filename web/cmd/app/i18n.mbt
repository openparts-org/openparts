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
  // KiCad plugin
  kicad_title : String
  kicad_intro : String
  kicad_quick_install : String
  kicad_notes : Array[String]
  // contributing
  contrib_title : String
  contrib_report_heading : String
  contrib_report_before : String
  contrib_report_link : String
  contrib_report_after : String
  contrib_pr_heading : String
  contrib_review_heading : String
  contrib_review_body : String
  contrib_review_link : String
  contrib_review_after : String
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
  load_failed : String
  artifacts_failed : String
  download : String
  zip_building : String
  zip_button : String
  stl_button : String
  zip_failed : String
  preview3d : String
  model_load_failed : String
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
  /// The source link's tooltip.
  sources_tip : String
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
  home_lead: "出典つきの部品データから、KiCadシンボル・フットプリント・3Dモデルを生成します。",
  home_cta_search: "部品を探す",
  kicad_title: "KiCadプラグイン",
  kicad_intro: "KiCadから部品を検索し、シンボル・フットプリント・STEPをプロジェクトのライブラリに追加します。",
  kicad_quick_install: "インストール",
  kicad_notes: [
    "プラグインディレクトリが検出されないとき: Tools > External Plugins > Open Plugin Directory で確認し --plugin-dir / -PluginDir で指定",
    "KiCadを再起動し、PCBエディタのツールバーの「OpenParts」から起動",
    "初めて部品を追加した後は、シンボル/フットプリントエディタと3Dビューアを開き直す",
  ],
  contrib_title: "コントリビュート",
  contrib_report_heading: "誤りを報告",
  contrib_report_before: "",
  contrib_report_link: "部品ページ",
  contrib_report_after: "の「誤りを報告」からIssueを作成できます。",
  contrib_review_heading: "レビュー",
  contrib_review_body: "AI候補の値を出典ページと照合し、「GitHubで編集」から修正または review ブロックを追加してPRを送ってください。手順: ",
  contrib_review_link: "CONTRIBUTING.md",
  contrib_review_after: "",
  contrib_pr_heading: "部品の追加・修正(Pull Request)",
  contrib_steps: [
    [
      "openparts-data を", "フォーク", "し、隣に ", "openparts", " もclone",
    ],
    [
      "", "Canonical Data Specification", " に沿ってYAMLを追加・編集",
      "", "",
    ],
    ["./scripts/validate.sh で検証", "", "", "", ""],
    ["Pull Requestを作成(CIが同じ検証を実行)", "", "", "", ""],
  ],
  contrib_specs_heading: "仕様書",
  search_title: "部品を探す",
  search_placeholder: "型番・メーカー・パッケージで検索(例: nRF52840, 0603, QFN)",
  loading: "読み込み中...",
  search_failed: "検索に失敗しました。",
  no_results: "該当する部品が見つかりませんでした。",
  not_found_title: "見つかりません",
  load_failed: "読み込みに失敗しました。",
  artifacts_failed: "CAD成果物の情報を取得できませんでした。",
  download: "ダウンロード",
  zip_building: "zip作成中...",
  zip_button: "KiCad用一式をダウンロード(.zip)",
  stl_button: "STLをダウンロード",
  zip_failed: "ダウンロードに失敗しました。",
  preview3d: "3Dプレビュー",
  model_load_failed: "3Dモデルを表示できませんでした。",
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
  sources_tip: "メーカーのサイト(新しい版に更新されている場合があります)",
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
  home_lead: "KiCad symbols, footprints and 3D models generated from part data with datasheet sources.",
  home_cta_search: "Search parts",
  kicad_title: "KiCad Plugin",
  kicad_intro: "Search parts from KiCad and add their symbol, footprint and STEP model to your project's libraries.",
  kicad_quick_install: "Install",
  kicad_notes: [
    "Plugin directory not detected: find it via Tools > External Plugins > Open Plugin Directory and pass --plugin-dir / -PluginDir",
    "Restart KiCad and start it from the \"OpenParts\" button in the PCB editor toolbar",
    "After adding the first part, reopen the Symbol/Footprint Editor and the 3D viewer",
  ],
  contrib_title: "Contributing",
  contrib_report_heading: "Report an error",
  contrib_report_before: "Use \"Report an error\" on the ",
  contrib_report_link: "part's page",
  contrib_report_after: ".",
  contrib_review_heading: "Review",
  contrib_review_body: "Check AI-candidate values against their cited pages, then fix them with \"Edit on GitHub\" or add a review block, and open a pull request. Steps: ",
  contrib_review_link: "CONTRIBUTING.md",
  contrib_review_after: "",
  contrib_pr_heading: "Add or fix parts (pull request)",
  contrib_steps: [
    ["", "Fork openparts-data", " and clone ", "openparts", " next to it"],
    [
      "Add or edit YAML following the ", "Canonical Data Specification", "", "",
      "",
    ],
    ["Run ./scripts/validate.sh", "", "", "", ""],
    ["Open a pull request (CI runs the same checks)", "", "", "", ""],
  ],
  contrib_specs_heading: "Specifications",
  search_title: "Search parts",
  search_placeholder: "Search by MPN, manufacturer or package (e.g. nRF52840, 0603, QFN)",
  loading: "Loading...",
  search_failed: "Search failed.",
  no_results: "No matching parts found.",
  not_found_title: "Not found",
  load_failed: "Failed to load.",
  artifacts_failed: "Could not fetch CAD artifact info.",
  download: "Download",
  zip_building: "Building zip...",
  zip_button: "Download KiCad bundle (.zip)",
  stl_button: "Download STL",
  zip_failed: "Download failed.",
  preview3d: "3D preview",
  model_load_failed: "Could not display the 3D model.",
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
  sources_tip: "The manufacturer's site (may now serve a newer revision)",
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
fn parameters_heading(lang : Lang) -> String {
  match lang {
    Ja => "電気的特性"
    En => "Electrical parameters"
  }
}

///|
/// Where a parameter's value is from: the part, or its device (family).
fn parameter_from_device(lang : Lang) -> String {
  match lang {
    Ja => "デバイス共通"
    En => "device-wide"
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
  match lang {
    Ja =>
      match c {
        Microcontroller => "マイコン"
        Capacitor => "コンデンサ"
        Resistor => "抵抗"
        Inductor => "インダクタ"
        Diode => "ダイオード"
        Transistor => "トランジスタ"
        Led => "LED"
        Crystal => "水晶発振子"
        Fuse => "ヒューズ"
        Connector => "コネクタ"
        Other => "その他"
        Optocoupler => "フォトカプラ"
        Power => "電源IC"
        LinearRegulator => "リニアレギュレータ"
        SwitchingRegulator => "スイッチングレギュレータ (DC-DC)"
        BatteryCharger => "充電IC"
        VoltageReference => "基準電圧IC"
        PowerSwitch => "電源スイッチ・パワーマルチプレクサ"
        Memory => "メモリ"
        Flash => "フラッシュメモリ"
        Eeprom => "EEPROM"
        Sram => "SRAM"
        Sensor => "センサ"
        PressureSensor => "気圧センサ"
        Imu => "加速度・IMUセンサ"
        TemperatureSensor => "温度センサ"
        Analog => "アナログIC"
        OpAmp => "オペアンプ"
        Comparator => "コンパレータ"
        CurrentSenseAmplifier => "電流センスアンプ"
        PowerMonitor => "電流・電力モニタ"
        AnalogSwitch => "アナログスイッチ・マルチプレクサ"
        Adc => "A/Dコンバータ"
        Dac => "D/Aコンバータ"
        Interface => "インターフェースIC"
        IoExpander => "I/Oエキスパンダ"
        CanController => "CANコントローラ"
        CanTransceiver => "CANトランシーバ"
        Rs485Transceiver => "RS-485トランシーバ"
        Rs232Transceiver => "RS-232トランシーバ"
        LevelShifter => "レベルシフタ"
        Logic => "ロジックIC"
        Timing => "タイミング (発振器・タイマ・RTC)"
        Oscillator => "水晶発振器・クロック発振器"
        Timer => "タイマIC"
        Rtc => "リアルタイムクロック (RTC)"
        Switch => "スイッチ"
        TactSwitch => "タクトスイッチ"
        Driver => "ドライバIC"
        GateDriver => "ゲートドライバ"
        DarlingtonArray => "ダーリントンアレイ"
        MotorDriver => "モータードライバ"
        LedDriver => "LEDドライバ"
        Relay => "リレー"
        RotaryEncoder => "ロータリーエンコーダ"
        Potentiometer => "可変抵抗・半固定抵抗"
        FerriteBead => "フェライトビーズ"
        CommonModeChoke => "コモンモードチョーク"
        LightSensor => "照度・近接センサ"
        GasSensor => "ガスセンサ"
        MagneticSensor => "磁気センサ (ホール)"
        Audio => "オーディオ"
        AudioAmplifier => "オーディオアンプ"
        AudioCodec => "オーディオコーデック"
        Microphone => "マイクロホン"
        Buzzer => "ブザー"
        Rf => "RF・無線"
        WirelessModule => "無線モジュール"
        Antenna => "アンテナ"
      }
    En =>
      match c {
        Microcontroller => "Microcontroller"
        Capacitor => "Capacitor"
        Resistor => "Resistor"
        Inductor => "Inductor"
        Diode => "Diode"
        Transistor => "Transistor"
        Led => "LED"
        Crystal => "Crystal"
        Fuse => "Fuse"
        Connector => "Connector"
        Other => "Other"
        Optocoupler => "Optocoupler"
        Power => "Power ICs"
        LinearRegulator => "Linear regulator"
        SwitchingRegulator => "Switching regulator (DC-DC)"
        BatteryCharger => "Battery charger"
        VoltageReference => "Voltage reference"
        PowerSwitch => "Power switch / power mux"
        Memory => "Memory"
        Flash => "Flash"
        Eeprom => "EEPROM"
        Sram => "SRAM"
        Sensor => "Sensor"
        PressureSensor => "Pressure sensor"
        Imu => "IMU / accelerometer"
        TemperatureSensor => "Temperature sensor"
        Analog => "Analog ICs"
        OpAmp => "Op amp"
        Comparator => "Comparator"
        CurrentSenseAmplifier => "Current-sense amplifier"
        PowerMonitor => "Current / power monitor"
        AnalogSwitch => "Analog switch / multiplexer"
        Adc => "ADC"
        Dac => "DAC"
        Interface => "Interface ICs"
        IoExpander => "I/O expander"
        CanController => "CAN controller"
        CanTransceiver => "CAN transceiver"
        Rs485Transceiver => "RS-485 transceiver"
        Rs232Transceiver => "RS-232 transceiver"
        LevelShifter => "Level shifter"
        Logic => "Logic ICs"
        Timing => "Timing (oscillators, timers, RTC)"
        Oscillator => "Oscillator"
        Timer => "Timer"
        Rtc => "Real-time clock (RTC)"
        Switch => "Switch"
        TactSwitch => "Tact switch"
        Driver => "Driver ICs"
        GateDriver => "Gate driver"
        DarlingtonArray => "Darlington array"
        MotorDriver => "Motor driver"
        LedDriver => "LED driver"
        Relay => "Relay"
        RotaryEncoder => "Rotary encoder"
        Potentiometer => "Potentiometer / trimmer"
        FerriteBead => "Ferrite bead"
        CommonModeChoke => "Common-mode choke"
        LightSensor => "Light / proximity sensor"
        GasSensor => "Gas sensor"
        MagneticSensor => "Magnetic (Hall) sensor"
        Audio => "Audio"
        AudioAmplifier => "Audio amplifier"
        AudioCodec => "Audio codec"
        Microphone => "Microphone"
        Buzzer => "Buzzer"
        Rf => "RF / wireless"
        WirelessModule => "Wireless module"
        Antenna => "Antenna"
      }
  }
}

// ---- catalog (faceted search, catalog.mbt) ----

///|
struct CatalogCopy {
  /// A parameter range input's tooltip: unit prefixes.
  unit_hint : String
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
  f_review : String
  f_rows : String
  f_gender : String
  f_orientation : String
  f_shrouded : String
  f_exposed_pad : String
  f_mechanism : String
  f_contact_side : String
  f_interface : String
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
  unit_hint: "例: 10n, 0.1u, 100nF, 4k7",
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
  f_category: "カテゴリ",
  f_manufacturer: "メーカー",
  f_family: "パッケージ",
  f_size: "チップサイズ (インチ / ミリ)",
  f_pins: "ピン数",
  f_pitch: "ピッチ (mm)",
  f_mounting: "実装方式",
  f_lifecycle: "ライフサイクル",
  f_existence: "実在確認",
  f_review: "レビュー",
  f_rows: "列 × ピン数",
  f_gender: "オス / メス",
  f_orientation: "向き",
  f_shrouded: "シュラウド",
  f_exposed_pad: "露出パッド",
  f_mechanism: "カード挿抜方式",
  f_contact_side: "接点位置",
  f_interface: "インターフェース",
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
  unit_hint: "e.g. 10n, 0.1u, 100nF, 4k7",
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
  f_category: "Category",
  f_manufacturer: "Manufacturer",
  f_family: "Package",
  f_size: "Chip size (inch / mm)",
  f_pins: "Pin count",
  f_pitch: "Pitch (mm)",
  f_mounting: "Mounting",
  f_lifecycle: "Lifecycle",
  f_existence: "Existence",
  f_review: "Review",
  f_rows: "Rows × pins",
  f_gender: "Gender",
  f_orientation: "Orientation",
  f_shrouded: "Shroud",
  f_exposed_pad: "Exposed pad",
  f_mechanism: "Card mechanism",
  f_contact_side: "Contact side",
  f_interface: "Interface",
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
    (Ja, "card_socket") => "カードソケット"
    (Ja, "barrel_jack") => "DCジャック"
    (Ja, "terminal_block") => "端子台"
    (Ja, "fpc_connector") => "FPC/FFCコネクタ"
    (Ja, "rf_coax") => "同軸コネクタ"
    (Ja, "micro_usb_b") => "マイクロUSB B"
    (Ja, "mini_usb_b") => "ミニUSB B"
    (Ja, "usb_b") => "USB Type-B"
    (Ja, "smd_led") => "表面実装LED"
    (Ja, "smd_electrolytic") => "チップ形アルミ電解"
    (Ja, "relay") => "リレー"
    (Ja, "buzzer") => "ブザー"
    (Ja, "trimmer") => "半固定抵抗 (トリマ)"
    (Ja, "rotary_encoder") => "ロータリーエンコーダ"
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
    (En, "card_socket") => "Card socket"
    (En, "barrel_jack") => "DC barrel jack"
    (En, "terminal_block") => "Terminal block"
    (En, "fpc_connector") => "FPC/FFC connector"
    (En, "rf_coax") => "RF coaxial"
    (En, "micro_usb_b") => "Micro-USB B"
    (En, "mini_usb_b") => "Mini-USB B"
    (En, "usb_b") => "USB Type-B"
    (En, "smd_led") => "SMD LED"
    (En, "smd_electrolytic") => "SMD electrolytic"
    (En, "relay") => "Relay"
    (En, "buzzer") => "Buzzer"
    (En, "trimmer") => "Trimmer"
    (En, "rotary_encoder") => "Rotary encoder"
    (_, "usb_c") => "USB Type-C"
    (_, "melf") => "MELF"
    (Ja, "chip_array") => "チップ抵抗アレイ"
    (En, "chip_array") => "Chip array"
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
/// The package-structure facets' options (`rows` keeps the API label).
fn structure_label(lang : Lang, key : String, value : String) -> String? {
  let label = match (lang, key, value) {
    (Ja, "gender", "male") => "オス"
    (Ja, "gender", "female") => "メス"
    (En, "gender", "male") => "Male"
    (En, "gender", "female") => "Female"
    (Ja, "orientation", "vertical") => "ストレート"
    (Ja, "orientation", "right_angle") => "ライトアングル"
    (En, "orientation", "vertical") => "Vertical"
    (En, "orientation", "right_angle") => "Right angle"
    (Ja, "shrouded", "yes") => "あり"
    (Ja, "shrouded", "no") => "なし"
    (En, "shrouded", "yes") => "Shrouded"
    (En, "shrouded", "no") => "Unshrouded"
    (Ja, "exposed_pad", "yes") => "あり"
    (Ja, "exposed_pad", "no") => "なし"
    (En, "exposed_pad", "yes") => "Yes"
    (En, "exposed_pad", "no") => "No"
    (Ja, "mechanism", "push_push") => "プッシュ・プッシュ"
    (Ja, "mechanism", "push_pull") => "プッシュ・プル"
    (Ja, "mechanism", "hinged") => "ヒンジ"
    (En, "mechanism", "push_push") => "Push-push"
    (En, "mechanism", "push_pull") => "Push-pull"
    (En, "mechanism", "hinged") => "Hinged"
    (Ja, "contact_side", "top") => "上接点"
    (Ja, "contact_side", "bottom") => "下接点"
    (Ja, "contact_side", "dual") => "両面接点"
    (En, "contact_side", "top") => "Top contact"
    (En, "contact_side", "bottom") => "Bottom contact"
    (En, "contact_side", "dual") => "Dual contact"
    (_, "interface", "micro_sd") => "microSD"
    (_, "interface", "sd") => "SD"
    (_, "interface", "sma") => "SMA"
    (_, "interface", "rp_sma") => "RP-SMA"
    (_, "interface", "smb") => "SMB"
    (_, "interface", "mcx") => "MCX"
    (_, "interface", "mmcx") => "MMCX"
    (_, "interface", "u_fl") => "U.FL"
    (_, "interface", "bnc") => "BNC"
    _ => return None
  }
  Some(label)
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
  ("hold_current", "保持電流 (Ihold)", "Hold current (Ihold)"),
  ("trip_current", "トリップ電流 (Itrip)", "Trip current (Itrip)"),
  ("max_current", "最大遮断電流 (Imax)", "Max interrupt current (Imax)"),
  ("max_voltage", "最大電圧 (Vmax)", "Max voltage (Vmax)"),
  ("frequency", "周波数", "Frequency"),
  ("load_capacitance", "負荷容量", "Load capacitance"),
  ("frequency_tolerance", "周波数許容差", "Frequency tolerance"),
  (
    "frequency_stability", "周波数温度特性", "Frequency stability (over temperature)",
  ),
  ("aging", "エージング (経年変化)", "Aging"),
  ("drive_level", "励振レベル", "Drive level"),
  ("shunt_capacitance", "並列容量 (C0)", "Shunt capacitance (C0)"),
  ("clock_output", "クロック出力", "Clock output"),
  ("forward_voltage", "順方向電圧 (Vf)", "Forward voltage (Vf)"),
  ("reverse_voltage", "逆方向電圧 (Vr)", "Reverse voltage (Vr)"),
  ("forward_current", "順方向電流 (If)", "Forward current (If)"),
  ("zener_voltage", "ツェナー電圧 (Vz)", "Zener voltage (Vz)"),
  (
    "zener_test_current", "ツェナー測定電流 (Izt)", "Zener test current (Izt)",
  ),
  ("color", "発光色", "Color"),
  ("wavelength", "主波長", "Dominant wavelength"),
  ("peak_wavelength", "ピーク波長", "Peak wavelength"),
  ("luminous_intensity", "光度", "Luminous intensity"),
  ("viewing_angle", "指向角 (2θ½)", "Viewing angle (2θ½)"),
  ("wavelength_red", "主波長 (赤)", "Dominant wavelength (red)"),
  ("wavelength_green", "主波長 (緑)", "Dominant wavelength (green)"),
  ("wavelength_blue", "主波長 (青)", "Dominant wavelength (blue)"),
  ("luminous_intensity_red", "光度 (赤)", "Luminous intensity (red)"),
  ("luminous_intensity_green", "光度 (緑)", "Luminous intensity (green)"),
  ("luminous_intensity_blue", "光度 (青)", "Luminous intensity (blue)"),
  ("data_interface", "データ入力", "Data interface"),
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
  ("isolation_voltage", "絶縁耐圧 (BVs)", "Isolation voltage (BVs)"),
  ("cmr", "同相除去 (CMR / CMTI)", "Common-mode rejection (CMR / CMTI)"),
  ("trigger_current", "トリガ電流 (IFT)", "Trigger current (IFT)"),
  (
    "off_state_voltage", "オフ時出力耐圧 (VDRM / VOFF)", "Off-state voltage (VDRM / VOFF)",
  ),
  ("on_resistance", "オン抵抗 (RON)", "On-resistance (RON)"),
  ("on_resistance_flatness", "オン抵抗平坦性", "On-resistance flatness"),
  (
    "current_transfer_ratio", "電流伝達率 (CTR)", "Current transfer ratio (CTR)",
  ),
  ("output_type", "出力タイプ", "Output type"),
  ("output_voltage", "出力電圧", "Output voltage"),
  ("input_voltage_max", "最大入力電圧", "Max input voltage"),
  ("output_current", "出力電流", "Output current"),
  ("dropout_voltage", "ドロップアウト電圧", "Dropout voltage"),
  ("capacity", "記憶容量", "Capacity"),
  ("actuation_force", "操作力", "Actuation force"),
  ("travel", "ストローク", "Travel"),
  ("supply_voltage", "電源電圧", "Supply voltage"),
  ("supply_current", "消費電流", "Supply current"),
  (
    "analog_supply_voltage", "アナログ電源電圧 (AVDD)", "Analog supply voltage (AVDD)",
  ),
  (
    "digital_supply_voltage", "デジタル電源電圧 (DVDD)", "Digital supply voltage (DVDD)",
  ),
  ("analog_supply_current", "アナログ消費電流", "Analog supply current"),
  (
    "digital_supply_current", "デジタル消費電流", "Digital supply current",
  ),
  ("operating_temperature", "動作温度範囲", "Operating temperature"),
  ("esd_hbm", "ESD耐圧 (HBM)", "ESD rating (HBM)"),
  ("channel_count", "チャネル数", "Channels"),
  ("gain_bandwidth", "利得帯域幅積 (GBW)", "Gain bandwidth (GBW)"),
  ("slew_rate", "スルーレート", "Slew rate"),
  (
    "input_offset_voltage", "入力オフセット電圧", "Input offset voltage",
  ),
  ("input_bias_current", "入力バイアス電流", "Input bias current"),
  (
    "supply_current_per_channel", "消費電流 (1回路あたり)", "Supply current per channel",
  ),
  ("rail_to_rail_input", "レールツーレール入力", "Rail-to-rail input"),
  (
    "rail_to_rail_output", "レールツーレール出力", "Rail-to-rail output",
  ),
  ("propagation_delay", "伝搬遅延時間", "Propagation delay"),
  ("output_structure", "出力構造", "Output structure"),
  ("logic_function", "論理機能", "Logic function"),
  (
    "output_current_high", "Hレベル出力電流 (IOH)", "High-level output current (IOH)",
  ),
  (
    "output_current_low", "Lレベル出力電流 (IOL)", "Low-level output current (IOL)",
  ),
  ("data_rate", "データレート", "Data rate"),
  ("bus_fault_voltage", "バス耐故障電圧", "Bus fault protection"),
  ("resolution", "分解能 (bit)", "Resolution (bits)"),
  ("sample_rate", "サンプリングレート", "Sample rate"),
  ("reference", "基準電圧源", "Reference"),
  ("motor_type", "対応モーター", "Motor type"),
  (
    "motor_supply_voltage", "モーター電源電圧 (VM)", "Motor supply voltage (VM)",
  ),
  ("output_current_peak", "ピーク出力電流", "Peak output current"),
  (
    "microstepping", "マイクロステップ (最小 1/N)", "Microstepping (finest 1/N)",
  ),
  ("led_driver_type", "駆動方式", "Driver type"),
  ("amplifier_class", "アンプ級", "Amplifier class"),
  ("output_power", "出力電力", "Output power"),
  ("thd_n", "全高調波歪み+雑音 (THD+N)", "THD+N"),
  ("snr", "S/N比", "Signal-to-noise ratio"),
  ("sensitivity", "感度 (dBV)", "Sensitivity (dBV)"),
  ("sensitivity_digital", "感度 (dBFS)", "Sensitivity (dBFS)"),
  (
    "acoustic_overload_point", "音響過負荷点 (AOP)", "Acoustic overload point (AOP)",
  ),
  ("sound_pressure", "音圧", "Sound pressure"),
  ("buzzer_type", "発音方式", "Buzzer type"),
  ("buzzer_drive", "駆動回路", "Drive"),
  ("coil_voltage", "コイル定格電圧", "Coil voltage"),
  ("coil_resistance", "コイル抵抗", "Coil resistance"),
  ("contact_current", "接点電流", "Contact current"),
  ("contact_voltage", "接点電圧", "Contact voltage"),
  ("must_operate_voltage", "感動電圧", "Must-operate voltage"),
  ("must_release_voltage", "開放電圧", "Must-release voltage"),
  ("input_impedance", "入力インピーダンス", "Input impedance"),
  ("contact_form", "接点構成", "Contact form"),
  ("relay_type", "リレー種別", "Relay type"),
  ("taper", "抵抗変化特性 (カーブ)", "Taper"),
  ("rotation_angle", "回転角度", "Rotation angle"),
  ("resistive_element", "抵抗体", "Resistive element"),
  ("pulses_per_revolution", "パルス数 (1回転)", "Pulses per revolution"),
  ("detents", "クリック数", "Detents"),
  ("push_switch", "プッシュスイッチ", "Push switch"),
  ("impedance", "インピーダンス", "Impedance"),
  ("light_sensor_type", "検出対象", "Sensing"),
  ("illuminance_range", "照度範囲", "Illuminance range"),
  ("target_gas", "検出ガス", "Target gas"),
  ("sensing_principle", "検出方式", "Sensing principle"),
  ("concentration_range", "濃度範囲", "Concentration range"),
  ("magnetic_sensor_type", "センサ種別", "Sensor type"),
  ("operate_point", "動作磁束密度 (BOP)", "Operate point (BOP)"),
  ("release_point", "復帰磁束密度 (BRP)", "Release point (BRP)"),
  ("magnetic_range", "磁束密度範囲", "Magnetic range"),
  ("magnetic_sensitivity", "感度 (mV/mT)", "Sensitivity (mV/mT)"),
  ("bandwidth", "帯域幅", "Bandwidth"),
  ("timekeeping_accuracy", "計時精度", "Timekeeping accuracy"),
  ("backup_current", "バックアップ電流 (VBAT)", "Backup current (VBAT)"),
  ("max_frequency", "最大動作周波数", "Max frequency"),
  ("radio_protocol", "無線規格", "Radio protocol"),
  ("frequency_band", "周波数帯", "Frequency band"),
  ("tx_power", "送信出力", "TX power"),
  ("rx_sensitivity", "受信感度", "RX sensitivity"),
  ("antenna_type", "アンテナ", "Antenna"),
  ("peak_gain", "ピーク利得", "Peak gain"),
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
  // A multi-valued key's label lists its values (`i2c / spi`).
  if value.contains(" / ") {
    return value
      .split(" / ")
      .map(v => param_word_label(lang, key, v.to_owned()))
      .collect()
      .join(" / ")
  }
  param_word_label(lang, key, value)
}

///|
/// One enumerated value's display name.
fn param_word_label(lang : Lang, key : String, value : String) -> String {
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
    (Ja, "data_interface", "single_wire") => "1線式シリアル"
    (Ja, "data_interface", "clock_data") => "クロック+データ"
    (En, "data_interface", "single_wire") => "Single-wire serial"
    (En, "data_interface", "clock_data") => "Clock + data"
    (Ja, "rail_to_rail_input" | "rail_to_rail_output", "yes") => "あり"
    (Ja, "rail_to_rail_input" | "rail_to_rail_output", "no") => "なし"
    (En, "rail_to_rail_input" | "rail_to_rail_output", "yes") => "Yes"
    (En, "rail_to_rail_input" | "rail_to_rail_output", "no") => "No"
    (_, "clock_output", "cmos") => "CMOS"
    (_, "clock_output", "ttl") => "TTL"
    (_, "clock_output", "lvds") => "LVDS"
    (_, "clock_output", "lvpecl") => "LVPECL"
    (_, "clock_output", "hcsl") => "HCSL"
    (_, "clock_output", "cml") => "CML"
    (Ja, "clock_output", "clipped_sine") => "クリップド正弦波"
    (Ja, "clock_output", "sine") => "正弦波"
    (En, "clock_output", "clipped_sine") => "Clipped sine"
    (En, "clock_output", "sine") => "Sine"
    (Ja, "output_structure", "push_pull") => "プッシュプル"
    (Ja, "output_structure", "open_drain") => "オープンドレイン"
    (Ja, "output_structure", "open_collector") => "オープンコレクタ"
    (En, "output_structure", "push_pull") => "Push-pull"
    (En, "output_structure", "open_drain") => "Open drain"
    (En, "output_structure", "open_collector") => "Open collector"
    (Ja, "logic_function", "buffer") => "バッファ"
    (Ja, "logic_function", "inverter") => "インバータ"
    (Ja, "logic_function", "bus_transceiver") => "バストランシーバ"
    (Ja, "logic_function", "flip_flop") => "フリップフロップ"
    (Ja, "logic_function", "latch") => "ラッチ"
    (Ja, "logic_function", "shift_register") => "シフトレジスタ"
    (Ja, "logic_function", "counter") => "カウンタ"
    (Ja, "logic_function", "decoder") => "デコーダ"
    (Ja, "logic_function", "multiplexer") => "マルチプレクサ"
    (_, "logic_function", "and") => "AND"
    (_, "logic_function", "nand") => "NAND"
    (_, "logic_function", "or") => "OR"
    (_, "logic_function", "nor") => "NOR"
    (_, "logic_function", "xor") => "XOR"
    (_, "logic_function", "xnor") => "XNOR"
    (En, "logic_function", "bus_transceiver") => "Bus transceiver"
    (En, "logic_function", "flip_flop") => "Flip-flop"
    (En, "logic_function", "shift_register") => "Shift register"
    (Ja, "data_interface", "analog") => "アナログ"
    (En, "data_interface", "analog") => "Analog"
    (_, "data_interface", "pdm") => "PDM"
    (_, "data_interface", "i2s") => "I²S"
    (_, "data_interface", "tdm") => "TDM"
    (_, "data_interface", "i2c") => "I²C"
    (_, "data_interface", "spi") => "SPI"
    (_, "data_interface", "i2c_spi") => "I²C / SPI"
    (_, "data_interface", "uart") => "UART"
    (_, "data_interface", "usb") => "USB"
    (_, "data_interface", "sdio") => "SDIO"
    (_, "data_interface", "pwm") => "PWM"
    (_, "data_interface", "phase_enable") => "PHASE / ENABLE"
    (_, "data_interface", "step_dir") => "STEP / DIR"
    (Ja, "data_interface", "parallel") => "パラレル"
    (En, "data_interface", "parallel") => "Parallel"
    (_, "data_interface", "lvds") => "LVDS"
    (_, "data_interface", "jesd204") => "JESD204"
    (Ja, "reference", "internal") => "内部"
    (Ja, "reference", "external") => "外部"
    (En, "reference", "internal") => "Internal"
    (En, "reference", "external") => "External"
    (Ja, "reference", "supply") => "電源電圧"
    (En, "reference", "supply") => "Supply"
    (Ja, "motor_type", "brushed_dc") => "ブラシ付きDC"
    (Ja, "motor_type", "stepper") => "ステッピング"
    (Ja, "motor_type", "brushed_dc_or_stepper") =>
      "ブラシ付きDC / ステッピング"
    (Ja, "motor_type", "bldc") => "ブラシレス (BLDC)"
    (En, "motor_type", "brushed_dc") => "Brushed DC"
    (En, "motor_type", "stepper") => "Stepper"
    (En, "motor_type", "brushed_dc_or_stepper") => "Brushed DC / stepper"
    (En, "motor_type", "bldc") => "Brushless (BLDC)"
    (Ja, "led_driver_type", "constant_current") => "定電流"
    (Ja, "led_driver_type", "switching") => "スイッチング"
    (Ja, "led_driver_type", "matrix") => "マトリクス"
    (Ja, "led_driver_type", "pwm") => "PWM"
    (En, "led_driver_type", "constant_current") => "Constant current"
    (En, "led_driver_type", "switching") => "Switching"
    (En, "led_driver_type", "matrix") => "Matrix"
    (En, "led_driver_type", "pwm") => "PWM"
    (Ja, "amplifier_class", v) => v.to_upper() + "級"
    (En, "amplifier_class", v) => "Class " + v.to_upper()
    (Ja, "buzzer_type", "magnetic") => "電磁式"
    (Ja, "buzzer_type", "piezo") => "圧電式"
    (En, "buzzer_type", "magnetic") => "Magnetic"
    (En, "buzzer_type", "piezo") => "Piezo"
    (Ja, "buzzer_drive", "internal") => "発振回路内蔵 (自励)"
    (Ja, "buzzer_drive", "external") => "外部駆動 (他励)"
    (En, "buzzer_drive", "internal") => "Internal (self-drive)"
    (En, "buzzer_drive", "external") => "External drive"
    (_, "contact_form", v) => v.to_upper()
    (Ja, "relay_type", "electromechanical") => "電磁リレー"
    (Ja, "relay_type", "latching") => "ラッチング"
    (Ja, "relay_type", "reed") => "リード"
    (Ja, "relay_type", "solid_state") => "ソリッドステート (SSR)"
    (En, "relay_type", "electromechanical") => "Electromechanical"
    (En, "relay_type", "latching") => "Latching"
    (En, "relay_type", "reed") => "Reed"
    (En, "relay_type", "solid_state") => "Solid state"
    (Ja, "taper", "linear") => "B (直線)"
    (Ja, "taper", "logarithmic") => "A (対数)"
    (Ja, "taper", "reverse_logarithmic") => "C (逆対数)"
    (En, "taper", "linear") => "Linear"
    (En, "taper", "logarithmic") => "Logarithmic"
    (En, "taper", "reverse_logarithmic") => "Reverse logarithmic"
    (Ja, "resistive_element", "carbon") => "カーボン"
    (Ja, "resistive_element", "cermet") => "サーメット"
    (Ja, "resistive_element", "conductive_plastic") =>
      "導電性プラスチック"
    (Ja, "resistive_element", "wirewound") => "巻線"
    (Ja, "resistive_element", "metal_film") => "金属皮膜"
    (En, "resistive_element", "conductive_plastic") => "Conductive plastic"
    (En, "resistive_element", "metal_film") => "Metal film"
    (Ja, "push_switch", "yes") => "あり"
    (Ja, "push_switch", "no") => "なし"
    (En, "push_switch", "yes") => "Yes"
    (En, "push_switch", "no") => "No"
    (Ja, "detents", "0") => "なし (スムーズ)"
    (En, "detents", "0") => "None (smooth)"
    (Ja, "light_sensor_type", "ambient") => "照度"
    (Ja, "light_sensor_type", "proximity") => "近接"
    (Ja, "light_sensor_type", "ambient_proximity") => "照度 + 近接"
    (Ja, "light_sensor_type", "ambient_color_proximity_gesture") =>
      "照度 + カラー + 近接 + ジェスチャ"
    (Ja, "light_sensor_type", "color") => "カラー (RGB)"
    (Ja, "light_sensor_type", "uv") => "紫外線"
    (Ja, "light_sensor_type", "infrared") => "赤外線"
    (En, "light_sensor_type", "ambient") => "Ambient light"
    (En, "light_sensor_type", "proximity") => "Proximity"
    (En, "light_sensor_type", "ambient_proximity") =>
      "Ambient light + proximity"
    (En, "light_sensor_type", "ambient_color_proximity_gesture") =>
      "Ambient light + color + proximity + gesture"
    (En, "light_sensor_type", "color") => "Color (RGB)"
    (En, "light_sensor_type", "uv") => "UV"
    (En, "light_sensor_type", "infrared") => "Infrared"
    (_, "target_gas", "voc") => "VOC"
    (_, "target_gas", "nox") => "NOx"
    (_, "target_gas", "co2") => "CO₂"
    (_, "target_gas", "co") => "CO"
    (_, "target_gas", "h2") => "H₂"
    (_, "target_gas", "ch4") => "CH₄"
    (_, "target_gas", "lpg") => "LPG"
    (_, "target_gas", "nh3") => "NH₃"
    (_, "target_gas", "no2") => "NO₂"
    (_, "target_gas", "o2") => "O₂"
    (_, "target_gas", "o3") => "O₃"
    (_, "target_gas", "h2s") => "H₂S"
    (Ja, "target_gas", "alcohol") => "アルコール"
    (Ja, "target_gas", "combustible") => "可燃性ガス"
    (Ja, "target_gas", "multiple") => "複数"
    (En, "target_gas", "alcohol") => "Alcohol"
    (En, "target_gas", "combustible") => "Combustible gas"
    (En, "target_gas", "multiple") => "Multiple"
    (Ja, "sensing_principle", "metal_oxide") => "金属酸化物半導体 (MOX)"
    (Ja, "sensing_principle", "ndir") => "非分散型赤外線 (NDIR)"
    (Ja, "sensing_principle", "photoacoustic") => "光音響"
    (Ja, "sensing_principle", "electrochemical") => "電気化学"
    (Ja, "sensing_principle", "catalytic") => "接触燃焼"
    (Ja, "sensing_principle", "thermal_conductivity") => "熱伝導"
    (En, "sensing_principle", "metal_oxide") => "Metal oxide (MOX)"
    (En, "sensing_principle", "ndir") => "NDIR"
    (En, "sensing_principle", "photoacoustic") => "Photoacoustic"
    (En, "sensing_principle", "electrochemical") => "Electrochemical"
    (En, "sensing_principle", "catalytic") => "Catalytic"
    (En, "sensing_principle", "thermal_conductivity") => "Thermal conductivity"
    (Ja, "magnetic_sensor_type", "unipolar_switch") =>
      "ユニポーラスイッチ"
    (Ja, "magnetic_sensor_type", "omnipolar_switch") =>
      "オムニポーラスイッチ"
    (Ja, "magnetic_sensor_type", "bipolar_latch") => "バイポーララッチ"
    (Ja, "magnetic_sensor_type", "linear") => "リニア"
    (Ja, "magnetic_sensor_type", "3d") => "3軸"
    (Ja, "magnetic_sensor_type", "angle") => "角度"
    (En, "magnetic_sensor_type", "unipolar_switch") => "Unipolar switch"
    (En, "magnetic_sensor_type", "omnipolar_switch") => "Omnipolar switch"
    (En, "magnetic_sensor_type", "bipolar_latch") => "Bipolar latch"
    (En, "magnetic_sensor_type", "linear") => "Linear"
    (En, "magnetic_sensor_type", "3d") => "3D"
    (En, "magnetic_sensor_type", "angle") => "Angle"
    (_, "radio_protocol", "wifi") => "Wi-Fi"
    (_, "radio_protocol", "wifi_ble") => "Wi-Fi + Bluetooth LE"
    (_, "radio_protocol", "wifi_bluetooth") =>
      "Wi-Fi + Bluetooth (Classic + LE)"
    (_, "radio_protocol", "wifi_ble_802154") =>
      "Wi-Fi + Bluetooth LE + 802.15.4"
    (_, "radio_protocol", "ble") => "Bluetooth LE"
    (_, "radio_protocol", "bluetooth") => "Bluetooth (Classic + LE)"
    (_, "radio_protocol", "ble_802154") => "Bluetooth LE + 802.15.4"
    (_, "radio_protocol", "ieee802154") => "IEEE 802.15.4 (Zigbee / Thread)"
    (_, "radio_protocol", "lora") => "LoRa"
    (Ja, "radio_protocol", "sub_ghz") => "サブGHz"
    (Ja, "radio_protocol", "cellular") => "セルラー (LTE)"
    (En, "radio_protocol", "sub_ghz") => "Sub-GHz"
    (En, "radio_protocol", "cellular") => "Cellular (LTE)"
    (_, "radio_protocol", "gnss") => "GNSS"
    (_, "radio_protocol", "uwb") => "UWB"
    (_, "radio_protocol", "nfc") => "NFC"
    (Ja, "antenna_type", "pcb_trace") => "基板パターン"
    (Ja, "antenna_type", "chip") => "チップ"
    (Ja, "antenna_type", "ceramic_patch") => "セラミックパッチ"
    (Ja, "antenna_type", "external_connector") => "外部 (コネクタ)"
    (Ja, "antenna_type", "external_pad") => "外部 (端子)"
    (Ja, "antenna_type", "whip") => "ホイップ"
    (Ja, "antenna_type", "flex") => "フレキシブル"
    (En, "antenna_type", "pcb_trace") => "PCB trace"
    (En, "antenna_type", "chip") => "Chip"
    (En, "antenna_type", "ceramic_patch") => "Ceramic patch"
    (En, "antenna_type", "external_connector") => "External (connector)"
    (En, "antenna_type", "external_pad") => "External (pad)"
    (En, "antenna_type", "whip") => "Whip"
    (En, "antenna_type", "flex") => "Flex"
    (En, "resistive_element", v) =>
      v[:1].to_owned().to_upper() + v[1:].to_owned()
    (En, "logic_function", v) => v[:1].to_owned().to_upper() + v[1:].to_owned()
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
