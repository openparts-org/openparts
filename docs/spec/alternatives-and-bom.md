# Alternative Parts and BOM Checking

Version 0.1 Draft. Implemented by `crates/openparts-bom` and served by
openparts-server as `GET /v1/parts/{manufacturer}/{mpn}/alternatives`
and `POST /v1/bom/check` (also `openparts bom check <file>`).

This is derived data: nothing here is stored in Canonical Data. Every
judgement is computed from the Part, Device and Package documents as
they are, and a value the data doesn't state is never assumed
(Canonical Data Specification section 7).

---

# 1. Alternatives

`GET /v1/parts/{manufacturer}/{mpn}/alternatives` returns the candidate
replacements of a Part from its own Device category, classified and
ranked.

## 1.1 Classes

| Class | `class` | Land pattern | Pinout | Parameters |
| --- | --- | --- | --- | --- |
| Drop-in | `drop_in` | same | compatible | meet or exceed (or unverified) |
| Footprint-compatible, different pinout | `footprint_compatible` | same | some pad differs | any |
| Functional equivalent | `functional` | different | - | identity verified, meet or exceed |
| Near miss | `near_miss` | same | compatible | some key worse |
| Listed replacement | `listed` | - | - | a `lifecycle.replacement` that fits no class above |

A candidate whose identity parameter (section 1.4) differs from the
original's is not an alternative. In another package, a candidate is a
functional equivalent only when every identity key of the category is
stated by both parts and equal (or it is the same Device); otherwise it
is left out and counted in `rejected`.

`lifecycle.replacement` (Canonical Data Specification section 13) is
always listed, first (`listed_replacement: true`), and still checked:
a named replacement is not by itself compatible.

## 1.2 Land pattern

Package ids are vendor-specific (`SOT25-...-DIODES`, TI `DBV0005A`), so
two packages are compared by the footprint pads OpenParts generates for
them (openparts-mcad IPC-7351B lands, as openparts-pcbcad places them in
the KiCad footprint):

- the same number of pads;
- each pad of the original matched to the nearest unmatched pad of the
  candidate, within 0.25 mm (centre to centre);
- SMD against SMD and through-hole against through-hole;
- pad width and length within 0.3 mm or 35 %, whichever is larger.

Chip packages (`chip` family) are compared by their standard size code
(`0603`, `0805`): every vendor's 0603 goes on the same 0603 land. A
package whose pads can't be generated (a missing dimension) is
compared by family, lead count, pitch (0.02 mm) and body size (0.2 mm).
The matched pads give the pad map the pinout is compared on.

## 1.3 Pinout

The Device pin maps are compared pad by pad. A pin's name is folded to
its function first:

| Function | Names |
| --- | --- |
| ground | `GND`, `VSS`, `GROUND`, `COMMON`, `AGND`, `PGND`, `DGND`, `EP`, `EPAD`, `VEE`, and any pin of type `ground` |
| input supply | `VIN`, `IN`, `VI`, `INPUT`, `VCCIN`, `VDDIN` |
| output | `VOUT`, `OUT`, `VO`, `OUTPUT` |
| supply | `VDD`, `VCC`, `VS`, `V+`, `VDDIO`, `VBAT` |
| enable | `EN`, `ENABLE`, `CE`, `ON`, `SHDN`, `SD`, `SHUTDOWN`, `OFF` |
| adjust | `ADJ`, `ADJUST`, `ADJUSTMENT` |
| bypass | `BYP`, `BP`, `BYPASS`, `NR`, `NRSS` |
| power good | `PG`, `PGOOD`, `POWERGOOD`, `PWRGD` |
| reset | `RST`, `RESET`, `MR` |
| not connected | `NC`, `N/C`, `NOT CONNECTED`, `DNC`, `NIC`, any pin of type `nc`/`reserved` |

A name with alternatives (`ADJ/GND`, `Adj (GND)`) stands for each. An
enable's active level comes from its negation mark (`/EN`, `EN#`,
`~EN`, `nEN`, `EN_N` are active low; `/SHDN` is active high); a bare
`SHDN`/`SD` has no stated level (datasheets draw the bar the name text
lacks).

Each pad is `same` (one name), `equivalent` (synonyms), `note` or
`differs`:

| Original | Candidate | Status |
| --- | --- | --- |
| NC | NC | same |
| NC | a passive pin (bypass) | note: works floating, reduced performance |
| NC | any other pin | differs: the candidate needs a connection the board doesn't have |
| a pin | NC | note: what the board connects there goes nowhere |
| ground | not ground (or the reverse) | differs |
| function A | function B | differs |
| enable, active high | enable, active low | differs |
| enable | enable of unstated level | note |

Two parts are pin-compatible when no pad differs. The classic trap --
two SOT-23-5 regulators on the same land with VIN and VOUT (or an
adjustable part's FB where the fixed one has NC) on other pads -- is
`footprint_compatible` with those pads listed in `pins.differing`,
never `drop_in`.

## 1.4 Parameters: meets or exceeds

Every key the original states is compared with the candidate's value
(`rules::PARAMETER_RULES`; a test keeps it in step with the vocabulary,
Canonical Data Specification section 11.1):

| Rule | Keys | Pass when |
| --- | --- | --- |
| `equal` | capacitance, resistance, inductance, frequency, load_capacitance, output_voltage (0.5 %), zener_voltage (0.5 %), wavelength, wavelength_red/_green/_blue (2 %), capacity, color, data_interface, polarity, output_type | the same value (relative tolerance 1e-6 unless stated) |
| `at_least` | rated_voltage, rated_current, saturation_current, power_rating, reverse_voltage, forward_current, drain_source_voltage, drain_current, collector_emitter_voltage, collector_current, isolation_voltage, current_transfer_ratio, input_voltage_max, output_current, data_rate | candidate >= original |
| `at_most` | tolerance, capacitance_tolerance, esr, dcr, temperature_coefficient, frequency_tolerance, forward_voltage, rds_on, dropout_voltage | candidate <= original |
| `ranked` | dielectric: C0G = NP0 > U2J > X8R > X8L > X7R > X7S > X7T > X6S > X6T > X5R > X5S > Y5V = Z5U | candidate's grade >= original's |
| `informational` | gate_threshold_voltage, zener_test_current, luminous_intensity (and its _red/_green/_blue keys), supply_voltage, actuation_force, travel | shown, not judged |

- A rating compares its guaranteed limit when both parts state one
  (an `at_least` key's `min`, an `at_most` key's `max`), else the stated
  value (typ, else the middle of min/max). A tolerance compares its
  larger magnitude.
- A key the original states and the candidate doesn't is `unverified`,
  never a pass. A key only the candidate states is not checked.
- Identity keys (`identity: true`): capacitance, resistance,
  inductance, frequency, zener_voltage, color, data_interface, polarity,
  output_type, output_voltage, capacity. One that differs makes the candidate a
  different part. The category's required identity keys (a capacitor's
  capacitance, a resistor's resistance, an inductor's inductance, a
  crystal's frequency, an LED's colour, a transistor's polarity, a
  memory's capacity, a linear regulator's output voltage) must be
  stated by the original too: when it isn't, nothing is `meets`.

The verdict: `meets` (every check passes or is better), `unverified`
(none fails, some unverified), `worse` (some check fails).

## 1.5 Ranking

Listed replacements first, then by class (drop-in, footprint-compatible,
functional, near miss), then by score:

- class: drop-in 300, footprint-compatible 200, functional 100, near
  miss 50; listed replacement +1000;
- verdict: meets +50, worse -50; +2 per key better than the original,
  -5 per unverified key, -10 per failing key;
- lifecycle: active +20, unknown/pre-release +5, NRND -20, EOL -40,
  obsolete -60;
- existence: verified +10, probable +5, rejected -100;
- the original's manufacturer +3.

---

# 2. BOM check

`POST /v1/bom/check` takes the BOM file as the request body (UTF-8, or
UTF-16 with a byte-order mark): at most 2 MB (larger: `413`) and 5000
rows or netlist components; anything that isn't a BOM is `400` with
`{ "error": ... }`.

## 2.1 Formats

The format is detected:

- **KiCad netlist** (`.net`): an s-expression `(export ...)`;
  components with their `value`, `footprint`, `datasheet` and user
  `fields` (`MPN`, `Manufacturer`...). Identical components are grouped
  into one line.
- **KiCad BOM CSV**: Reference, Value and Footprint columns (KiCad 8's
  BOM export, the legacy `bom_csv_grouped_by_value` script with its
  metadata lines before the header), plus custom fields.
- **Generic CSV**: an MPN column, optionally a manufacturer and a
  quantity column.

CSV is read as spreadsheets write it: a byte-order mark, `,` `;` or tab
delimiters (detected), quoted fields with `""` and line breaks, CRLF.
Columns are found by header name in any case and spacing: reference
(`Reference`, `Refs`, `Designator`...), value (`Value`, `Comment`),
footprint (`Footprint`, `Package`), quantity (`Qty`, `Quantity`), MPN
(`MPN`, `Manufacturer Part Number`, `Mfr. Part #`, `MFR PN`, `Part
Number`...), manufacturer (`Manufacturer`, `MFR`, `Mfg`), DNP. Reference
lists (`C1, C2`, `C1 C2`) and ranges (`R1-R4`) are expanded.

## 2.2 Matching

Each line is matched by the strongest evidence it carries:

1. `exact`: the MPN column (else the Value, which KiCad designs often
   set to the MPN) is a Part's MPN; a manufacturer column narrows it
   (by manufacturer id, display name, its first word or initials).
2. `normalized`: equal after the search index's folding (case,
   full-width characters, separators: `rc0603fr-07 10kl`).
3. `variant`: one normalized MPN is the other plus a suffix, at least 6
   characters in common (`AP2112K-3.3` -> `AP2112K-3.3TRG1`).
4. `value_footprint`: for a passive (reference `C`/`R`/`L`, or a value
   with a capacitance/inductance unit), the value (`100nF`, `4k7`,
   `10uH`, optionally `50V`, `X7R`, `1%` after it) and a chip size from
   the footprint name (`C_0603_1608Metric`) select the parts with that
   value and size code (a stated voltage/tolerance/dielectric must be
   met).
5. `none`: not found.

Confidence: exact 1.0, normalized 0.95, variant 0.5-0.8 (by the share of
characters in common), value + footprint 0.6 (0.4 without a size); an
MPN taken from the Value when the BOM also has an MPN column, or
several parts at once, lower it.

## 2.3 Checks

For the best match: lifecycle (`obsolete`, `eol_announced`, `nrnd`:
warning; `unknown`: info), existence (`rejected`: warning; `unverified`
/ `probable`: info), data coverage (which of the category's catalog
parameters the part states), and the footprint name against the part's
package (`0805` on the BOM, `0603` part: warning; chip size codes,
`SOT-23-5`, `SOIC-8`, `QFN-56`, `SOT-223`, `TO-252`... are read from
KiCad footprint names). EOL/NRND/obsolete lines get the part's top three
alternatives (section 1) that are not EOL or obsolete themselves and
not of another pinout (`footprint_compatible`).

Line status: `not_found`; `warning` (any warning); `review` (any info,
or an inexact match); `ok`. The summary counts lines by status and by
match kind.
