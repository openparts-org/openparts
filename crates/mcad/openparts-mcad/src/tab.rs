//! Power packages with a wide tab soldered to the board: SOT-223 and
//! TO-252 (DPAK).
//!
//! Frame (both families, the same as KiCad's footprints and TI's outline
//! drawings): the leads leave the LEFT body edge, the tab the RIGHT one.
//! Leads are numbered 1..`lead_count` from the top of the left edge
//! downward at `pitch`, centred on the body; `body_width` is the body
//! along X (across the lead direction), `body_length` along Y, and
//! `overall_width` the whole span along X (lead toe to tab toe / tab end).
//! The tab's pad number comes from `Package::tab` -- never assumed -- and
//! the tab is always the LAST lead of the geometry (its number may repeat
//! a lead's, e.g. a tab tied to pin 2).

use crate::defaults::{self, value, Axis, GullWing};
use crate::{
    Body, BodyShape, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Mounting, Point3,
    Size3,
};
use openparts_core::{Package, PackageFamily as F};

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Sot223,
    Dpak,
    D2pak,
}

/// Generates Mechanical Geometry for a SOT-223 (JEDEC TO-261): gull-wing
/// leads on the left and a wide gull-wing tab, `dimensions.tab_width`
/// wide, on the right, both reaching `overall_width / 2` (the package is
/// symmetric about the body centre). Unstated lead dimensions fall back
/// to `defaults::SOT223`.
pub fn generate_sot223(package: &Package) -> Result<MechanicalGeometry, McadError> {
    generate_tabbed(package, Kind::Sot223)
}

/// Generates Mechanical Geometry for a TO-252 / DPAK: gull-wing leads on
/// the left (lead positions listed in `tab.trimmed_leads` -- the centre
/// lead, cut at the body -- get no pad) and the flat tab on the right,
/// reaching `dimensions.tab_length` (L3) past the body edge, so the
/// span is asymmetric: lead toe at `-(overall_width - body_width/2 -
/// tab_length)`, tab end at `body_width/2 + tab_length`. The tab's pad is
/// its soldered underside: from the tab end back under the body
/// (`defaults::DPAK_TAB.under_body_ratio` of the body, since drawings
/// give that only as a minimum).
pub fn generate_dpak(package: &Package) -> Result<MechanicalGeometry, McadError> {
    generate_tabbed(package, Kind::Dpak)
}

/// Generates Mechanical Geometry for a TO-263 / D2PAK: the TO-252 layout
/// ([`generate_dpak`]) at TO-263's size, with `defaults::D2PAK` for what
/// the package doesn't state.
pub fn generate_d2pak(package: &Package) -> Result<MechanicalGeometry, McadError> {
    generate_tabbed(package, Kind::D2pak)
}

fn generate_tabbed(package: &Package, kind: Kind) -> Result<MechanicalGeometry, McadError> {
    let expected = match kind {
        Kind::Sot223 => F::Sot223,
        Kind::Dpak => F::Dpak,
        Kind::D2pak => F::D2pak,
    };
    if package.family != expected {
        return Err(McadError::UnsupportedFamily(package.family));
    }

    let lead_count = package.lead_count;
    if lead_count == 0 {
        return Err(McadError::InvalidLeadCount(lead_count));
    }
    let tab = package.tab.as_ref().ok_or(McadError::MissingTab {
        family: package.family,
    })?;
    if tab.number.trim().is_empty() {
        return Err(McadError::InvalidTab("tab.number is empty".into()));
    }
    let mut trimmed = Vec::new();
    for lead in &tab.trimmed_leads {
        match lead.parse::<u32>() {
            Ok(n) if (1..=lead_count).contains(&n) => trimmed.push(n),
            _ => {
                return Err(McadError::InvalidTab(format!(
                    "trimmed lead \"{lead}\" is not a lead position (1..={lead_count})"
                )))
            }
        }
    }
    if trimmed.len() as u32 >= lead_count {
        return Err(McadError::InvalidTab(
            "every lead is trimmed; nothing would reach the board".into(),
        ));
    }

    let pitch = defaults::dim(&package.pitch, "pitch.nominal")?;
    let d = &package.dimensions;
    // Body size: nominal, else the stated min..max midpoint.
    let body_w = defaults::required(&d.body_width)
        .ok_or(McadError::MissingDimension("dimensions.body_width"))?;
    let body_l = defaults::required(&d.body_length)
        .ok_or(McadError::MissingDimension("dimensions.body_length"))?;
    let tab_w = value(&d.tab_width).ok_or(McadError::MissingDimension("dimensions.tab_width"))?;
    if tab_w > body_l {
        return Err(McadError::InconsistentDimensions(format!(
            "dimensions.tab_width ({tab_w}) is wider than the body ({body_l})"
        )));
    }

    let shape = GullWing::resolve(
        package,
        match kind {
            Kind::Sot223 => &defaults::SOT223,
            Kind::Dpak => &defaults::DPAK,
            Kind::D2pak => &defaults::D2PAK,
        },
    );
    let lead_t = shape.thickness;
    let half_w = body_w / 2.0;

    // Lead reach (body edge to toe) and the tab's outline (x from..to,
    // thickness).
    let (reach, tab_from, tab_to, tab_t) = match kind {
        Kind::Sot223 => {
            let reach = shape.protrusion(package, Axis::X)?;
            (reach, half_w, half_w + reach, lead_t)
        }
        Kind::Dpak | Kind::D2pak => {
            let (tab_defaults, lead_defaults) = if kind == Kind::Dpak {
                (&defaults::DPAK_TAB, &defaults::DPAK)
            } else {
                (&defaults::D2PAK_TAB, &defaults::D2PAK)
            };
            let tab_len = value(&d.tab_length).unwrap_or(tab_defaults.length);
            let reach = match value(&d.overall_width) {
                Some(span) => {
                    let reach = span - body_w - tab_len;
                    if reach <= 0.0 {
                        return Err(McadError::InconsistentDimensions(format!(
                            "dimensions.overall_width ({span}) must exceed body_width ({body_w}) \
                             plus tab_length ({tab_len})"
                        )));
                    }
                    reach
                }
                None => lead_defaults.protrusion,
            };
            let end = half_w + tab_len;
            let from = half_w - body_w * tab_defaults.under_body_ratio;
            (reach, from, end, tab_defaults.thickness)
        }
    };

    let body = Body {
        position: Point3 {
            x: 0.0,
            y: 0.0,
            z: shape.body_height / 2.0,
        },
        size: Size3 {
            x: body_w,
            y: body_l,
            z: shape.body_height,
        },
        shape: BodyShape::Box,
    };

    let half_span = (lead_count as f64 - 1.0) * pitch / 2.0;
    let mut leads: Vec<Lead> = (1..=lead_count)
        .filter(|n| !trimmed.contains(n))
        .map(|n| Lead {
            number: n.to_string(),
            position: Point3 {
                x: -(half_w + reach / 2.0),
                y: half_span - (n - 1) as f64 * pitch,
                z: lead_t / 2.0,
            },
            size: Size3 {
                x: reach,
                y: shape.lead_width,
                z: lead_t,
            },
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
        })
        .collect();
    leads.push(Lead {
        number: tab.number.clone(),
        position: Point3 {
            x: (tab_from + tab_to) / 2.0,
            y: 0.0,
            z: tab_t / 2.0,
        },
        size: Size3 {
            x: tab_to - tab_from,
            y: tab_w,
            z: tab_t,
        },
        mounting: Mounting::Smd,
        drill: None,
        slot: None,
    });

    let inset = (body_w.min(body_l) * 0.15).min(0.5);
    let markers = vec![Marker {
        kind: MarkerKind::Pin1Dot,
        position: Point3 {
            x: -half_w + inset,
            y: body_l / 2.0 - inset,
            z: shape.body_height,
        },
    }];

    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],

        pitch: crate::defaults::required(&package.pitch),

        details: crate::Details::TrimmedLeads(
            trimmed
                .iter()
                .map(|&n| (n, half_span - (n - 1) as f64 * pitch))
                .collect(),
        ),
        body,
        leads,
        markers,
        lead_form: Some(shape.lead_form()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Dimension, Kind as DocKind, PackageDimensions, PackageTab};
    use std::collections::BTreeMap;

    fn mm(v: f64) -> Dimension {
        Dimension {
            nominal: Some(v),
            min: None,
            max: None,
            unit: "mm".into(),
        }
    }
    fn range(min: f64, max: f64) -> Option<Dimension> {
        Some(Dimension {
            nominal: None,
            min: Some(min),
            max: Some(max),
            unit: "mm".into(),
        })
    }

    /// TI DCY0004A-like SOT-223, tab numbered "4".
    fn sot223() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: DocKind::Package,
            id: openparts_core::PackageId::from("standards/SOT223-TEST"),
            family: openparts_core::PackageFamily::Sot223,
            lead_count: 3,
            pitch: mm(2.3),
            dimensions: PackageDimensions {
                body_width: mm(3.5),
                body_length: mm(6.5),
                overall_width: range(6.7, 7.3),
                standoff: range(0.02, 0.10),
                lead_width: range(0.66, 0.84),
                lead_thickness: range(0.23, 0.35),
                tab_width: range(2.9, 3.1),
                ..Default::default()
            },
            lead_layout: None,
            tab: Some(PackageTab {
                number: "4".into(),
                trimmed_leads: vec![],
            }),
            aux_pads: vec![],
            pad_order: vec![],
            pads: vec![],
            construction: None,
            geometry: None,
            provenance: BTreeMap::new(),
            review: None,
        }
    }

    /// TI NDP0003B-like TO-252, centre lead trimmed, tab numbered "2".
    fn dpak() -> Package {
        let mut p = sot223();
        p.family = openparts_core::PackageFamily::Dpak;
        p.pitch = mm(2.285);
        p.dimensions.body_width = mm(6.1);
        p.dimensions.body_length = mm(6.54);
        p.dimensions.overall_width = range(9.40, 10.42);
        p.dimensions.lead_width = range(0.64, 0.88);
        p.dimensions.tab_width = range(4.96, 5.46);
        p.dimensions.tab_length = range(0.88, 1.27);
        p.tab = Some(PackageTab {
            number: "2".into(),
            trimmed_leads: vec!["2".into()],
        });
        p
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn sot223_leads_left_tab_right_numbered_from_the_source() {
        let g = generate_sot223(&sot223()).unwrap();
        let numbers: Vec<&str> = g.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, ["1", "2", "3", "4"]);
        // Pin 1 at the top of the left edge, pitch 2.3 downward.
        assert!(close(g.leads[0].position.y, 2.3));
        assert!(close(g.leads[2].position.y, -2.3));
        assert!(g.leads[..3].iter().all(|l| l.position.x < 0.0));
        // Lead toe at -HE/2, tab toe at +HE/2 (HE = 6.7..7.3 midpoint).
        let pin1 = &g.leads[0];
        assert!(close(pin1.position.x - pin1.size.x / 2.0, -3.5));
        let tab = g.leads.last().unwrap();
        assert!(close(tab.position.x + tab.size.x / 2.0, 3.5));
        assert!(close(tab.position.x - tab.size.x / 2.0, 1.75));
        // Pad sizes from the data: b 0.75, b2 3.0.
        assert!(close(pin1.size.y, 0.75) && close(tab.size.y, 3.0));
        assert!(close(tab.position.y, 0.0));
    }

    #[test]
    fn sot223_tab_may_repeat_a_lead_number() {
        let mut p = sot223();
        p.tab.as_mut().unwrap().number = "2".into();
        let g = generate_sot223(&p).unwrap();
        assert_eq!(g.leads.iter().filter(|l| l.number == "2").count(), 2);
    }

    #[test]
    fn dpak_trims_the_centre_lead_and_spans_overall_width_asymmetrically() {
        let g = generate_dpak(&dpak()).unwrap();
        let numbers: Vec<&str> = g.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, ["1", "3", "2"], "pin 2 is the tab, not a lead");
        let toe = g
            .leads
            .iter()
            .map(|l| l.position.x - l.size.x / 2.0)
            .fold(f64::MAX, f64::min);
        let end = g
            .leads
            .iter()
            .map(|l| l.position.x + l.size.x / 2.0)
            .fold(f64::MIN, f64::max);
        assert!(close(end - toe, 9.91), "toe-to-tab span is H");
        assert!(close(end, 3.05 + 1.075), "tab ends L3 past the body");
        assert!(close(g.leads[0].position.y, 2.285));
        assert!(close(g.leads[1].position.y, -2.285));
        let tab = g.leads.last().unwrap();
        assert!(close(tab.size.y, 5.21));
        // Soldered under the body, never reaching the lead-side edge.
        assert!(tab.position.x - tab.size.x / 2.0 > -3.05);
        assert!(close(g.leads[0].size.y, 0.76));
    }

    #[test]
    fn dpak_without_spans_uses_to252_defaults() {
        let mut p = dpak();
        p.dimensions.overall_width = None;
        p.dimensions.tab_length = None;
        let g = generate_dpak(&p).unwrap();
        let pin1 = &g.leads[0];
        assert!(close(pin1.size.x, 3.0));
        let tab = g.leads.last().unwrap();
        assert!(close(tab.position.x + tab.size.x / 2.0, 3.05 + 1.0));
    }

    #[test]
    fn missing_tab_or_tab_width_is_an_error_not_a_guess() {
        let mut p = sot223();
        p.tab = None;
        assert!(matches!(
            generate_sot223(&p),
            Err(McadError::MissingTab { .. })
        ));
        let mut p = sot223();
        p.dimensions.tab_width = None;
        assert!(matches!(
            generate_sot223(&p),
            Err(McadError::MissingDimension("dimensions.tab_width"))
        ));
        let mut p = dpak();
        p.dimensions.body_width = range(6.0, 6.2).unwrap();
        let g = generate_dpak(&p).unwrap();
        assert!(close(g.body.size.x, 6.1), "min/max-only body: midpoint");
        // A single stated limit ("6.2 max") is drawn at that limit.
        p.dimensions.body_width.min = None;
        let g = generate_dpak(&p).unwrap();
        assert!(close(g.body.size.x, 6.2), "max-only body: the limit");
        // Nothing stated at all is still an error, never a guess.
        p.dimensions.body_width.max = None;
        assert!(matches!(
            generate_dpak(&p),
            Err(McadError::MissingDimension(_))
        ));
    }

    #[test]
    fn d2pak_uses_the_dpak_layout_with_to263_defaults() {
        let mut p = dpak();
        p.family = openparts_core::PackageFamily::D2pak;
        p.pitch = mm(2.54);
        p.dimensions.body_width = range(8.38, 9.65).unwrap();
        p.dimensions.body_length = range(9.65, 10.67).unwrap();
        p.dimensions.overall_width = range(14.61, 15.88);
        p.dimensions.tab_length = None;
        p.dimensions.tab_width = range(6.5, 8.0);
        p.dimensions.lead_width = None;
        let g = generate_d2pak(&p).unwrap();
        let numbers: Vec<&str> = g.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, ["1", "3", "2"]);
        assert!(close(g.leads[0].position.y, 2.54));
        assert!(close(g.leads[0].size.y, 0.81));
        let tab = g.leads.last().unwrap();
        assert!(close(tab.position.x + tab.size.x / 2.0, 9.015 / 2.0 + 1.3));
        assert!(close(g.body.size.z, 4.45));
        assert!(matches!(
            generate_dpak(&p),
            Err(McadError::UnsupportedFamily(_))
        ));
    }

    #[test]
    fn rejects_bad_trimmed_leads_spans_and_families() {
        let mut p = dpak();
        p.tab.as_mut().unwrap().trimmed_leads = vec!["4".into()];
        assert!(matches!(generate_dpak(&p), Err(McadError::InvalidTab(_))));
        let mut p = dpak();
        p.dimensions.overall_width = Some(mm(7.0));
        assert!(matches!(
            generate_dpak(&p),
            Err(McadError::InconsistentDimensions(_))
        ));
        assert!(matches!(
            generate_sot223(&dpak()),
            Err(McadError::UnsupportedFamily(_))
        ));
        let mut p = dpak();
        p.family = openparts_core::PackageFamily::Dpak;
        assert!(generate_dpak(&p).is_ok());
    }
}
