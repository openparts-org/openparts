//! Part search for `GET /v1/search`.
//!
//! Each Part is indexed under more than its MPN: manufacturer id and
//! display name, the Device's id and family, the category, the Package's
//! id, family, lead count, pitch and body size (plus imperial/metric
//! chip codes), and the Device's pin names. Text is normalised the same
//! way on both sides (full-width to ASCII, case, diacritics, `µ`/`Ω`,
//! trailing decimal zeros, `3v3`-style values, separators removed), so
//! `ＲＰ２０４０`, `rp-2040` and `rp 2040` all find the RP2040. A query is
//! split into words that must all match (AND); Japanese and common
//! English terms are first rewritten to the indexed vocabulary.

use openparts_core::{ComponentCategory, PackageFamily, Part};
use openparts_data::Dataset;

/// What a match was found in; the weight orders the results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Mpn,
    Device,
    Manufacturer,
    Category,
    Package,
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
            Field::Pin => 3,
        }
    }
}

/// One Part's searchable text, already normalised.
#[derive(Debug)]
struct Entry {
    mpn: String,
    terms: Vec<(Field, Term)>,
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
    for (field, text) in &e.terms {
        if text.matches(word) {
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
    if let Some(d) = ds.devices.get(&p.device.0) {
        add(Field::Device, &d.id.0);
        if let Some(f) = &d.family {
            add(Field::Device, f);
        }
        add(Field::Category, category_name(d.category));
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
        if let Some(pitch) = pkg.pitch.nominal {
            add(Field::Package, &format!("{pitch}mm"));
        }
        let body = (
            pkg.dimensions.body_length.nominal,
            pkg.dimensions.body_width.nominal,
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
    }
    Entry {
        mpn: normalize(&p.mpn).replace(" ", ""),
        terms,
    }
}

fn category_name(c: ComponentCategory) -> &'static str {
    match c {
        ComponentCategory::Microcontroller => "microcontroller mcu",
        ComponentCategory::Capacitor => "capacitor",
        ComponentCategory::Resistor => "resistor",
        ComponentCategory::Inductor => "inductor",
        ComponentCategory::Diode => "diode",
        ComponentCategory::Transistor => "transistor",
        ComponentCategory::Led => "led",
        ComponentCategory::Crystal => "crystal",
        ComponentCategory::Fuse => "fuse",
        ComponentCategory::Connector => "connector",
        ComponentCategory::Other => "other",
    }
}

/// Imperial and metric size codes of a chip body (length x width, mm).
fn chip_codes(l: f64, w: f64) -> &'static [&'static str] {
    const SIZES: &[(f64, f64, &[&str])] = &[
        (0.4, 0.2, &["01005", "0402m"]),
        (0.6, 0.3, &["0201", "0603m"]),
        (1.0, 0.5, &["0402", "1005"]),
        (1.6, 0.8, &["0603", "1608"]),
        (2.0, 1.25, &["0805", "2012"]),
        (3.2, 1.6, &["1206", "3216"]),
        (3.2, 2.5, &["1210", "3225"]),
        (4.5, 3.2, &["1812", "4532"]),
        (5.0, 2.5, &["2010", "5025"]),
        (6.4, 3.2, &["2512", "6432"]),
    ];
    SIZES
        .iter()
        .find(|(sl, sw, _)| (l - sl).abs() <= 0.15 && (w - sw).abs() <= 0.15)
        .map_or(&[], |s| s.2)
}

/// Japanese and shorthand terms rewritten to the indexed vocabulary.
/// Longer keys first, so a compound wins over its parts.
const SYNONYMS: &[(&str, &str)] = &[
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

/// Full normalisation: [`fold`], then per whitespace word value
/// notation and separator removal, joined with spaces.
fn normalize(s: &str) -> String {
    fold(s)
        .split_whitespace()
        .map(normalize_folded)
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Character-level folding: full-width ASCII and the ideographic space
/// to ASCII, lowercase, Latin diacritics dropped, `µ`/`μ` to `u`, `Ω` to
/// `ohm`.
fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let c = match c {
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            '\u{3000}' => ' ',
            _ => c,
        };
        match c {
            '\u{00B5}' | '\u{03BC}' => out.push('u'),
            '\u{03A9}' | '\u{2126}' => out.push_str("ohm"),
            'ß' => out.push_str("ss"),
            'à'..='å' | 'À'..='Å' => out.push('a'),
            'è'..='ë' | 'È'..='Ë' => out.push('e'),
            'ì'..='ï' | 'Ì'..='Ï' => out.push('i'),
            'ò'..='ö' | 'Ò'..='Ö' => out.push('o'),
            'ù'..='ü' | 'Ù'..='Ü' => out.push('u'),
            'ç' | 'Ç' => out.push('c'),
            'ñ' | 'Ñ' => out.push('n'),
            _ => out.extend(c.to_lowercase()),
        }
    }
    out
}

/// One folded word: `3v3`/`4k7` to `3.3v`/`4.7k`, trailing decimal
/// zeros trimmed (`12.000` to `12`), then separators removed.
fn normalize_folded(w: &str) -> String {
    let w = infix_value(w).unwrap_or_else(|| w.to_string());
    let w = trim_decimal_zeros(&w);
    w.chars().filter(|&c| !is_separator(c)).collect()
}

fn is_separator(c: char) -> bool {
    matches!(
        c,
        '-' | '_' | '.' | '/' | ',' | '(' | ')' | '+' | '#' | '\'' | '"' | ':' | ';'
    )
}

/// `<digits><unit><digit>` as `<digits>.<digit><unit>` (`3v3`, `4k7`,
/// `2n2`). Only a whole word with a single trailing digit, so MPNs
/// such as `1n4148` stay as they are.
fn infix_value(w: &str) -> Option<String> {
    let b = w.as_bytes();
    let n = b.len();
    if n < 3
        || !b[n - 1].is_ascii_digit()
        || !matches!(b[n - 2], b'v' | b'k' | b'r' | b'u' | b'n' | b'p' | b'm')
    {
        return None;
    }
    let head = &w[..n - 2];
    if !head.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let unit = if b[n - 2] == b'r' {
        String::new()
    } else {
        (b[n - 2] as char).to_string()
    };
    Some(format!("{head}.{}{unit}", b[n - 1] as char))
}

/// Drops trailing zeros after a decimal point (`12.000mhz` to `12mhz`,
/// `3.30` to `3.3`).
fn trim_decimal_zeros(w: &str) -> String {
    let chars: Vec<char> = w.chars().collect();
    let mut out = String::with_capacity(w.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '.'
            && i > 0
            && chars[i - 1].is_ascii_digit()
            && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())
        {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            let frac: String = chars[i + 1..j].iter().collect();
            let frac = frac.trim_end_matches('0');
            if !frac.is_empty() {
                out.push('.');
                out.push_str(frac);
            }
            i = j;
            continue;
        }
        out.push(c);
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
    fn chip_codes_cover_both_systems() {
        assert_eq!(chip_codes(1.6, 0.8), &["0603", "1608"]);
        assert!(chip_codes(7.0, 7.0).is_empty());
    }
}
