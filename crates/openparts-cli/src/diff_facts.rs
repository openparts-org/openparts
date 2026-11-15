//! `openparts diff-facts`: the facts a change to openparts-data adds,
//! changes or removes, each with the evidence its provenance cites --
//! the review table a pull request gets as a comment (openparts-data's
//! `.github/workflows`), so a reviewer can check every new value against
//! the cited page.
//!
//! It reads the two trees as plain YAML (a document the typed loaders
//! reject still shows), compares every value outside `provenance` and
//! `review`, and reports one row per changed fact: the provenance key
//! that covers the value when there is one and its value is short, else
//! the smallest record (a pin, a dimension) or the value itself. A
//! provenance change without a value change is a row too ("evidence
//! changed"), and so is a change of the `review` block. A document that
//! changed but still carries the `review` block it had is called out:
//! its block should have been dropped (section 26.1).

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

/// Longest value shown as one unit; longer ones are split into leaves.
const UNIT_MAX: usize = 120;
/// GitHub refuses comments over 65536 characters.
const MARKDOWN_MAX: usize = 60_000;

/// One evidence entry, with its Source's URL when the Source is known.
#[derive(Debug, Clone, PartialEq)]
pub struct Evidence {
    pub source: String,
    pub page: Option<u64>,
    pub locator: Option<String>,
    pub url: Option<String>,
}

/// One changed fact.
#[derive(Debug, Clone, PartialEq)]
pub struct FactChange {
    /// The file, relative to the tree root.
    pub file: String,
    /// JSON Pointer of the value within the document.
    pub pointer: String,
    pub old: Option<Value>,
    pub new: Option<Value>,
    /// The value is unchanged; only its cited evidence changed.
    pub evidence_only: bool,
    pub evidence: Vec<Evidence>,
}

/// A document that changed and still carries its old `review` block.
#[derive(Debug, Clone, PartialEq)]
pub struct StaleReview {
    pub file: String,
    pub pr: Option<u64>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct FactDiff {
    pub changes: Vec<FactChange>,
    pub stale_reviews: Vec<StaleReview>,
    /// Files that are no YAML document (shown, not compared).
    pub unreadable: Vec<String>,
}

/// Every `*.yaml` document under `root` (hidden directories skipped), by
/// path relative to `root`; files that don't parse are listed apart.
fn load_tree(root: &Path) -> anyhow::Result<(BTreeMap<String, Value>, Vec<String>)> {
    let mut files = Vec::new();
    openparts_data::yaml_files(root, &mut files)?;
    let mut docs = BTreeMap::new();
    let mut bad = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let parsed = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_yaml::from_str::<serde_yaml::Value>(&t).ok())
            .and_then(|y| serde_json::to_value(y).ok());
        match parsed {
            Some(v) => {
                docs.insert(rel, v);
            }
            None => bad.push(rel),
        }
    }
    Ok((docs, bad))
}

fn escape_token(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Every leaf (scalar, empty array or object) of `v` below `prefix`.
fn leaves(v: &Value, prefix: &str, out: &mut BTreeMap<String, Value>) {
    match v {
        Value::Object(map) if !map.is_empty() => {
            for (k, child) in map {
                leaves(child, &format!("{prefix}/{}", escape_token(k)), out);
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (i, child) in items.iter().enumerate() {
                leaves(child, &format!("{prefix}/{i}"), out);
            }
        }
        other => {
            out.insert(prefix.to_string(), other.clone());
        }
    }
}

/// The document's values outside `provenance` and `review`, as leaves.
fn value_leaves(doc: &Value) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    if let Value::Object(map) = doc {
        for (k, v) in map {
            if k != "provenance" && k != "review" {
                leaves(v, &format!("/{}", escape_token(k)), &mut out);
            }
        }
    }
    out
}

fn provenance_of(doc: Option<&Value>) -> BTreeMap<String, Value> {
    doc.and_then(|d| d.get("provenance"))
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}

/// `pointer` is `ancestor` or below it.
fn is_within(pointer: &str, ancestor: &str) -> bool {
    pointer == ancestor
        || (pointer.starts_with(ancestor) && pointer[ancestor.len()..].starts_with('/'))
}

/// A short, YAML-flow-like rendering of a value.
pub fn display(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::String(s) => s.clone(),
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(display).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{k}: {}", display(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => other.to_string(),
    }
}

fn short(v: Option<&Value>) -> bool {
    v.is_none_or(|v| display(v).chars().count() <= UNIT_MAX)
}

/// The pointer a changed leaf is reported under: its provenance key
/// when that value is short, else its smallest short ancestor record
/// below the key, else the leaf itself.
fn unit_of(leaf: &str, key: Option<&str>, old: &Value, new: &Value) -> String {
    if let Some(k) = key {
        if short(old.pointer(k)) && short(new.pointer(k)) {
            return k.to_string();
        }
    }
    let mut best = leaf.to_string();
    let mut p = leaf;
    while let Some(i) = p.rfind('/') {
        p = &p[..i];
        if p.is_empty() || key.is_some_and(|k| !is_within(p, k)) {
            break;
        }
        let record = |doc: &Value| {
            doc.pointer(p).is_none_or(|v| {
                v.as_object()
                    .is_some_and(|m| m.values().all(|c| !c.is_object() && !c.is_array()))
                    && display(v).chars().count() <= UNIT_MAX
            })
        };
        if record(old) && record(new) {
            best = p.to_string();
        } else {
            break;
        }
    }
    best
}

/// Evidence entries of a provenance value, with Source URLs.
fn evidence_of(entries: Option<&Value>, urls: &BTreeMap<String, String>) -> Vec<Evidence> {
    entries
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|e| {
                    let source = e
                        .get("source")
                        .map(|s| s.as_str().map_or_else(|| s.to_string(), String::from))
                        .unwrap_or_default();
                    Evidence {
                        url: urls.get(&source).cloned(),
                        page: e.get("page").and_then(Value::as_u64),
                        locator: e.get("locator").and_then(Value::as_str).map(String::from),
                        source,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Source id -> url, from `kind: source` documents.
fn source_urls<'a>(docs: impl Iterator<Item = &'a Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for d in docs {
        if d.get("kind").and_then(Value::as_str) == Some("source") {
            if let (Some(id), Some(url)) = (
                d.get("id").and_then(Value::as_str),
                d.get("url").and_then(Value::as_str),
            ) {
                out.insert(id.to_string(), url.to_string());
            }
        }
    }
    out
}

/// The facts that differ between the trees `base` and `head`.
pub fn diff_trees(base: &Path, head: &Path) -> anyhow::Result<FactDiff> {
    let (base_docs, _) = load_tree(base)?;
    let (head_docs, unreadable) = load_tree(head)?;
    Ok(diff_docs(&base_docs, &head_docs, unreadable))
}

pub fn diff_docs(
    base_docs: &BTreeMap<String, Value>,
    head_docs: &BTreeMap<String, Value>,
    unreadable: Vec<String>,
) -> FactDiff {
    // Head's Sources first: a URL fixed in this change is the one to open.
    let mut urls = source_urls(base_docs.values());
    urls.extend(source_urls(head_docs.values()));
    let files: BTreeSet<&String> = base_docs.keys().chain(head_docs.keys()).collect();
    let mut diff = FactDiff {
        unreadable,
        ..Default::default()
    };
    let empty = Value::Object(Default::default());
    for file in files {
        let (old_doc, new_doc) = (base_docs.get(file), head_docs.get(file));
        if old_doc == new_doc {
            continue;
        }
        let (old, new) = (old_doc.unwrap_or(&empty), new_doc.unwrap_or(&empty));
        let (old_prov, new_prov) = (provenance_of(old_doc), provenance_of(new_doc));
        let keys: BTreeSet<&String> = old_prov.keys().chain(new_prov.keys()).collect();
        // The longest provenance key covering `leaf`.
        let key_of = |leaf: &str| {
            keys.iter()
                .filter(|k| is_within(leaf, k))
                .max_by_key(|k| k.len())
                .map(|k| k.as_str())
        };
        let (old_leaves, new_leaves) = (value_leaves(old), value_leaves(new));
        let leaf_ptrs: BTreeSet<&String> = old_leaves.keys().chain(new_leaves.keys()).collect();
        let mut units: BTreeMap<String, Option<String>> = BTreeMap::new();
        for leaf in leaf_ptrs {
            if old_leaves.get(leaf) != new_leaves.get(leaf) {
                let key = key_of(leaf);
                units
                    .entry(unit_of(leaf, key, old, new))
                    .or_insert(key.map(String::from));
            }
        }
        let mut changes: Vec<FactChange> = units
            .into_iter()
            .map(|(pointer, key)| {
                let entries = key
                    .as_deref()
                    .and_then(|k| new_prov.get(k).or_else(|| old_prov.get(k)));
                FactChange {
                    file: file.clone(),
                    old: old_doc.and_then(|d| d.pointer(&pointer)).cloned(),
                    new: new_doc.and_then(|d| d.pointer(&pointer)).cloned(),
                    evidence_only: false,
                    evidence: evidence_of(entries, &urls),
                    pointer,
                }
            })
            .collect();
        for key in &keys {
            let (o, n) = (old_prov.get(*key), new_prov.get(*key));
            let covered = changes.iter().any(|c| is_within(&c.pointer, key));
            if o != n && n.is_some() && !covered {
                changes.push(FactChange {
                    file: file.clone(),
                    pointer: key.to_string(),
                    old: old.pointer(key).cloned(),
                    new: new.pointer(key).cloned(),
                    evidence_only: true,
                    evidence: evidence_of(n, &urls),
                });
            }
        }
        let (old_review, new_review) = (old.get("review"), new.get("review"));
        if old_review != new_review {
            changes.push(FactChange {
                file: file.clone(),
                pointer: "/review".into(),
                old: old_review.cloned(),
                new: new_review.cloned(),
                evidence_only: false,
                evidence: vec![],
            });
        } else if old_review.is_some() && old_doc.is_some() && new_doc.is_some() {
            diff.stale_reviews.push(StaleReview {
                file: file.clone(),
                pr: old_review.and_then(|r| r.get("pr")).and_then(Value::as_u64),
            });
        }
        changes.sort_by(|a, b| a.pointer.cmp(&b.pointer));
        diff.changes.extend(changes);
    }
    diff
}

/// Markdown-escapes free text for a table cell.
fn md_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '|' | '\\' | '`' | '*' | '_' | '[' | ']' | '#' | '~' => {
                out.push('\\');
                out.push(c);
            }
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            // No @-mentions from data.
            '@' => out.push_str("&#64;"),
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// A value as inline code in a table cell.
fn md_code(v: Option<&Value>) -> String {
    match v {
        None => "*(none)*".into(),
        Some(v) => {
            let mut s: String = display(v)
                .chars()
                .map(|c| match c {
                    '`' => '\'',
                    '\n' | '\r' => ' ',
                    c => c,
                })
                .collect();
            if s.chars().count() > 300 {
                s = s.chars().take(300).collect::<String>() + "…";
            }
            format!("`{}`", s.replace('|', "\\|"))
        }
    }
}

/// An http(s) URL safe inside a Markdown link, or `None`.
fn md_url(url: &str, page: Option<u64>) -> Option<String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return None;
    }
    let mut out = String::new();
    for c in url.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '"' => out.push_str("%22"),
            '|' => out.push_str("%7C"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    let base = out.split('#').next().unwrap_or("").to_string();
    Some(match page {
        Some(p) => format!("{base}#page={p}"),
        None => base,
    })
}

/// The review table, as Markdown (one PR comment).
pub fn markdown(diff: &FactDiff) -> String {
    let mut out = String::new();
    let files: BTreeSet<&str> = diff.changes.iter().map(|c| c.file.as_str()).collect();
    let _ = writeln!(out, "### Facts changed by this pull request\n");
    if diff.changes.is_empty() {
        let _ = writeln!(out, "No data values changed.");
    } else {
        let _ = writeln!(
            out,
            "{} changed fact(s) in {} file(s). Check each new value against the cited page \
             (the link opens the source at that page; the locator says where on it).\n",
            diff.changes.len(),
            files.len()
        );
    }
    if !diff.stale_reviews.is_empty() {
        let _ = writeln!(
            out,
            "> [!WARNING]\n> These files changed but still carry their old `review` block. \
             Drop it (the changed values need a new review), or review the changes and update it:"
        );
        for s in &diff.stale_reviews {
            let pr =
                s.pr.map_or(String::new(), |p| format!(" (reviewed in #{p})"));
            let _ = writeln!(out, "> - `{}`{pr}", s.file.replace('`', "'"));
        }
        out.push('\n');
    }
    if !diff.unreadable.is_empty() {
        let _ = writeln!(out, "Files that are not readable YAML (see the validator):");
        for f in &diff.unreadable {
            let _ = writeln!(out, "- `{}`", f.replace('`', "'"));
        }
        out.push('\n');
    }
    if diff.changes.is_empty() {
        return out;
    }
    out.push_str("| File | Pointer | Old → New | Source | Page | Locator |\n");
    out.push_str("|---|---|---|---|---|---|\n");
    for (i, c) in diff.changes.iter().enumerate() {
        let change = if c.evidence_only {
            format!("{} *(evidence changed)*", md_code(c.new.as_ref()))
        } else {
            format!("{} → {}", md_code(c.old.as_ref()), md_code(c.new.as_ref()))
        };
        let join = |f: &dyn Fn(&Evidence) -> String, none: &str| {
            if c.evidence.is_empty() {
                none.to_string()
            } else {
                c.evidence.iter().map(f).collect::<Vec<_>>().join("<br>")
            }
        };
        let no_source = if c.pointer == "/review" {
            ""
        } else {
            "*no provenance*"
        };
        let source = join(
            &|e| {
                let name = md_text(&e.source);
                match e.url.as_deref().and_then(|u| md_url(u, e.page)) {
                    Some(url) => format!("[{name}]({url})"),
                    None => name,
                }
            },
            no_source,
        );
        let page = join(&|e| e.page.map_or("-".into(), |p| p.to_string()), "");
        let locator = join(&|e| e.locator.as_deref().map_or("-".into(), md_text), "");
        let row = format!(
            "| `{}` | `{}` | {change} | {source} | {page} | {locator} |\n",
            c.file.replace('`', "'").replace('|', "\\|"),
            c.pointer.replace('`', "'").replace('|', "\\|"),
        );
        if out.len() + row.len() > MARKDOWN_MAX {
            let _ = writeln!(
                out,
                "\n… and {} more row(s), not shown (comment size limit). Run \
                 `openparts diff-facts --base <base> --head <head>` locally for all of them.",
                diff.changes.len() - i
            );
            break;
        }
        out.push_str(&row);
    }
    out
}

/// The same, as plain text lines.
pub fn plain(diff: &FactDiff) -> String {
    let mut out = String::new();
    for c in &diff.changes {
        let old = c.old.as_ref().map_or("(none)".into(), display);
        let new = c.new.as_ref().map_or("(none)".into(), display);
        let change = if c.evidence_only {
            format!("{new} (evidence changed)")
        } else {
            format!("{old} -> {new}")
        };
        let _ = writeln!(out, "{} {}: {change}", c.file, c.pointer);
        for e in &c.evidence {
            let page = e.page.map_or(String::new(), |p| format!(" p.{p}"));
            let locator = e
                .locator
                .as_deref()
                .map_or(String::new(), |l| format!(" \"{l}\""));
            let url = e
                .url
                .as_deref()
                .and_then(|u| md_url(u, e.page))
                .map_or(String::new(), |u| format!(" <{u}>"));
            let _ = writeln!(out, "    {}{page}{locator}{url}", e.source);
        }
    }
    for s in &diff.stale_reviews {
        let _ = writeln!(
            out,
            "warning: {} changed but still carries its review block",
            s.file
        );
    }
    for f in &diff.unreadable {
        let _ = writeln!(out, "warning: {f} is not readable YAML");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docs(files: &[(&str, &str)]) -> BTreeMap<String, Value> {
        files
            .iter()
            .map(|(f, y)| {
                let v: serde_yaml::Value = serde_yaml::from_str(y).unwrap();
                (f.to_string(), serde_json::to_value(v).unwrap())
            })
            .collect()
    }

    const SOURCE: &str = "kind: source\nid: ex/DS1@1\nurl: https://example.com/ds 1.pdf\n";
    const PKG: &str = "kind: package\nid: ex/P\npitch: {nominal: 0.5, unit: mm}\n\
        dimensions:\n  body_width: {nominal: 3.9, unit: mm}\n  body_length: {nominal: 4.9, unit: mm}\n\
        provenance:\n  /pitch: [{source: ex/DS1@1, page: 4, locator: 'Table 2, \"e\"'}]\n\
        \x20 /dimensions/body_width: [{source: ex/DS1@1, page: 4, locator: 'E1 | max'}]\n";

    #[test]
    fn a_changed_dimension_is_one_row_with_its_evidence() {
        let base = docs(&[("sources/ds.yaml", SOURCE), ("packages/p.yaml", PKG)]);
        let head = docs(&[
            ("sources/ds.yaml", SOURCE),
            ("packages/p.yaml", &PKG.replace("3.9", "4.0")),
        ]);
        let d = diff_docs(&base, &head, vec![]);
        assert_eq!(d.changes.len(), 1, "{d:?}");
        let c = &d.changes[0];
        assert_eq!(c.pointer, "/dimensions/body_width");
        assert_eq!(display(c.old.as_ref().unwrap()), "{nominal: 3.9, unit: mm}");
        assert_eq!(display(c.new.as_ref().unwrap()), "{nominal: 4.0, unit: mm}");
        assert_eq!(c.evidence[0].page, Some(4));
        let md = markdown(&d);
        assert!(
            md.contains("[ex/DS1&#64;1](https://example.com/ds%201.pdf#page=4)"),
            "{md}"
        );
        // The locator's pipe can't break the table.
        assert!(md.contains("E1 \\| max"), "{md}");
    }

    #[test]
    fn values_without_provenance_and_new_files_are_listed() {
        let base = docs(&[("packages/p.yaml", PKG)]);
        let head = docs(&[
            (
                "packages/p.yaml",
                &PKG.replace("body_length: {nominal: 4.9", "body_length: {nominal: 5.0"),
            ),
            ("sources/ds.yaml", SOURCE),
        ]);
        let d = diff_docs(&base, &head, vec![]);
        let rows: Vec<(&str, &str)> = d
            .changes
            .iter()
            .map(|c| (c.file.as_str(), c.pointer.as_str()))
            .collect();
        assert!(
            rows.contains(&("packages/p.yaml", "/dimensions/body_length")),
            "{rows:?}"
        );
        assert!(rows.contains(&("sources/ds.yaml", "/url")), "{rows:?}");
        assert!(markdown(&d).contains("*no provenance*"));
    }

    #[test]
    fn evidence_only_changes_and_review_blocks() {
        let reviewed = format!("{PKG}review: {{status: reviewed, by: [alice], pr: 3}}\n");
        let base = docs(&[("packages/p.yaml", &reviewed)]);
        // A new page for the same value; the review block kept: stale.
        let head = docs(&[(
            "packages/p.yaml",
            &reviewed.replace("page: 4, locator: 'T", "page: 5, locator: 'T"),
        )]);
        let d = diff_docs(&base, &head, vec![]);
        assert_eq!(d.changes.len(), 1, "{d:?}");
        assert!(d.changes[0].evidence_only);
        assert_eq!(d.changes[0].pointer, "/pitch");
        assert_eq!(
            d.stale_reviews,
            [StaleReview {
                file: "packages/p.yaml".into(),
                pr: Some(3)
            }]
        );
        assert!(markdown(&d).contains("still carry their old `review` block"));

        // Marking it reviewed is a row of its own, not a warning.
        let d = diff_docs(&docs(&[("packages/p.yaml", PKG)]), &base, vec![]);
        assert_eq!(d.changes.len(), 1);
        assert_eq!(d.changes[0].pointer, "/review");
        assert!(d.stale_reviews.is_empty());
    }

    #[test]
    fn long_values_split_into_records() {
        let pins = |name: &str| {
            let mut y = String::from("kind: device\nid: ex/D\npins:\n");
            for n in 1..=20 {
                let pin = if n == 7 { name } else { "GPIO" };
                y += &format!("  \"{n}\": {{name: {pin}{n}, type: bidirectional}}\n");
            }
            y + "provenance:\n  /pins: [{source: ex/DS1@1, page: 9}]\n"
        };
        let d = diff_docs(
            &docs(&[("devices/d.yaml", &pins("GPIO"))]),
            &docs(&[("devices/d.yaml", &pins("ADC"))]),
            vec![],
        );
        assert_eq!(d.changes.len(), 1, "{d:?}");
        assert_eq!(d.changes[0].pointer, "/pins/7");
        assert_eq!(d.changes[0].evidence[0].page, Some(9));
    }

    #[test]
    fn markdown_escapes_data() {
        assert_eq!(md_text("@alice <b>|x"), "&#64;alice &lt;b&gt;\\|x");
        assert_eq!(md_url("javascript:alert(1)", None), None);
        assert_eq!(
            md_url("https://a.example/x.pdf#page=2", Some(7)).as_deref(),
            Some("https://a.example/x.pdf#page=7")
        );
        assert_eq!(md_code(Some(&Value::String("a`b|c".into()))), "`a'b\\|c`");
    }

    #[test]
    fn identical_trees_have_no_rows() {
        let t = docs(&[("packages/p.yaml", PKG)]);
        let d = diff_docs(&t, &t, vec![]);
        assert!(d.changes.is_empty());
        assert!(markdown(&d).contains("No data values changed."));
    }
}
