export type Lang = "ja" | "en";

export const LANGS: Lang[] = ["ja", "en"];

export const t = {
  ja: {
    langLabel: "日本語",
    nav: {
      search: "部品を探す",
      kicadPlugin: "KiCadプラグイン",
      contributing: "コントリビュート",
    },
    home: {
      metaDescription: "オープンでprovenance追跡された電子部品データプラットフォーム",
      title: "オープンで、出典のたどれる電子部品データ",
      lead: "OpenPartsは、電子部品のCADアセット(KiCadシンボル/フットプリント、STEP/STLモデル)を既製ファイルとして配布するのではなく、検証可能な正規データから再現可能に生成するプラットフォームです。",
      ctaSearch: "部品を探す",
      ctaKicad: "KiCadプラグインを入れる",
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
    },
    kicadPlugin: {
      title: "KiCadプラグイン",
      metaDescription: "OpenParts KiCadプラグインのインストール方法",
      intro:
        "KiCadのツールバーから直接OpenPartsレジストリを検索し、部品のSymbol / Footprint / STEPモデルをプロジェクトのローカルライブラリにインストールできる、小さなネイティブGUIアプリです。スキーマティックやボードへの配置は行いません。インストール後はKiCad標準のSymbol/Footprintピッカーからいつも通り使ってください。",
      quickInstall: "クイックインストール",
      autoDetectNote:
        "どちらのスクリプトもKiCadのプラグインディレクトリを自動検出しようとします。検出できない場合はKiCadの「Tools > External Plugins > Open Plugin Directory」で場所を確認し、--plugin-dir(Linux)/ -PluginDir(Windows)で明示的に指定して再実行してください。",
      restartNote:
        "インストール後はKiCadを再起動するか、「Tools > External Plugins > Refresh Plugins」を実行し、PCBエディタのツールバーに現れる「OpenParts」ボタンを探してください。Plugin and Content Managerの「Installed」一覧には表示されません。",
      goodToKnow: "知っておくとよいこと",
      sessionNote:
        "あるKiCadセッションで初めて部品をインストールした直後は、Symbol Editor / Footprint Editor / 3Dビューアのウィンドウを一度閉じて開き直してください。KiCadはこれらのウィンドウを開いた時点でライブラリテーブルを読み込み、開いている間はライブラリテーブルの変更をホットリロードしないためです(KiCad自体の仕様です)。",
    },
    contributing: {
      title: "コントリビュート",
      metaDescription: "OpenPartsのデータを追加・修正する方法",
      intro:
        "OpenPartsの部品データは openparts-data リポジトリの正規YAMLとして管理されています。ピン配置や寸法などの誤りに気づいた場合や、新しい部品を追加したい場合は、誰でもPull Requestを送ることができます。",
      reportHeading: "個別データの問題を報告する",
      reportBodyBefore: "自分で直す時間が無い場合は、まず",
      reportBodyLink: "部品を探し",
      reportBodyAfter:
        "、詳細ページのピン表にある「このデータの問題を報告」リンクから openparts-data へIssueを作成してください。対象の部品・デバイスIDが自動的に入力されます。",
      prHeading: "新規部品を追加する / 既存データを修正する(Pull Request)",
      steps: [
        {
          before: "openparts-data を",
          linkLabel: "フォーク",
          after:
            "します。検証コマンドのビルドに openparts(コアリポジトリ)が必要なので、兄弟ディレクトリとして",
          link2Label: "openparts",
          after2: "もcloneしてください。",
        },
        {
          before: "",
          linkLabel: "Canonical Data Specification",
          after:
            "(特にRepository Layoutの節)に従い、parts/・devices/・packages/・manufacturers/・sources/配下にYAMLを追加/編集します。",
        },
        {
          before: "openparts-data のリポジトリルートで ./scripts/validate.sh を実行し、Part/Device/Package/Sourcesの参照整合性とバリデーションが通ることを確認します(このスクリプトは兄弟の ../openparts をビルドして使います)。",
        },
        {
          before: "openparts-data へPull Requestを作成します。作成後はCI上の validate ワークフローが同じ検証を自動実行します。",
        },
      ],
      specsHeading: "関連する仕様書",
    },
    search: {
      title: "部品を探す",
      metaDescription: "OpenPartsの部品を検索する",
      placeholder: "MPNまたはメーカー名で検索(例: nRF52840)",
      loading: "読み込み中...",
      searchFailed: "検索に失敗しました。",
      noResults: "該当する部品が見つかりませんでした。",
      all: "すべて",
    },
    part: {
      metaTitle: "部品詳細",
      metaDescription: "OpenPartsの部品詳細",
      loading: "読み込み中...",
      notFoundTitle: "部品が見つかりません",
      notFoundBefore: "指定された部品は見つかりませんでした。",
      notFoundLink: "検索",
      notFoundAfter: "からお探しください。",
      loadFailed: "読み込みに失敗しました。",
      artifactsFailed: "CAD成果物の情報を取得できませんでした。",
      download: "ダウンロード",
      zipBuilding: "zip作成中...",
      zipButton: "KiCad用一式をダウンロード(.zip)",
      stlButton: "STLをダウンロード",
      zipFailed: "ダウンロードに失敗しました。",
      preview3d: "3Dプレビュー",
      modelLoading: "読み込み中...",
      modelLoadFailed: "3Dモデルを表示できませんでした。",
      pinsHeading: (deviceId: string) => `ピン配置 (${deviceId})`,
      reportIssue: "このデータの問題を報告",
      pinsShow: (n: number) => `${n}ピンを表示`,
      pinNumber: "ピン番号",
      pinName: "名前",
      pinType: "種別",
      pinAltFunctions: "代替機能",
      revisions: "リビジョン",
      packageHeading: (pkgId: string) => `パッケージ (${pkgId})`,
      family: "ファミリー",
      leadCount: "リード数",
      pitch: "ピッチ",
      bodyWidth: "本体幅",
      bodyLength: "本体長",
      bodyHeight: "本体高さ",
    },
    footer: { openSource: "OpenParts is open source." },
  },
  en: {
    langLabel: "English",
    nav: {
      search: "Search",
      kicadPlugin: "KiCad Plugin",
      contributing: "Contributing",
    },
    home: {
      metaDescription: "An open, provenance-tracked electronic component data platform",
      title: "Open electronic component data, with sources you can trace",
      lead: "OpenParts generates CAD assets (KiCad symbols/footprints, STEP/STL models) reproducibly from verifiable canonical data, instead of distributing pre-made files.",
      ctaSearch: "Search parts",
      ctaKicad: "Install the KiCad plugin",
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
    },
    kicadPlugin: {
      title: "KiCad Plugin",
      metaDescription: "How to install the OpenParts KiCad plugin",
      intro:
        "A small native GUI app that searches the OpenParts registry and installs a part's Symbol / Footprint / STEP model into your project's local libraries, right from KiCad's toolbar. It doesn't place anything on the schematic or board -- use KiCad's own Symbol/Footprint picker as usual after installing.",
      quickInstall: "Quick install",
      autoDetectNote:
        "Both scripts try to auto-detect your KiCad plugin directory. If they can't, find it via KiCad's Tools > External Plugins > Open Plugin Directory, then re-run with --plugin-dir (Linux) / -PluginDir (Windows) set explicitly.",
      restartNote:
        "Afterwards, restart KiCad (or Tools > External Plugins > Refresh Plugins) and look for the new \"OpenParts\" button in the PCB editor toolbar -- not in the Plugin and Content Manager's own \"Installed\" list.",
      goodToKnow: "Good to know",
      sessionNote:
        "Right after installing a part for the first time in a KiCad session, close and reopen the Symbol Editor / Footprint Editor / 3D viewer windows. KiCad reads library tables once when each window opens and doesn't hot-reload them while it's open -- this is KiCad's own behavior.",
    },
    contributing: {
      title: "Contributing",
      metaDescription: "How to add or fix OpenParts data",
      intro:
        "OpenParts' component data is maintained as canonical YAML in the openparts-data repository. If you spot an error in a pin assignment or dimension, or want to add a new part, anyone can send a Pull Request.",
      reportHeading: "Report a problem with existing data",
      reportBodyBefore: "If you don't have time to fix it yourself, ",
      reportBodyLink: "search for the part",
      reportBodyAfter:
        ", then use the \"Report a problem with this data\" link on its pin table to open an Issue against openparts-data -- the part and device IDs are pre-filled.",
      prHeading: "Add a new part / fix existing data (Pull Request)",
      steps: [
        {
          before: "",
          linkLabel: "Fork openparts-data",
          after:
            ". The validation command needs openparts (the core repo), so also clone",
          link2Label: "openparts",
          after2: "as a sibling directory.",
        },
        {
          before: "Add or edit YAML under parts/, devices/, packages/, manufacturers/, sources/, following the ",
          linkLabel: "Canonical Data Specification",
          after: " (especially the Repository Layout section).",
        },
        {
          before: "Run ./scripts/validate.sh from openparts-data's repo root to confirm Part/Device/Package/Sources references resolve and validation passes (it builds the sibling ../openparts to do so).",
        },
        {
          before: "Open a Pull Request against openparts-data. The validate workflow runs the same checks in CI once it's up.",
        },
      ],
      specsHeading: "Related specifications",
    },
    search: {
      title: "Search parts",
      metaDescription: "Search OpenParts' component data",
      placeholder: "Search by MPN or manufacturer (e.g. nRF52840)",
      loading: "Loading...",
      searchFailed: "Search failed.",
      noResults: "No matching parts found.",
      all: "All",
    },
    part: {
      metaTitle: "Part detail",
      metaDescription: "OpenParts part detail",
      loading: "Loading...",
      notFoundTitle: "Part not found",
      notFoundBefore: "The requested part could not be found. Try ",
      notFoundLink: "searching",
      notFoundAfter: " instead.",
      loadFailed: "Failed to load.",
      artifactsFailed: "Could not fetch CAD artifact info.",
      download: "Download",
      zipBuilding: "Building zip...",
      zipButton: "Download KiCad bundle (.zip)",
      stlButton: "Download STL",
      zipFailed: "Download failed.",
      preview3d: "3D preview",
      modelLoading: "Loading...",
      modelLoadFailed: "Could not display the 3D model.",
      pinsHeading: (deviceId: string) => `Pinout (${deviceId})`,
      reportIssue: "Report a problem with this data",
      pinsShow: (n: number) => `Show ${n} pins`,
      pinNumber: "Pin",
      pinName: "Name",
      pinType: "Type",
      pinAltFunctions: "Alternate functions",
      revisions: "Revisions",
      packageHeading: (pkgId: string) => `Package (${pkgId})`,
      family: "Family",
      leadCount: "Lead count",
      pitch: "Pitch",
      bodyWidth: "Body width",
      bodyLength: "Body length",
      bodyHeight: "Body height",
    },
    footer: { openSource: "OpenParts is open source." },
  },
} as const;
