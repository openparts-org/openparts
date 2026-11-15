//! The comments of a data file, kept across re-formatting: the header
//! (before the first key), the comment block before each top-level key,
//! and the trailer. A comment inside a value (an indented line, or one
//! after a value) moves before its top-level key -- its text is kept,
//! its place within the value is not.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Comments {
    pub header: Vec<String>,
    /// Top-level key -> the comment lines before it ("" = a blank line
    /// between two comment groups).
    pub before: BTreeMap<String, Vec<String>>,
    pub trailer: Vec<String>,
}

/// The start of a ` #` comment outside quotes in a value line.
fn inline_comment(line: &str) -> Option<usize> {
    let (mut single, mut double, mut escaped) = (false, false, false);
    let mut prev = ' ';
    for (i, c) in line.char_indices() {
        if double {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                double = false;
            }
        } else if single {
            if c == '\'' {
                single = false;
            }
        } else {
            match c {
                '"' if prev.is_whitespace() || "[{,:".contains(prev) => double = true,
                '\'' if prev.is_whitespace() || "[{,:".contains(prev) => single = true,
                '#' if prev.is_whitespace() => return Some(i),
                _ => {}
            }
        }
        prev = c;
    }
    None
}

fn top_level_key(line: &str) -> Option<String> {
    if line.starts_with(|c: char| c.is_whitespace() || c == '#' || c == '-') {
        return None;
    }
    let colon = line.find(':')?;
    let key = line[..colon].trim();
    Some(key.trim_matches('"').trim_matches('\'').to_string())
}

fn push(list: &mut Vec<String>, pending: &mut Vec<String>) {
    while pending.last().is_some_and(String::is_empty) {
        pending.pop();
    }
    while pending.first().is_some_and(String::is_empty) {
        pending.remove(0);
    }
    if pending.is_empty() {
        return;
    }
    if !list.is_empty() {
        list.push(String::new());
    }
    list.append(pending);
}

pub fn extract(text: &str) -> Comments {
    let mut out = Comments::default();
    let mut pending: Vec<String> = Vec::new();
    // Comments found inside the current top-level value.
    let mut inner: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !pending.is_empty() {
                pending.push(String::new());
            }
            continue;
        }
        if trimmed == "---" || trimmed == "..." {
            continue;
        }
        if trimmed.starts_with('#') {
            if line.starts_with('#') {
                pending.push(trimmed.to_string());
            } else {
                // Indented: inside the current value.
                inner.push(trimmed.to_string());
            }
            continue;
        }
        if let Some(key) = top_level_key(line) {
            if let Some(k) = current.take() {
                let list = out.before.entry(k).or_default();
                push(list, &mut inner);
            }
            if out.before.is_empty()
                && out.header.is_empty()
                && current.is_none()
                && !seen_key(&out)
            {
                push(&mut out.header, &mut pending);
            } else {
                let list = out.before.entry(key.clone()).or_default();
                push(list, &mut pending);
            }
            mark_seen(&mut out, &key);
            current = Some(key);
        } else if !pending.is_empty() {
            // A column-0 comment followed by a nested line: inside.
            inner.extend(pending.drain(..).filter(|l| !l.is_empty()));
        }
        if let Some(i) = inline_comment(line) {
            inner.push(line[i..].trim().to_string());
        }
    }
    if let Some(k) = current.take() {
        let list = out.before.entry(k).or_default();
        push(list, &mut inner);
    }
    push(&mut out.trailer, &mut pending);
    out.before.retain(|_, v| !v.is_empty());
    out
}

// The first key seen is recorded as an empty `before` entry so the
// header is only the block before it.
fn seen_key(c: &Comments) -> bool {
    !c.before.is_empty()
}

fn mark_seen(c: &mut Comments, key: &str) {
    c.before.entry(key.to_string()).or_default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_header_body_inner_and_trailer() {
        let text = "# head 1\n# head 2\nschema_version: \"0.2\"\nkind: part\n\n# about pins\n#\n# more\npins:\n  # inner\n  \"1\": { name: A }  # trailing\nother: \"a # not a comment\"\n# tail\n";
        let c = extract(text);
        assert_eq!(c.header, ["# head 1", "# head 2"]);
        assert_eq!(
            c.before["pins"],
            ["# about pins", "#", "# more", "", "# inner", "# trailing"]
        );
        assert!(!c.before.contains_key("other"));
        assert_eq!(c.trailer, ["# tail"]);
    }
}
