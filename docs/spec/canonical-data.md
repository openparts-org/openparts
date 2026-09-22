# OpenParts Canonical Data Specification

**Version:** 0.2  
**Status:** Draft  
**Data format:** YAML  
**Canonical repository:** `openparts-data`

---

# 1. Purpose

本仕様は、OpenParts Canonical Dataの構造と意味を定義する。

対象は主に以下である。

```text
Manufacturer
Part
Device
Package
Source
Revision
Erratum
Provenance
Relationship
```

PCB CAD、STEP等の生成済みArtifact Formatは本仕様の対象外とする。

---

# 2. Design Goals

Canonical Dataは以下を満たす必要がある。

- 人間がGit上でReviewできる
- Machine-readableである
- Stable Identifierを持つ
- Field-level provenanceを表現できる
- Revisionを表現できる
- Unknownを推測せず保持できる
- EDA/MCAD固有形式へ依存しない
- AIと人間が同一Schemaを使用できる
- Dataset全体を再構築可能である

---

# 3. General Rules

## 3.1 Encoding

UTF-8を使用する。

---

## 3.2 File Format

Canonical text representationにはYAMLを使用する。

YAMLの便利機能へ過度に依存しない。

原則として以下を避ける。

```text
anchors
aliases
custom tags
implicit application-specific types
```

単純なYAMLを優先する。

---

# 4. Required Document Header

各Canonical Documentは最低限、

```yaml
schema_version: "0.1"
kind: part
id: st/STM32G431CBT6
```

を持つ。

`schema_version`はStringとして扱う。

---

# 5. `kind`

初期Kind：

```text
manufacturer
part
device
package
source
erratum
rejected
```

KindごとにSchema Validationを行う。

---

# 6. Identifier

Canonical EntityはStable IDを持つ。

原則：

```text
ASCII
case-sensitive
path-like
human-readable
```

例：

```text
st
st/STM32G431CBT6
st/STM32G431CB
jedec/LQFP48-7x7-P0.5
st/DS12589@rev8
```

表示名と内部IDを同一概念としない。

Manufacturerが製品名表記を変更してもStable IDを不用意に変更しない。

---

# 7. Unknown Values

未知の値を推測して埋めてはならない。

値が存在しない場合は原則としてFieldを省略する。

```yaml
body_width:
  min: 6.9
  max: 7.1
  unit: mm
```

Datasheetにnominalが存在しない場合、

```yaml
nominal: 7.0
```

を勝手に生成してはならない。

`null`はSchemaが明示的に許可する場合以外使用しない。

---

# 8. Units

物理量にはUnitを明記する。

例：

```yaml
pitch:
  nominal: 0.5
  unit: mm
```

初期Canonical Unit候補：

```text
mm
um
nm

V
mV

A
mA
uA

ohm
kohm
Mohm

Hz
kHz
MHz
GHz

s
ms
us
ns

C
```

Unit StringはSchemaで管理する。

値のString埋め込み、

```yaml
width: "7.0 mm"
```

は使用しない。

---

# 9. Numeric Range

寸法やElectrical Specificationは必要に応じて、

```yaml
body_width:
  nominal: 7.0
  min: 6.9
  max: 7.1
  unit: mm
```

として表す。

すべてのFieldに`nominal/min/max`を要求しない。

---

# 10. Manufacturer

例：

```yaml
schema_version: "0.1"
kind: manufacturer
id: st

name: STMicroelectronics

website:
  url: "https://..."
```

Manufacturer Entityには製品情報を直接含めない。

---

# 11. Part

PartはOrderable MPNを表す。

```yaml
schema_version: "0.1"
kind: part
id: st/STM32G431CBT6

manufacturer: st
mpn: STM32G431CBT6

device: st/STM32G431CB
package: st/LQFP48-7x7-P0.5

existence:
  status: verified

lifecycle:
  status: active

sources:
  - st/DS12589@rev8
```

PartへPinoutやPackage dimensionsを重複保存しない。

---

# 12. Existence

初期Status：

```text
unverified
probable
verified
historical_verified
rejected
```

意味：

- `unverified`: 存在を十分確認できていない
- `probable`: 信頼できる兆候はあるがPrimary Source不足
- `verified`: Primary Source等で存在を確認済み
- `historical_verified`: 過去に存在していたことを確認済み
- `rejected`: 誤型番等と判断された

存在確認とLifecycleを混同しない。

---

# 13. Lifecycle

初期Status：

```text
pre_release
active
nrnd
eol_announced
obsolete
unknown
```

例：

```yaml
lifecycle:
  status: obsolete
  replacement:
    - st/NEWPART123
```

Replacementが完全互換であることをLifecycle Fieldだけから意味しない。

---

# 14. Device

例：

```yaml
schema_version: "0.1"
kind: device
id: st/STM32G431CB

manufacturer: st
family: STM32G4

pins:
  "1":
    name: VBAT
    type: power

  "2":
    name: PC13
    type: io
    alternate_functions:
      - RTC_OUT
      - TAMP1
```

Pin keyはStringとして扱う。

そのため以下も扱える。

```text
1
48
A1
B12
EP
TAB
```

---

# 15. Pin Type

初期Vocabulary：

```text
power
ground
input
output
io
analog
clock
reset
nc
reserved
passive
unknown
```

Manufacturer固有Categoryが必要な場合、Canonical Typeを置き換えず補助Metadataとして追加する。

---

# 16. Alternate Functions

```yaml
pins:
  "2":
    name: PC13
    type: io

    alternate_functions:
      - RTC_OUT
      - TAMP1
```

Alternate Functionの詳細SchemaはDevice Typeごとの要求が明確になった段階で拡張する。

初期段階で過度に一般化しない。

---

# 17. Device Revision

Device内にSilicon Revisionを定義できる。

```yaml
revisions:
  rev-a:
    manufacturer_revision: A

  rev-b:
    manufacturer_revision: B
```

Revision IDはOpenParts内部でStableにする。

Manufacturer Revision Labelと同一である必要はない。

---

# 18. Revision Detection

必要に応じてRevision Detection Methodを記録する。

例：

```yaml
revisions:
  rev-b:
    manufacturer_revision: B

    detection:
      - method: device_register
        register: DBGMCU_IDCODE
        mask: "0xffff0000"
        value: "0x20000000"
```

想定Method：

```text
package_marking
device_register
date_code
lot_code
manufacturer_documentation
unknown
```

---

# 19. Revision Overrides

Base Deviceとの差分をOverrideとして表現できる。

```yaml
revisions:
  rev-b:
    manufacturer_revision: B

    overrides:
      pins:
        "42":
          name: PDR_ON
```

RevisionごとにDevice全体を複製しない。

---

# 20. Package

例：

```yaml
schema_version: "0.1"
kind: package
id: st/LQFP48-7x7-P0.5

family: lqfp
lead_count: 48

pitch:
  nominal: 0.5
  unit: mm

dimensions:
  body_width:
    nominal: 7.0
    min: 6.9
    max: 7.1
    unit: mm

  body_length:
    nominal: 7.0
    min: 6.9
    max: 7.1
    unit: mm
```

Package Parameter NameはCanonical Vocabularyとして段階的に標準化する。

---

# 21. Package Geometry

Geometryの生成方法を指定できる。

```yaml
geometry:
  type: parametric
  generator: lqfp
```

初期Geometry Type：

```text
parametric
procedural
handcrafted
external
none
```

`generator`はKiCadやSTEPのGeneratorではなく、Package Geometryを構築するためのCAD-independent generatorを指す。

Generator Parameterの詳細は将来的なGeometry Specificationで規定する。

---

# 22. Footprint Information

Physical PackageとPCB Land Patternを区別する。

Canonical Packageは必要に応じてLand Pattern Recommendationを参照できるが、

```text
Package Geometry
```

と、

```text
PCB Footprint
```

を同一Data Structureへ統合しない。

Land Patternの詳細SchemaはPCB CAD Model側で定義する。

---

# 23. Source

例：

```yaml
schema_version: "0.1"
kind: source
id: st/DS12589@rev8

publisher: st
document_id: DS12589
document_type: datasheet

revision: "8"
date: "2026-03"

url: "https://..."
```

初期Document Type：

```text
datasheet
reference_manual
errata
application_note
package_drawing
product_page
pcn
eol_notice
other
```

---

# 24. Source Identity

同じDocumentの異なるRevisionは別Source Entityとして扱う。

```text
st/DS12589@rev7
st/DS12589@rev8
```

Document IDのみをProvenanceとして使用しない。

---

# 25. Provenance

ProvenanceはField単位で指定可能とする。

Canonical Data本体へSource Wrapperを大量に埋め込むのではなく、Document内の`provenance` Mapを使用する。

Path表現にはJSON Pointer形式を採用する。

例：

```yaml
provenance:
  "/dimensions/body_width":
    - source: st/DS12589@rev8
      page: 142
      locator: "Table 123, D"

  "/pins/1/name":
    - source: st/DS12589@rev8
      page: 73
      locator: "Pin definitions"
```

これにより、

```yaml
dimensions:
  body_width:
    nominal: 7.0
    unit: mm
```

自体を単純に保てる。

---

# 26. Provenance Rules

重要なEngineering Factは可能な限りProvenanceを持つ。

特に以下では強く要求する。

```text
MPN existence
Pin number
Pin name
Package assignment
Critical dimensions
Silicon revision
Errata applicability
Lifecycle
Replacement
```

同じFieldについて複数Sourceを持ってよい。

```yaml
provenance:
  "/pins/1/name":
    - source: st/DS12589@rev8
      page: 73

    - source: st/RM0440@rev5
      page: 91
```

---

# 27. Conflicting Sources

Source間で情報が競合する場合、AIやContributorが勝手に一方を選択してはいけない。

必要に応じて、

- Issueを作成する
- `unknown`として扱う
- Revision differenceを調査する
- Erratumとして記録する

等を行う。

競合そのものを隠さない。

---

# 28. Erratum

Erratumを独立Entityとして保持できる。

```yaml
schema_version: "0.1"
kind: erratum
id: st/STM32G431/ES0431/item-2.3.4

device: st/STM32G431CB

category: silicon_erratum
hardware_changed: true

applies_to:
  silicon_revisions:
    - rev-a

description: >
  Short normalized description of the issue.

sources:
  - st/ES0431@rev6
```

---

# 29. Erratum Category

初期Category：

```text
silicon_erratum
documentation_erratum
datasheet_clarification
package_documentation_erratum
unknown
```

---

# 30. Hardware Change

Document correctionと物理Hardware差異を区別する。

```yaml
hardware_changed: true
```

```yaml
hardware_changed: false
```

不明ならFieldを省略するか、Schema上明示的に`unknown`を許可する。

---

# 31. Applicability

ErratumやRevision情報には適用範囲を持たせられる。

例：

```yaml
applies_to:
  silicon_revisions:
    - rev-a

  packages:
    - st/LQFP48-7x7-P0.5
```

将来的にDate CodeやLot Rangeも追加可能とする。

---

# 32. Relationships

Entity間Relationを表現可能とする。

初期Relation例：

```text
replaces
replaced_by
variant_of
package_variant_of
related
```

完全互換性を意味するRelation名を安易に追加しない。

互換性は別途複数軸で評価する。

---

# 33. Rejected Records

誤型番や存在しないと判断されたCandidateを完全削除せずTombstoneとして保持できる。

例：

```yaml
schema_version: "0.1"
kind: rejected
id: rejected/st/STM32G431CBT66

claimed_mpn: STM32G431CBT66
manufacturer: st

reason: no_manufacturer_evidence

sources_checked:
  - st/product-catalog@2026-09
```

これにより同じ誤情報の再登録を防ぐ。

---

# 34. Repository Layout

推奨Layout：

```text
openparts-data/
├── manufacturers/
│
├── parts/
│   └── st/
│       └── stm32/
│           └── g4/
│
├── devices/
│   └── st/
│
├── packages/
│   ├── standards/
│   └── manufacturers/
│
├── sources/
│   └── st/
│
├── errata/
│   └── st/
│
└── rejected/
```

大量のMPNを一つのDirectoryへ置かない。

---

# 35. Duplication Policy

同じEngineering Factを複数Entityへコピーしない。

例：

```text
Part
 ├─ references Device
 └─ references Package
```

とし、PartへDevice PinoutやPackage Dimensionsを複製しない。

---

# 36. Canonical vs Derived Data

Canonical Dataへ保存するもの：

```text
Engineering facts
Source references
Revision relationships
Lifecycle
Provenance
Geometry parameters
```

Derived Dataとして生成するもの：

```text
KiCad symbol
KiCad footprint
STEP
STL
glTF
Search index
PostgreSQL rows
Rendered documentation
```

Derived DataをCanonical Sourceにしてはならない。

---

# 37. Validation

Canonical Datasetは最低限以下を検証する。

```text
YAML parse
Schema validation
ID uniqueness
Reference existence
MPN consistency
Pin consistency
Dimension sanity
Revision references
Source references
Provenance paths
Geometry compatibility
```

Schema Validationを通ることとEngineering Factが正しいことは同一ではない。

具体的な検証段階、基準データ、PRの採否条件は[Testing and Quality Specification](testing-and-quality-specification.md)に従う。

構文・Schema、参照整合性、技術的整合性、Provenance構造、一次資料との一致を区別する。Validator通過だけで技術値を`verified`へ昇格させない。

UnknownはSchemaに従って保持し、必要情報が不足する操作を停止する。共有Entityの変更は依存するEntityも検証する。各検証ルールには正常例と異常例を用意し、実部品の基準データと架空のFixtureを分離する。

---

# 38. Schema Evolution

すべてのDocumentは`schema_version`を持つ。

Breaking Schema ChangeではVersionを変更する。

Schema Migration Toolを将来的に提供可能とする。

Dataset RevisionとSchema Versionを混同しない。

---

# 39. AI Metadata Boundary

AIのModel名、Prompt、Agent Run ID等を通常のCanonical Engineering Factへ埋め込まない。

Canonical Dataには、

> 採用されたEngineering FactとそのSource

を保存する。

AIによる抽出履歴はCandidate/Contribution MetadataとしてAgent Ingestion Specification側で管理する。

Git historyにより最終変更履歴も追跡できる。

---

# 40. Fundamental Rule

Canonical Dataは、

> OpenPartsが知っていること

だけでなく、

> OpenPartsがその情報をなぜ知っているのか

を機械可読に表現しなければならない。

Canonical Dataにおいて、推測された完全なDataより、

**不完全だが出典が明確なDataを優先する。**
