//! BOM line -> OpenParts parts.
//!
//! Each line is matched by the strongest evidence it carries, in order:
//!
//! 1. **exact** MPN (the MPN column, else the Value, which KiCad
//!    projects often set to the MPN), narrowed by the manufacturer
//!    column when one is given;
//! 2. **normalized** MPN: the search index's folding (case, full-width,
//!    separators: `rc0603fr-07 10kl` = `RC0603FR-0710KL`);
//! 3. **variant**: one MPN is the other plus a suffix (packaging, grade:
//!    `AP2112K-3.3` -> `AP2112K-3.3TRG1`), at least
//!    [`MIN_VARIANT_PREFIX`] characters in common;
//! 4. **value + footprint** for passives: a value (`100nF`, `10k`,
//!    `4.7uH`, with optional `50V`/`X7R`/`1%` after it) and a chip size
//!    from the footprint name (`C_0603_1608Metric`) give the parts with
//!    that capacitance/resistance/inductance and size code;
//! 5. otherwise **not found**.
//!
//! Every match is then checked: lifecycle (EOL, NRND, obsolete,
//! unknown), existence, data coverage (are the category's parameters
//! stated?) and the footprint name against the part's package (the BOM
//! says 0805, the part is 0603). Lines with a lifecycle problem get the
//! part's top alternatives ([`crate::alternatives`]).

use crate::alternatives::{Alternative, AlternativeClass, AlternativesIndex, PartBrief};
use crate::parse::{BomFormat, BomLine, DetectedColumn, ParsedBom};
use crate::rules::{expected_parameters_of, DIELECTRIC_RANKS};
use openparts_core::family::CHIP_SIZES;
use openparts_core::text::normalize_mpn;
use openparts_core::{
    parameter_spec, parse_value, ComponentCategory, ExistenceStatus, LifecycleStatus, Normalized,
    PackageFamily,
};
use openparts_data::Dataset;
use serde::Serialize;
use std::collections::BTreeMap;

/// The shortest common MPN prefix (normalized) a variant match needs.
pub const MIN_VARIANT_PREFIX: usize = 6;
/// Most matches listed per line.
pub const MAX_MATCHES: usize = 8;
/// Alternatives listed per problem line.
pub const MAX_ALTERNATIVES: usize = 3;

/// How a line was matched.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    Exact,
    Normalized,
    Variant,
    ValueFootprint,
    None,
}

/// A line's overall result.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineStatus {
    /// Matched exactly, nothing to look at.
    Ok,
    /// Matched, but worth a look: an inexact match, several
    /// candidates, unknown lifecycle, thin data.
    Review,
    /// A problem: EOL/NRND/obsolete, a footprint mismatch, a rejected
    /// part, another manufacturer.
    Warning,
    NotFound,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
}

/// What a check found.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueCode {
    NotFound,
    VariantMatch,
    ValueMatch,
    Ambiguous,
    ManufacturerMismatch,
    LifecycleObsolete,
    LifecycleEol,
    LifecycleNrnd,
    LifecycleUnknown,
    ExistenceRejected,
    ExistenceUnverified,
    NoParameters,
    FootprintMismatch,
    FootprintUnchecked,
    Dnp,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BomIssue {
    pub code: IssueCode,
    pub severity: Severity,
    /// Specifics, when the code needs them (`0805 / 0603`).
    pub detail: Option<String>,
}

/// How much of the category's expected data the part states.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Coverage {
    /// Parameters stated (any key).
    pub parameters: usize,
    /// Of the category's expected keys, how many are stated.
    pub expected_present: usize,
    pub expected: usize,
    /// Expected keys the data lacks.
    pub missing: Vec<String>,
    /// Sources the part cites.
    pub sources: usize,
}

/// The footprint name against the part's package.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FootprintStatus {
    Match,
    Mismatch,
    /// The footprint name names no package OpenParts can read.
    Unknown,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FootprintNameCheck {
    pub status: FootprintStatus,
    /// What the BOM footprint name says (`0805`, `SOT-23-5`).
    pub bom: Option<String>,
    /// What the part's package is.
    pub part: String,
}

/// One candidate part of a line.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BomMatch {
    pub part: PartBrief,
    pub kind: MatchKind,
    /// 0..1.
    pub confidence: f64,
    pub coverage: Coverage,
    pub footprint: FootprintNameCheck,
}

/// One checked BOM line.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BomLineResult {
    pub row: usize,
    pub references: Vec<String>,
    pub quantity: u32,
    pub value: String,
    pub footprint: String,
    pub mpn: String,
    pub manufacturer: String,
    pub dnp: bool,
    pub status: LineStatus,
    pub kind: MatchKind,
    /// The best match's confidence (0 when not found).
    pub confidence: f64,
    /// Best first.
    pub matches: Vec<BomMatch>,
    pub issues: Vec<BomIssue>,
    /// For EOL/NRND/obsolete matches: the top alternatives.
    pub alternatives: Vec<Alternative>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct BomSummary {
    pub lines: usize,
    /// Sum of the lines' quantities.
    pub components: u64,
    pub ok: usize,
    pub review: usize,
    pub warning: usize,
    pub not_found: usize,
    pub exact: usize,
    pub normalized: usize,
    pub variant: usize,
    pub value_footprint: usize,
}

/// `POST /v1/bom/check`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BomReport {
    pub format: BomFormat,
    pub columns: Vec<DetectedColumn>,
    pub summary: BomSummary,
    pub lines: Vec<BomLineResult>,
}

/// MPN lookup tables over a dataset, in `Dataset::parts` order.
#[derive(Debug, Default)]
pub struct MpnIndex {
    exact: BTreeMap<String, Vec<usize>>,
    normalized: BTreeMap<String, Vec<usize>>,
}

impl MpnIndex {
    pub fn build(ds: &Dataset) -> MpnIndex {
        let mut ix = MpnIndex::default();
        for (i, p) in ds.parts.iter().enumerate() {
            ix.exact.entry(p.mpn.clone()).or_default().push(i);
            ix.normalized
                .entry(normalize_mpn(&p.mpn))
                .or_default()
                .push(i);
        }
        ix
    }

    /// Parts whose normalized MPN extends `n` or is extended by it.
    fn variants(&self, n: &str) -> Vec<usize> {
        let mut out = Vec::new();
        if n.len() < MIN_VARIANT_PREFIX {
            return out;
        }
        // Longer MPNs starting with `n`.
        for (k, v) in self.normalized.range(n.to_string()..) {
            if !k.starts_with(n) {
                break;
            }
            if k != n {
                out.extend(v);
            }
        }
        // Shorter MPNs `n` starts with.
        for len in (MIN_VARIANT_PREFIX..n.len()).rev() {
            if let Some(v) = self.normalized.get(&n[..len]) {
                out.extend(v);
            }
        }
        out
    }
}

/// Everything a BOM check needs, built once per dataset.
#[derive(Debug, Default)]
pub struct BomIndex {
    pub mpns: MpnIndex,
    pub alternatives: AlternativesIndex,
}

impl BomIndex {
    pub fn build(ds: &Dataset) -> BomIndex {
        BomIndex {
            mpns: MpnIndex::build(ds),
            alternatives: AlternativesIndex::build(ds),
        }
    }

    /// Checks every line of `bom`.
    pub fn check(&self, ds: &Dataset, bom: &ParsedBom) -> BomReport {
        let lines: Vec<BomLineResult> = bom.lines.iter().map(|l| self.check_line(ds, l)).collect();
        let mut summary = BomSummary {
            lines: lines.len(),
            ..Default::default()
        };
        for l in &lines {
            summary.components += l.quantity as u64;
            match l.status {
                LineStatus::Ok => summary.ok += 1,
                LineStatus::Review => summary.review += 1,
                LineStatus::Warning => summary.warning += 1,
                LineStatus::NotFound => summary.not_found += 1,
            }
            match l.kind {
                MatchKind::Exact => summary.exact += 1,
                MatchKind::Normalized => summary.normalized += 1,
                MatchKind::Variant => summary.variant += 1,
                MatchKind::ValueFootprint => summary.value_footprint += 1,
                MatchKind::None => {}
            }
        }
        BomReport {
            format: bom.format,
            columns: bom.columns.clone(),
            summary,
            lines,
        }
    }

    /// Matches one line.
    pub fn check_line(&self, ds: &Dataset, line: &BomLine) -> BomLineResult {
        let mut issues = Vec::new();
        let (kind, found) = self.find(ds, line, &mut issues);
        let alt = &self.alternatives;
        let mut matches: Vec<BomMatch> = found
            .iter()
            .take(MAX_MATCHES)
            .map(|&(i, confidence)| {
                let part = &ds.parts[i];
                let category = ds.devices.get(&part.device.0).map(|d| d.category);
                let params = alt.parameters(i);
                let expected = category.map_or(&[][..], |c| {
                    expected_parameters_of(c, |k| params.is_some_and(|p| p.contains_key(k)))
                });
                let missing: Vec<String> = expected
                    .iter()
                    .filter(|k| !params.is_some_and(|p| p.contains_key(**k)))
                    .map(|k| k.to_string())
                    .collect();
                BomMatch {
                    part: alt.brief(ds, i),
                    kind,
                    confidence,
                    coverage: Coverage {
                        parameters: params.map_or(0, |p| p.len()),
                        expected_present: expected.len() - missing.len(),
                        expected: expected.len(),
                        missing,
                        sources: part.sources.len(),
                    },
                    footprint: footprint_check(&line.footprint, alt, ds, i),
                }
            })
            .collect();
        matches.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
        let mut alternatives = Vec::new();
        if let Some(best) = found.first().map(|(i, _)| *i) {
            let part = &ds.parts[best];
            let m = &matches[0];
            let warn = |code, detail: Option<String>| BomIssue {
                code,
                severity: Severity::Warning,
                detail,
            };
            let info = |code, detail: Option<String>| BomIssue {
                code,
                severity: Severity::Info,
                detail,
            };
            match part.lifecycle.status {
                LifecycleStatus::Obsolete => issues.push(warn(IssueCode::LifecycleObsolete, None)),
                LifecycleStatus::EolAnnounced => issues.push(warn(IssueCode::LifecycleEol, None)),
                LifecycleStatus::Nrnd => issues.push(warn(IssueCode::LifecycleNrnd, None)),
                LifecycleStatus::Unknown => issues.push(info(IssueCode::LifecycleUnknown, None)),
                LifecycleStatus::Active | LifecycleStatus::PreRelease => {}
            }
            match part.existence.status {
                ExistenceStatus::Rejected => issues.push(warn(IssueCode::ExistenceRejected, None)),
                ExistenceStatus::Unverified | ExistenceStatus::Probable => issues.push(info(
                    IssueCode::ExistenceUnverified,
                    Some(existence_wire(part.existence.status).to_string()),
                )),
                _ => {}
            }
            if m.coverage.expected > 0 && m.coverage.expected_present == 0 {
                issues.push(info(
                    IssueCode::NoParameters,
                    Some(m.coverage.missing.join(", ")),
                ));
            }
            match m.footprint.status {
                FootprintStatus::Mismatch => issues.push(warn(
                    IssueCode::FootprintMismatch,
                    Some(format!(
                        "{} / {}",
                        m.footprint.bom.clone().unwrap_or_default(),
                        m.footprint.part
                    )),
                )),
                FootprintStatus::Unknown if !line.footprint.is_empty() => {}
                _ => {}
            }
            if matches!(
                part.lifecycle.status,
                LifecycleStatus::Obsolete | LifecycleStatus::EolAnnounced | LifecycleStatus::Nrnd
            ) {
                alternatives = alt
                    .alternatives(ds, best)
                    .alternatives
                    .into_iter()
                    // Replacements only: not one with another pinout.
                    .filter(|a| {
                        a.listed_replacement || a.class != AlternativeClass::FootprintCompatible
                    })
                    .filter(|a| {
                        a.listed_replacement
                            || !matches!(
                                a.part.lifecycle,
                                LifecycleStatus::Obsolete | LifecycleStatus::EolAnnounced
                            )
                    })
                    .take(MAX_ALTERNATIVES)
                    .collect();
            }
        } else {
            issues.push(BomIssue {
                code: IssueCode::NotFound,
                severity: Severity::Warning,
                detail: None,
            });
        }
        if line.dnp {
            issues.push(BomIssue {
                code: IssueCode::Dnp,
                severity: Severity::Info,
                detail: None,
            });
        }
        let status = if found.is_empty() {
            LineStatus::NotFound
        } else if issues.iter().any(|i| i.severity == Severity::Warning) {
            LineStatus::Warning
        } else if !issues.is_empty() || !matches!(kind, MatchKind::Exact | MatchKind::Normalized) {
            LineStatus::Review
        } else {
            LineStatus::Ok
        };
        BomLineResult {
            row: line.row,
            references: line.references.clone(),
            quantity: line.quantity,
            value: line.value.clone(),
            footprint: line.footprint.clone(),
            mpn: line.mpn.clone(),
            manufacturer: line.manufacturer.clone(),
            dnp: line.dnp,
            status,
            kind: if found.is_empty() {
                MatchKind::None
            } else {
                kind
            },
            confidence: matches.first().map_or(0.0, |m| m.confidence),
            matches,
            issues,
            alternatives,
        }
    }

    /// The candidate parts (index, confidence), best first.
    fn find(
        &self,
        ds: &Dataset,
        line: &BomLine,
        issues: &mut Vec<BomIssue>,
    ) -> (MatchKind, Vec<(usize, f64)>) {
        let mut keys: Vec<&str> = Vec::new();
        for k in [line.mpn.trim(), line.value.trim()] {
            if !k.is_empty() && !keys.contains(&k) {
                keys.push(k);
            }
        }
        let manufacturer = line.manufacturer.trim();
        // Narrows by manufacturer: the matching ones when there are any
        // (else all, flagged).
        let by_maker = |found: Vec<usize>, issues: &mut Vec<BomIssue>| -> Vec<usize> {
            if manufacturer.is_empty() || found.is_empty() {
                return found;
            }
            let same: Vec<usize> = found
                .iter()
                .copied()
                .filter(|&i| manufacturer_matches(ds, i, manufacturer))
                .collect();
            if same.is_empty() {
                issues.push(BomIssue {
                    code: IssueCode::ManufacturerMismatch,
                    severity: Severity::Warning,
                    detail: Some(manufacturer.to_string()),
                });
                found
            } else {
                same
            }
        };
        for (n, key) in keys.iter().enumerate() {
            // The Value column is weaker evidence than an MPN column.
            let from_value = n > 0 || line.mpn.trim().is_empty();
            let base = if from_value && !line.mpn.trim().is_empty() {
                0.9
            } else {
                1.0
            };
            if let Some(v) = self.mpns.exact.get(*key) {
                let v = by_maker(v.clone(), issues);
                let c = if v.len() > 1 { base * 0.8 } else { base };
                if v.len() > 1 {
                    issues.push(ambiguous(v.len()));
                }
                return (MatchKind::Exact, v.into_iter().map(|i| (i, c)).collect());
            }
            let norm = normalize_mpn(key);
            if norm.is_empty() {
                continue;
            }
            if let Some(v) = self.mpns.normalized.get(&norm) {
                let v = by_maker(v.clone(), issues);
                if v.len() > 1 {
                    issues.push(ambiguous(v.len()));
                }
                let c = base * if v.len() > 1 { 0.75 } else { 0.95 };
                return (
                    MatchKind::Normalized,
                    v.into_iter().map(|i| (i, c)).collect(),
                );
            }
        }
        for key in &keys {
            let norm = normalize_mpn(key);
            let found = self.mpns.variants(&norm);
            if found.is_empty() {
                continue;
            }
            let found = by_maker(found, issues);
            let mut scored: Vec<(usize, f64)> = found
                .into_iter()
                .map(|i| {
                    let other = normalize_mpn(&ds.parts[i].mpn);
                    let common = norm.len().min(other.len()) as f64;
                    let longest = norm.len().max(other.len()) as f64;
                    (i, (0.5 + 0.3 * common / longest).min(0.8))
                })
                .collect();
            sort_candidates(ds, &mut scored);
            issues.push(BomIssue {
                code: IssueCode::VariantMatch,
                severity: Severity::Info,
                detail: None,
            });
            if scored.len() > 1 {
                issues.push(ambiguous(scored.len()));
            }
            return (MatchKind::Variant, scored);
        }
        if let Some(found) = self.passive(ds, line) {
            if !found.is_empty() {
                issues.push(BomIssue {
                    code: IssueCode::ValueMatch,
                    severity: Severity::Info,
                    detail: None,
                });
                if found.len() > 1 {
                    issues.push(ambiguous(found.len()));
                }
                return (MatchKind::ValueFootprint, found);
            }
        }
        (MatchKind::None, vec![])
    }

    /// A passive's value + footprint match.
    fn passive(&self, ds: &Dataset, line: &BomLine) -> Option<Vec<(usize, f64)>> {
        let prefix: String = line
            .references
            .first()
            .map(|r| r.chars().take_while(|c| c.is_ascii_alphabetic()).collect())
            .unwrap_or_default();
        let value = line.value.trim();
        let (category, key) = match prefix.to_ascii_uppercase().as_str() {
            "C" => (ComponentCategory::Capacitor, "capacitance"),
            "R" => (ComponentCategory::Resistor, "resistance"),
            "L" => (ComponentCategory::Inductor, "inductance"),
            _ => {
                // No reference: by the value's unit.
                let lower = value.to_ascii_lowercase();
                if lower.contains('f')
                    && parse_value(first_token(value), parameter_spec("capacitance")?).is_some()
                {
                    (ComponentCategory::Capacitor, "capacitance")
                } else if lower.contains('h')
                    && parse_value(first_token(value), parameter_spec("inductance")?).is_some()
                {
                    (ComponentCategory::Inductor, "inductance")
                } else {
                    return None;
                }
            }
        };
        let spec = parameter_spec(key)?;
        let tokens: Vec<&str> = value
            .split(|c: char| c.is_whitespace() || c == '/' || c == ',' || c == '_')
            .filter(|t| !t.is_empty())
            .collect();
        let target = parse_value(tokens.first()?, spec)?;
        // Optional qualifiers after the value.
        let mut min_voltage = None;
        let mut max_tolerance = None;
        let mut dielectric = None;
        for t in &tokens[1..] {
            let up = t.to_ascii_uppercase();
            if let Some((d, _)) = DIELECTRIC_RANKS.iter().find(|(d, _)| *d == up) {
                dielectric = Some(*d);
            } else if up.ends_with('V') {
                min_voltage = parse_value(t, parameter_spec("rated_voltage")?);
            } else if up.ends_with('%') {
                max_tolerance = parse_value(t, parameter_spec("tolerance")?);
            }
        }
        let size = footprint_hint(&line.footprint).and_then(|h| h.size_code);
        let alt = &self.alternatives;
        let mut out = Vec::new();
        for &i in alt.in_category(category) {
            let params = alt.parameters(i)?;
            let stated = |k: &str| -> Option<f64> {
                let e = params.get(k)?;
                e.primary().stated(e.spec)
            };
            let Some(v) = stated(key) else { continue };
            if (v - target).abs() > 1e-6 * v.abs().max(target.abs()) {
                continue;
            }
            let part_size = alt.land(&ds.parts[i].package.0).and_then(|l| l.size_code);
            if size.is_some() && part_size != size {
                continue;
            }
            if let Some(minv) = min_voltage {
                if stated("rated_voltage").is_some_and(|rv| rv + 1e-9 < minv) {
                    continue;
                }
            }
            if let Some(maxt) = max_tolerance {
                if stated("tolerance").is_some_and(|t| t > maxt + 1e-12) {
                    continue;
                }
            }
            if let Some(d) = dielectric {
                let rank = |x: &str| DIELECTRIC_RANKS.iter().find(|(n, _)| *n == x).map(|r| r.1);
                let part_d = params.get("dielectric").and_then(|e| match e.primary() {
                    Normalized::Text(t) => Some(*t),
                    _ => None,
                });
                if part_d
                    .and_then(rank)
                    .zip(rank(d))
                    .is_some_and(|(p, w)| p < w)
                {
                    continue;
                }
            }
            out.push((i, if size.is_some() { 0.6 } else { 0.4 }));
        }
        sort_candidates(ds, &mut out);
        if out.len() == 1 {
            out[0].1 += 0.1;
        }
        Some(out)
    }
}

fn first_token(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

fn ambiguous(n: usize) -> BomIssue {
    BomIssue {
        code: IssueCode::Ambiguous,
        severity: Severity::Info,
        detail: Some(n.to_string()),
    }
}

/// Orders candidates: confidence, then active and verified parts, then
/// better-documented ones, then by name.
fn sort_candidates(ds: &Dataset, v: &mut [(usize, f64)]) {
    let rank = |i: usize| {
        let p = &ds.parts[i];
        let life = match p.lifecycle.status {
            LifecycleStatus::Active => 0,
            LifecycleStatus::PreRelease | LifecycleStatus::Unknown => 1,
            LifecycleStatus::Nrnd => 2,
            LifecycleStatus::EolAnnounced => 3,
            LifecycleStatus::Obsolete => 4,
        };
        let exist = match p.existence.status {
            ExistenceStatus::Verified | ExistenceStatus::HistoricalVerified => 0,
            ExistenceStatus::Probable => 1,
            ExistenceStatus::Unverified => 2,
            ExistenceStatus::Rejected => 3,
        };
        (life, exist, std::cmp::Reverse(p.parameters.len()))
    };
    v.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| rank(a.0).cmp(&rank(b.0)))
            .then_with(|| ds.parts[a.0].id.cmp(&ds.parts[b.0].id))
    });
}

/// Whether the BOM's manufacturer text names part `i`'s manufacturer
/// (its id or display name: `TI`, `Texas Instruments`, `Murata
/// Electronics`, `Samsung Electro-Mechanics`).
pub fn manufacturer_matches(ds: &Dataset, i: usize, text: &str) -> bool {
    let p = &ds.parts[i];
    let bom = normalize_mpn(text);
    if bom.is_empty() {
        return true;
    }
    let id = normalize_mpn(&p.manufacturer.0);
    if bom == id {
        return true;
    }
    let Some(m) = ds.manufacturers.get(&p.manufacturer.0) else {
        return bom.starts_with(&id) && id.len() >= 3;
    };
    let name = normalize_mpn(&m.name);
    let first = m
        .name
        .split_whitespace()
        .next()
        .map(normalize_mpn)
        .unwrap_or_default();
    (bom.len() >= 2 && name.starts_with(&bom))
        || (first.len() >= 3 && bom.starts_with(&first))
        || (id.len() >= 3 && bom.starts_with(&id))
        || initials(&m.name) == bom
}

/// `Texas Instruments` -> `ti`.
fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_lowercase)
        .collect()
}

/// What a footprint name says about the package.
#[derive(Debug, Clone, PartialEq)]
pub struct FootprintHint {
    pub size_code: Option<&'static str>,
    pub family: Option<PackageFamily>,
    pub leads: Option<u32>,
    pub label: String,
}

/// Reads a footprint name (`Capacitor_SMD:C_0603_1608Metric`,
/// `Package_TO_SOT_SMD:SOT-23-5`, `SOIC-8_3.9x4.9mm_P1.27mm`).
pub fn footprint_hint(footprint: &str) -> Option<FootprintHint> {
    let name = footprint.rsplit(':').next().unwrap_or(footprint);
    let upper = name.to_ascii_uppercase();
    let tokens: Vec<&str> = upper.split(['_', ' ']).filter(|t| !t.is_empty()).collect();
    for t in &tokens {
        // `0603`, `1608METRIC`
        let code = t.strip_suffix("METRIC").unwrap_or(t);
        if let Some(s) = CHIP_SIZES.iter().find(|s| {
            s.2[0] == code || (t.ends_with("METRIC") && s.2[1].trim_end_matches('m') == code)
        }) {
            return Some(FootprintHint {
                size_code: Some(s.2[0]),
                family: Some(PackageFamily::Chip),
                leads: Some(2),
                label: s.2[0].to_string(),
            });
        }
    }
    for t in &tokens {
        let letters: String = t
            .chars()
            .take_while(|c| c.is_ascii_alphabetic() || *c == '-')
            .filter(|c| *c != '-')
            .collect();
        let numbers: Vec<u32> = t
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        let n0 = numbers.first().copied();
        let n1 = numbers.get(1).copied();
        use PackageFamily as F;
        let (family, leads) = match (letters.as_str(), n0) {
            _ if t.starts_with("D2PAK") || t.starts_with("DDPAK") => (F::D2pak, None),
            ("SOT" | "TSOT", Some(23)) => (F::Sot, Some(n1.unwrap_or(3))),
            ("SOT", Some(223)) => (F::Sot223, None),
            ("SOT", Some(323)) => (F::Sot, Some(3)),
            ("SOT", Some(353)) => (F::Sot, Some(5)),
            ("SOT", Some(363)) => (F::Sot, Some(6)),
            ("SC", Some(70)) => (F::Sot, Some(n1.unwrap_or(3))),
            ("SOIC" | "SO", Some(n)) => (F::Soic, Some(n)),
            ("TSSOP", Some(n)) => (F::Tssop, Some(n)),
            ("SSOP", Some(n)) => (F::Ssop, Some(n)),
            ("MSOP" | "VSSOP", Some(n)) => (F::Msop, Some(n)),
            ("QFN" | "VQFN" | "WQFN" | "UQFN", Some(n)) => (F::Qfn, Some(n)),
            ("DFN" | "WDFN" | "UDFN" | "TDFN", Some(n)) => (F::Dfn, Some(n)),
            ("SON" | "WSON" | "VSON" | "USON", Some(n)) => (F::Son, Some(n)),
            ("LQFP" | "TQFP", Some(n)) => (F::Lqfp, Some(n)),
            ("TO", Some(252)) | ("DPAK", _) => (F::Dpak, None),
            ("TO", Some(263)) => (F::D2pak, None),
            ("TO", Some(220)) => (F::To220, None),
            ("TO", Some(92)) => (F::To92, None),
            ("SOD", Some(_)) => (F::Sod, None),
            ("SMA" | "SMB" | "SMC", None) => (F::Do214, None),
            ("DO", Some(214)) => (F::Do214, None),
            _ => continue,
        };
        let label = t.split('-').take(3).collect::<Vec<_>>().join("-");
        return Some(FootprintHint {
            size_code: None,
            family: Some(family),
            leads,
            label,
        });
    }
    None
}

/// Families one footprint name stands for.
fn same_family(a: PackageFamily, b: PackageFamily) -> bool {
    use PackageFamily as F;
    let group = |f: PackageFamily| match f {
        F::SodFlat => F::Sod,
        F::Son => F::Dfn,
        other => other,
    };
    group(a) == group(b)
}

fn footprint_check(
    fp: &str,
    alt: &AlternativesIndex,
    ds: &Dataset,
    i: usize,
) -> FootprintNameCheck {
    let part = &ds.parts[i];
    let pkg = ds.packages.get(&part.package.0);
    let land = alt.land(&part.package.0);
    let part_label = match (land.and_then(|l| l.size_code), pkg) {
        (Some(code), _) => code.to_string(),
        (None, Some(k)) => format!("{} {}", k.family.name().to_uppercase(), k.lead_count),
        (None, None) => part.package.0.clone(),
    };
    let hint = footprint_hint(fp);
    let status = match (&hint, pkg) {
        (Some(h), Some(k)) => {
            let size_ok = match (h.size_code, land.and_then(|l| l.size_code)) {
                (Some(a), Some(b)) => a == b,
                (Some(_), None) => false,
                _ => true,
            };
            let family_ok = h.family.is_none_or(|f| same_family(f, k.family));
            let leads_ok = h.leads.is_none_or(|n| n == k.lead_count);
            if size_ok && family_ok && leads_ok {
                FootprintStatus::Match
            } else {
                FootprintStatus::Mismatch
            }
        }
        _ => FootprintStatus::Unknown,
    };
    FootprintNameCheck {
        status,
        bom: hint.map(|h| h.label),
        part: part_label,
    }
}

fn existence_wire(s: ExistenceStatus) -> &'static str {
    match s {
        ExistenceStatus::Unverified => "unverified",
        ExistenceStatus::Probable => "probable",
        ExistenceStatus::Verified => "verified",
        ExistenceStatus::HistoricalVerified => "historical_verified",
        ExistenceStatus::Rejected => "rejected",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footprint_names() {
        let h = |s: &str| footprint_hint(s).map(|h| (h.size_code, h.family, h.leads));
        use PackageFamily as F;
        assert_eq!(
            h("Capacitor_SMD:C_0603_1608Metric"),
            Some((Some("0603"), Some(F::Chip), Some(2)))
        );
        assert_eq!(
            h("R_0805_2012Metric"),
            Some((Some("0805"), Some(F::Chip), Some(2)))
        );
        assert_eq!(
            h("Package_TO_SOT_SMD:SOT-23-5"),
            Some((None, Some(F::Sot), Some(5)))
        );
        assert_eq!(h("SOT-23"), Some((None, Some(F::Sot), Some(3))));
        assert_eq!(
            h("Package_TO_SOT_SMD:SOT-223-3_TabPin2"),
            Some((None, Some(F::Sot223), None))
        );
        assert_eq!(
            h("Package_SO:SOIC-8_3.9x4.9mm_P1.27mm"),
            Some((None, Some(F::Soic), Some(8)))
        );
        assert_eq!(
            h("Package_DFN_QFN:QFN-56-1EP_7x7mm_P0.4mm_EP3.2x3.2mm"),
            Some((None, Some(F::Qfn), Some(56)))
        );
        assert_eq!(h("Diode_SMD:D_SMA"), Some((None, Some(F::Do214), None)));
        // MPN-named footprints say nothing (and `0201` inside a number is not a size).
        assert_eq!(h("openparts:RC0603FR-0727RL"), None);
        assert_eq!(h("openparts:61302011121"), None);
    }
}
