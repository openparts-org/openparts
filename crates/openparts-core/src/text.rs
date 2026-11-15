//! Text normalisation shared by part search and BOM matching: the same
//! folding on both sides, so `ＲＰ２０４０`, `rp-2040` and `RP 2040` compare
//! equal, and an MPN normalises the way the search index stores it.

/// Full normalisation: [`fold`], then per whitespace word value
/// notation and separator removal, joined with spaces.
pub fn normalize(s: &str) -> String {
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
pub fn fold(s: &str) -> String {
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
pub fn normalize_folded(w: &str) -> String {
    let w = infix_value(w).unwrap_or_else(|| w.to_string());
    let w = trim_decimal_zeros(&w);
    w.chars().filter(|&c| !is_separator(c)).collect()
}

pub fn is_separator(c: char) -> bool {
    matches!(
        c,
        '-' | '_' | '.' | '/' | ',' | '(' | ')' | '+' | '#' | '\'' | '"' | ':' | ';'
    )
}

/// `<digits><unit><digit>` as `<digits>.<digit><unit>` (`3v3`, `4k7`,
/// `2n2`). Only a whole word with a single trailing digit, so MPNs
/// such as `1n4148` stay as they are.
pub fn infix_value(w: &str) -> Option<String> {
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
pub fn trim_decimal_zeros(w: &str) -> String {
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

/// An MPN as the search index stores it: [`normalize`] with the spaces
/// dropped too (`RC0603FR-07 10KL` and `rc0603fr0710kl` are one MPN).
pub fn normalize_mpn(s: &str) -> String {
    normalize(s).replace(' ', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpns_normalise_like_the_index() {
        assert_eq!(normalize_mpn("RC0603FR-07 10KL"), "rc0603fr0710kl");
        assert_eq!(normalize_mpn("ＡＰ２１１２Ｋ－３．３"), "ap2112k33");
        assert_eq!(normalize_mpn("LM1117MPX-3.3/NOPB"), "lm1117mpx33nopb");
    }
}
