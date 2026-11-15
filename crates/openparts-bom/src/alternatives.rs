//! Alternative parts: candidate replacements for a Part, classified and
//! ranked (docs/spec/alternatives-and-bom.md).
//!
//! - **drop-in**: the same land pattern ([`crate::footprint`]), a
//!   pin-compatible pinout ([`crate::pins`]) and parameters that meet or
//!   exceed the original's ([`crate::rules`]);
//! - **footprint-compatible**: the same land pattern, but some pad's pin
//!   differs (the per-pad diff says which);
//! - **functional equivalent**: the same category, its identity
//!   parameters (capacitance, output voltage...) stated and equal, the
//!   rest meeting the original, in another package;
//! - **near miss**: a drop-in but for some parameter that is worse
//!   than the original's.
//!
//! Candidates come from the original's own category. One whose identity
//! parameter differs (another capacitance) is not an alternative, nor
//! is one in another package whose identity can't be verified. A
//! replacement the data names (`lifecycle.replacement`) is always
//! listed, first, with its checks as they come out (Canonical Data
//! Specification section 13: a named replacement is not by itself
//! compatible).

use crate::footprint::{self, FootprintCheck, LandPattern};
use crate::pins::{self, PinComparison};
use crate::rules::{self, CheckStatus, EffectiveParameters, ParameterCheck, Verdict};
use openparts_core::{ComponentCategory, ExistenceStatus, LifecycleStatus, PackageFamily, Part};
use openparts_data::Dataset;
use serde::Serialize;
use std::collections::BTreeMap;

/// The class of an alternative.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlternativeClass {
    DropIn,
    FootprintCompatible,
    Functional,
    /// Would be a drop-in (same land pattern, compatible pinout), but
    /// some parameter is worse than the original's (a lower rating, a
    /// larger dropout): listed so the trade-off shows.
    NearMiss,
    /// A replacement the data names that fits none of the classes above
    /// (listed so its differences show).
    Listed,
}

/// A part as alternatives and BOM results show it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PartBrief {
    pub manufacturer: String,
    pub manufacturer_name: String,
    pub mpn: String,
    pub category: Option<ComponentCategory>,
    pub device: String,
    pub package: String,
    pub package_family: Option<PackageFamily>,
    pub lead_count: Option<u32>,
    /// Imperial chip size code (`0603`).
    pub size_code: Option<String>,
    pub lifecycle: LifecycleStatus,
    pub existence: ExistenceStatus,
    /// Parameter labels by key (`capacitance` -> `100 nF`).
    pub parameters: BTreeMap<String, String>,
}

/// One candidate replacement.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Alternative {
    pub part: PartBrief,
    pub class: AlternativeClass,
    /// Named in the original's `lifecycle.replacement`.
    pub listed_replacement: bool,
    /// Ranking score (higher first); see the docs for its terms.
    pub score: i32,
    pub verdict: Verdict,
    /// Every parameter the original states, against the candidate.
    pub parameters: Vec<ParameterCheck>,
    pub footprint: FootprintCheck,
    /// Pad-by-pad pinout comparison (same land pattern only).
    pub pins: Option<PinComparison>,
}

/// `GET /v1/parts/{manufacturer}/{mpn}/alternatives`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlternativesResponse {
    pub part: PartBrief,
    /// Best first: listed replacements, then drop-ins, footprint-
    /// compatible and functional equivalents, each by score.
    pub alternatives: Vec<Alternative>,
    pub drop_in: usize,
    pub footprint_compatible: usize,
    pub functional: usize,
    pub near_miss: usize,
    /// Same-category candidates left out: their identity (output
    /// voltage, capacitance...) differs, or, in another package, it
    /// can't be verified or some parameter is worse.
    pub rejected: usize,
}

/// Per-Part facts computed once for the whole dataset.
#[derive(Debug, Default)]
pub struct AlternativesIndex {
    /// Land pattern per Package id.
    lands: BTreeMap<String, LandPattern>,
    /// Effective parameters per Part, in `Dataset::parts` order.
    params: Vec<EffectiveParameters>,
    /// Part indices per category.
    by_category: BTreeMap<ComponentCategory, Vec<usize>>,
    /// Part index per id.
    by_id: BTreeMap<String, usize>,
}

impl AlternativesIndex {
    pub fn build(ds: &Dataset) -> AlternativesIndex {
        let mut index = AlternativesIndex::default();
        for (id, pkg) in &ds.packages {
            index.lands.insert(id.clone(), LandPattern::of(pkg));
        }
        let none = Default::default();
        for (i, p) in ds.parts.iter().enumerate() {
            let device = ds.devices.get(&p.device.0);
            index.params.push(rules::effective(
                &p.parameters,
                device.map_or(&none, |d| &d.parameters),
            ));
            if let Some(d) = device {
                index.by_category.entry(d.category).or_default().push(i);
            }
            index.by_id.insert(p.id.0.clone(), i);
        }
        index
    }

    /// The part's index in `Dataset::parts`.
    pub fn position(&self, part: &Part) -> Option<usize> {
        self.by_id.get(&part.id.0).copied()
    }

    pub fn land(&self, package: &str) -> Option<&LandPattern> {
        self.lands.get(package)
    }

    pub fn parameters(&self, i: usize) -> Option<&EffectiveParameters> {
        self.params.get(i)
    }

    /// Parts of `category`.
    pub fn in_category(&self, category: ComponentCategory) -> &[usize] {
        self.by_category
            .get(&category)
            .map_or(&[], |v| v.as_slice())
    }

    /// The summary of part `i`.
    pub fn brief(&self, ds: &Dataset, i: usize) -> PartBrief {
        let p = &ds.parts[i];
        let device = ds.devices.get(&p.device.0);
        let pkg = ds.packages.get(&p.package.0);
        let land = self.lands.get(&p.package.0);
        PartBrief {
            manufacturer: p.manufacturer.0.clone(),
            manufacturer_name: ds
                .manufacturers
                .get(&p.manufacturer.0)
                .map_or_else(|| p.manufacturer.0.clone(), |m| m.name.clone()),
            mpn: p.mpn.clone(),
            category: device.map(|d| d.category),
            device: p.device.0.clone(),
            package: p.package.0.clone(),
            package_family: pkg.map(|k| k.family),
            lead_count: pkg.map(|k| k.lead_count),
            size_code: land.and_then(|l| l.size_code).map(str::to_string),
            lifecycle: p.lifecycle.status,
            existence: p.existence.status,
            parameters: self.params[i]
                .iter()
                .map(|(k, (n, spec))| (k.clone(), n.label(spec)))
                .collect(),
        }
    }

    /// Every alternative to part `orig` (an index into `Dataset::parts`),
    /// best first.
    pub fn alternatives(&self, ds: &Dataset, orig: usize) -> AlternativesResponse {
        let part = &ds.parts[orig];
        let mut out = AlternativesResponse {
            part: self.brief(ds, orig),
            alternatives: Vec::new(),
            drop_in: 0,
            footprint_compatible: 0,
            functional: 0,
            near_miss: 0,
            rejected: 0,
        };
        let Some(device) = ds.devices.get(&part.device.0) else {
            return out;
        };
        let Some(orig_land) = self.lands.get(&part.package.0) else {
            return out;
        };
        let listed: Vec<usize> = part
            .lifecycle
            .replacement
            .iter()
            .filter_map(|id| self.by_id.get(&id.0).copied())
            .collect();
        let mut pool: Vec<usize> = self.in_category(device.category).to_vec();
        for i in &listed {
            if !pool.contains(i) {
                pool.push(*i);
            }
        }
        for c in pool {
            if c == orig {
                continue;
            }
            let cand = &ds.parts[c];
            let is_listed = listed.contains(&c);
            let (Some(cdev), Some(cland)) = (
                ds.devices.get(&cand.device.0),
                self.lands.get(&cand.package.0),
            ) else {
                continue;
            };
            let cmp = rules::compare(&self.params[orig], &self.params[c], Some(device.category));
            let fp = footprint::compare(&part.package.0, orig_land, &cand.package.0, cland);
            let pins = fp
                .check
                .same_land_pattern
                .then(|| pins::compare(device, cdev, &fp.pad_map));
            let same_device = cand.device == part.device;
            let worse = cmp.verdict == Verdict::Worse;
            let class = if cmp.identity_mismatch {
                None
            } else if fp.check.same_land_pattern {
                if !pins.as_ref().is_some_and(|p| p.compatible) {
                    Some(AlternativeClass::FootprintCompatible)
                } else if worse {
                    Some(AlternativeClass::NearMiss)
                } else {
                    Some(AlternativeClass::DropIn)
                }
            } else if !worse
                && (cmp.identity_complete() || (same_device && cmp.identity_missing.is_empty()))
            {
                Some(AlternativeClass::Functional)
            } else {
                None
            };
            let class = match (class, is_listed) {
                (Some(c), _) => c,
                (None, true) => AlternativeClass::Listed,
                (None, false) => {
                    out.rejected += 1;
                    continue;
                }
            };
            match class {
                AlternativeClass::DropIn => out.drop_in += 1,
                AlternativeClass::FootprintCompatible => out.footprint_compatible += 1,
                AlternativeClass::Functional => out.functional += 1,
                AlternativeClass::NearMiss => out.near_miss += 1,
                AlternativeClass::Listed => {}
            }
            let score = score(part, cand, class, is_listed, &cmp);
            out.alternatives.push(Alternative {
                part: self.brief(ds, c),
                class,
                listed_replacement: is_listed,
                score,
                verdict: cmp.verdict,
                parameters: cmp.checks,
                footprint: fp.check,
                pins,
            });
        }
        out.alternatives.sort_by(|a, b| {
            b.listed_replacement
                .cmp(&a.listed_replacement)
                .then(a.class.cmp(&b.class))
                .then(b.score.cmp(&a.score))
                .then_with(|| {
                    (&a.part.manufacturer, &a.part.mpn).cmp(&(&b.part.manufacturer, &b.part.mpn))
                })
        });
        out
    }
}

/// The ranking score: the class, the parameter verdict (and keys
/// better than the original), lifecycle and existence of the candidate,
/// and a small bonus for the original's own manufacturer.
fn score(
    orig: &Part,
    cand: &Part,
    class: AlternativeClass,
    listed: bool,
    cmp: &rules::Comparison,
) -> i32 {
    let mut s = match class {
        AlternativeClass::DropIn => 300,
        AlternativeClass::FootprintCompatible => 200,
        AlternativeClass::Functional => 100,
        AlternativeClass::NearMiss => 50,
        AlternativeClass::Listed => 0,
    };
    if listed {
        s += 1000;
    }
    s += match cmp.verdict {
        Verdict::Meets => 50,
        Verdict::Unverified => 0,
        Verdict::Worse => -50,
    };
    s += 2 * cmp.better as i32;
    s -= 10
        * cmp
            .checks
            .iter()
            .filter(|c| c.status == CheckStatus::Fail)
            .count() as i32;
    s -= 5 * cmp
        .checks
        .iter()
        .filter(|c| c.status == CheckStatus::Unverified)
        .count() as i32;
    s += match cand.lifecycle.status {
        LifecycleStatus::Active => 20,
        LifecycleStatus::PreRelease => 5,
        LifecycleStatus::Unknown => 5,
        LifecycleStatus::Nrnd => -20,
        LifecycleStatus::EolAnnounced => -40,
        LifecycleStatus::Obsolete => -60,
    };
    s += match cand.existence.status {
        ExistenceStatus::Verified | ExistenceStatus::HistoricalVerified => 10,
        ExistenceStatus::Probable => 5,
        ExistenceStatus::Unverified => 0,
        ExistenceStatus::Rejected => -100,
    };
    if cand.manufacturer == orig.manufacturer {
        s += 3;
    }
    s
}
