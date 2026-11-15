//! Responses of `GET /v1/catalog`, `POST /v1/bom/check`,
//! `GET .../alternatives` and `GET .../placement`, and the catalog
//! request builder.
//!
//! These are deliberately loose mirrors of the server's types: enums
//! are strings, every field defaults, and unknown fields are ignored,
//! so a client keeps working while the server's schema grows (a new
//! facet, a renamed parameter) instead of failing to parse.

use crate::percent_encode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A `GET /v1/catalog` query. Multi-select filters go out as one
/// comma-separated parameter each; parameter ranges as `<key>_min` /
/// `<key>_max` (the server parses unit prefixes: `10n`, `0.1u`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CatalogRequest {
    pub q: String,
    /// Filter parameter (`category`, `manufacturer`, `capacitance`)
    /// to the selected option values.
    pub filters: BTreeMap<String, Vec<String>>,
    /// Range parameter stem (`capacitance`, `width`) to (min, max),
    /// as typed.
    pub ranges: BTreeMap<String, (String, String)>,
    pub sort: Option<String>,
    pub descending: bool,
    pub offset: usize,
    pub limit: Option<usize>,
}

impl CatalogRequest {
    /// The query string, without the leading `?`. Deterministic
    /// (keys sorted), empty values left out.
    pub fn query_string(&self) -> String {
        let mut pairs: Vec<(String, String)> = Vec::new();
        if !self.q.trim().is_empty() {
            pairs.push(("q".into(), self.q.trim().into()));
        }
        for (key, values) in &self.filters {
            let values: Vec<&str> = values
                .iter()
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
                .collect();
            if !values.is_empty() {
                pairs.push((key.clone(), values.join(",")));
            }
        }
        for (key, (min, max)) in &self.ranges {
            if !min.trim().is_empty() {
                pairs.push((format!("{key}_min"), min.trim().into()));
            }
            if !max.trim().is_empty() {
                pairs.push((format!("{key}_max"), max.trim().into()));
            }
        }
        if let Some(sort) = self.sort.as_deref().filter(|s| !s.is_empty()) {
            pairs.push(("sort".into(), sort.into()));
            if self.descending {
                pairs.push(("order".into(), "desc".into()));
            }
        }
        if self.offset > 0 {
            pairs.push(("offset".into(), self.offset.to_string()));
        }
        if let Some(limit) = self.limit {
            pairs.push(("limit".into(), limit.to_string()));
        }
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// `/v1/catalog?...`
    pub fn path(&self) -> String {
        let qs = self.query_string();
        if qs.is_empty() {
            "/v1/catalog".into()
        } else {
            format!("/v1/catalog?{qs}")
        }
    }

    /// Toggles one option of a multi-select filter.
    pub fn toggle(&mut self, key: &str, value: &str) {
        let values = self.filters.entry(key.to_string()).or_default();
        if let Some(i) = values.iter().position(|v| v == value) {
            values.remove(i);
        } else {
            values.push(value.to_string());
        }
        if values.is_empty() {
            self.filters.remove(key);
        }
    }

    pub fn is_selected(&self, key: &str, value: &str) -> bool {
        self.filters
            .get(key)
            .is_some_and(|v| v.iter().any(|x| x == value))
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct CatalogResponse {
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub items: Vec<CatalogItem>,
    pub facets: CatalogFacets,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct CatalogItem {
    pub manufacturer: String,
    pub manufacturer_name: String,
    pub mpn: String,
    pub category: Option<String>,
    pub device: String,
    pub package: String,
    pub package_family: Option<String>,
    pub lead_count: Option<u32>,
    pub pitch: Option<f64>,
    pub body_width: Option<f64>,
    pub body_length: Option<f64>,
    pub body_height: Option<f64>,
    pub size_code: Option<String>,
    pub size_code_metric: Option<String>,
    pub mounting: Option<String>,
    pub rows: Option<String>,
    pub gender: Option<String>,
    pub orientation: Option<String>,
    pub shrouded: Option<bool>,
    pub exposed_pad: Option<bool>,
    pub lifecycle: String,
    pub existence: String,
    pub review: ReviewSummary,
    pub parameters: BTreeMap<String, CatalogParameter>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ReviewSummary {
    pub state: String,
    pub part: bool,
    pub device: bool,
    pub package: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct CatalogParameter {
    pub value: Option<f64>,
    pub label: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct FacetValue {
    pub value: String,
    pub label: String,
    pub count: usize,
    pub selected: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct CategoryNode {
    pub value: String,
    pub label: String,
    pub count: usize,
    pub selected: bool,
    pub children: Vec<CategoryNode>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct RangeFacet {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ParameterFacet {
    pub key: String,
    /// `numeric` or `enumerated`.
    pub kind: String,
    pub unit: String,
    pub values: Vec<FacetValue>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub min_label: Option<String>,
    pub max_label: Option<String>,
}

impl ParameterFacet {
    pub fn is_numeric(&self) -> bool {
        self.kind == "numeric"
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct CatalogFacets {
    pub category: Vec<FacetValue>,
    pub category_tree: Vec<CategoryNode>,
    pub manufacturer: Vec<FacetValue>,
    pub package_family: Vec<FacetValue>,
    pub size: Vec<FacetValue>,
    pub pins: Vec<FacetValue>,
    pub pitch: Vec<FacetValue>,
    pub mounting: Vec<FacetValue>,
    pub lifecycle: Vec<FacetValue>,
    pub existence: Vec<FacetValue>,
    pub review: Vec<FacetValue>,
    pub rows: Vec<FacetValue>,
    pub gender: Vec<FacetValue>,
    pub orientation: Vec<FacetValue>,
    pub shrouded: Vec<FacetValue>,
    pub exposed_pad: Vec<FacetValue>,
    pub body_width: RangeFacet,
    pub body_length: RangeFacet,
    pub body_height: RangeFacet,
    pub parameters: Vec<ParameterFacet>,
}

/// `PartBrief`: a part as BOM check and alternatives describe it.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct PartBrief {
    pub manufacturer: String,
    pub manufacturer_name: String,
    pub mpn: String,
    pub category: Option<String>,
    pub device: String,
    pub package: String,
    pub package_family: Option<String>,
    pub lead_count: Option<u32>,
    pub size_code: Option<String>,
    pub lifecycle: String,
    pub existence: String,
    pub parameters: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct BomReport {
    pub format: String,
    pub summary: BomSummary,
    pub lines: Vec<BomLine>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct BomSummary {
    pub lines: usize,
    pub components: u64,
    pub ok: usize,
    pub review: usize,
    pub warning: usize,
    pub not_found: usize,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct BomLine {
    pub row: usize,
    pub references: Vec<String>,
    pub quantity: u32,
    pub value: String,
    pub footprint: String,
    pub mpn: String,
    pub manufacturer: String,
    pub dnp: bool,
    /// `ok`, `review`, `warning`, `not_found`.
    pub status: String,
    /// `exact`, `normalized`, `variant`, `value_footprint`, `none`.
    pub kind: String,
    pub confidence: f64,
    pub matches: Vec<BomMatch>,
    pub issues: Vec<BomIssue>,
    pub alternatives: Vec<Alternative>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct BomMatch {
    pub part: PartBrief,
    pub kind: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct BomIssue {
    pub code: String,
    /// `info` or `warning`.
    pub severity: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Alternative {
    pub part: PartBrief,
    /// `drop_in`, `footprint_compatible`, `functional`, `near_miss`.
    pub class: String,
    pub listed_replacement: bool,
    pub score: i32,
    pub verdict: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct AlternativesResponse {
    pub part: PartBrief,
    pub alternatives: Vec<Alternative>,
}

/// Only the `hash` of a Placement Model (`sha256:<hex>`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct PlacementHash {
    pub hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_query_string_is_sorted_encoded_and_skips_empties() {
        let mut r = CatalogRequest {
            q: " 100n ".into(),
            limit: Some(100),
            ..Default::default()
        };
        r.toggle("category", "capacitor");
        r.toggle("size", "0603");
        r.toggle("size", "0402");
        r.toggle("manufacturer", "x");
        r.toggle("manufacturer", "x");
        r.ranges
            .insert("rated_voltage".into(), ("16".into(), String::new()));
        r.ranges
            .insert("width".into(), (String::new(), String::new()));
        r.sort = Some("capacitance".into());
        r.descending = true;
        assert_eq!(
            r.path(),
            "/v1/catalog?q=100n&category=capacitor&size=0603%2C0402&rated_voltage_min=16\
             &sort=capacitance&order=desc&limit=100"
        );
        assert!(r.is_selected("size", "0402"));
        assert!(!r.filters.contains_key("manufacturer"));
        assert_eq!(CatalogRequest::default().path(), "/v1/catalog");
    }

    #[test]
    fn catalog_response_tolerates_missing_and_unknown_fields() {
        let json = r#"{"total":1,"items":[{"manufacturer":"m","mpn":"P","lifecycle":"active",
            "parameters":{"capacitance":{"value":1e-7,"label":"100 nF"}},"new_field":3}],
            "facets":{"parameters":[{"key":"capacitance","kind":"numeric","unit":"F","values":[]}],
            "some_new_facet":[]}}"#;
        let r: CatalogResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.items[0].parameters["capacitance"].label, "100 nF");
        assert!(r.facets.parameters[0].is_numeric());
        assert!(r.facets.manufacturer.is_empty());
    }
}
