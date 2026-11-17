//! Two-terminal chip parts. The `chip` family spans very different real
//! constructions, so the device category -- refined by the package's
//! stated `construction` where the category alone is ambiguous (a molded
//! tantalum and an MLCC share EIA case sizes) -- picks the template:
//!
//! - thick-film resistor (`chip_resistor`): alumina core, overcoat, end caps
//! - MLCC / chip fuse / multilayer inductor (`chip_ceramic`)
//! - molded diode or tantalum (`chip_molded` + `chip_polarity_band`)
//! - chip LED (`chip_led`), SMD crystal (`chip_crystal`)
//! - shielded power inductor (`chip_power_inductor`)
//!
//! The terminal length always comes from the geometry.

use crate::common::{self, edge_points};
use crate::script::{looks, mm, Look};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_core::{ComponentCategory, PackageConstruction};

enum Style {
    Resistor,
    Ceramic(Look),
    Molded { body: Look, band: Option<Look> },
    Led,
    Crystal,
    PowerInductor,
}

fn style(input: &ModelInput<'_>) -> Style {
    let b = &input.geometry.body.size;
    let construction = input.package.construction;
    if construction == Some(PackageConstruction::TantalumMolded) {
        return Style::Molded {
            body: looks::TANTALUM,
            band: Some(looks::TANTALUM_BAND),
        };
    }
    match input.category {
        Some(ComponentCategory::Resistor) => Style::Resistor,
        // Unless stated otherwise (above), a chip capacitor is an MLCC.
        Some(ComponentCategory::Capacitor) => Style::Ceramic(looks::CERAMIC_TAN),
        Some(ComponentCategory::Inductor) if b.x.min(b.y) >= 3.0 => Style::PowerInductor,
        Some(ComponentCategory::Inductor) => Style::Ceramic(looks::CHIP_INDUCTOR),
        // Ferrite beads look like chip inductors (grey ferrite).
        Some(ComponentCategory::FerriteBead) => Style::Ceramic(looks::CHIP_INDUCTOR),
        Some(ComponentCategory::Fuse) => Style::Ceramic(looks::FUSE_BODY),
        Some(ComponentCategory::Led) => Style::Led,
        Some(ComponentCategory::Crystal) => Style::Crystal,
        Some(ComponentCategory::Diode) => Style::Molded {
            body: looks::MOLD,
            band: Some(looks::MARK_LIGHT),
        },
        _ => Style::Ceramic(looks::NEUTRAL),
    }
}

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let term = g
        .leads
        .first()
        .ok_or(ModelError::NoLeads(input.geometry.family))?;
    let b = g.body.size;
    common::body_params(s, g)?;
    // The footprint's terminal length, but never more than 30 % of the
    // body each end (an 0201's footprint terminal is half its body).
    let term_len = term.size.x.min(b.x * 0.3);
    s.fragment(fragment!("chip_params"), &vars!(term_len = mm(term_len)))?;
    let terminals = || vec!["Terminal1".to_string(), "Terminal2".to_string()];

    match style(input) {
        Style::Resistor => {
            s.fragment(
                fragment!("chip_resistor"),
                &vars!(overcoat_t = mm((b.z * 0.12).clamp(0.02, 0.06))),
            )?;
            s.paint(&["Overcoat".into()], looks::OVERCOAT);
            s.paint(&["Core".into()], looks::ALUMINA);
            s.paint(&terminals(), looks::TIN);
        }
        Style::Ceramic(core) => {
            // Points on the edges to round: the four long edges of core and
            // caps, and each cap's outer end.
            let (hl, h) = (b.y / 2.0, b.z);
            let long = |x: f64| vec![[x, -hl, 0.0], [x, hl, 0.0], [x, -hl, h], [x, hl, h]];
            let cap = |sign: f64| {
                let (mid, end) = (sign * (b.x / 2.0 - term_len / 2.0), sign * b.x / 2.0);
                let mut e = long(mid);
                e.extend([
                    [end, 0.0, 0.0],
                    [end, 0.0, h],
                    [end, -hl, h / 2.0],
                    [end, hl, h / 2.0],
                ]);
                e
            };
            s.fragment(
                fragment!("chip_ceramic"),
                &vars!(
                    edge_r = mm((b.y.min(b.z) * 0.12).max(0.01)),
                    core_edges = edge_points(&long(0.0)),
                    cap1_edges = edge_points(&cap(-1.0)),
                    cap2_edges = edge_points(&cap(1.0)),
                ),
            )?;
            s.paint(&["Core".into()], core);
            s.paint(&terminals(), looks::NICKEL);
        }
        Style::Molded { body, band } => {
            let v = common::molded_vars("body_w / 2 - term_len", "body_l / 2", 0.12, 0.35, 6.0)
                .set("tab_w", mm((b.y * 0.6).max(0.2)))
                .set("tab_embed", mm((b.x * 0.08).max(0.1)));
            s.fragment(fragment!("chip_molded"), &v)?;
            s.paint(&["Body".into()], body);
            s.paint(&["Lead1".into(), "Lead2".into()], looks::TIN);
            if let Some(band) = band {
                let at_minus_x = common::marked_lead(input).is_none_or(|l| l.position.x <= 0.0);
                s.fragment(
                    fragment!("chip_polarity_band"),
                    &vars!(neg = if at_minus_x { "-" } else { "" }),
                )?;
                s.paint(&["PolarityBand".into()], band);
            }
        }
        Style::Led => {
            s.fragment(fragment!("chip_led"), &vars!())?;
            s.paint(&["Base".into()], looks::LED_HOUSING);
            s.paint(&terminals(), looks::GOLD);
            s.paint(&["Die".into()], looks::DIE);
            s.paint(&["Lens".into()], looks::LENS);
        }
        Style::Crystal => {
            s.fragment(fragment!("chip_crystal"), &vars!())?;
            s.paint(&["Base".into()], looks::ALUMINA);
            s.paint(&["Lid".into()], looks::NICKEL);
            s.paint(&terminals(), looks::GOLD);
        }
        Style::PowerInductor => {
            let (hw, hl, zc) = (b.x / 2.0, b.y / 2.0, b.z / 2.0);
            s.fragment(
                fragment!("chip_power_inductor"),
                &vars!(
                    corner_r = mm(b.x.min(b.y) * 0.12),
                    corner_edges =
                        edge_points(&[[hw, hl, zc], [-hw, hl, zc], [hw, -hl, zc], [-hw, -hl, zc]]),
                ),
            )?;
            s.paint(&["Core".into()], looks::FERRITE);
            s.paint(&terminals(), looks::TIN);
        }
    }
    Ok(())
}
