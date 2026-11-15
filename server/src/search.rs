//! Part search for `GET /v1/search`.
//!
//! Each Part is indexed under more than its MPN: manufacturer id and
//! display name, the Device's id and family, the category, the Package's
//! id, family, lead count, pitch and body size (plus imperial/metric
//! chip codes), the Device's pin names, and the Part's electrical
//! parameters as formatted values (`100 nF`, also `0.1 µF`; `10 kΩ`;
//! `X7R`), so `100nF` or `10k` finds the parts that have them. Text is normalised the same
//! way on both sides (full-width to ASCII, case, diacritics, `µ`/`Ω`,
//! trailing decimal zeros, `3v3`-style values, separators removed), so
//! `ＲＰ２０４０`, `rp-2040` and `rp 2040` all find the RP2040. A query is
//! split into words that must all match (AND); Japanese and common
//! English terms are first rewritten to the indexed vocabulary. The
//! package's structure is indexed too: pin rows x pins per row (`1x20`,
//! matched exactly, so `1x2` doesn't find a 1x20 header), connector
//! gender (`male`/`female`, オス/メス) and orientation (`vertical`,
//! `right_angle`, ライトアングル), a shroud and an exposed pad.

pub(crate) use openparts_core::family::{chip_codes, CHIP_SIZES};
use openparts_core::text::{
    fold, infix_value, is_separator, normalize, normalize_folded, trim_decimal_zeros,
};
use openparts_core::{
    effective_parameters, format_value, parameter_spec, ComponentCategory, Gender, Normalized,
    Orientation, PackageFamily, ParameterSpec, Part,
};
use openparts_data::Dataset;

/// What a match was found in; the weight orders the results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Mpn,
    Device,
    Manufacturer,
    Category,
    Package,
    Parameter,
    Pin,
}

impl Field {
    fn weight(self) -> u32 {
        match self {
            Field::Mpn => 40,
            Field::Device => 25,
            Field::Manufacturer => 20,
            Field::Category => 15,
            Field::Package => 15,
            Field::Parameter => 12,
            Field::Pin => 3,
        }
    }
}

/// One Part's searchable text, already normalised.
#[derive(Debug)]
struct Entry {
    mpn: String,
    terms: Vec<(Field, Term)>,
    /// Pin rows x pins per row (`1x20`), matched whole.
    rows: Option<String>,
}

/// Normalised text with its word starts: a query word matches only
/// from a start (after a separator, or where letters and digits meet),
/// so `ldo` does not match inside `axial-do41` but `2040` matches
/// `rp2040` and `0603` matches `c0603c104`.
#[derive(Debug)]
struct Term {
    text: String,
    starts: Vec<usize>,
}

impl Term {
    fn new(s: &str) -> Term {
        let mut text = String::new();
        let mut starts = Vec::new();
        for word in fold(s).split_whitespace() {
            let word = infix_value(word).unwrap_or_else(|| word.to_string());
            let mut boundary = true;
            let mut prev = None;
            for c in trim_decimal_zeros(&word).chars() {
                if is_separator(c) {
                    boundary = true;
                    continue;
                }
                let kind = if c.is_ascii_digit() {
                    0
                } else if c.is_alphabetic() && c.is_ascii() {
                    1
                } else {
                    2
                };
                if boundary || prev != Some(kind) {
                    starts.push(text.len());
                }
                boundary = false;
                prev = Some(kind);
                text.push(c);
            }
        }
        Term { text, starts }
    }

    fn matches(&self, word: &str) -> bool {
        self.starts
            .iter()
            .any(|&i| self.text[i..].starts_with(word))
    }

    /// [`Term::matches`], not followed by another digit.
    fn matches_whole(&self, word: &str) -> bool {
        self.starts.iter().any(|&i| {
            self.text[i..].starts_with(word)
                && !self.text[i + word.len()..].starts_with(|c: char| c.is_ascii_digit())
        })
    }
}

/// The search index over every Part of a dataset, in `Dataset::parts`
/// order.
#[derive(Debug, Default)]
pub struct SearchIndex {
    entries: Vec<Entry>,
}

impl SearchIndex {
    pub fn build(ds: &Dataset) -> Self {
        let entries = ds.parts.iter().map(|p| entry(ds, p)).collect();
        SearchIndex { entries }
    }

    /// Indices into `Dataset::parts` matching every word of `query`,
    /// with their scores (unsorted). An empty query matches everything (score 0).
    pub fn search(&self, query: &str) -> Vec<(usize, u32)> {
        let words = query_words(query);
        let hits: Vec<(usize, u32)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let mut total = 0;
                for alternatives in &words {
                    total += alternatives.iter().filter_map(|w| score(e, w)).max()?;
                }
                Some((i, total))
            })
            .collect();
        hits
    }
}

/// The best score of one normalised word against an entry, or `None`
/// if it matches nothing.
fn score(e: &Entry, word: &str) -> Option<u32> {
    if word.is_empty() {
        return Some(0);
    }
    let mut best = None;
    if e.mpn == word {
        best = Some(100);
    } else if e.mpn.starts_with(word) {
        best = Some(60);
    }
    if e.rows.as_deref() == Some(word) {
        best = best.max(Some(Field::Package.weight()));
    }
    // A rows x pins word (`1x2`) must not match inside a longer number
    // (`1x20` in a package id).
    let whole = crate::catalog::rows_key(word).is_some();
    for (field, text) in &e.terms {
        if (whole && text.matches_whole(word)) || (!whole && text.matches(word)) {
            let s = field.weight();
            if best.is_none_or(|b| s > b) {
                best = Some(s);
            }
        }
    }
    best
}

fn entry(ds: &Dataset, p: &Part) -> Entry {
    let mut terms = Vec::new();
    let mut rows = None;
    let mut add = |field: Field, text: &str| {
        let t = Term::new(text);
        if !t.text.is_empty() {
            terms.push((field, t));
        }
    };
    add(Field::Mpn, &p.mpn);
    add(Field::Mpn, &p.id.0);
    add(Field::Manufacturer, &p.manufacturer.0);
    if let Some(m) = ds.manufacturers.get(&p.manufacturer.0) {
        add(Field::Manufacturer, &m.name);
    }
    let no_params = Default::default();
    let device_params = ds
        .devices
        .get(&p.device.0)
        .map_or(&no_params, |d| &d.parameters);
    for (key, (values, _)) in effective_parameters(&p.parameters, device_params) {
        let Some(spec) = parameter_spec(key) else {
            continue;
        };
        // Every condition variant is a value the part states.
        for value in values {
            if let Ok(n) = value.normalize(spec) {
                for text in parameter_terms(&n, spec) {
                    add(Field::Parameter, &text);
                }
            }
        }
    }
    if let Some(d) = ds.devices.get(&p.device.0) {
        add(Field::Device, &d.id.0);
        if let Some(f) = &d.family {
            add(Field::Device, f);
        }
        // The category and its group: a linear regulator is also found
        // as `power`.
        let mut c = Some(d.category);
        while let Some(cat) = c {
            add(Field::Category, category_name(cat));
            c = cat.parent();
        }
        for pin in d.pins.values() {
            add(Field::Pin, &pin.name);
        }
    }
    if let Some(pkg) = ds.packages.get(&p.package.0) {
        add(Field::Package, &pkg.id.0);
        let n = pkg.lead_count;
        for fam in std::iter::once(pkg.family.name()).chain(pkg.family.aliases().iter().copied()) {
            add(Field::Package, fam);
            add(Field::Package, &format!("{fam}{n}"));
        }
        add(Field::Package, &format!("{n}pin"));
        if let Some(pitch) = pkg.pitch.typ {
            add(Field::Package, &format!("{pitch}mm"));
        }
        let body = (
            pkg.dimensions.body_length.typ,
            pkg.dimensions.body_width.typ,
        );
        if let (Some(l), Some(w)) = body {
            add(Field::Package, &format!("{l}x{w}"));
            add(Field::Package, &format!("{w}x{l}"));
            if pkg.family == PackageFamily::Chip {
                for code in chip_codes(l.max(w), l.min(w)) {
                    add(Field::Package, code);
                }
            }
        }
        let st = pkg.structure();
        rows = st.rows.map(|(r, n)| format!("{r}x{n}"));
        if let Some(g) = st.gender {
            add(
                Field::Package,
                match g {
                    Gender::Male => "male plug",
                    Gender::Female => "female receptacle",
                },
            );
        }
        if let Some(o) = st.orientation {
            add(
                Field::Package,
                match o {
                    Orientation::Vertical => "vertical straight",
                    Orientation::RightAngle => "right_angle horizontal",
                },
            );
        }
        if st.shrouded == Some(true) {
            add(Field::Package, "shrouded box_header");
        }
        if st.exposed_pad == Some(true) {
            add(Field::Package, "exposed_pad thermal_pad epad");
        }
    }
    Entry {
        mpn: normalize(&p.mpn).replace(" ", ""),
        terms,
        rows,
    }
}

/// The ways a parameter value is written: its label (`100 nF`) and, for
/// a value of 100 or more in its prefix, the next prefix up (`0.1 µF`),
/// as capacitors are labelled both ways.
fn parameter_terms(n: &Normalized, spec: &ParameterSpec) -> Vec<String> {
    let mut out = vec![n.label(spec)];
    if let (Some(v), None) = (n.stated(spec), spec.display_unit) {
        if !spec.deviation {
            let label = format_value(v, spec);
            let mantissa: f64 = label
                .split(' ')
                .next()
                .and_then(|m| m.parse().ok())
                .unwrap_or(0.0);
            if mantissa >= 100.0 {
                let up = format_value(v * 1000.0, spec);
                // One prefix up: 100 nF -> (100 µF) / 1000 = 0.1 µF.
                if let Some((m, unit)) = up.split_once(' ') {
                    if let Ok(m) = m.parse::<f64>() {
                        let small = format!("{}", m / 1000.0);
                        out.push(format!("{small} {unit}"));
                    }
                }
            }
        }
    }
    out
}

/// The words a category is found by (the query side rewrites Japanese
/// and shorthand terms to these).
fn category_name(c: ComponentCategory) -> &'static str {
    use ComponentCategory as C;
    match c {
        C::Microcontroller => "microcontroller mcu",
        C::Capacitor => "capacitor",
        C::Resistor => "resistor",
        C::Inductor => "inductor",
        C::Diode => "diode",
        C::Transistor => "transistor",
        C::Led => "led",
        C::Crystal => "crystal",
        C::Fuse => "fuse",
        C::Connector => "connector",
        C::Other => "other",
        C::Optocoupler => "optocoupler photocoupler optoisolator",
        C::Power => "power",
        C::LinearRegulator => "linear_regulator regulator ldo",
        C::SwitchingRegulator => "switching_regulator regulator dc-dc converter buck boost",
        C::BatteryCharger => "battery_charger charger",
        C::VoltageReference => "voltage_reference reference",
        C::PowerSwitch => "power_switch load_switch power_mux",
        C::Memory => "memory",
        C::Flash => "flash",
        C::Eeprom => "eeprom",
        C::Sram => "sram ram",
        C::Sensor => "sensor",
        C::PressureSensor => "pressure_sensor pressure barometer",
        C::Imu => "imu accelerometer gyroscope motion",
        C::TemperatureSensor => "temperature_sensor temperature",
        C::Analog => "analog",
        C::OpAmp => "op_amp opamp operational_amplifier amplifier",
        C::Comparator => "comparator",
        C::CurrentSenseAmplifier => "current_sense_amplifier current_sense amplifier",
        C::PowerMonitor => "power_monitor current_monitor current_sense",
        C::AnalogSwitch => "analog_switch multiplexer mux",
        C::Adc => "adc a/d_converter",
        C::Dac => "dac d/a_converter",
        C::Interface => "interface",
        C::IoExpander => "io_expander gpio_expander expander",
        C::CanController => "can_controller can",
        C::CanTransceiver => "can_transceiver can transceiver",
        C::Rs485Transceiver => "rs485_transceiver rs485 transceiver",
        C::Rs232Transceiver => "rs232_transceiver rs232 transceiver",
        C::LevelShifter => "level_shifter level_translator",
        C::Logic => "logic",
        C::Timing => "timing",
        C::Timer => "timer",
        C::Rtc => "rtc real_time_clock",
        C::Switch => "switch",
        C::TactSwitch => "tact_switch tactile_switch",
        C::Driver => "driver",
        C::GateDriver => "gate_driver mosfet_driver",
        C::DarlingtonArray => "darlington_array darlington",
    }
}

/// Japanese and shorthand terms rewritten to the indexed vocabulary.
/// Longer keys first, so a compound wins over its parts.
const SYNONYMS: &[(&str, &str)] = &[
    ("ライトアングル", "right_angle"),
    ("シュラウド付き", "shrouded"),
    ("シュラウド", "shrouded"),
    ("ボックスヘッダー", "shrouded"),
    ("ボックスヘッダ", "shrouded"),
    ("露出パッド", "exposed_pad"),
    ("サーマルパッド", "exposed_pad"),
    ("ストレート", "vertical"),
    ("横向き", "right_angle"),
    ("縦型", "vertical"),
    ("垂直", "vertical"),
    ("l型", "right_angle"),
    ("メス", "female"),
    ("オス", "male"),
    ("リアルタイムクロック", "rtc"),
    ("スイッチングレギュレータ", "switching_regulator"),
    ("スイッチングレギュレーター", "switching_regulator"),
    ("リニアレギュレータ", "linear_regulator"),
    ("リニアレギュレーター", "linear_regulator"),
    ("三端子レギュレータ", "linear_regulator"),
    ("三端子レギュレーター", "linear_regulator"),
    ("オペアンプ", "op_amp"),
    ("演算増幅器", "op_amp"),
    ("フォトカプラ", "optocoupler"),
    ("フォトカプラー", "optocoupler"),
    ("温度センサー", "temperature_sensor"),
    ("温度センサ", "temperature_sensor"),
    ("圧力センサー", "pressure_sensor"),
    ("圧力センサ", "pressure_sensor"),
    ("気圧センサー", "pressure_sensor"),
    ("気圧センサ", "pressure_sensor"),
    ("加速度センサー", "imu"),
    ("加速度センサ", "imu"),
    ("ジャイロセンサ", "imu"),
    ("ジャイロ", "imu"),
    ("電流センスアンプ", "current_sense"),
    ("基準電圧源", "voltage_reference"),
    ("基準電圧", "voltage_reference"),
    ("レベルシフタ", "level_shifter"),
    ("レベル変換", "level_shifter"),
    ("トランシーバー", "transceiver"),
    ("トランシーバ", "transceiver"),
    ("マルチプレクサ", "multiplexer"),
    ("ゲートドライバ", "gate_driver"),
    ("エキスパンダ", "expander"),
    ("コンパレーター", "comparator"),
    ("コンパレータ", "comparator"),
    ("レギュレーター", "regulator"),
    ("レギュレータ", "regulator"),
    ("充電IC", "charger"),
    ("充電", "charger"),
    ("フラッシュメモリ", "flash"),
    ("フラッシュ", "flash"),
    ("メモリー", "memory"),
    ("メモリ", "memory"),
    ("センサー", "sensor"),
    ("センサ", "sensor"),
    ("加速度", "imu"),
    ("気圧", "pressure"),
    ("圧力", "pressure"),
    ("ロジックIC", "logic"),
    ("ロジック", "logic"),
    ("タイマー", "timer"),
    ("タイマ", "timer"),
    ("ドライバー", "driver"),
    ("ドライバ", "driver"),
    ("電源IC", "power"),
    ("電源", "power"),
    ("積層セラミックコンデンサ", "capacitor"),
    ("電解コンデンサ", "capacitor"),
    ("タンタルコンデンサ", "capacitor"),
    ("マイクロコントローラ", "microcontroller"),
    ("発光ダイオード", "led"),
    ("水晶振動子", "crystal"),
    ("タクトスイッチ", "tact_switch"),
    ("ピンヘッダー", "header"),
    ("ピンヘッダ", "header"),
    ("ピンソケット", "socket"),
    ("コンデンサー", "capacitor"),
    ("コンデンサ", "capacitor"),
    ("キャパシタ", "capacitor"),
    ("インダクタ", "inductor"),
    ("ダイオード", "diode"),
    ("トランジスタ", "transistor"),
    ("クリスタル", "crystal"),
    ("ヒューズ", "fuse"),
    ("コネクター", "connector"),
    ("コネクタ", "connector"),
    ("ソケット", "socket"),
    ("スイッチ", "switch"),
    ("ヘッダー", "header"),
    ("ヘッダ", "header"),
    ("マイコン", "microcontroller"),
    ("抵抗器", "resistor"),
    ("抵抗", "resistor"),
    ("コイル", "inductor"),
    ("水晶", "crystal"),
    ("発振子", "crystal"),
    ("セラミック", "capacitor"),
    ("村田製作所", "murata"),
    ("ムラタ", "murata"),
    ("村田", "murata"),
    ("東芝", "toshiba"),
    ("パナソニック", "panasonic"),
    ("オムロン", "omron"),
    ("エプソン", "epson"),
    ("ルネサス", "renesas"),
    ("ニチコン", "nichicon"),
    ("日本圧着端子", "jst"),
    ("ビシェイ", "vishay"),
    ("インフィニオン", "infineon"),
    ("ラズベリーパイ", "raspberrypi"),
    ("ラズパイ", "raspberrypi"),
    ("サムスン", "samsung"),
    ("ウルト", "wurth"),
    ("ヴルト", "wurth"),
];

/// Single English words that also mean an indexed term.
const WORD_SYNONYMS: &[(&str, &str)] = &[
    ("cap", "capacitor"),
    ("mlcc", "capacitor"),
    ("res", "resistor"),
    ("xtal", "crystal"),
    ("mosfet", "transistor"),
    ("fet", "transistor"),
    ("bjt", "transistor"),
    ("typec", "usbc"),
    ("usbtypec", "usbc"),
    ("pinheader", "header"),
    ("dcdc", "dc-dc"),
    ("opamp", "op_amp"),
    ("ram", "sram"),
    ("tact", "tact_switch"),
    ("accel", "accelerometer"),
    ("angled", "rightangle"),
    ("horizontal", "rightangle"),
    ("straight", "vertical"),
    ("boxheader", "shrouded"),
    ("epad", "exposedpad"),
];

/// The query as words, each with its alternative spellings (a word
/// matches if any alternative does).
fn query_words(query: &str) -> Vec<Vec<String>> {
    let mut q = fold(query);
    for (ja, en) in SYNONYMS {
        q = q.replace(ja, &format!(" {en} "));
    }
    let words: Vec<&str> = q.split_whitespace().collect();
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        // "usb c" / "type c" read as one word.
        if i + 1 < words.len() && words[i + 1] == "c" && matches!(words[i], "usb" | "type") {
            out.push(vec!["usbc".into()]);
            i += 2;
            continue;
        }
        // "right angle", "exposed pad", "box header" too.
        if i + 1 < words.len() {
            let pair = match (words[i], words[i + 1]) {
                ("right", "angle") => Some("rightangle"),
                ("exposed" | "thermal", "pad") => Some("exposedpad"),
                ("box", "header") => Some("shrouded"),
                _ => None,
            };
            if let Some(w) = pair {
                out.push(vec![w.into()]);
                i += 2;
                continue;
            }
        }
        // Pin rows x pins per row: `1x20`, `1×20`, `1*20`.
        if let Some(key) = crate::catalog::rows_key(words[i]) {
            out.push(vec![key]);
            i += 1;
            continue;
        }
        let w = normalize_folded(words[i]);
        if !w.is_empty() {
            let mut alts = vec![w.clone()];
            if let Some((_, syn)) = WORD_SYNONYMS.iter().find(|(k, _)| *k == w) {
                alts.push((*syn).to_string());
            }
            out.push(alts);
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_spellings() {
        assert_eq!(normalize("ＲＰ２０４０"), "rp2040");
        assert_eq!(normalize("RP-2040"), "rp2040");
        assert_eq!(normalize("Würth"), "wurth");
        assert_eq!(normalize("10µF"), "10uf");
        assert_eq!(normalize("12.000MHz"), "12mhz");
        assert_eq!(normalize("3v3"), normalize("3.3V"));
        assert_eq!(normalize("4k7"), normalize("4.7k"));
        assert_eq!(normalize("1N4148"), "1n4148");
    }

    #[test]
    fn matches_only_from_word_starts() {
        let t = Term::new("standards/AXIAL-DO41");
        assert!(t.matches("do41") && t.matches("axial"));
        assert!(!t.matches("ldo"));
        let t = Term::new("C0603C104K5RACTU");
        assert!(t.matches("0603") && t.matches("104k"));
        assert!(Term::new("RP2040").matches("2040"));
        assert!(Term::new("12.000 MHz").matches("12mhz"));
    }

    #[test]
    fn rewrites_japanese_and_usb_c() {
        assert_eq!(query_words("0603 コンデンサ")[1], vec!["capacitor"]);
        assert_eq!(
            query_words("積層セラミックコンデンサ"),
            vec![vec!["capacitor"]]
        );
        assert_eq!(query_words("USB C"), vec![vec!["usbc"]]);
        assert_eq!(query_words("xtal")[0], vec!["xtal", "crystal"]);
    }

    #[test]
    fn reads_structure_words() {
        assert_eq!(query_words("1x20"), vec![vec!["1x20"]]);
        assert_eq!(query_words("1×20"), vec![vec!["1x20"]]);
        assert_eq!(query_words("2.54 header 1x20")[2], vec!["1x20"]);
        assert_eq!(query_words("right angle"), vec![vec!["rightangle"]]);
        assert_eq!(query_words("ライトアングル"), vec![vec!["rightangle"]]);
        assert_eq!(query_words("ピンソケット メス")[1], vec!["female"]);
        assert_eq!(query_words("オス"), vec![vec!["male"]]);
        assert_eq!(query_words("exposed pad"), vec![vec!["exposedpad"]]);
        let t = Term::new("WR-PHD-PinHeader-1x20-P2.54");
        assert!(t.matches_whole("1x20") && !t.matches_whole("1x2"));
    }

    #[test]
    fn parameter_values_are_searchable_as_typed() {
        let spec = |k: &str| parameter_spec(k).unwrap();
        let value = |n: f64, unit: &str| openparts_core::ParameterValue {
            typ: Some(n),
            unit: Some(unit.into()),
            ..Default::default()
        };
        let terms = |n: f64, unit: &str, k: &str| -> Vec<Term> {
            let n = value(n, unit).normalize(spec(k)).unwrap();
            parameter_terms(&n, spec(k))
                .iter()
                .map(|t| Term::new(t))
                .collect()
        };
        let hit = |terms: &[Term], q: &str| {
            let words = query_words(q);
            words
                .iter()
                .all(|alts| alts.iter().any(|w| terms.iter().any(|t| t.matches(w))))
        };
        let cap = terms(100.0, "nF", "capacitance");
        assert!(hit(&cap, "100nF") && hit(&cap, "100 nF") && hit(&cap, "0.1uF"));
        assert!(hit(&cap, "0.1µF") && !hit(&cap, "10nF"));
        let res = terms(10.0, "kΩ", "resistance");
        assert!(hit(&res, "10k") && hit(&res, "10kΩ") && hit(&res, "10 kohm"));
        let res = terms(4.7, "kohm", "resistance");
        assert!(hit(&res, "4k7") && hit(&res, "4.7k"));
    }

    #[test]
    fn category_terms_cover_the_japanese_names() {
        let indexed = |c: ComponentCategory| {
            let mut terms = vec![Term::new(category_name(c))];
            if let Some(p) = c.parent() {
                terms.push(Term::new(category_name(p)));
            }
            terms
        };
        let hit = |c: ComponentCategory, q: &str| {
            let terms = indexed(c);
            query_words(q)
                .iter()
                .all(|alts| alts.iter().any(|w| terms.iter().any(|t| t.matches(w))))
        };
        use ComponentCategory as C;
        for (q, c) in [
            ("スイッチ", C::TactSwitch),
            ("タクトスイッチ", C::TactSwitch),
            ("レギュレータ", C::LinearRegulator),
            ("レギュレータ", C::SwitchingRegulator),
            ("LDO", C::LinearRegulator),
            ("DC-DC", C::SwitchingRegulator),
            ("dcdc", C::SwitchingRegulator),
            ("フラッシュ", C::Flash),
            ("EEPROM", C::Eeprom),
            ("センサ", C::Imu),
            ("加速度", C::Imu),
            ("気圧", C::PressureSensor),
            ("温度センサ", C::TemperatureSensor),
            ("オペアンプ", C::OpAmp),
            ("コンパレータ", C::Comparator),
            ("ADC", C::Adc),
            ("DAC", C::Dac),
            ("ロジック", C::Logic),
            ("タイマ", C::Timer),
            ("RTC", C::Rtc),
            ("電源", C::BatteryCharger),
            ("CAN トランシーバ", C::CanTransceiver),
        ] {
            assert!(hit(c, q), "{q} should find {c:?}");
        }
        assert!(!hit(C::Flash, "温度センサ"));
        assert!(!hit(C::LinearRegulator, "DC-DC"));
        assert!(!hit(C::Comparator, "オペアンプ"));
    }

    #[test]
    fn chip_codes_cover_both_systems() {
        assert_eq!(chip_codes(1.6, 0.8), &["0603", "1608"]);
        assert!(chip_codes(7.0, 7.0).is_empty());
    }
}
