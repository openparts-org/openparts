//! Footprint lands for SMD terminals per IPC-7351B (density level N,
//! "nominal"), for packages that don't state a recommended land pattern.
//!
//! Each land extends the terminal's contact area by the solder-fillet
//! goals for its lead form -- toe (outward), heel (inward) and side --
//! plus the RMS of the terminal's own tolerance and the fabrication and
//! placement tolerances:
//!
//! ```text
//! Z/2 = Lmin/2 + Jt + sqrt(CL^2 + F^2 + P^2) / 2      outer edge
//! G/2 = Smax/2 - Jh - sqrt(CS^2 + F^2 + P^2) / 2      inner edge
//! X   = Wmin   + 2 Js + sqrt(CW^2 + F^2 + P^2)        width
//! ```
//!
//! with L the toe-to-toe span, S the heel-to-heel span (L - 2T, T the
//! foot / terminal length, RMS-combined: Smax = Smin + sqrt(CL^2 + 2 CT^2))
//! and W the terminal width; C* are min..max ranges where the package
//! states them (else 0). Done per terminal from its distance to the body
//! centre, so asymmetric packages (TO-252 leads) work too. The same
//! method as KiCad's own footprint generator (`ipc_pad_size_calculators`).

use crate::defaults::{self, ipc};
use crate::{Lead, LeadForm, MechanicalGeometry, Mounting, PackageFamily as F, Point3, Size3};
use openparts_core::{Dimension, Package};

/// Fillet goals (mm): toe, heel, side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Fillets {
    pub toe: f64,
    pub heel: f64,
    pub side: f64,
}

/// The lands for `geometry`'s SMD terminals, or `None` when the family's
/// terminals aren't sized by IPC rules here (LGA, through-hole, lands
/// that are already the board-side pad).
pub(crate) fn lands(package: &Package, g: &MechanicalGeometry) -> Option<Vec<Lead>> {
    let pitch = g.pitch.unwrap_or(f64::INFINITY);
    let (fillets, swap) = match (package.family, g.lead_form) {
        (F::Lga, _) => return None,
        (_, Some(LeadForm::GullWing { .. })) if pitch <= 0.625 => (ipc::GULL_WING_FINE, false),
        (_, Some(LeadForm::GullWing { .. })) => (ipc::GULL_WING, false),
        (_, Some(LeadForm::Flush { .. })) => (ipc::FLAT_NO_LEAD, false),
        (_, Some(LeadForm::Flat { .. })) => (ipc::FLAT_LEAD, false),
        // Inward L: the foot's free end (the "toe") is under the body,
        // the bend (the "heel") at the overall end.
        (_, Some(LeadForm::JBend { .. })) => (ipc::INWARD_L, true),
        (F::Chip, None) => {
            if g.body.size.x >= 1.6 {
                (ipc::CHIP, false)
            } else {
                (ipc::CHIP_SMALL, false)
            }
        }
        _ => return None,
    };
    let d = &package.dimensions;
    let range = |dim: &Option<Dimension>| dim.as_ref().map_or(0.0, spread);
    let ct = range(&d.lead_length);
    let cw = range(&d.lead_width);
    let ft = ipc::FABRICATION.powi(2) + ipc::PLACEMENT.powi(2);
    let ep = g.leads.iter().find(|l| l.number == "EP");

    let mut lands: Vec<(Lead, Option<Axis>)> = g
        .leads
        .iter()
        .map(|lead| {
            if lead.mounting != Mounting::Smd || lead.number == "EP" || is_tab(package, g, lead) {
                return (lead.clone(), None);
            }
            let Some(axis) = outward_axis(g, lead) else {
                return (lead.clone(), None);
            };
            let (c, len, wid) = match axis {
                Axis::X => (lead.position.x, lead.size.x, lead.size.y),
                Axis::Y => (lead.position.y, lead.size.y, lead.size.x),
            };
            let sign = c.signum();
            let toe = c.abs() + len / 2.0; // nominal half-span
                                           // The contact length: the gull-wing foot, else the terminal.
            let foot = match g.lead_form {
                Some(LeadForm::GullWing { foot_length, .. }) => foot_length.min(len),
                _ => len,
            };
            let cl = span_tolerance(package, toe * 2.0);
            let (out_fillet, in_fillet) = if swap {
                (fillets.heel, fillets.toe)
            } else {
                (fillets.toe, fillets.heel)
            };
            let outer = toe - cl / 4.0 + out_fillet + (cl * cl + ft).sqrt() / 2.0;
            // Half-spans: Lmin/2 = toe - CL/4; Smin/2 = Lmin/2 - Tmax.
            let s_min = toe - cl / 4.0 - (foot + ct / 2.0);
            let cs = (cl * cl + 2.0 * ct * ct).sqrt();
            let mut inner = s_min + cs / 2.0 - in_fillet - (cs * cs + ft).sqrt() / 2.0;
            // Keep clear of an exposed pad.
            if let Some(ep) = ep {
                let ep_half = match axis {
                    Axis::X => ep.size.x,
                    Axis::Y => ep.size.y,
                } / 2.0;
                inner = inner.max(ep_half + ipc::EP_CLEARANCE);
            }
            let inner = inner.max(ipc::MIN_GAP / 2.0).min(outer - 0.1);
            let width = (wid - cw / 2.0 + 2.0 * fillets.side + (cw * cw + ft).sqrt()).max(0.1);
            let length = round(outer - inner);
            let centre = sign * round((outer + inner) / 2.0);
            let width = round(width);
            let (position, size) = match axis {
                Axis::X => (
                    Point3 {
                        x: centre,
                        ..lead.position
                    },
                    Size3 {
                        x: length,
                        y: width,
                        z: lead.size.z,
                    },
                ),
                Axis::Y => (
                    Point3 {
                        y: centre,
                        ..lead.position
                    },
                    Size3 {
                        x: width,
                        y: length,
                        z: lead.size.z,
                    },
                ),
            };
            (
                Lead {
                    position,
                    size,
                    ..lead.clone()
                },
                Some(axis),
            )
        })
        .collect();
    clear_corners(&mut lands);
    Some(lands.into_iter().map(|(lead, _)| lead).collect())
}

/// On a four-sided package with short lands per side (fine-pitch QFN),
/// the heels of the two lands meeting at a corner can run into each
/// other. When any do, every land of the package has its heel pulled
/// back -- uniformly per side, toes unchanged -- until the corner lands
/// keep `ipc::CORNER_CLEARANCE` from the neighbouring side's corner
/// land. Packages whose lands don't collide are left exactly as they are.
fn clear_corners(lands: &mut [(Lead, Option<Axis>)]) {
    let rect = |l: &Lead| {
        let (hx, hy) = (l.size.x / 2.0, l.size.y / 2.0);
        (
            l.position.x - hx,
            l.position.x + hx,
            l.position.y - hy,
            l.position.y + hy,
        )
    };
    // The smallest heel half-span each axis' lands need.
    let (mut need_x, mut need_y) = (0.0_f64, 0.0_f64);
    for (a, axis_a) in lands.iter() {
        let Some(Axis::X) = axis_a else { continue };
        for (b, axis_b) in lands.iter() {
            let Some(Axis::Y) = axis_b else { continue };
            let (ra, rb) = (rect(a), rect(b));
            let overlap = ra.0 < rb.1 - 1e-9
                && rb.0 < ra.1 - 1e-9
                && ra.2 < rb.3 - 1e-9
                && rb.2 < ra.3 - 1e-9;
            if overlap {
                // Clear the other land's side edge across this axis.
                need_x = need_x.max(b.position.x.abs() + b.size.x / 2.0 + ipc::CORNER_CLEARANCE);
                need_y = need_y.max(a.position.y.abs() + a.size.y / 2.0 + ipc::CORNER_CLEARANCE);
            }
        }
    }
    if need_x == 0.0 && need_y == 0.0 {
        return;
    }
    for (lead, axis) in lands.iter_mut() {
        let (c, len, need) = match axis {
            Some(Axis::X) => (lead.position.x, &mut lead.size.x, need_x),
            Some(Axis::Y) => (lead.position.y, &mut lead.size.y, need_y),
            None => continue,
        };
        let (outer, inner) = (c.abs() + *len / 2.0, c.abs() - *len / 2.0);
        if inner >= need - 1e-9 {
            continue;
        }
        let inner = need.min(outer - 0.1);
        *len = round(outer - inner);
        let centre = c.signum() * round((outer + inner) / 2.0);
        match axis {
            Some(Axis::X) => lead.position.x = centre,
            _ => lead.position.y = centre,
        }
    }
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

/// Which body axis a terminal leaves along: the edge it is nearest to,
/// relative to the body size.
fn outward_axis(g: &MechanicalGeometry, lead: &Lead) -> Option<Axis> {
    let b = &g.body;
    let (hw, hl) = (b.size.x / 2.0, b.size.y / 2.0);
    let (dx, dy) = (
        lead.position.x - b.position.x,
        lead.position.y - b.position.y,
    );
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return None;
    }
    Some(if dx.abs() / hw >= dy.abs() / hl {
        Axis::X
    } else {
        Axis::Y
    })
}

/// A TO-252/TO-263 tab (the plate under the body) keeps its outline.
fn is_tab(package: &Package, g: &MechanicalGeometry, lead: &Lead) -> bool {
    matches!(package.family, F::Dpak | F::D2pak)
        && package
            .tab
            .as_ref()
            .is_some_and(|t| t.number == lead.number)
        && std::ptr::eq(lead, g.leads.last().unwrap())
}

/// The min..max range of the stated dimension that spans `span` (the
/// lead-span, body or overall dimension it is), else 0.
fn span_tolerance(package: &Package, span: f64) -> f64 {
    let d = &package.dimensions;
    [
        d.overall_width.as_ref(),
        d.overall_length.as_ref(),
        Some(&d.body_width),
        Some(&d.body_length),
    ]
    .into_iter()
    .flatten()
    .find(|dim| defaults::required(dim).is_some_and(|v| (v - span).abs() < 0.02))
    .map_or(0.0, spread)
}

fn spread(dim: &Dimension) -> f64 {
    match (dim.min, dim.max) {
        (Some(a), Some(b)) => (b - a).max(0.0),
        _ => 0.0,
    }
}

/// To 0.01 mm, as KiCad's generator rounds pad sizes and positions.
fn round(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use crate::generate;
    use openparts_core::Package;

    fn pkg(yaml: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\n{yaml}"
        ))
        .unwrap()
    }

    /// SOIC-8 (JEDEC MS-012): E 5.80..6.20, b 0.31..0.51, L 0.40..1.27.
    /// By hand: Z = 5.8 + 0.7 + sqrt(0.4^2 + 0.1^2 + 0.05^2) = 6.915;
    /// Smin = 5.8 - 2 x 1.27 = 3.26, CS = sqrt(0.4^2 + 2 x 0.87^2) = 1.294,
    /// G = 3.26 + 1.294 - 0.7 - sqrt(1.294^2 + 0.0125) = 2.555;
    /// X = 0.31 + 0.06 + sqrt(0.2^2 + 0.0125) = 0.599.
    #[test]
    fn soic8_lands_follow_ipc_7351b() {
        let p = pkg(
            "family: soic\nlead_count: 8\npitch: {nominal: 1.27, unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {nominal: 3.9, unit: mm}\n\
             \x20 body_length: {nominal: 4.9, unit: mm}\n\
             \x20 overall_width: {min: 5.80, max: 6.20, unit: mm}\n\
             \x20 lead_width: {min: 0.31, max: 0.51, unit: mm}\n\
             \x20 lead_length: {min: 0.40, max: 1.27, unit: mm}\n",
        );
        let g = generate(&p).unwrap();
        let pad1 = &g.lands[0];
        let (outer, inner) = (
            -pad1.position.x + pad1.size.x / 2.0,
            -pad1.position.x - pad1.size.x / 2.0,
        );
        assert!((outer - 6.915 / 2.0).abs() < 0.011, "{pad1:?}");
        assert!((inner - 2.555 / 2.0).abs() < 0.011, "{pad1:?}");
        assert!((pad1.size.y - 0.6).abs() < 0.006, "{pad1:?}");
        // The lead (3D) keeps its own outline.
        assert!((g.leads[0].size.y - 0.41).abs() < 1e-9);
    }

    /// QFN-24 3x3, 0.4 pitch, L 0.30, no exposed pad (TDK ICM-20948):
    /// the plain IPC lands of pins 1 and 24 cross at the corner, so every
    /// heel is pulled back to clear the other side's corner land by
    /// `CORNER_CLEARANCE`, toes where they were.
    #[test]
    fn corner_lands_that_collide_have_their_heels_pulled_back() {
        let p = pkg(
            "family: qfn\nlead_count: 24\npitch: {nominal: 0.4, unit: mm}\n\
             dimensions:\n\
             \x20 body_width: {min: 2.9, nominal: 3.0, max: 3.1, unit: mm}\n\
             \x20 body_length: {min: 2.9, nominal: 3.0, max: 3.1, unit: mm}\n\
             \x20 lead_width: {min: 0.15, nominal: 0.2, max: 0.25, unit: mm}\n\
             \x20 lead_length: {min: 0.25, nominal: 0.3, max: 0.35, unit: mm}\n",
        );
        let g = generate(&p).unwrap();
        let land = |n: &str| g.lands.iter().find(|l| l.number == n).unwrap();
        let (p1, p3, p24) = (land("1"), land("3"), land("24"));
        // Unchanged toe (1.87 before the corner pass), shortened heel.
        let toe = |l: &crate::Lead| -l.position.x + l.size.x / 2.0;
        let heel = |l: &crate::Lead| -l.position.x - l.size.x / 2.0;
        assert!((toe(p1) - 1.87).abs() < 0.006, "{p1:?}");
        assert!((toe(p3) - toe(p1)).abs() < 1e-9 && (heel(p3) - heel(p1)).abs() < 1e-9);
        // Pin 1's heel clears pin 24's side edge by the clearance...
        let side_24 = -p24.position.x + p24.size.x / 2.0;
        assert!(heel(p1) >= side_24 + 0.2 - 0.006, "{p1:?} {p24:?}");
        // ... and pin 24's heel (top side) likewise clears pin 1.
        let heel_24 = p24.position.y.abs() - p24.size.y / 2.0;
        let side_1 = p1.position.y.abs() + p1.size.y / 2.0;
        assert!(heel_24 >= side_1 + 0.2 - 0.006, "{p1:?} {p24:?}");
        assert!((p24.size.y - p1.size.x).abs() < 1e-9);
    }

    #[test]
    fn lga_and_stated_lands_are_left_alone() {
        let p = pkg("family: lga\nlead_count: 8\nlead_layout: [4, 0, 4, 0]\n\
             pitch: {nominal: 0.65, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 2.5, unit: mm}\n\
             \x20 body_length: {nominal: 2.5, unit: mm}\n");
        assert!(generate(&p).unwrap().lands.is_empty());
    }
}
