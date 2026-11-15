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

Electrical parameters (section 11.1) use a per-quantity unit table
with SI prefixes (`nF`, `kΩ`, `ppm/°C`); their values normalise to SI.

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

## 11.1 Electrical Parameters

A Part may carry a `parameters` map of electrical facts of that
orderable MPN (capacitance, rated voltage, resistance, ...). It is
optional: an absent key means Unknown (section 7), never a default.

```yaml
parameters:
  capacitance:
    nominal: 100
    unit: nF
  tolerance:
    min: -10
    max: 10
    unit: "%"
  rated_voltage:
    nominal: 50
    unit: V
  dielectric:
    value: X7R

provenance:
  "/parameters/capacitance":
    - source: murata/GRM155R71C104KA88
      page: 1
      locator: "Specifications table, Capacitance: 0.10µF ±10%"
```

**Part vs Device.** Parameters are per MPN, so they live on the Part.
A Device may also carry `parameters` for a fact every one of its Parts
shares (a memory's capacity, a fixed regulator's output voltage when
the Device is one voltage option). The value in force for a Part is the
Device's value overlaid by the Part's own: a Part key replaces the
Device key of the same name entirely (no field-by-field merge). Record
a fact once, at the level the source states it for; don't copy a
Device value into each Part.

**Closed, typed vocabulary.** Keys come from a closed vocabulary
(`openparts_core::PARAMETERS`). Each key has exactly one shape:

- *numeric*: `nominal`/`min`/`max` + `unit`, like a Dimension (section
  9). Only what the source states is recorded; a `nominal` is never
  computed from `min`/`max` (section 7). `min <= nominal <= max`.
- *enumerated*: `value`, one of the key's listed spellings
  (case-sensitive).

Each numeric key has a fixed physical quantity; its value normalises to
that quantity's SI base unit. Accepted units (case-sensitive: `m` is
milli-, `M` mega-):

| Quantity | SI | Accepted units |
|---|---|---|
| capacitance | F | `pF` `nF` `uF` `µF` `mF` `F` |
| resistance | Ω | `mΩ` `mohm` `Ω` `ohm` `kΩ` `kohm` `MΩ` `Mohm` |
| inductance | H | `nH` `uH` `µH` `mH` `H` |
| voltage | V | `uV` `µV` `mV` `V` `kV` |
| current | A | `nA` `uA` `µA` `mA` `A` |
| power | W | `uW` `µW` `mW` `W` |
| frequency | Hz | `Hz` `kHz` `MHz` `GHz` |
| ratio | 1 | `%` `ppm` |
| temperature coefficient | 1/K | `ppm/K` `ppm/°C` `%/K` `%/°C` |
| length | m | `nm` `um` `µm` `mm` `m` |
| force | N | `mN` `N` `gf` (= 9.80665 mN) |
| information | bit | `bit` `Kbit` `kbit` `Mbit` `Gbit` (binary: 2^10, 2^20, 2^30, as memory datasheets use them) |

Vocabulary (key: quantity or values; categories it is plausible for):

| Key | Shape | Plausible for |
|---|---|---|
| `capacitance` | capacitance | capacitor |
| `tolerance` | ratio, deviation | capacitor, resistor, inductor |
| `capacitance_tolerance` | capacitance, deviation (absolute, e.g. ±0.25 pF) | capacitor |
| `rated_voltage` | voltage | capacitor, fuse, resistor |
| `dielectric` | `C0G` `NP0` `U2J` `X5R` `X5S` `X6S` `X6T` `X7R` `X7S` `X7T` `X8L` `X8R` `Y5V` `Z5U` | capacitor |
| `esr` | resistance | capacitor, crystal |
| `resistance` | resistance | resistor |
| `power_rating` | power | resistor, diode, LED |
| `temperature_coefficient` | temperature coefficient, deviation | resistor |
| `inductance` | inductance | inductor |
| `rated_current` | current | inductor, fuse |
| `saturation_current` | current | inductor |
| `dcr` | resistance | inductor |
| `frequency` | frequency | crystal |
| `load_capacitance` | capacitance | crystal |
| `frequency_tolerance` | ratio, deviation | crystal |
| `forward_voltage` | voltage (an optocoupler's: its input LED) | diode, LED, optocoupler |
| `reverse_voltage` | voltage (an optocoupler's: its input LED) | diode, LED, optocoupler |
| `forward_current` | current (an optocoupler's: its input LED) | diode, LED, optocoupler |
| `zener_voltage` | voltage (Vz at the test current; nominal/min/max as printed) | diode |
| `zener_test_current` | current (Izt, the current Vz is specified at) | diode |
| `color` | `red` `orange` `amber` `yellow` `green` `blue` `white` `infrared` `ultraviolet` `rgb` | LED |
| `wavelength` | length (dominant wavelength; peak only if that is all the source gives -- say so in the locator) | LED |
| `polarity` | `npn` `pnp` `n_channel` `p_channel` | transistor |
| `drain_source_voltage` | voltage (Vds max) | transistor |
| `drain_current` | current (Id continuous) | transistor |
| `rds_on` | resistance | transistor |
| `gate_threshold_voltage` | voltage (Vgs(th)) | transistor |
| `collector_emitter_voltage` | voltage (Vceo; an optocoupler's: its output transistor) | transistor, optocoupler |
| `collector_current` | current (Ic; an optocoupler's: its output transistor) | transistor, optocoupler |
| `isolation_voltage` | voltage (input-output isolation, BVs; the rms/duration condition goes into the locator) | optocoupler |
| `current_transfer_ratio` | ratio (CTR = Ic/IF, in `%`; a range, not a deviation) | optocoupler |
| `output_type` | `fixed` `adjustable` | `power` |
| `output_voltage` | voltage | `power` |
| `input_voltage_max` | voltage | `power` |
| `output_current` | current (max) | `power` |
| `dropout_voltage` | voltage | `linear_regulator` |
| `capacity` | information | `memory` |
| `actuation_force` | force | `switch` |
| `travel` | length | `switch` |

A group in the last column covers its subcategories (section 14.1);
`rated_voltage` and `rated_current` are also plausible for `switch`
(contact ratings).

A *deviation* key (a tolerance) records signed bounds: ±10 % is
`min: -10, max: 10, unit: "%"`; an asymmetric -20/+80 % is
`min: -20, max: 80`. It is compared and filtered by its larger
magnitude (10 %).

A rating with conditions (power at 70 °C, current at a temperature
rise) records the number; the condition goes into the provenance
`locator`. When a value depends on a condition the Part does not fix
(a different power per operation mode, a range across output options),
leave the key out rather than pick one.

Mounting style is not a parameter: it is derived from the Package
geometry (SMD/THT) and never duplicated.

**Provenance is mandatory per value.** Every key has a provenance entry
at `/parameters/<key>` in the same document. Two rules for where values
may come from:

1. Never derive a value by decoding an MPN, unless the datasheet's
   ordering-code (part numbering) table explicitly maps that code to the
   value. Then cite that table as the provenance (page + table name +
   the code: `"ORDERING INFORMATION, (2) TOLERANCE: B = ±0.1%"`).
2. When one datasheet covers many MPNs, cite the row: the table and the
   row or column naming this MPN (or its value), e.g.
   `"Table 3, RT0805 column, Standard: 1/8W"`.

**Validation** (openparts-validator): an unknown key, a unit
incompatible with the key's quantity, the wrong shape for the key or an
enumerated value outside its list, `min <= nominal <= max` not holding,
and a missing `/parameters/<key>` provenance entry are errors
(`parameter.unknown_key`, `parameter.unit`, `parameter.invalid`,
`parameter.out_of_range`, `provenance.missing`). A key that is unusual
for the Device's category (capacitance on a resistor) is a warning
(`parameter.implausible`).

The vocabulary grows like `PinType` (section 15): a new component type
adds keys here, in `PARAMETERS`, in the web catalog's labels and in the
alternatives' comparison rules (`PARAMETER_RULES`: whether a value must
be equal, at least or at most the original's;
[Alternative Parts and BOM Checking](alternatives-and-bom.md) section
1.4), in one change. A key is never reused for another quantity.

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

Alternatives (`GET /v1/parts/{manufacturer}/{mpn}/alternatives`) list a
`replacement` first and still check its land pattern, pinout and
parameters ([Alternative Parts and BOM Checking](alternatives-and-bom.md)
section 1).

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

ソースが「Tabはpin 2に接続」のように内部接続を明記している場合、そのPinに`connected_to`で接続先のPin番号を記録する。内部接続されたPinは同一端子として扱い、同じPin名（例：`COLLECTOR`）を重複して使用できる。接続先は同じDevice内の別Pinでなければならない。

```yaml
pins:
  "2": { name: COLLECTOR, type: passive }
  "4": { name: COLLECTOR, type: passive, connected_to: "2" }
```

## 14.1 Category

Every Device states a `category` (required; no default -- section 7).
It classifies the component type for catalog browsing, not for
engineering decisions.

```yaml
category: linear_regulator
```

The vocabulary is a two-level tree: top-level categories, some of them
*groups* with *subcategories*. Every node is a valid value. A Device
states the most specific node its own datasheet supports (its title or
description, e.g. "800-mA Low-Dropout Linear Regulator" ->
`linear_regulator`); the tree supplies the group, so data never states
both and they can't disagree. A group is used as a value only when none
of its subcategories fits (a sensor of a kind not listed is `sensor`).

| Top level | Subcategories |
|---|---|
| `microcontroller` | |
| `capacitor` | |
| `resistor` | |
| `inductor` | |
| `crystal` | |
| `diode` | |
| `transistor` | |
| `led` | |
| `optocoupler` | |
| `fuse` | |
| `connector` | |
| `switch` | `tact_switch` |
| `power` | `linear_regulator`, `switching_regulator`, `battery_charger`, `voltage_reference`, `power_switch` (load switches, power muxes) |
| `memory` | `flash`, `eeprom`, `sram` |
| `sensor` | `pressure_sensor`, `imu` (accelerometers, gyroscopes, IMUs), `temperature_sensor` |
| `analog` | `op_amp`, `comparator`, `current_sense_amplifier`, `power_monitor` (current/power monitors with a digital interface), `analog_switch` (analog switches, multiplexers), `adc`, `dac` |
| `interface` | `io_expander`, `can_controller`, `can_transceiver`, `rs485_transceiver`, `rs232_transceiver`, `level_shifter` |
| `logic` | (gates, buffers, bus transceivers, shift registers, decoders) |
| `timing` | `timer`, `rtc` |
| `driver` | `gate_driver`, `darlington_array` |
| `other` | |

Why a tree in one field rather than `category` + `subcategory`: a
single value can't contradict itself, existing flat values
(`capacitor`, ...) stay valid unchanged, and a later split of a
top-level value (say `capacitor` > `mlcc`) needs no migration -- the old
value stays a valid, less specific node.

`other` is for a real component type the tree doesn't have yet; the
validator warns on it (`category.other`). Grow the tree instead: add
the node here, in `openparts_core::ComponentCategory` (with its parent)
and in the web catalog's labels, in one change. Never rename or remove
a node that data uses.

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

`lead_layout`（省略可）は、`lead_count`だけでは物理的なピン配置（どちら側に何本か）
を一意に決められないPackage Family向けの明示フィールドである。例えばSOT-23のような
非対称2辺配置は、物理形状自体はJEDEC標準でほぼ共通だが、どちら側から採番するかは
ベンダーによって流儀が分かれる（統一規格ではない）。このようなFamilyでは、Geometry
Generatorが黙って多数派の慣習を仮定することはせず、`lead_layout`が無ければ生成を
停止する（Testing and Quality Specification: 一意に安全な生成結果を決められない
操作は停止する）。

```yaml
schema_version: "0.1"
kind: package
id: standards/SOT23-3

family: sot
lead_count: 3
lead_layout: [2, 1]   # pin 1-2側の本数, pin 3側の本数 (pin番号順)

pitch:
  nominal: 0.95
  unit: mm

dimensions:
  body_width:
    nominal: 1.3
    unit: mm
  body_length:
    nominal: 2.9
    unit: mm
```

`lqfp`/`qfn`/`soic`のように`lead_count`から均等に辺数で割り切れるFamilyは
`lead_layout`を必要としない。`bga`とHeader系Family（`header`、`socket`、`shrouded_header`、
`right_angle_header`）では`lead_layout`を`[行数, 列数]`（Pin headerでは`[列数, 1列あたりのピン数]`）
として使う。

## 20.1 Package Dimensions

`dimensions`の各値はDatasheetのPackage Outline（Mechanical Data表）の記号に対応する
`Dimension`（`nominal`/`min`/`max`/`unit`）である。`body_width`はPackage座標系のX方向、
`body_length`はY方向を表し、`overall_*`は同名のbody寸法と同じ軸で測る。

| Field | 意味 | JEDEC記号（例） | 必須 |
|---|---|---|---|
| `body_width` | Body幅（X） | E1（QFP/SOIC）、D（SOT-23の長辺） | 必須 |
| `body_length` | Body長（Y） | D1（QFP）、D（SOIC） | 必須 |
| `body_height` | Moldの厚み | A2（leadless packageでA2が無い場合はA） | 省略可 |
| `exposed_pad` | 裏面Exposed Padの`width`/`length` | E2 / D2 | 省略可 |
| `overall_width` | X方向のLead先端間距離（Leadを含む全幅） | E（QFP/SOIC）、HE（SOT） | 省略可 |
| `overall_length` | Y方向のLead先端間距離 | D（QFP）、HE（SOT-23ではY方向） | 省略可 |
| `overall_height` | Seating planeからPackage上面まで | A | 省略可 |
| `standoff` | Seating planeからBody下面まで | A1 | 省略可 |
| `lead_width` | Lead／Terminal幅 | b | 省略可 |
| `lead_length` | Gull-wing LeadのFoot長、Leadless packageのTerminal長、Chip部品の端子電極長 | L | 省略可 |
| `lead_thickness` | Lead frame厚 | c（QFN/DFNではA3） | 省略可 |
| `tab_width` | Tab（SOT-223/TO-252の放熱Tab）の辺方向の幅 | b2（SOT-223）、E1/b3（TO-252） | Tabを持つPackageでは必須 |
| `tab_length` | Body端からTab先端まで（TO-252のみ） | L3 | 省略可 |
| `row_spacing` | `module`: 2列のピン穴（Plated through-hole）の中心間距離。`header`/`socket`: 隣り合うピン列の中心間距離（X方向） | Pico: 17.78、2.54 mm header: 2.54 | 省略可（2列の`header`/`socket`では必須） |
| `hole_diameter` | `module`: ピン穴および端面のCastellation（半穴）の径。`header`/`socket`: 一次資料の推奨PCB穴径（FootprintのDrill） | Pico: ∅1、Würth WR-PHD: ∅1.10 ±0.15 | 省略可（`header`/`socket`では必須） |
| `aux_pad_width` | `aux_pads`列の各PadのX方向の幅（`module`のみ） | — | `aux_pads`があれば必須 |
| `aux_pad_length` | 同、Y方向の長さ | — | `aux_pads`があれば必須 |
| `aux_row_x` | Board左端（pin 1側）から`aux_pads`列の中心までのX距離 | — | `aux_pads`があれば必須 |
| `aux_row_y` | Board下端（pin 1と反対の端）から`aux_pads`の中心までのY距離 | — | `aux_pads`があれば必須 |
| `mounting_hole_diameter` | 取付穴の径（`module`のみ） | Pico: 2.1 ±0.05 | 省略可 |
| `mounting_hole_spacing` | 各端の取付穴2個のX方向中心間距離 | Pico: 11.4 | 省略可 |
| `mounting_hole_inset` | Boardの短辺から取付穴中心までのY距離（両端で対称に描く） | Pico: 2 | 省略可 |
| `mating_length` | `header`: Housing上面からピン先端まで（嵌合部）。`right_angle_header`: Housing前面（+X側）からピン先端まで | WR-PHD 2.54: 6.0 ±0.20 | 省略可 |
| `tail_length` | Housing下面からはんだ付けTail先端まで（Header系Family） | WR-PHD 2.54: 3.0 ±0.20 | 省略可 |
| `pad_diameter` | 一次資料の推奨Land（丸Pad）径（Header系Family） | — | 省略可 |
| `body_offset` | ピングリッド中心からHousing中心までのX方向の符号付き距離（pin 1を左上にした上面視で+X向きが正。`shrouded_header`/`right_angle_header`） | JST XH: 5.75/2 − (2.35) = 0.525（−X向き）、WR-PHD Angled: 1.5 + 2.54/2 = 2.77 | 省略可（`right_angle_header`では必須） |
| `lead_inset` | Body端から端子外端までの距離（`lga`の端子が端から引っ込んでいる場合） | — | 省略可（省略時は端と面一） |
| `actuator_diameter` | スイッチ上面の丸いアクチュエータ（押し子）の直径 | — | 省略可（3D表示のみ） |

省略可能なDimensionは、一次資料に記載がある場合だけ記録する。記載がなければFieldごと省略し
（Unknown）、Geometry Generatorが自身のFamily既定値を使う。この既定値はGenerator実装の
近似であり、Canonical Dataへ書き戻さない（section 7）。表に`nominal`が無く`min`/`max`
だけが記載されている場合は`min`/`max`だけを記録し、中央値を`nominal`として捏造しない
（Generatorは描画のためにのみ範囲の中央を使う）。

新しく追加した寸法（`overall_*`、`standoff`、`lead_*`、`tab_*`、`row_spacing`、`hole_diameter`、`aux_*`、`mounting_hole_*`、`mating_length`、`tail_length`、`pad_diameter`、`body_offset`、`lead_inset`、`actuator_diameter`）と`construction`、`tab`、`aux_pads`、`pad_order`、`pads`には、Fieldごとの
Provenance（Source、page、表名＋記号のlocator）を必須とする（Validator rule
`provenance.missing`）。また、Lead span がBody以下である、`standoff`や`body_height`が
`overall_height`以上である、といった矛盾は`dimension.inconsistent`として検出する。

```yaml
dimensions:
  body_width:            # E1
    nominal: 7.0
    min: 6.8
    max: 7.2
    unit: mm
  overall_width:         # E
    nominal: 9.0
    min: 8.8
    max: 9.2
    unit: mm
  lead_thickness:        # c -- nominal無記載
    min: 0.09
    max: 0.20
    unit: mm
```

SOT-223やTO-252のようにTabを持つPackageは、Tabの番号をPackage直下の`tab`に記録する。
番号はVendorによって異なる（"4"、"TAB"、あるいはpin 2と同一）ため推測せず、一次資料の
Pin表や外形図の記載に従う。TO-252の中央Leadのように、Body端で切断されて基板に届かない
Lead位置は`trimmed_leads`に列挙する。`tab`にもProvenance（`/tab`）を必須とする。

```yaml
lead_count: 3            # Tabを除くLead位置の数（切断されたLeadを含む）
tab:
  number: "2"
  trimmed_leads: ["2"]
```

Castellated module（`family: module`、Raspberry Pi Picoなど）は、小さなPCBを端面の半穴
（Castellation）で母基板にはんだ付けするPackageである。`body_width`/`body_length`は
Boardの外形、`body_height`はBoard厚（部品は含まない）。ピンは上面視でpin 1が左辺の上、
1..`lead_count`/2が左辺を下へ、残りが右辺を上へ（DIPと同じ）、両列ともBoard中央に
揃えて`pitch`で並ぶ。Footprint padは一次資料の推奨Land pattern（Pico datasheetの
SMT footprint）で、`overall_width`が両列のPad外端間、`lead_length`/`lead_width`が
Padの長さ／幅である（PadはBoard端より外へ出てよい）。Pin header用の穴径が一次資料に
無ければ、Footprintに穴を作らない。Debug padのような追加のPad列は`aux_pads`に
一次資料の番号（Pico: "D1","D2","D3"）を左から順に記録し、大きさと位置を`aux_*`に
記録する（Validator rule `aux_pads.invalid`）。`row_spacing`、`hole_diameter`、
`mounting_hole_*`は3D Modelだけが使う。

```yaml
family: module
lead_count: 40           # 両辺のピン数（aux_padsを含まない）
aux_pads: [D1, D2, D3]   # 上面視で左から、pitch間隔
```

### pad_order と pads

`pad_order`は、Familyの位置順（例: タクトスイッチの左上・左下・右下・右上）に対して、Sourceが振っているPad番号を並べたもの。Vendorごとに角の番号付けが違う部品で使う。要素数は`lead_count`と同じ。

`pad_order`と`pads`はすべてのFamilyで使える。`pads`は推奨ランドパターンを1 Padずつ明示的に記録する（`usb_c`では必須、他のFamilyではFootprintのPadをLead外形の代わりにこれで作る）。各要素は`number`、`x`、`y`、`width`（X方向）、`length`（Y方向）と、メッキ穴なら`drill`、長穴なら`drill_length`（長穴の全長、Padの長辺方向）、メッキなし穴（位置決めピン）なら`plated: false`（`number`は空でよい）。座標系はFamilyが定義する。Sourceの図から座標を算出した場合は、Provenanceのlocatorに計算を書く。

Pin header（`family: header`）とSocket strip（Female header、`family: socket`）は、
ストレート（垂直）のThrough-hole部品である。`lead_layout`は`[列数, 1列あたりのピン数]`
（1列または2列）で必須。上面視（KiCadのライブラリと同じ向き）で1列は`pitch`間隔でY方向に
並び、列どうしは`row_spacing`（2列では必須）だけX方向に離れ、グリッドは原点中心。pin 1は
左上で、番号は列をまたいでジグザグに進む（pin 2はpin 1の隣の列、pin 3はpin 1の下。2列なら
奇数ピンが1列目、偶数ピンが2列目）。一次資料の図面（推奨穴パターンの"1"、"2"の位置）で
この採番であることを確かめて記録し、列ごとに採番する部品や3列以上の部品はこのFamilyで
表さない。Footprintは各ピンに丸Padと一次資料の推奨穴（`hole_diameter`、必須）のDrillを
置く。Pad径は`pad_diameter`、記載が無ければ穴径＋Generator既定のAnnular ring（0.35 mm、
隣接Padとの銅間隔0.25 mmを保つように縮める）。`body_*`はHousing（絶縁体）で、
`body_width`が列をまたぐ方向（X）、`body_length`が列方向（Y）の長さ、ピン
グリッドの中心に置く。`lead_width`は角ピンの太さ、`mating_length`はHousing上面から
ピン先端まで（`header`のみ）、`tail_length`はHousing下面からTail先端まで。Validator rule
`header.invalid`が`lead_layout`、`hole_diameter`、2列の`row_spacing`を検査する。

Shrouded header（`family: shrouded_header`、JST XH/PHのような箱形Wire-to-board header）と
Right-angle pin header（`family: right_angle_header`）は、`header`と同じ座標系・採番・
Footprint規則に従う（一次資料の推奨穴パターンを部品面から見て、pin 1が上、番号が下へ進む
向きに回して読む）。`shrouded_header`はHousingが上面の開いた箱で、ピンはその中に立つ。
外形（`body_*`）は一次資料の寸法だが、壁厚・底面高さ・ピン先端の高さは外形図に寸法が
無いのが普通なので、Generatorの既定値で3D Modelだけを描き、記録しない。図に描かれていても
寸法の無いLock窓や極性Slotは描かない。`right_angle_header`は1列のみで、Housingが基板上に
寝て穴の横にあり、ピンはTailから90°曲がってHousingを貫き+X（または−X）へ伸びる。
`body_width`はピン方向のHousing奥行き、`body_height`は基板からの高さ、`mating_length`は
Housing前面からピン先端まで、曲げたピンの高さは記載が無ければHousing高さの中央に描く。

`body_offset`は、Housingがピン列に対して中心からずれている量（Housing中心のX座標、
ピングリッド中心基準）である。一次資料が直接印刷するのは通常「ピン列からHousing端面まで」
の距離なので、その値とHousing奥行きから算出し、locatorに計算式を書く（例：JST XHは
ピン列から端面まで(2.35)、奥行き5.75 → 5.75/2 − 2.35 = 0.525、端面がpin 1を上にした
上面視で+X側なので`-0.525`）。どちら側の端面かは推奨穴パターン上のNo. 1 circuitとBoss、
Connector outline、Housingの位置から決め、記載が無ければ記録しない（`shrouded_header`は
中心に描き、`right_angle_header`は生成を停止する）。Validator rule `header.invalid`は
`right_angle_header`の1列と`body_offset`も検査する。

```yaml
family: shrouded_header
lead_count: 4
lead_layout: [1, 4]
dimensions:
  body_width: {nominal: 5.75, unit: mm}       # Housing奥行き（X）
  body_offset: {nominal: -0.525, unit: mm}    # 5.75/2 - (2.35)、-X向き
  hole_diameter: {nominal: 0.9, min: 0.9, max: 1.0, unit: mm}
```

```yaml
family: header
lead_count: 10
lead_layout: [2, 5]      # 2列 x 5ピン、pin 2はpin 1の隣の列
dimensions:
  row_spacing: {nominal: 2.54, min: 2.39, max: 2.69, unit: mm}
  hole_diameter: {nominal: 1.1, min: 0.95, max: 1.25, unit: mm}
```

## 20.2 Package Construction

`construction`（省略可）は、一次資料が明記しているPackageの物理的な構造であり、外観
（3D Modelの材質表現）や取り扱いに影響する事実を表す。寸法やPackage IDの文字列からの
推測で設定してはならない。記載が無ければ省略する。

閉じた語彙（初期値）：

```text
molded           Transfer mold（樹脂/epoxy）Body。IC、SOD/SMA diode、"molded plastic" DO-41など
tantalum_molded  固体タンタルコンデンサ素子をMoldしたCase（KEMET T49xなど）
glass            ガラス封止Body（一次資料がglassと明記するDO-35小信号diodeなど。"DO-35"というCase名だけでは設定しない）
ceramic          焼成セラミックBody＋端子電極（MLCC、セラミックChip inductor/fuse）
metal_can        金属Can（水晶振動子、アルミ電解コンデンサ）
```

```yaml
construction: molded

provenance:
  "/construction":
    - source: vishay/88503
      page: 1
      locator: "MECHANICAL DATA: \"Case: DO-41 (DO-204AL), molded epoxy body\""
```

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
retrieved: "2026-04-10"   # 省略可: 文書を取得して読んだ日
```

`url`はその文書を読んだ場所。メーカーの「最新版」リンク（TIの`.../symlink/<part>.pdf`など）は、あとで`revision`より新しい版を返すことがある。表示側は`revision`と`date`を必ず併記する。`retrieved`は取得日（`YYYY-MM-DD`）で、分かる場合に記録する。PDFの複製は配布しない（リンクのみ）。

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
