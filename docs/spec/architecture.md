# OpenParts Architecture Specification

**Version:** 0.8  
**Status:** Draft  
**Project:** OpenParts  
**Primary implementation language:** Rust

---

# 1. Purpose

OpenPartsは、電子部品について公開されているEngineering Factsを、出典付き・Revision-aware・機械可読なCanonical Dataとして管理し、そのデータからEDA/MCAD資産を再現可能に生成・配信するオープンな基盤である。

OpenPartsの目的はCADファイルを収集することではない。

```text
Manufacturer Documentation
          ↓
   Engineering Facts
          ↓
    Canonical Data
          ↓
      OpenParts
          ↓
 ┌────────┼────────┐
 ▼        ▼        ▼
PCB CAD  MCAD   Distribution
```

最終的には、

> MPNを指定すると、その部品について検証可能な技術情報と、それに基づいて再生成可能なCAD資産を取得できる

状態を目指す。

---

# 2. Scope

OpenPartsが扱う主な領域は以下である。

- Manufacturer
- Orderable Part / MPN
- Device
- Pinout
- Package
- Dimensions
- Silicon Revision
- Package Revision
- Datasheet Revision
- Errata
- Lifecycle
- Provenance
- PCB CAD Symbol
- PCB Footprint
- Mechanical Geometry
- CAD Artifact
- Component dependency lock

Canonical Dataの詳細な表現方法は **OpenParts Canonical Data Specification** で規定する。

AIによる探索・抽出・候補作成は **OpenParts Agent Ingestion Specification** で規定する。

---

# 3. Non-Goals

OpenPartsは少なくとも初期段階では以下を目的としない。

- 世界中の全電子部品を即座に登録すること
- Manufacturer CAD libraryのMirror
- Datasheet PDFそのものの再配布
- Manufacturer logoの再現
- フォトリアルな製品外観の完全再現
- 不明情報の推測による補完
- 特定EDA形式をCanonical Formatとすること
- 一般利用者へ全データRepositoryのcloneを要求すること
- AIが検証を経ずCanonical Dataを直接更新すること
- 初期段階からMicroservicesへ細分化すること

---

# 4. Design Principles

## 4.1 Engineering Knowledge is Canonical

OpenPartsで保存すべき中心情報は、生成済みCADファイルではなく、それを再構築できるEngineering Knowledgeである。

```text
Engineering Facts
      +
Provenance
      +
Revision
      ↓
Effective Model
      ↓
Domain Model
      ↓
CAD Artifact
```

---

## 4.2 Provenance First

技術値は可能な限り、その根拠まで追跡できなければならない。

```text
Fact
 ↓
Source Document
 ↓
Revision
 ↓
Page / Locator
```

「値が正しい」だけでなく、

> なぜOpenPartsがその値を採用しているのか

を追跡可能にする。

---

## 4.3 Unknown is Better Than Invented

Datasheet等から確認できない値を推測してCanonical Dataへ追加しない。

不明・未確認・競合がある状態そのものを表現可能にする。

---

## 4.4 Generated First

一般的なPackageやCAD資産については、生成可能なものをBinary Assetとして保存しない。

```text
Canonical Data
+
Generator
=
Artifact
```

Handcrafted assetは必要な場合のみ使用する。

---

## 4.5 Historical Preservation

以下を最新版で上書きして消さない。

- Obsolete Part
- Old Silicon Revision
- Old Package Revision
- Superseded Datasheet
- Historical Errata

修理、Legacy System、再現性のためにHistorical Dataを保持する。

---

## 4.6 CAD Independence

Canonical Modelを特定CAD形式へ依存させない。

```text
                  Canonical Model
                        │
             ┌──────────┴──────────┐
             ▼                     ▼
        PCB CAD Model         MCAD Model
             │                     │
      ┌──────┼──────┐        ┌─────┼────┐
      ▼      ▼      ▼        ▼     ▼    ▼
   KiCad LibrePCB Altium    STEP   STL glTF
```

---

## 4.7 Git is the Review Database, Not the Runtime Database

Gitは以下の用途へ使用する。

- Canonical Data
- Review
- History
- Contribution
- Audit
- Dataset release

大量検索や通常配信にはDatabase/APIを使用する。

---

## 4.8 Server is Optional

OpenPartsのデータ検証とCAD生成はServerなしでも実行できなければならない。

```text
openparts-data
      +
openparts
      ↓
Validation
Generation
Offline Use
```

`openparts-server`はDistribution Layerである。

---

# 5. Repository Architecture

初期の主要Repositoryは3つとする。

```text
OpenParts Organization
│
├── openparts
│   └── Core libraries / Validators / Generators / Client / CLI / Skills
│
├── openparts-data
│   └── Canonical Engineering Data
│
└── openparts-server
    └── API / Database / Search / Artifact Cache / Deployment
```

必要になった場合のみ以下を追加する。

```text
openparts-web
```

EDAごと、出力形式ごとにGit Repositoryを分割することは初期設計では行わない。

---

# 6. `openparts` Repository

`openparts`はRust Workspaceとする。

```text
openparts/
│
├── Cargo.toml
├── Cargo.lock
│
├── crates/
│   ├── openparts-core/
│   ├── openparts-data/
│   ├── openparts-validator/
│   ├── openparts-client/
│   ├── openparts-cli/
│   │
│   ├── pcbcad/
│   │   ├── openparts-pcbcad/
│   │   ├── openparts-kicad/
│   │   ├── openparts-librepcb/
│   │   └── openparts-altium/
│   │
│   └── mcad/
│       ├── openparts-mcad/
│       ├── openparts-step/
│       ├── openparts-stl/
│       └── openparts-gltf/
│
├── skills/
├── schemas/
├── tests/
└── docs/
```

`pcbcad/`と`mcad/`は整理用Directoryであり、crateではない。

`openparts-*` DirectoryをRust crateとする。

すべてのAdapterを初期リリースで実装する必要はない。

---

# 7. Core Crates

## 7.1 `openparts-core`

OpenPartsのCanonical Rust Modelを提供する。

以下へ依存してはならない。

- YAML
- HTTP
- PostgreSQL
- KiCad
- STEP
- glTF
- Server implementation

概念的には以下を扱う。

```text
Manufacturer
Part
Device
Pin
Package
Revision
Erratum
Source
Provenance
Effective Model
```

---

## 7.2 `openparts-data`

Canonical YAMLと`openparts-core` Model間の読み書きを担当する。

```text
Canonical YAML
      ↓
openparts-data
      ↓
openparts-core
```

Canonical YAMLの意味を独自に定義せず、Canonical Data Specificationへ従う。

---

## 7.3 `openparts-validator`

以下を検証する。

- Schema
- Referential integrity
- Identifier consistency
- Pin consistency
- Package dimensions
- Revision relationships
- Provenance
- Geometry generation
- PCB CAD generation
- MCAD generation

ValidatorはAI出力、人間によるContribution、Server Importのすべてで共通利用する。

---

## 7.4 `openparts-client`

Serverとの通信を抽象化する。

担当範囲：

- Search
- Component retrieval
- Artifact retrieval
- Cache
- Hash validation
- Lock resolution
- Offline fallback

CLIやEDA Integrationが独自のHTTP処理を重複実装しないようにする。

---

## 7.5 `openparts-cli`

人間およびAutomation向けの標準Command Interfaceを提供する。

概念例：

```text
openparts search STM32G431
openparts show st/STM32G431CBT6
openparts validate
openparts generate st/STM32G431CBT6 --kicad --target 8
openparts generate st/STM32G431CBT6 --step
openparts install
openparts update
openparts bundle
```

正確なCLI仕様は実装段階で別途定義する。

---

# 8. Domain Model

OpenPartsの中心的な関係は以下とする。

```text
Manufacturer
     │
     ▼
    Part
     │
 ┌───┴─────────────┐
 ▼                 ▼
Device            Package
 │                 │
 ▼                 ▼
Silicon         Package
Revision        Geometry
 │                 │
 └───────┬─────────┘
         ▼
   Effective Model
```

## 8.1 Part

Partは実際に注文可能、または歴史的に存在したMPNを表す。

Partは主に、

```text
Manufacturer
MPN
Device reference
Package reference
Lifecycle
Existence
```

を保持する。

DeviceやPackageの内容をPartへ複製しない。

---

## 8.2 Device

Deviceは論理的・電気的なデバイスを表す。

複数MPNが同じDeviceを参照できる。

Deviceは主に、

- Pinout
- Electrical roles
- Alternate functions
- Silicon revisions

を表す。

---

## 8.3 Package

Packageは物理的なPackage Definitionを表す。

Deviceとは独立させる。

```text
Device + Package = Orderable Part configuration
```

---

# 9. Revision Model

以下のRevisionを区別する。

```text
Silicon Revision
Package Revision
Document Revision
OpenParts Data Revision
Generator Version
Artifact Revision
CAD Format Version
```

これらを単一の`version`へまとめない。

---

# 10. Effective Model

CAD生成や利用者向け情報表示では、Base DefinitionへRevision情報を適用したEffective Modelを使用する。

```text
Base Device
+
Silicon Revision
+
Revision Overrides
+
Package
+
Package Revision
        ↓
Effective Model
```

CAD GeneratorはRaw YAMLを直接処理せず、Effective Modelを入力とする。

---

# 11. Compatibility

Revision間の互換性は単一Scoreではなく、複数軸で扱う。

```text
pinout
footprint
mechanical
electrical
```

各軸は例えば、

```text
compatible
incompatible
unknown
```

で表す。

---

# 12. PCB CAD Architecture

PCB CAD共通表現を`openparts-pcbcad`が提供する。

```text
Effective Model
      ↓
openparts-pcbcad
      ↓
PCB CAD Intermediate Representation
      │
 ┌────┼──────────┐
 ▼    ▼          ▼
KiCad LibrePCB Altium
```

共通表現の例：

```rust
pub struct PcbSymbol {
    pub units: Vec<SymbolUnit>,
    pub pins: Vec<SymbolPin>,
    pub graphics: Vec<Graphic>,
}

pub struct PcbFootprint {
    pub pads: Vec<Pad>,
    pub graphics: Vec<Graphic>,
    pub courtyard: Option<Courtyard>,
}
```

EDA固有概念は可能な限りAdapter側へ閉じ込める。

---

# 13. KiCad Adapter

KiCad Supportは、

```text
crates/pcbcad/openparts-kicad/
```

へ実装する。

概念構造：

```text
openparts-kicad/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── version.rs
│   ├── symbol/
│   └── footprint/
│
└── tests/
    └── golden/
        ├── v6/
        ├── v7/
        ├── v8/
        └── v9/
```

KiCad Versionごとに最初からcrateを分けない。

---

# 14. CAD Format Versioning

Application VersionとFile Format Versionを同一概念として扱わない。

利用者はTarget Applicationを指定できる。

```text
KiCad Target
     ↓
Format Resolver
     ↓
Serializer
```

複数Application Versionで同じSerializerを共有してもよい。

Format差異はGolden Testで検証する。

---

# 15. MCAD Architecture

MCAD共通表現を`openparts-mcad`が提供する。

```text
Package
   ↓
Geometry Generator
   ↓
Mechanical Geometry
   │
 ┌─┼─────┐
 ▼ ▼     ▼
STEP STL glTF
```

STEP自体をCanonical Geometryとはしない。

STEP/STL/glTFはMechanical Geometryから派生するArtifactである。

---

# 16. Canonical Data Repository

`openparts-data`はEngineering Factsを保持する。

概念構造：

```text
openparts-data/
├── manufacturers/
├── parts/
├── devices/
├── packages/
├── sources/
├── errata/
└── rejected/
```

Generated CAD Artifactは通常保存しない。

データ形式、Identifier、Provenance等はCanonical Data Specificationで定義する。

---

# 17. AI-Assisted Data Ingestion

大規模なComponent Databaseを人間だけで構築することを前提としない。

OpenPartsはAI Agentによる、

```text
Discover
Extract
Normalize
Compare
Validate
Propose
```

を正式なData Ingestion Pipelineとして想定する。

ただし、

```text
AI Agent
   ↓
Candidate Change
   ↓
Validator
   ↓
Review
   ↓
Canonical Data
```

とし、AgentがProduction Canonical Dataを直接書き換える構造にはしない。

Agent/Skillの詳細はAgent Ingestion Specificationで規定する。

---

# 18. Skills

AI Research Skillは初期段階では`openparts` Repositoryの、

```text
skills/
```

以下に置く。

想定されるSkill：

```text
discover-part
find-primary-source
extract-pinout
extract-package
extract-dimensions
find-revisions
find-errata
compare-document-revisions
validate-provenance
propose-canonical-data
```

SkillはCanonical Data SchemaとValidatorを共通Interfaceとして利用する。

Agent Systemが十分大規模になった場合のみRepository分離を検討する。

---

# 19. Community Workflow

人間・AIを問わずCanonical Data変更は同じReview Pathへ収束させる。

```text
Discovery
   ↓
Candidate Change
   ↓
Pull Request
   ↓
CI
   ↓
Review
   ↓
Merge
   ↓
Dataset Build
   ↓
Server Import
```

Canonical Repositoryへ直接書き込む特別経路を作らない。

---

# 20. `openparts-server`

Serverは独立Repositoryとする。

```text
openparts-server/
├── Cargo.toml
├── Cargo.lock
├── src/
├── migrations/
├── config/
├── container/
│   └── Containerfile
├── deploy/
│   └── quadlet/
├── tests/
└── docs/
```

Serverの責務：

- API
- Search
- PostgreSQL
- Artifact cache
- Object storage integration
- Import
- Rate limiting
- Metrics
- Service deployment

Canonical Data編集機能はServerの責務に含めない。

---

# 21. Podman Deployment

OpenParts Serverの公式Container Runtimeは **Podman** を第一対象とする。

Container imageはOCI互換とし、Runtime固有依存は可能な限り限定する。

Production Deploymentの基本形：

```text
                 systemd
                    │
                 Quadlet
                    │
                  Podman
        ┌───────────┼───────────┐
        ▼           ▼           ▼
 OpenParts Server PostgreSQL Object Storage
```

Container定義には`Containerfile`を使用する。

`docker/`という名称をRepository構造へ使用せず、Container Runtime非依存な、

```text
container/
```

を使用する。

Deploymentの詳細は将来的に独立したDeployment Specificationへ分離する。

---

# 22. Production Data Flow

```text
openparts-data
Canonical Data
      │
      ▼
 Dataset Validation
      │
      ▼
    Importer
      │
      ▼
 PostgreSQL
      │
      ▼
openparts-server
      │
 ┌────┼────────┐
 ▼    ▼        ▼
CLI   Web   EDA Integration
```

Production DatabaseはDerived DataでありCanonical Sourceではない。

---

# 23. Artifact Generation

```text
Request
   ↓
Part Resolution
   ↓
Effective Model
   ↓
Domain IR
   ↓
Format Adapter
   ↓
Artifact
   ↓
Content Hash
```

Artifact Metadataには少なくとも以下を関連付けられるようにする。

```text
Generator version
Dataset revision
Schema version
Target format
Target format version
Content hash
```

---

# 24. Content Addressing

Generated ArtifactはContent Hashによって識別可能にする。

初期方式としてSHA-256を想定する。

```text
sha256:<digest>
```

同じ入力とGenerator Versionから同じArtifactを再生成できることを目標とする。

---

# 25. Client Cache

一般利用者は全Datasetを保持しない。

```text
Server
  ↓
Required Component Data
  ↓
Local Cache
```

概念例：

```text
~/.cache/openparts/
├── metadata/
├── components/
└── artifacts/
```

DownloadしたArtifactはHashを検証する。

---

# 26. Project Manifest and Lockfile

Projectが利用する部品をManifestへ記録できるようにする。

```toml
[[parts]]
manufacturer = "st"
mpn = "STM32G431CBT6"
```

Resolution結果はLockfileへ固定できるようにする。

Lock対象には必要に応じて、

```text
Part revision
Silicon revision
Package revision
Dataset revision
Generator version
CAD target version
Artifact hashes
```

を含める。

OpenParts Lockfileは、

> 電子部品とCAD生成環境を再現するための固定点

として設計する。

---

# 27. Offline Use

Cache済みComponent、Dataset Snapshot、Project Bundleを利用してOffline動作できることを目標とする。

Bulk Distribution Formatとして将来的に、

```text
SQLite
DuckDB
Parquet
Compressed JSON
```

等を利用可能とする。

Git cloneを唯一のOffline方法とはしない。

---

# 28. Verification Model

単一の「信頼度Score」は採用しない。

VerificationはDomainごとに独立させる。

例：

```text
Existence       verified
Pinout          verified
Package         verified
Footprint       probable
Mechanical      verified
Visual model    unavailable
```

異なる種類の不確実性を一つの数値へ潰さない。

---

# 29. Legal and Source Policy

OpenPartsでは原則として、

```text
Manufacturer Primary Documentation
             ↓
       Engineering Facts
             ↓
      Canonical Open Data
             ↓
       Own Generators
```

という経路を使用する。

Canonical Repositoryへ通常保存しないもの：

- Manufacturer Datasheet PDF
- Datasheet screenshot
- Manufacturer logo
- License不明なCAD asset
- License不明なSTEP model

Factual technical informationと、その表現物の著作権を区別して設計する。

---

# 30. Licensing

以下は別々に決定する。

```text
Software License
Database License
Generated Artifact License
Contribution Terms
```

一つのLicenseですべてを扱うことを前提としない。

---

# 31. Versioning

独立して管理する。

```text
Software Version
Canonical Schema Version
Dataset Revision
API Version
Generator Version
CAD Format Version
Artifact Hash
```

SoftwareにはSemantic Versioningを基本とする。

DatasetにはDataset RevisionまたはCalVer方式を採用できる。

---

# 32. Initial Implementation Scope

最初のVertical Sliceでは以下へ集中する。

```text
Canonical YAML
      ↓
Parser
      ↓
Validator
      ↓
Effective Model
      │
 ┌────┴────┐
 ▼         ▼
KiCad     STEP
```

初期Package：

```text
QFN
LQFP
```

初期Component：

```text
MCU
Common IC
```

初期データ規模は正確な部品数よりも、

- 複数Manufacturer
- DeviceとMPNの共有
- Revision
- Provenance
- CAD生成

を一通り実証できることを優先する。

---

# 33. Scaling Principle

MPN数と実装量は比例しない。

```text
Many MPNs
   ↓
Fewer Devices
   ↓
Fewer Package Definitions
   ↓
Small Number of Reusable Generators
```

Device Family、Package Definition、Generatorを共有することで大規模化する。

AI Agentも個々のMPNを独立に処理するのではなく、Family単位の共有構造を利用する。

---

# 34. Specification Set

OpenPartsの仕様書を以下のように分離する。

```text
docs/
└── spec/
    ├── architecture-specification.md
    ├── canonical-data-specification.md
    ├── agent-ingestion-specification.md
    └── testing-and-quality-specification.md
```

将来的に必要になった場合、以下を追加する。

```text
api.md
geometry.md
deployment.md
lockfile.md
contribution.md
```

各文書は責務を越えて詳細を重複記述しない。

コード、Canonical Dataset、CAD Artifact、AI Researchのテスト・品質要件は[Testing and Quality Specification](testing-and-quality-specification.md)で定義する。

Validator通過、一次資料との一致、生成CADの利用可能性を別々に確認する。CI・Releaseではコード、Dataset、Schema、生成環境の組合せを固定し、必須検証の失敗や未実施を成功として扱わない。

---

# 35. Fundamental Principle

OpenPartsの中心思想は次の通りである。

> CADファイルを集めるのではなく、CADを再構築できるEngineering Knowledgeを保存する。

> 値だけでなく、その値の根拠を保存する。

> 最新情報だけでなく、その情報がどのRevisionへ適用されるのかを保存する。

> AIを使ってデータ作成を高速化しても、検証可能性を失わない。

```text
Engineering Facts
       +
Provenance
       +
Revision
       ↓
Effective Model
       ↓
Domain Representation
       ↓
Format Adapter
       ↓
Reproducible Artifact
```
