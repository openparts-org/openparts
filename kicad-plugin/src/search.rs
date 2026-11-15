//! Catalog search state: the query and facet selection, the results,
//! and which table columns they call for. No egui here, so all of it
//! is testable without a display.

use crate::jobs::{Job, Waker};
use openparts_client::api::{
    CatalogItem, CatalogRequest, CatalogResponse, CategoryNode, FacetValue, ParameterFacet,
};
use openparts_client::Client;
use std::time::{Duration, Instant};

/// Delay after the last edit before a search fires on its own.
pub const DEBOUNCE: Duration = Duration::from_millis(250);
/// A one-character query is too broad to fire on its own (an explicit
/// search still runs it). An empty one browses the catalog.
pub const MIN_QUERY_LEN: usize = 2;
pub const PAGE: usize = 100;
/// Parameter columns at most.
const MAX_PARAM_COLUMNS: usize = 6;

pub fn should_auto_dispatch(elapsed_since_edit: Duration, query_len_chars: usize) -> bool {
    elapsed_since_edit >= DEBOUNCE && (query_len_chars == 0 || query_len_chars >= MIN_QUERY_LEN)
}

/// A results-table column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Column {
    Mpn,
    Manufacturer,
    Category,
    Package,
    Size,
    Pins,
    Pitch,
    Mounting,
    Rows,
    Gender,
    Orientation,
    Body,
    Param(String),
    Lifecycle,
    Review,
}

impl Column {
    pub fn title(&self) -> String {
        match self {
            Column::Mpn => "MPN".into(),
            Column::Manufacturer => "Manufacturer".into(),
            Column::Category => "Category".into(),
            Column::Package => "Package".into(),
            Column::Size => "Size".into(),
            Column::Pins => "Pins".into(),
            Column::Pitch => "Pitch".into(),
            Column::Mounting => "Mount".into(),
            Column::Rows => "Rows".into(),
            Column::Gender => "Gender".into(),
            Column::Orientation => "Orient.".into(),
            Column::Body => "Body W×L×H".into(),
            Column::Param(k) => humanize(k),
            Column::Lifecycle => "Lifecycle".into(),
            Column::Review => "Review".into(),
        }
    }

    /// The server's `sort` value for this column, if it sorts by it.
    pub fn sort_key(&self, facets: &[ParameterFacet]) -> Option<String> {
        Some(match self {
            Column::Mpn => "mpn".into(),
            Column::Manufacturer => "manufacturer".into(),
            Column::Pins => "pins".into(),
            Column::Pitch => "pitch".into(),
            Column::Body => "size".into(),
            Column::Param(k) if facets.iter().any(|f| &f.key == k && f.is_numeric()) => k.clone(),
            _ => return None,
        })
    }

    pub fn cell(&self, item: &CatalogItem) -> String {
        let opt = |o: &Option<String>| o.as_deref().map(humanize).unwrap_or_default();
        match self {
            Column::Mpn => item.mpn.clone(),
            Column::Manufacturer => item.manufacturer_name.clone(),
            Column::Category => opt(&item.category),
            Column::Package => {
                let family = item.package_family.as_deref().unwrap_or_default();
                if family.is_empty() {
                    short_package(&item.package)
                } else {
                    family.to_uppercase().replace('_', "-")
                }
            }
            Column::Size => match (&item.size_code, &item.size_code_metric) {
                (Some(i), Some(m)) => format!("{i} ({m})"),
                (Some(i), None) => i.clone(),
                _ => String::new(),
            },
            Column::Pins => item.lead_count.map(|n| n.to_string()).unwrap_or_default(),
            Column::Pitch => item
                .pitch
                .map(|p| format!("{} mm", fmt_mm(p)))
                .unwrap_or_default(),
            Column::Mounting => item
                .mounting
                .as_deref()
                .map(str::to_uppercase)
                .unwrap_or_default(),
            Column::Rows => item.rows.clone().unwrap_or_default(),
            Column::Gender => opt(&item.gender),
            Column::Orientation => opt(&item.orientation),
            Column::Body => {
                let dims: Vec<String> = [item.body_width, item.body_length, item.body_height]
                    .iter()
                    .map(|d| d.map(fmt_mm).unwrap_or_else(|| "?".into()))
                    .collect();
                if dims.iter().all(|d| d == "?") {
                    String::new()
                } else {
                    dims.join("×")
                }
            }
            Column::Param(k) => item
                .parameters
                .get(k)
                .map(|p| p.label.clone())
                .unwrap_or_default(),
            Column::Lifecycle => humanize(&item.lifecycle),
            Column::Review => humanize(&item.review.state),
        }
    }
}

/// `rated_voltage` -> `Rated voltage`.
pub fn humanize(key: &str) -> String {
    let s = key.replace('_', " ");
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn fmt_mm(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn short_package(id: &str) -> String {
    id.rsplit('/').next().unwrap_or(id).to_string()
}

/// The columns a result page calls for: identity, then the structure
/// and parameters its parts state (in the facets' order), then status.
pub fn columns(response: &CatalogResponse, request: &CatalogRequest) -> Vec<Column> {
    let items = &response.items;
    let any = |f: &dyn Fn(&CatalogItem) -> bool| items.iter().any(f);
    let mut cols = vec![Column::Mpn, Column::Manufacturer];
    let one_category = {
        let mut cats = items.iter().map(|i| &i.category);
        let first = cats.next();
        first.is_some() && cats.all(|c| Some(c) == first)
    };
    if !one_category && !request.filters.contains_key("category") {
        cols.push(Column::Category);
    }
    cols.push(Column::Package);
    let all_chips = !items.is_empty() && items.iter().all(|i| i.size_code.is_some());
    if any(&|i| i.size_code.is_some()) {
        cols.push(Column::Size);
    }
    if !all_chips && any(&|i| i.lead_count.is_some()) {
        cols.push(Column::Pins);
    }
    if any(&|i| i.pitch.is_some()) {
        cols.push(Column::Pitch);
    }
    let mut mountings = items.iter().filter_map(|i| i.mounting.as_deref());
    if let Some(first) = mountings.next() {
        if mountings.any(|m| m != first) {
            cols.push(Column::Mounting);
        }
    }
    if any(&|i| i.rows.is_some()) {
        cols.push(Column::Rows);
    }
    if any(&|i| i.gender.is_some()) {
        cols.push(Column::Gender);
    }
    if any(&|i| i.orientation.is_some()) {
        cols.push(Column::Orientation);
    }
    let mut keys: Vec<String> = response
        .facets
        .parameters
        .iter()
        .map(|f| f.key.clone())
        .filter(|k| any(&|i| i.parameters.contains_key(k)))
        .collect();
    // Keys the facets don't list (should not happen): after, by name.
    let mut extra: Vec<String> = items
        .iter()
        .flat_map(|i| i.parameters.keys())
        .filter(|k| !keys.contains(k))
        .cloned()
        .collect();
    extra.sort();
    extra.dedup();
    keys.extend(extra);
    // Parameters of mixed categories make a sparse table: only within one.
    let params = if one_category || request.filters.contains_key("category") {
        keys.len().min(MAX_PARAM_COLUMNS)
    } else {
        0
    };
    cols.extend(keys.into_iter().take(params).map(Column::Param));
    if any(&|i| i.body_width.is_some() || i.body_height.is_some()) {
        cols.push(Column::Body);
    }
    cols.push(Column::Lifecycle);
    cols.push(Column::Review);
    cols
}

/// One multi-select facet: its query parameter, title and options.
pub struct FacetGroup<'a> {
    pub key: &'static str,
    pub title: &'static str,
    pub values: &'a [FacetValue],
}

/// The non-empty multi-select facets, in display order (the category
/// tree is separate).
pub fn facet_groups(response: &CatalogResponse) -> Vec<FacetGroup<'_>> {
    let f = &response.facets;
    [
        ("manufacturer", "Manufacturer", &f.manufacturer),
        ("family", "Package family", &f.package_family),
        ("size", "Size code", &f.size),
        ("pins", "Pins", &f.pins),
        ("pitch", "Pitch (mm)", &f.pitch),
        ("mounting", "Mounting", &f.mounting),
        ("rows", "Rows × pins", &f.rows),
        ("gender", "Gender", &f.gender),
        ("orientation", "Orientation", &f.orientation),
        ("shrouded", "Shrouded", &f.shrouded),
        ("exposed_pad", "Exposed pad", &f.exposed_pad),
        ("review", "Review", &f.review),
        ("lifecycle", "Lifecycle", &f.lifecycle),
        ("existence", "Existence", &f.existence),
    ]
    .into_iter()
    .filter(|(_, _, v)| !v.is_empty())
    .map(|(key, title, values)| FacetGroup {
        key,
        title,
        values: values.as_slice(),
    })
    .collect()
}

/// Whether a category node or one of its descendants is selected
/// (open it by default).
pub fn category_open(node: &CategoryNode) -> bool {
    node.selected || node.children.iter().any(category_open)
}

pub struct SearchState {
    pub request: CatalogRequest,
    pub response: Option<CatalogResponse>,
    pub selected: Option<usize>,
    pub status: String,
    pub revision: String,
    job: Option<Job<CatalogResponse>>,
    edited_at: Option<Instant>,
}

impl Default for SearchState {
    fn default() -> Self {
        SearchState {
            request: CatalogRequest {
                limit: Some(PAGE),
                ..Default::default()
            },
            response: None,
            selected: None,
            status: String::new(),
            revision: String::new(),
            job: None,
            // Browse the catalog on start.
            edited_at: Some(Instant::now() - DEBOUNCE),
        }
    }
}

impl SearchState {
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The query or a filter changed: search again after the debounce,
    /// from the first page.
    pub fn edited(&mut self) {
        self.request.offset = 0;
        self.edited_at = Some(Instant::now());
    }

    /// Changed without the debounce (a checkbox, a page button).
    pub fn edited_now(&mut self) {
        self.edited_at = Some(Instant::now() - DEBOUNCE);
    }

    pub fn toggle(&mut self, key: &str, value: &str) {
        self.request.toggle(key, value);
        self.request.offset = 0;
        self.edited_now();
    }

    pub fn clear_filters(&mut self) {
        self.request.filters.clear();
        self.request.ranges.clear();
        self.request.offset = 0;
        self.edited_now();
    }

    pub fn has_filters(&self) -> bool {
        !self.request.filters.is_empty()
            || self
                .request
                .ranges
                .values()
                .any(|(a, b)| !a.trim().is_empty() || !b.trim().is_empty())
    }

    /// Clicking a sortable header: sort by it, or flip the order.
    pub fn sort_by(&mut self, key: String) {
        if self.request.sort.as_deref() == Some(key.as_str()) {
            self.request.descending = !self.request.descending;
        } else {
            self.request.sort = Some(key);
            self.request.descending = false;
        }
        self.request.offset = 0;
        self.edited_now();
    }

    pub fn page(&mut self, forward: bool) {
        let total = self.response.as_ref().map_or(0, |r| r.total);
        if forward {
            if self.request.offset + PAGE < total {
                self.request.offset += PAGE;
            }
        } else {
            self.request.offset = self.request.offset.saturating_sub(PAGE);
        }
        self.edited_now();
    }

    pub fn selected_item(&self) -> Option<&CatalogItem> {
        self.response.as_ref()?.items.get(self.selected?)
    }

    /// Per-frame: collects a finished search and fires a due one.
    /// Returns when to wake up again for a pending debounce.
    pub fn tick(&mut self, registry: &str, waker: &Waker) -> Option<Duration> {
        if let Some(r) = self.job.as_ref().and_then(Job::poll) {
            self.job = None;
            match r {
                Ok(response) => {
                    // Keep the selection when the same part is still listed.
                    let keep = self
                        .selected_item()
                        .map(|i| (i.manufacturer.clone(), i.mpn.clone()));
                    self.selected = keep.and_then(|(m, p)| {
                        response
                            .items
                            .iter()
                            .position(|i| i.manufacturer == m && i.mpn == p)
                    });
                    self.status = String::new();
                    self.response = Some(response);
                }
                Err(e) => self.status = format!("Search failed: {e}"),
            }
        }
        let edit_at = self.edited_at?;
        let elapsed = edit_at.elapsed();
        if should_auto_dispatch(elapsed, self.request.q.trim().chars().count()) {
            self.dispatch(registry, waker);
            None
        } else if elapsed < DEBOUNCE {
            Some(DEBOUNCE - elapsed)
        } else {
            None
        }
    }

    /// Runs the search now (a superseded one's result is dropped).
    pub fn dispatch(&mut self, registry: &str, waker: &Waker) {
        self.edited_at = None;
        let client = Client::new(registry);
        let request = self.request.clone();
        self.job = Some(Job::spawn(waker, move || {
            client.catalog(&request).map_err(|e| e.to_string())
        }));
    }

    #[cfg(test)]
    pub fn wait(&mut self) {
        if let Some(job) = &self.job {
            let r = job.wait();
            self.job = None;
            match r {
                Ok(response) => self.response = Some(response),
                Err(e) => self.status = e,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_client::api::CatalogParameter;

    fn item(
        category: &str,
        size: Option<&str>,
        leads: u32,
        params: &[(&str, &str)],
    ) -> CatalogItem {
        CatalogItem {
            manufacturer: "m".into(),
            manufacturer_name: "M".into(),
            mpn: format!("{category}{leads}"),
            category: Some(category.into()),
            size_code: size.map(str::to_string),
            lead_count: Some(leads),
            mounting: Some("smd".into()),
            body_width: Some(1.6),
            body_length: Some(0.8),
            body_height: None,
            lifecycle: "active".into(),
            parameters: params
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        CatalogParameter {
                            value: None,
                            label: v.to_string(),
                        },
                    )
                })
                .collect(),
            ..Default::default()
        }
    }

    fn facet(key: &str) -> ParameterFacet {
        ParameterFacet {
            key: key.into(),
            kind: "numeric".into(),
            ..Default::default()
        }
    }

    #[test]
    fn capacitor_columns_are_size_and_parameters_in_facet_order() {
        let mut response = CatalogResponse {
            items: vec![
                item(
                    "capacitor",
                    Some("0603"),
                    2,
                    &[("capacitance", "100 nF"), ("dielectric", "X7R")],
                ),
                item(
                    "capacitor",
                    Some("0402"),
                    2,
                    &[("capacitance", "1 µF"), ("rated_voltage", "16 V")],
                ),
            ],
            ..Default::default()
        };
        response.facets.parameters = vec![
            facet("capacitance"),
            facet("rated_voltage"),
            facet("dielectric"),
            facet("resistance"),
        ];
        let cols = columns(&response, &CatalogRequest::default());
        assert_eq!(
            cols,
            vec![
                Column::Mpn,
                Column::Manufacturer,
                Column::Package,
                Column::Size,
                Column::Param("capacitance".into()),
                Column::Param("rated_voltage".into()),
                Column::Param("dielectric".into()),
                Column::Body,
                Column::Lifecycle,
                Column::Review,
            ]
        );
        assert_eq!(Column::Body.cell(&response.items[0]), "1.6×0.8×?");
        assert_eq!(cols[5].cell(&response.items[0]), "");
        assert_eq!(cols[5].title(), "Rated voltage");
        assert_eq!(
            cols[4].sort_key(&response.facets.parameters).as_deref(),
            Some("capacitance")
        );
        assert_eq!(Column::Lifecycle.sort_key(&[]), None);
    }

    #[test]
    fn mixed_results_show_category_and_pins() {
        let mut ic = item("microcontroller", None, 56, &[]);
        ic.pitch = Some(0.4);
        ic.mounting = Some("smd".into());
        let mut header = item("connector", None, 20, &[]);
        header.rows = Some("1x20".into());
        header.gender = Some("male".into());
        header.mounting = Some("tht".into());
        let response = CatalogResponse {
            items: vec![ic, header],
            ..Default::default()
        };
        let cols = columns(&response, &CatalogRequest::default());
        for c in [
            Column::Category,
            Column::Pins,
            Column::Pitch,
            Column::Mounting,
            Column::Rows,
            Column::Gender,
        ] {
            assert!(cols.contains(&c), "{c:?}");
        }
        assert!(!cols.contains(&Column::Size));
        assert_eq!(Column::Pitch.cell(&response.items[0]), "0.4 mm");
        assert_eq!(Column::Gender.cell(&response.items[1]), "Male");
    }

    #[test]
    fn sorting_toggles_order_and_resets_the_page() {
        let mut s = SearchState::default();
        s.request.offset = 200;
        s.sort_by("pins".into());
        assert_eq!(s.request.sort.as_deref(), Some("pins"));
        assert!(!s.request.descending);
        assert_eq!(s.request.offset, 0);
        s.sort_by("pins".into());
        assert!(s.request.descending);
        s.toggle("size", "0603");
        assert!(s.has_filters());
        assert!(s.request.path().contains("size=0603"));
        assert!(s.request.path().contains("sort=pins&order=desc"));
        s.clear_filters();
        assert!(!s.has_filters());
    }

    #[test]
    fn debounce_rules() {
        assert!(!should_auto_dispatch(Duration::from_millis(100), 3));
        assert!(should_auto_dispatch(Duration::from_millis(300), 2));
        assert!(!should_auto_dispatch(Duration::from_millis(300), 1));
        // An empty query browses the catalog.
        assert!(should_auto_dispatch(Duration::from_millis(300), 0));
    }

    #[test]
    fn facet_groups_skip_empty_ones() {
        let mut response = CatalogResponse::default();
        response.facets.size = vec![FacetValue {
            value: "0603".into(),
            label: "0603 (1608)".into(),
            count: 3,
            selected: false,
        }];
        let groups = facet_groups(&response);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "size");
    }
}
