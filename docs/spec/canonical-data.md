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

## 3.3 Schema 0.2 Layout

A file keeps each fact next to its evidence and writes quantities as a
reader would:

```yaml
schema_version: "0.2"
kind: part
id: aos/AO3400A
manufacturer: aos
mpn: AO3400A
device: aos/AO3400A
package: standards/SOT23-2.90x1.60-P1.90-AOS
source: aos/AO3400A-DS            # the file's default source

existence: { status: probable, src: p1 "Title" }

parameters:
  drain_current: { typ: 5.7 A, at: { TA: 25 °C }, src: p1 "Abs. Max. Ratings, ID" }
  rds_on:
    - { typ: 18 mΩ, max: 26.5 mΩ, at: { VGS: 10 V, ID: 5.7 A }, src: p2 "Elec. Char., RDS(ON)" }
    - { typ: 24 mΩ, max: 32 mΩ, at: { VGS: 4.5 V, ID: 5 A }, src: p2 "Elec. Char., RDS(ON)" }
```

- **Quantities** are unit strings: `26.5 mΩ`, `0.4 mm`; a symmetric
  tolerance is `tol: ±10 %` (= `min: -10 %, max: 10 %`). `min`, `typ`
  and `max` of one value use one unit. Coordinates (`pads`, `rect`,
  `size`, `pcb_edge`) stay plain numbers in mm.
- **`src`** cites the fact it sits in (section 25). **`at`** holds the
  measurement conditions (section 11.2).
- **Layout** is fixed by `openparts data fmt` (deterministic key order,
  flow style for an entry that fits in 100 columns, block style
  otherwise); CI fails on an unformatted file (`fmt --check`). Comments
  are kept: before a top-level key, or at the top of the file.
- Schema 0.1 files (a `provenance:` map at the end, `nominal`, separate
  `unit`) are converted with `openparts data migrate --to 0.2`. Readers
  accept 0.1 during the transition.

In memory and in the API the model is unchanged in shape: quantities
are `min`/`typ`/`max` + `unit`, evidence is a `provenance` map keyed by
JSON Pointer, and a parameter key holds a list of values.

---

# 4. Required Document Header

各Canonical Documentは最低限、

```yaml
schema_version: "0.2"
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
circuit
```

KindごとにSchema Validationを行う。`circuit`はsection 41。

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
body_width: { min: 6.9 mm, max: 7.1 mm }
```

Datasheetにtyp（nominal）が存在しない場合、

```yaml
typ: 7.0 mm
```

を勝手に生成してはならない。

`null`はSchemaが明示的に許可する場合以外使用しない。

---

# 8. Units

物理量にはUnitを明記する。値とUnitは一つの文字列に書く（section 3.3）。

例：

```yaml
pitch: { typ: 0.5 mm }
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

Unitの無い数値（`typ: 7.0`）はエラーとする。Unitは上の表とsection 11.1の表で検証する。

---

# 9. Numeric Range

寸法やElectrical Specificationは必要に応じて、

```yaml
body_width: { min: 6.9 mm, typ: 7.0 mm, max: 7.1 mm }
```

として表す。

すべてのFieldに`min/typ/max`を要求しない。`typ`はschema 0.1の`nominal`である。

---

# 10. Manufacturer

例：

```yaml
schema_version: "0.2"
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
schema_version: "0.2"
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
source: murata/GRM155R71C104KA88
parameters:
  capacitance:   { typ: 100 nF, src: p1 "Specifications, Capacitance" }
  tolerance:     { tol: ±10 %, src: p1 "Specifications, Capacitance" }
  rated_voltage: { typ: 50 V, src: p1 "Specifications, Rated voltage" }
  dielectric:    { value: X7R, src: p1 "Specifications, Temperature characteristics" }
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

- *numeric*: `min`/`typ`/`max` as unit strings, like a Dimension
  (section 9). Only what the source states is recorded; a `typ` is never
  computed from `min`/`max` (section 7). `min <= typ <= max`.
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
| `zener_voltage` | voltage (Vz at the test current; min/typ/max as printed) | diode |
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
| `isolation_voltage` | voltage (input-output isolation, BVs; the rms/duration condition goes into the locator, or `at: { t: 60 s }`) | optocoupler |
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
`tol: ±10 %` (`min: -10 %, max: 10 %`); an asymmetric -20/+80 % is
`{ min: -20 %, max: 80 % }`. It is compared and filtered by its larger
magnitude (10 %).

A value measured under stated conditions carries them in `at` (section
11.2). When a value depends on a condition the Part does not fix (a
different power per operation mode, a range across output options),
leave the key out rather than pick one.

Mounting style is not a parameter: it is derived from the Package
geometry (SMD/THT) and never duplicated.

**Provenance is mandatory per value.** Every value has a `src` (section
25; in memory `/parameters/<key>`, or `/parameters/<key>/<i>` for one of
several variants). Two rules for where values may come from:

1. Never derive a value by decoding an MPN, unless the datasheet's
   ordering-code (part numbering) table explicitly maps that code to the
   value. Then cite that table as the provenance (page + table name +
   the code: `"ORDERING INFORMATION, (2) TOLERANCE: B = ±0.1%"`).
2. When one datasheet covers many MPNs, cite the row: the table and the
   row or column naming this MPN (or its value), e.g.
   `"Table 3, RT0805 column, Standard: 1/8W"`.

**Validation** (openparts-validator): an unknown key, a unit
incompatible with the key's quantity, the wrong shape for the key or an
enumerated value outside its list, `min <= typ <= max` not holding,
a bad condition (section 11.2) and an uncited value are errors
(`parameter.unknown_key`, `parameter.unit`, `parameter.invalid`,
`parameter.out_of_range`, `parameter.condition`, `provenance.missing`).
A key that is unusual for the Device's category (capacitance on a
resistor) is a warning (`parameter.implausible`).

The vocabulary grows like `PinType` (section 15): a new component type
adds keys here, in `PARAMETERS`, in the web catalog's labels and in the
alternatives' comparison rules (`PARAMETER_RULES`: whether a value must
be equal, at least or at most the original's;
[Alternative Parts and BOM Checking](alternatives-and-bom.md) section
1.4), in one change. A key is never reused for another quantity.

## 11.2 Measurement Conditions

`at` maps a condition symbol to the quantity the source states the value
under: `at: { VGS: 10 V, ID: 5.7 A }`, a range as `TA: -40..85 °C`. Record
only conditions the source prints for that value (its row, or its
table's heading when the heading states it without "unless otherwise
noted").

A key may hold a list of condition variants, each with its own `at` and
`src`. The **first** is the *primary* value: the catalog, search facets,
filters and sorting use it (shown as `18 mΩ @ VGS 10 V`). List the
variant a reader would look for first (the datasheet's headline
condition) first.

Comparisons are like-for-like only: alternatives (`PARAMETER_RULES`)
compare the original's primary value with the candidate's variant under
the same conditions (compared in SI); a candidate without one is
*unverified*. Two values without conditions compare as before.

Symbols (closed vocabulary, `openparts_core::CONDITION_SYMBOLS`; units
from the quantity's table in section 11.1):

| Quantity | Symbols |
|---|---|
| voltage | `VGS` `VDS` `VCE` `VCC` `VDD` `VIN` `VOUT` `VR` `VF` |
| current | `ID` `IC` `IB` `IF` `IR` `IZT` `IPP` `IOUT` `IL` |
| temperature (`°C`) | `TA` (ambient) `TJ` (junction) `TC` (case) `TL` (lead) `TS` (solder point) `TT` (terminal) |
| frequency | `f` |
| time (`ns` `µs` `ms` `s`) | `t` |

An unknown symbol, a unit that doesn't fit it, or two variants with the
same conditions are errors (`parameter.condition`). Grow the table like
the parameter vocabulary.

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
schema_version: "0.2"
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

## 14.2 Differential Pairs

A pin that is one half of a differential pair carries `diff_pair`, but
only where the source names the pair: both halves as the positive and
negative line of one interface (USB_DP/USB_DM "USB Data +ve/-ve", D+/D-,
a receptacle's Dp1/Dn1). Two pins that merely look alike are not a pair.

```yaml
pins:
  "46": { name: USB_DM, type: io, diff_pair: { partner: "47", polarity: n, standard: usb2, src: p615 "Table 620" } }
  "47": { name: USB_DP, type: io, diff_pair: { partner: "46", polarity: p, standard: usb2, src: p615 "Table 620" } }
  # A pair carried only in an alternate function (ESP32-C6 GPIO12/13):
  "18":
    name: GPIO12
    type: io
    alternate_functions: [USB_D_MINUS]
    diff_pair: { partner: "19", polarity: n, standard: usb2, function: USB_D_MINUS, src: p12 "Table 2-1" }
```

| Field | Meaning |
|---|---|
| `partner` | The pin number of the other half. |
| `polarity` | `p` (D+, DP, _P) or `n` (D-, DM, _N). |
| `standard` | The interface the pair carries. Closed vocabulary: `usb2`. It grows like `PinType` (section 15). |
| `function` | Optional. The alternate function the pin carries the pair in, when it is not the pin's primary function. One of the pin's `alternate_functions`. |
| `impedance_ohm` | Optional. A differential impedance *the part's own source* states. |

The impedance an interface asks for (USB 2.0: 90 ohm differential) is a
fact of the standard, not of the part: it is not recorded here. Board
tools apply it per `standard` (documented and cited there).

**Validation** (`pin.diff_pair.invalid`): the partner is another pin of
the Device; its `diff_pair` names this pin back, with the opposite
polarity and the same standard; either both halves name a `function` or
neither does, and a `function` is one of the pin's
`alternate_functions`; `impedance_ohm` is positive. Every `diff_pair`
needs a `src` of its own or of its pin (`/pins/<n>/diff_pair` or `/pins/<n>`)
(`provenance.missing`; the pin table's `src` does not cover it,
since the pair is a claim of its own).

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
schema_version: "0.2"
kind: package
id: st/LQFP48-7x7-P0.5

family: lqfp
lead_count: 48
source: st/DS12589@rev8

pitch: { typ: 0.5 mm, src: p142 "Table 123, e" }

dimensions:
  body_width:  { min: 6.9 mm, typ: 7.0 mm, max: 7.1 mm, src: p142 "Table 123, E1" }
  body_length: { min: 6.9 mm, typ: 7.0 mm, max: 7.1 mm, src: p142 "Table 123, D1" }
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
schema_version: "0.2"
kind: package
id: standards/SOT23-3

family: sot
lead_count: 3
lead_layout: [2, 1]   # pin 1-2側の本数, pin 3側の本数 (pin番号順)

pitch: { typ: 0.95 mm }

dimensions:
  body_width:  { typ: 1.3 mm }
  body_length: { typ: 2.9 mm }
```

`lqfp`/`qfn`/`soic`のように`lead_count`から均等に辺数で割り切れるFamilyは
`lead_layout`を必要としない。`bga`とHeader系Family（`header`、`socket`、`shrouded_header`、
`right_angle_header`）では`lead_layout`を`[行数, 列数]`（Pin headerでは`[列数, 1列あたりのピン数]`）
として使う。

## 20.1 Package Dimensions

`dimensions`の各値はDatasheetのPackage Outline（Mechanical Data表）の記号に対応する
`Dimension`（`min`/`typ`/`max`、単位付き）である。`body_width`はPackage座標系のX方向、
`body_length`はY方向を表し、`overall_*`は同名のbody寸法と同じ軸で測る。

| Field | 意味 | JEDEC記号（例） | 必須 |
|---|---|---|---|
| `body_width` | Body幅（X） | E1（QFP/SOIC）、D（SOT-23の長辺） | 必須 |
| `body_length` | Body長（Y） | D1（QFP）、D（SOIC） | 必須 |
| `body_height` | Moldの厚み | A2（leadless packageでA2が無い場合はA） | 省略可 |
| `exposed_pad` | 裏面Exposed Padの`width`/`length`（任意で推奨Land `land_width`/`land_length`、20.5参照） | E2 / D2 | 省略可 |
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
近似であり、Canonical Dataへ書き戻さない（section 7）。表に`typ`が無く`min`/`max`
だけが記載されている場合は`min`/`max`だけを記録し、中央値を`typ`として捏造しない
（Generatorは描画のためにのみ範囲の中央を使う）。

新しく追加した寸法（`overall_*`、`standoff`、`lead_*`、`tab_*`、`row_spacing`、`hole_diameter`、`aux_*`、`mounting_hole_*`、`mating_length`、`tail_length`、`pad_diameter`、`body_offset`、`lead_inset`、`actuator_diameter`）と`construction`、`tab`、`aux_pads`、`pad_order`、`pads`には、Fieldごとの
Provenance（Source、page、表名＋記号のlocator）を必須とする（Validator rule
`provenance.missing`）。また、Lead span がBody以下である、`standoff`や`body_height`が
`overall_height`以上である、といった矛盾は`dimension.inconsistent`として検出する。

```yaml
dimensions:
  body_width:     { min: 6.8 mm, typ: 7.0 mm, max: 7.2 mm, src: p12 "Package outline, E1" }
  overall_width:  { min: 8.8 mm, typ: 9.0 mm, max: 9.2 mm, src: p12 "Package outline, E" }
  lead_thickness: { min: 0.09 mm, max: 0.20 mm, src: p12 "Package outline, c" }   # typ無記載
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
  body_width: { typ: 5.75 mm }             # Housing奥行き（X）
  body_offset: { typ: -0.525 mm }          # 5.75/2 - (2.35)、-X向き
  hole_diameter: { min: 0.9 mm, typ: 0.9 mm, max: 1.0 mm }
```

```yaml
family: header
lead_count: 10
lead_layout: [2, 5]      # 2列 x 5ピン、pin 2はpin 1の隣の列
dimensions:
  row_spacing: { min: 2.39 mm, typ: 2.54 mm, max: 2.69 mm }
  hole_diameter: { min: 0.95 mm, typ: 1.1 mm, max: 1.25 mm }
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
construction: { value: molded, src: p1 "MECHANICAL DATA, Case" }
```

## 20.3 Board Edge and Keep-outs

Placement facts a source states about the board around the part. They feed the
Placement Model ([placement-model.md](placement-model.md), sections 8 and 9).
Both are optional, both need provenance, and neither is ever filled with a
generic default: absent means the source states nothing.

`edge`: the part sits at the board edge.

```yaml
edge:
  side: "+y"            # the package side (its own frame) facing the edge: +x, -x, +y, -y
  requirement: required # required (default): the source draws the edge; recommended
  pcb_edge: 2.6         # the PCB edge line the source draws, along side's axis (y = 2.6); omit if none
```

`keepouts`: areas the board keeps clear, each either located (`rect`, centre and
size in the package frame) or only sized (`size`, the two lengths in the source's
order when it gives no orientation), optionally against a package `side`.

```yaml
keepouts:
  - purpose: antenna            # antenna, mechanical, thermal, other
    restricts: [components, copper]   # components, copper, tracks, vias
    side: "-y"
    size: [14, 9]
```

Rules: `edge.invalid` (a land reaches past `pcb_edge`), `keepout.invalid`
(both or neither of `rect` and `size`, a non-positive length, nothing restricted),
`provenance.missing` for `/edge` and `/keepouts`.

## 20.4 Modules with Pad Rows and LGA Grids

A `module` whose pads are not two centred long-edge rows (ESP32-WROOM: pins on
three edges at their own offsets, a segmented ground pad; LGA modules) states
them as `pad_rows` and `pad_grids`, taken from the source's recommended land
pattern. They replace `overall_width`/`lead_width`/`lead_length`; `lead_count`
is the number of distinct pad numbers.

```yaml
pad_rows:
  value:
    # side: the edge the row runs along. Rows run counter-clockwise in the top
    # view: down -x, left to right along -y, up +x, right to left along +y.
    # start: from the corner the row starts at to the first pad centre;
    # inset: board edge inward to the pad centres (negative: outside);
    # pad: [along the edge, across it]. All mm.
    - { side: -x, first: 1, count: 14, pitch: 1.27, start: 7.49, inset: 0.25, pad: [0.9, 1.5] }
    - { side: -y, first: 15, count: 10, pitch: 1.27, start: 3.285, inset: 0.25, pad: [0.9, 1.5] }
  src: p43 "Figure 13"
pad_grids:
  # numbers: one shared number, or one per pad row by row from the top-left;
  # pitch and pad [X, Y]; x, y: the grid centre in the package frame.
  value: [{ numbers: ["39"], columns: 3, rows: 3, pitch: [1.4, 1.4], pad: [0.9, 0.9], x: -1.5, y: -2.46 }]
  src: p43 "Figure 13"
shield:                 # the can on the top face; its top is dimensions.overall_height
  value: { x: 0, y: -2.9, width: 15.8, length: 17.6 }
  src: p40 "Figure 10"
```

A pad reaching the board edge is a castellation (Placement Model `castellated`;
the 3D model drills a plated half-hole of `dimensions.hole_diameter` there when
stated); the others are LGA pads. Rule `pad_layout.invalid`: these fields on
another family, non-positive sizes or counts, a pitch not above the pad, a pad
number in two rows or in a row and a grid, a grid with neither one number nor
one per pad, `pad_grids` without `pad_rows`, a `shield` without
`dimensions.overall_height`. `provenance.missing` for `/pad_rows`,
`/pad_grids`, `/shield`.

## 20.5 Surface-Mount Wire-to-Board Headers

`shrouded_header` (top entry) and `right_angle_header` (side entry) with
surface-mount contacts state `smt` instead of `dimensions.hole_diameter`: one
contact row along Y at `pitch` (`lead_layout: [1, n]`), pin 1 at the top, the
signal lands centred on the row (x = 0), and two mechanical (retention) pads,
numbered `mech_number` (default `MP`), at `mech_pad_at` and its mirror across
y = 0. The housing is placed by `body_offset` along X from the contact row as
for the through-hole families.

```yaml
smt:
  value: { pad: [1.55, 0.6], mech_pad: [1.8, 1.2], mech_pad_at: [-3.875, 2.8] }   # [X, Y] mm
  src: p1 "PC board layout, Side entry type"
```

Exposed pads on leaded packages (`soic`, `tssop`, `ssop`, `msop`: TI PowerPAD
HSOIC/HTSSOP/HVSSOP, Toshiba HSOP) use `dimensions.exposed_pad` as on QFN: a
centred terminal numbered `EP`. `exposed_pad.land_width`/`land_length` record
the source's recommended thermal land when it differs from the pad (the 3D slug
keeps the pad size).

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
schema_version: "0.2"
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

`sha256`（省略可）は読んだ文書のSHA-256（小文字16進64桁）。Validator rule `source.checksum`は形式を検査し、openparts-serverの文書キャッシュ（opt-in、Architecture Specification section 20）は一致しないダウンロードを拒否する。

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
standard
reference_design
other
```

`standard`は規格書（USB Type-C Specificationなど。`publisher`は`usb-if`など規格団体）、`reference_design`はメーカーのリファレンスデザイン文書（ガイド、設計ファイル）。どちらも主にsection 41の参考回路の出典。

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

ProvenanceはField単位で指定する。A fact's evidence sits next to it, as
`src`:

```yaml
source: st/DS12589@rev8                       # the file's default source
dimensions:
  body_width: { typ: 7.0 mm, src: p142 "Table 123, D" }
pins:
  src: p73 "Table 17"                         # covers the whole table (/pins)
  "1": { name: VBAT, type: power }
  "2": { name: { value: PC13, src: st/RM0440@rev5 p91 "Table 3" }, type: io }
```

**Citation.** `[<source-id>] [p<page>] ["<locator>"]`; the source id
defaults to the file's `source:`. The full form is
`{ source: st/RM0440@rev5, page: 91, locator: "Table 3" }`. Several
citations are a list: `src: [p73 "Table 17", st/RM0440@rev5 p91]`.

**Where `src` lives.** It cites the value it is in:

| Fact | `src` |
|---|---|
| a record (`existence`, a dimension, a parameter value, `edge`, `tab`, `review`, a pin, a `diff_pair`) | inside it, last |
| a table (`pins`, `dimensions`, `revisions`) | inside it, first: covers every entry without its own |
| a scalar or list (`package`, `lead_count`, `construction`, a pin's `name`, `pad_order`, `keepouts`) | wrap it: `{ value: <fact>, src: ... }` |
| one of several parameter variants | inside that variant |

In memory and in the API the same evidence is a `provenance` map keyed
by the fact's JSON Pointer (`/dimensions/body_width`, `/pins`,
`/parameters/rds_on/1`); `openparts-data` converts both ways. A pointer
the layout can't place (none exists today) stays in a trailing
`provenance:` map of that form.

## 25.1 Locators

A locator is a short identifier of where on the page the fact is: a
table or figure number, a row and column name, a section title
(`"Table 3, RT0805 column"`, `"Elec. Char., RDS(ON)"`). It is not copied
text: the value lives in the fact, its conditions in `at` (section
11.2). A locator over 120 characters is a warning
(`provenance.locator_long`).

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
"1": { name: { value: VBAT, src: [p73, st/RM0440@rev5 p91] }, type: power }
```

## 26.1 Review Status

A Part, Device, Package or Source document may carry a top-level
`review` block saying that a person has checked it: every value against
the location its provenance cites (a Source: that its id, revision,
date and URL name the document read).

```yaml
review:
  status: reviewed        # the only value
  by: [alice, bob]        # GitHub logins of the reviewers (required)
  pr: 123                 # the openparts-data pull request that reviewed it (required)
  date: 2026-11-20        # optional, YYYY-MM-DD
```

- **No block means unreviewed.** Data AI agents extract (Agent
  Ingestion Specification) starts without one: it is a candidate. There
  is no `status: unreviewed`; an absent block is the only way to say it.
- **Reviewed documents come only from human review in a pull request**
  to openparts-data. The reviewer -- typically an external contributor
  -- checks the values (the pull request's fact table links each changed
  value to its cited page) and adds or updates the block in that pull
  request: `by` lists the GitHub logins of who checked it, `pr` that
  pull request's number. `CONTRIBUTING.md` in openparts-data describes
  the steps.
- **The block covers the whole document as it was in that pull
  request.** Any later change to a value, a pin, a dimension or the
  provenance of a reviewed document must drop the block in the same
  change -- or the change is reviewed and the block updated (`by`,
  `pr`) in a pull request. The validator cannot see that a value changed
  after its review, so this is a rule for every editor, human or agent;
  the pull request's fact table warns when a changed file keeps its old
  block.
- A block does not promote anything: `existence`, `lifecycle` and the
  values keep the status their evidence gives them (section 37).
- **Validation** (`review.block`, an error): `by` is a non-empty list of
  GitHub logins without duplicates, `pr` a pull request number (1 or
  more), `date` `YYYY-MM-DD`.
- A Part's review status as the site shows it combines the Part, its
  Device and its Package: reviewed only when all three are.

## 26.2 Copyright and Source Use

Engineering policy, not legal advice.

- **Facts only.** Store values, units, conditions, min/typ/max and
  identifiers. Never reproduce a source's table layout, prose, figures,
  graphs or drawings.
- **No curve digitizing.** Take only values printed as numbers in text
  or tables; never read values off a graph.
- **Short locators** (section 25.1): table/figure numbers, row/column
  names, section titles.
- **Manufacturer documents only** as sources of facts (datasheets,
  package drawings, application notes). No bulk extraction from
  manufacturer or distributor parametric databases or search tools:
  database rights (in the EU, Directive 96/9/EC) and site terms of use
  apply to them.
- **No rehosting.** Sources are cited by URL; PDFs are never published
  ([Architecture Specification](architecture.md) sections 3 and 20: the
  document cache is opt-in and off on public deployments).
- **Reference circuits** (section 41) are facts too: roles, values,
  connections, constraints, conditions, the level and the source's
  one modal word. No schematic images, redrawn figures or prose. A
  reference design's own files (KiCad, Allegro, BOM) are not stored,
  even when their licence allows it: record the link and what a
  cross-check found. Standards are used under their own terms (the USB
  specifications' limited copyright license: facts only).
- **Trademarks.** Manufacturer names identify the source; logos are
  never stored or shown, and pages are not branded as the
  manufacturer's.

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
schema_version: "0.2"
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
schema_version: "0.2"
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
├── circuits/
│   └── raspberrypi/          # circuits/<publisher>/<name>.yaml
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

---

# 41. Reference Circuit

A reference circuit is the peripheral circuit a manufacturer's
document or a standard gives for a device (the RP2040's decoupling,
the USB Type-C sink's Rd), stored as facts with provenance so that
the part page lists it and a board tool can expand it
(`tools/kicad-board`, `use:`). It lives in
`circuits/<publisher>/<name>.yaml`; its id is `<publisher>/<name>`.

```yaml
schema_version: "0.2"
kind: circuit
id: raspberrypi/rp2040-xosc-12mhz
title: RP2040 12 MHz crystal oscillator
host: raspberrypi/RP2040
source: raspberrypi/RP-008279-DS@rev2

conditions: { IOVDD: { typ: 3.3 V, src: p11 "2.3.1" } }
nets:
  GND:      { kind: ground }
  XTAL_OUT: { kind: local }
roles:
  C_XIN:
    requires: { category: capacitor, capacitance: { typ: 15 pF }, src: p11 "2.3" }
    pins:     { "1": host.XIN, "2": GND }
    level:    { value: recommended, word: recommend, src: p11 "2.3" }
    example:  { size: "0402", dielectric: { value: C0G }, src: p14 "Figure 11, C2" }
  X1:
    requires: { part: abracon/ABM8-272-T3, src: p11 "2.3.1" }
    pins:     { XTAL1: host.XIN, XTAL2: XTAL_OUT, GND: GND }
    level:    { value: recommended, word: recommend, src: p11 "2.3.1" }
    expects:  { load_capacitance: { typ: 10 pF }, esr: { max: 50 Ω }, src: p11 "Table 1" }
formulas:
  load_capacitance:
    expr: C_XIN * C_XOUT / (C_XIN + C_XOUT) + C_stray
    inputs:
      C_XIN:   { role: C_XIN, param: capacitance }
      C_XOUT:  { role: C_XOUT, param: capacitance }
      C_stray: { typ: 3 pF, src: p11 "2.3" }
    target: { role: X1, param: load_capacitance }
    result: { typ: 10.5 pF, src: p11 "2.3" }
    src: p11 "2.3"
```

## 41.1 Keys

| Key | Meaning |
|---|---|
| `host` | the device it serves: a device id, `{ devices: [...] }`, or a generic match `{ category: connector, has_pins: [CC1, CC2] }` (any device with those pin names) |
| `conditions` | where it is valid: host pin or net name -> value (`IOVDD: { typ: 3.3 V }`); outside them it needs re-tuning |
| `nets` | `kind: power / ground / signal` (interface nets the board binds) or `local` (one per instance); optional `voltage` |
| `connect` | host pin name -> `{ net }` or `{ no_connect: true }`, for every host pin of that name; `level` defaults to `required` |
| `roles.<R>` | one component: `requires`, `pins`, `level`, and optionally `per_pin`/`count`, `place`, `example`, `expects` |
| `formulas` | a value the source computes (below) |
| `placement`, `routing` | hints: `keep_short`, `short_stub` + `max_length`; `diff_pair` + `impedance`, `reference_plane`, `example` geometry |
| `includes` | a composite: `{ circuit, with: { their net: our net }, host: { role } }` |
| `review` | section 26.1; absent: unreviewed |

**Role.** `requires` is what the source states the part must be: a
`category` (one or a list) and catalog parameters (section 11.1 keys,
units and shapes; `typ` is the value, `min`/`max` a bound), or a `part`
the source names. Only what the source states. `pins` maps the part's
pin (name or number) to `$pin` (the host pin of a `per_pin` instance),
`host.<PIN>` or a circuit net; its evidence is `requires`'. `per_pin:
<PIN>` makes one instance per host pin of that name, in pin-number
order. `place: { near }` names `$pin`, `host.<PIN>` or another role.
`example` holds what the reference design used without stating a rule
(ratings, dielectric, `size`, `part`): it guides selection, it never
filters. `expects` holds facts about the chosen part the circuit was
tuned to (a crystal's load capacitance, maximum ESR).

## 41.2 Levels

| `value` | the source says | `word` |
|---|---|---|
| `required` | must, shall, required, mandatory, tie to, need, important | the one modal word, lower case or as printed |
| `recommended` | should, recommend | |
| `example` | the design has it; no rule stated | none |
| `optional` | may, DNF | |

`level: example` or `level: { value: required, word: must }`. When two
places of a source disagree, the stronger one with its own citation.
A board may drop or replace a `required` fact only with a stated
waiver; a `recommended` one with a warning.

## 41.3 Formulas

`expr` uses `+ - * /`, parentheses, `sqrt()`, numbers and input names,
evaluated in SI units. An input is a role's parameter (`{ role, param
}`: its `requires` value, or the selected part's on a board) or a
literal the source assumes, with its own `src`. `target` names what the
result is compared with; `result` is the source's own worked value;
`unit` the display unit. Without a tolerance stated by the source a
result is shown, never judged pass or fail.

## 41.4 Provenance

Every fact cites its source (section 25): a net's `voltage`, each
`conditions` and `connect` entry, a role's `requires`, `level`,
`place`, `example` and `expects`, each formula and literal input and
`result`, each hint and include -- at the fact or an ancestor. Sources
are manufacturer documents (`datasheet`, `application_note`,
`reference_design`) or a `standard`. Copyright: section 26.2.

## 41.5 Validation

`openparts validate --data` checks: the host device exists and every
host pin named (`connect`, `per_pin`, `host.<PIN>`, `conditions`)
exists on it (`circuit.host`, `circuit.pin`); parameter keys and units
as for parts (`parameter.*`); `requires.part` and `example.part` exist
(`circuit.part`); every target and hint names a declared net, a role or
`$pin` with `per_pin` (`circuit.connection`); level words fit their
level (`circuit.level`); formulas parse and have their inputs
(`circuit.formula`); includes exist and bind their nets
(`circuit.include`); every fact has provenance (`provenance.missing`);
locators are short (`provenance.locator_long`); the file is at
`circuits/<id>.yaml` (`id.location`).
