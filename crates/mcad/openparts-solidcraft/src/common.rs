//! The decisions shared by every family: which body edge each lead row is
//! on and how a row is laid out in parameters, plus the few fragments
//! every family uses (body size, molded body, pin-1 dimple, lead rows).
//! The shapes themselves are in `templates/`.
//!
//! Lead positions always come from `MechanicalGeometry` (the same source
//! the footprint generators use), so a 3D lead sits exactly on its pad.

use crate::script::{mm, num, Script};
use crate::template::{fragment, vars, TemplateError, Vars};
use openparts_mcad::{Lead, MarkerKind, MechanicalGeometry};
use serde_json::{json, Value};

const TOL: f64 = 1e-6;

/// Declares `body_w` (X), `body_l` (Y) and `body_h` (Z) from the geometry.
pub fn body_params(s: &mut Script, g: &MechanicalGeometry) -> Result<(), TemplateError> {
    let b = g.body.size;
    s.fragment(
        fragment!("body_params"),
        &vars!(w = mm(b.x), l = mm(b.y), h = mm(b.z)),
    )
}

/// Variables for `molded_body`: an outline of `half_x` x `half_y`.
pub fn molded_vars(
    half_x: &str,
    half_y: &str,
    standoff: f64,
    parting_ratio: f64,
    draft_deg: f64,
) -> Vars {
    vars!(
        half_x = half_x,
        half_y = half_y,
        standoff = mm(standoff),
        parting_ratio = num(parting_ratio),
        draft = format!("{} deg", num(draft_deg)),
    )
}

/// The drafted mold body `Body` over the whole `body_w` x `body_l`.
pub fn molded_body(
    s: &mut Script,
    standoff: f64,
    parting_ratio: f64,
    draft_deg: f64,
) -> Result<(), TemplateError> {
    s.fragment(
        fragment!("molded_body"),
        &molded_vars(
            "body_w / 2",
            "body_l / 2",
            standoff,
            parting_ratio,
            draft_deg,
        ),
    )
}

/// The pin-1 dimple at the geometry's `Pin1Dot` marker, if it has one.
pub fn pin1_dimple(
    s: &mut Script,
    g: &MechanicalGeometry,
    body: &str,
    top: &str,
) -> Result<(), TemplateError> {
    let Some(marker) = g.markers.iter().find(|m| m.kind == MarkerKind::Pin1Dot) else {
        return Ok(());
    };
    let b = &g.body;
    let d = (b.size.x.min(b.size.y) * 0.1).clamp(0.3, 0.9);
    // At least the marker's inset, and always far enough in that the
    // dimple clears the top edge however far the draft pulls it in.
    let min_inset = d / 2.0 + 0.25 + b.size.z * 0.25;
    let inset_x = (marker.position.x - b.position.x + b.size.x / 2.0).max(min_inset);
    let inset_y = (b.position.y + b.size.y / 2.0 - marker.position.y).max(min_inset);
    s.fragment(
        fragment!("pin1_dimple"),
        &vars!(
            d = mm(d),
            inset_x = mm(inset_x),
            inset_y = mm(inset_y),
            top = top,
            body = body
        ),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Bottom,
    Right,
    Top,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Left, Side::Bottom, Side::Right, Side::Top];

    /// Whether the side's outward direction is along X (left/right).
    pub fn across_x(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }
    pub fn tag(self) -> &'static str {
        match self {
            Side::Left => "L",
            Side::Bottom => "B",
            Side::Right => "R",
            Side::Top => "T",
        }
    }
    fn word(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Bottom => "bottom",
            Side::Right => "right",
            Side::Top => "top",
        }
    }
    /// `"-"` where the outward direction is negative (left, bottom).
    pub fn neg(self) -> &'static str {
        if matches!(self, Side::Left | Side::Bottom) {
            "-"
        } else {
            ""
        }
    }
    fn along_dir(self) -> Value {
        if self.across_x() {
            json!([0, 1, 0])
        } else {
            json!([1, 0, 0])
        }
    }
    /// The variables a row fragment draws with: the vertical plane through
    /// the row at `first` (sketch x = world +X on left/right, +Y on
    /// bottom/top; sketch y = world +Z), the outward sign and the body's
    /// half size outward.
    pub fn row_vars(self, first: &str) -> Vars {
        let (plane, flip) = if self.across_x() {
            ("XZ", json!("x"))
        } else {
            ("YZ", json!(false))
        };
        vars!(
            tag = self.tag(),
            plane = plane,
            flip = flip,
            first = first,
            neg = self.neg(),
            half = if self.across_x() {
                "body_w / 2"
            } else {
                "body_l / 2"
            },
        )
    }
}

/// The leads along one body edge, in ascending order along the edge.
/// A side whose leads are not evenly spaced (a lead position left
/// empty) is split into evenly spaced runs, `run` 0, 1, ...
#[derive(Debug, Clone)]
pub struct Row<'a> {
    pub side: Side,
    pub leads: Vec<&'a Lead>,
    pub run: u32,
}

impl Row<'_> {
    /// The row's name in the script: the side's tag, plus the run's
    /// number from the second run on (`L`, `L2`).
    pub fn tag(&self) -> String {
        if self.run == 0 {
            self.side.tag().to_string()
        } else {
            format!("{}{}", self.side.tag(), self.run + 1)
        }
    }

    pub fn along(&self, lead: &Lead) -> f64 {
        if self.side.across_x() {
            lead.position.y
        } else {
            lead.position.x
        }
    }
    pub fn first(&self) -> f64 {
        self.along(self.leads[0])
    }
    /// The common spacing, when the leads are evenly spaced.
    pub fn spacing(&self) -> Option<f64> {
        if self.leads.len() < 2 {
            return None;
        }
        let d = self.along(self.leads[1]) - self.along(self.leads[0]);
        self.leads
            .windows(2)
            .all(|w| (self.along(w[1]) - self.along(w[0]) - d).abs() < TOL)
            .then_some(d)
    }
    /// Lead size outward from the body edge, and along the edge.
    pub fn lead_size(&self) -> (f64, f64) {
        let l = self.leads[0];
        if self.side.across_x() {
            (l.size.x, l.size.y)
        } else {
            (l.size.y, l.size.x)
        }
    }
    /// How far the lead's outer end lies from the body centre.
    pub fn outer_extent(&self) -> f64 {
        let l = self.leads[0];
        let c = if self.side.across_x() {
            l.position.x
        } else {
            l.position.y
        };
        c.abs() + self.lead_size().0 / 2.0
    }
}

/// Groups the perimeter leads (everything but an exposed pad and an LGA's
/// extra terminals) by the body edge they belong to, each edge in evenly
/// spaced runs.
pub fn perimeter_rows(geometry: &MechanicalGeometry) -> Vec<Row<'_>> {
    let extra: &[String] = match &geometry.details {
        openparts_mcad::Details::ExtraTerminals(names) => names,
        _ => &[],
    };
    let b = &geometry.body;
    let (hw, hl) = (b.size.x / 2.0, b.size.y / 2.0);
    let side_of = |l: &Lead| {
        let (dx, dy) = (l.position.x - b.position.x, l.position.y - b.position.y);
        if dx.abs() / hw >= dy.abs() / hl {
            if dx < 0.0 {
                Side::Left
            } else {
                Side::Right
            }
        } else if dy < 0.0 {
            Side::Bottom
        } else {
            Side::Top
        }
    };
    Side::ALL
        .iter()
        .filter_map(|&side| {
            let mut leads: Vec<&Lead> = geometry
                .leads
                .iter()
                .filter(|l| l.number != "EP" && !extra.contains(&l.number) && side_of(l) == side)
                .collect();
            if leads.is_empty() {
                return None;
            }
            let row = Row {
                side,
                leads: vec![],
                run: 0,
            };
            leads.sort_by(|a, b| row.along(a).total_cmp(&row.along(b)));
            Some(split_runs(side, leads))
        })
        .flatten()
        .collect()
}

/// `leads` (sorted along `side`) as runs of even spacing, greedily: a
/// run takes its first gap and ends where the next gap differs.
fn split_runs(side: Side, leads: Vec<&Lead>) -> Vec<Row<'_>> {
    let probe = Row {
        side,
        leads: vec![],
        run: 0,
    };
    let mut runs: Vec<Vec<&Lead>> = Vec::new();
    for lead in leads {
        let fits = runs.last().is_some_and(|r| match r.as_slice() {
            [.., a, b] => {
                let d = probe.along(b) - probe.along(a);
                (probe.along(lead) - probe.along(b) - d).abs() < TOL
            }
            _ => true,
        });
        match runs.last_mut() {
            Some(r) if fits => r.push(lead),
            _ => runs.push(vec![lead]),
        }
    }
    runs.into_iter()
        .enumerate()
        .map(|(i, leads)| Row {
            side,
            leads,
            run: i as u32,
        })
        .collect()
}

/// How a row is laid out in parameters: its lead count, the position of
/// its first lead and the spacing, each an expression.
pub struct RowLayout {
    pub count: String,
    pub first: String,
    pub spacing: String,
    pub n: u32,
}

/// Declares the count/spacing parameters for `rows`: one shared
/// `leads_per_side` when every row has the same count, otherwise one per
/// edge; `pitch` when the spacing is the package pitch. A centred row's
/// first lead is written as `-(count - 1) * pitch / 2`, so the whole row
/// follows edits to either.
pub fn row_layouts(s: &mut Script, rows: &[Row<'_>], pitch: Option<f64>) -> Vec<RowLayout> {
    if let Some(p) = pitch {
        s.param("pitch", mm(p), "lead pitch (datasheet)");
    }
    let shared = rows.len() > 1 && rows.iter().all(|r| r.leads.len() == rows[0].leads.len());
    rows.iter()
        .map(|row| {
            let n = row.leads.len() as u32;
            let count = if n == 1 {
                "1".to_string()
            } else if shared {
                s.count("leads_per_side", n, "leads on each edge");
                "leads_per_side".to_string()
            } else if row.run == 0 {
                let name = format!("leads_{}", row.side.word());
                s.count(&name, n, &format!("leads on the {} edge", row.side.word()));
                name
            } else {
                let name = format!("leads_{}{}", row.side.word(), row.run + 1);
                s.count(
                    &name,
                    n,
                    &format!(
                        "leads in run {} of the {} edge",
                        row.run + 1,
                        row.side.word()
                    ),
                );
                name
            };
            let spacing = match (row.spacing(), pitch) {
                (Some(d), Some(p)) if (d - p).abs() < TOL => "pitch".to_string(),
                (Some(d), _) => mm(d),
                (None, _) => "0 mm".to_string(),
            };
            let centred = row.spacing().map_or(row.first().abs() < TOL, |d| {
                (row.first() + (n as f64 - 1.0) * d / 2.0).abs() < TOL
            });
            let first = if centred && n > 1 {
                format!("-({count} - 1) * {spacing} / 2")
            } else {
                mm(row.first())
            };
            RowLayout {
                count,
                first,
                spacing,
                n,
            }
        })
        .collect()
}

/// For each row: `seed_fragment` at the row's first lead (it must create
/// the body `{prefix}{tag}`), repeated along the row. Returns every body.
pub fn lead_rows(
    s: &mut Script,
    rows: &[Row<'_>],
    layouts: &[RowLayout],
    seed_fragment: crate::template::Fragment,
    prefix: &str,
) -> Result<Vec<String>, TemplateError> {
    lead_rows_with(s, rows, layouts, seed_fragment, prefix, &Vars::new())
}

/// [`lead_rows`] with `extra` variables for the seed fragment (e.g. which
/// width parameter a gull-wing row extrudes).
pub fn lead_rows_with(
    s: &mut Script,
    rows: &[Row<'_>],
    layouts: &[RowLayout],
    seed_fragment: crate::template::Fragment,
    prefix: &str,
    extra: &Vars,
) -> Result<Vec<String>, TemplateError> {
    let mut bodies = Vec::new();
    for (row, layout) in rows.iter().zip(layouts) {
        let at = row
            .side
            .row_vars(&layout.first)
            .with(&vars!(tag = row.tag()))
            .with(extra);
        s.fragment(seed_fragment, &at)?;
        let seed = format!("{prefix}{}", row.tag());
        if layout.n > 1 {
            s.fragment(
                fragment!("pattern_row"),
                &vars!(
                    seed = seed.clone(),
                    dir = row.side.along_dir(),
                    count = layout.count.clone(),
                    spacing = layout.spacing.clone()
                ),
            )?;
        }
        bodies.extend(copies(&seed, layout.n));
    }
    Ok(bodies)
}

/// The names SolidCraft gives a body and its pattern copies.
pub fn copies(seed: &str, n: u32) -> Vec<String> {
    (1..=n)
        .map(|i| {
            if i == 1 {
                seed.to_string()
            } else {
                format!("{seed} ({i})")
            }
        })
        .collect()
}

/// A point on an edge, for picking it in `solid.fillet` (rounded to 1 nm
/// so scripts print reproducibly).
pub fn edge_point(p: [f64; 3]) -> Value {
    json!(p.map(|v| (v * 1e6).round() / 1e6))
}

pub fn edge_points(points: &[[f64; 3]]) -> Value {
    Value::Array(points.iter().copied().map(edge_point).collect())
}

/// The lead the polarity mark belongs to ([`crate::ModelInput::marked_pin`],
/// else pin 1), falling back to the first lead.
pub fn marked_lead<'a>(input: &crate::ModelInput<'a>) -> Option<&'a Lead> {
    let leads = &input.geometry.leads;
    leads
        .iter()
        .find(|l| l.number == input.mark())
        .or(leads.first())
}
