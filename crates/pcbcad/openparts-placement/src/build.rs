//! Building the model from Mechanical Geometry.
//!
//! The generators lay a package out in their own frame (`openparts-mcad`;
//! each family's origin and turn are its own). The model frame is that
//! frame moved to the land-pattern centroid and turned by a quarter-turn
//! multiple so pin 1 is upper left (IPC-7351B Level A): every point is
//! `p = R(90°·q) · (p_gen − c)`. The 3D model, built in the generator
//! frame, gets the same transform as its `model3d` alignment.

use crate::hash::content_hash;
use crate::*;
use openparts_core::{Device, Package, PackageFamily as F, PackageSide};
use openparts_mcad::{BodyShape, Details, HeaderKind, LeadForm, MarkerKind, MechanicalGeometry};

/// Courtyard corners snap outward to this grid (KiCad's libraries' rule).
const COURTYARD_GRID: f64 = 0.05;
/// IPC-7351B courtyard excess for surface-mount lands: L, N, M.
const COURTYARD_SMD: [f64; 3] = [0.10, 0.25, 0.50];
/// Through-hole parts (IPC-7351B covers SMT only): the generator's
/// default, N as KiCad's library convention.
const COURTYARD_THT: [f64; 3] = [0.25, 0.50, 1.00];
/// Segments of a circle outline.
const CIRCLE_SEGMENTS: usize = 32;
const EPS: f64 = 1e-6;

/// The model of a geometry alone (no Package / Device): pads without pin
/// names, no edge or keep-out statements, every basis
/// `generator_default` except the IPC courtyard.
pub fn from_geometry(geometry: &MechanicalGeometry, density: Density) -> PlacementModel {
    make(geometry, None, None, density)
}

/// The model of `package`'s `geometry` (`openparts_mcad::generate`), with
/// the device's pin names and shared contacts when given.
pub fn build(
    package: &Package,
    device: Option<&Device>,
    geometry: &MechanicalGeometry,
    density: Density,
) -> PlacementModel {
    make(geometry, Some(package), device, density)
}

/// Generator frame -> model frame: `R(90°·q) · (p − c)`.
#[derive(Clone, Copy)]
struct ToModel {
    q: u8,
    c: Point2,
}

impl ToModel {
    fn point(&self, x: f64, y: f64) -> Point2 {
        let (dx, dy) = (x - self.c.x, y - self.c.y);
        let (x, y) = turn(self.q, dx, dy);
        Point2 { x: r4(x), y: r4(y) }
    }
    fn size(&self, x: f64, y: f64) -> Size2 {
        if self.q.is_multiple_of(2) {
            Size2 { x: r4(x), y: r4(y) }
        } else {
            Size2 { x: r4(y), y: r4(x) }
        }
    }
    fn side(&self, side: PackageSide) -> PackageSide {
        let (x, y) = side.normal();
        let (x, y) = turn(self.q, x, y);
        PackageSide::from_normal(x, y)
    }
}

/// (x, y) turned q quarter turns CCW.
fn turn(q: u8, x: f64, y: f64) -> (f64, f64) {
    match q % 4 {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        _ => (y, -x),
    }
}

/// The pad IPC-7351B's zero orientation places: "1", else "A1", else
/// the first plated pad.
fn pin1_index(numbers: &[(&str, bool)]) -> Option<usize> {
    numbers
        .iter()
        .position(|(n, plated)| *plated && *n == "1")
        .or_else(|| numbers.iter().position(|(n, p)| *p && *n == "A1"))
        .or_else(|| numbers.iter().position(|(_, p)| *p))
}

/// Level A: pin 1 upper left of the centroid (x ≤ 0, y ≥ 0, not at it).
/// The generator's own turn is kept when it already is, else the first
/// of −90°, 180°, +90° that makes it so.
fn level_a_turn(pin1: Option<Point2>) -> u8 {
    let Some(p) = pin1 else { return 0 };
    for q in [0u8, 3, 2, 1] {
        let (x, y) = turn(q, p.x, p.y);
        if x <= EPS && y >= -EPS && (x < -EPS || y > EPS) {
            return q;
        }
    }
    0
}

fn make(
    g: &MechanicalGeometry,
    package: Option<&Package>,
    device: Option<&Device>,
    density: Density,
) -> PlacementModel {
    use openparts_mcad::Mounting;
    let lands = if g.lands.is_empty() {
        &g.leads
    } else {
        &g.lands
    };

    // The centroid: the centre of the plated lands' extents.
    let plated: Vec<_> = lands
        .iter()
        .filter(|l| l.mounting != Mounting::NonPlated)
        .collect();
    let c = if plated.is_empty() {
        Point2 {
            x: g.body.position.x,
            y: g.body.position.y,
        }
    } else {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for l in &plated {
            x0 = x0.min(l.position.x - l.size.x / 2.0);
            x1 = x1.max(l.position.x + l.size.x / 2.0);
            y0 = y0.min(l.position.y - l.size.y / 2.0);
            y1 = y1.max(l.position.y + l.size.y / 2.0);
        }
        Point2 {
            x: r4((x0 + x1) / 2.0),
            y: r4((y0 + y1) / 2.0),
        }
    };
    let numbers: Vec<(&str, bool)> = lands
        .iter()
        .map(|l| (l.number.as_str(), l.mounting != Mounting::NonPlated))
        .collect();
    let pin1 = pin1_index(&numbers);
    let q = level_a_turn(pin1.map(|i| Point2 {
        x: lands[i].position.x - c.x,
        y: lands[i].position.y - c.y,
    }));
    let t = ToModel { q, c };

    // Pads.
    let exposed_dims = package.and_then(|p| p.dimensions.exposed_pad.as_ref());
    let mut pads: Vec<Pad> = lands
        .iter()
        .map(|lead| {
            let square = (lead.size.x - lead.size.y).abs() < 1e-9;
            let (kind, shape) = match lead.mounting {
                Mounting::Smd => (PadKind::Smd, PadShape::Rect),
                // A square through-hole pad marks pin 1, as KiCad's own
                // footprints do (CP_Radial_D5.0mm_P2.00mm).
                Mounting::ThroughHole if lead.number == "1" && square => {
                    (PadKind::Tht, PadShape::Rect)
                }
                Mounting::ThroughHole if square => (PadKind::Tht, PadShape::Circle),
                Mounting::ThroughHole => (PadKind::Tht, PadShape::Oval),
                Mounting::NonPlated if square => (PadKind::Npth, PadShape::Circle),
                Mounting::NonPlated => (PadKind::Npth, PadShape::Oval),
            };
            let layers = match kind {
                PadKind::Smd => PadLayers {
                    copper: CopperLayers::Top,
                    mask: MaskLayers::Top,
                    paste: true,
                },
                PadKind::Tht => PadLayers {
                    copper: CopperLayers::All,
                    mask: MaskLayers::Both,
                    paste: false,
                },
                PadKind::Npth => PadLayers {
                    copper: CopperLayers::None,
                    mask: MaskLayers::Both,
                    paste: false,
                },
            };
            let at_centre = (lead.position.x - g.body.position.x).abs() < 1e-6
                && (lead.position.y - g.body.position.y).abs() < 1e-6;
            Pad {
                number: lead.number.clone(),
                shape,
                position: t.point(lead.position.x, lead.position.y),
                rotation: 0.0,
                size: t.size(lead.size.x, lead.size.y),
                corner_ratio: None,
                polygon: None,
                kind,
                layers,
                drill: lead.drill.map(r4),
                slot: lead.slot.map(r4),
                plated: kind != PadKind::Npth,
                mask_expansion: None,
                paste: (kind == PadKind::Smd).then_some(Paste::Full),
                pin: None,
                exposed: kind == PadKind::Smd
                    && (lead.number == "EP" || (exposed_dims.is_some() && at_centre)),
                castellated: false,
                shares_contact_with: None,
            }
        })
        .collect();

    // A module's lands that reach its own board edge are castellations.
    let body_rect = body_bounds(g, t);
    if g.family == F::Module {
        for pad in &mut pads {
            let (lo, hi) = pad.bounds();
            if pad.kind != PadKind::Npth
                && (lo.x <= body_rect.0.x + EPS
                    || hi.x >= body_rect.1.x - EPS
                    || lo.y <= body_rect.0.y + EPS
                    || hi.y >= body_rect.1.y - EPS)
            {
                pad.castellated = true;
            }
        }
    }

    // Device pins: names, and contacts shared under a second number.
    if let Some(device) = device {
        let mut extra = Vec::new();
        for (number, pin) in &device.pins {
            if pads.iter().any(|p| &p.number == number) {
                continue;
            }
            let Some(target) = &pin.connected_to else {
                continue;
            };
            extra.extend(pads.iter().filter(|p| &p.number == target).map(|p| Pad {
                number: number.clone(),
                shares_contact_with: Some(target.clone()),
                ..p.clone()
            }));
        }
        pads.extend(extra);
        for pad in &mut pads {
            if let Some(pin) = device.pins.get(&pad.number) {
                pad.pin = Some(PadPin {
                    name: pin.name.clone(),
                    pin_type: pin.pin_type,
                });
            }
        }
    }

    let regions = body_regions(g, t);
    let standoff = r4(match (g.lead_form, &g.details) {
        (Some(LeadForm::GullWing { standoff, .. }), _) => standoff,
        (_, Details::Header(f)) => f.standoff,
        _ => regions.first().map_or(0.0, |r| r.z_min.max(0.0)),
    });
    let height = r4(regions.iter().map(|r| r.z_max).fold(0.0, f64::max));

    let tht_pads = pads.iter().any(|p| p.kind == PadKind::Tht);
    let level = match density {
        Density::L => 0,
        Density::N => 1,
        Density::M => 2,
    };
    let excess = if tht_pads {
        COURTYARD_THT[level]
    } else {
        COURTYARD_SMD[level]
    };
    let courtyard = courtyard(&pads, &regions, excess, density);

    let pin1 = pin1.map(|i| {
        let marker = g
            .markers
            .iter()
            .find(|m| m.kind == MarkerKind::Pin1Dot)
            .map(|m| t.point(m.position.x, m.position.y));
        Pin1 {
            pad: lands[i].number.clone(),
            position: t.point(lands[i].position.x, lands[i].position.y),
            marker,
        }
    });

    let edge = package.and_then(|p| p.edge.as_ref()).map(|e| {
        let side = t.side(e.side);
        let n = e.side.normal();
        let pcb_edge = e
            .pcb_edge
            .map(|line| r4((n.0 + n.1) * line - (n.0 * c.x + n.1 * c.y)));
        let (nx, ny) = side.normal();
        let reach = regions
            .iter()
            .flat_map(|r| r.outline.iter())
            .map(|p| nx * p.x + ny * p.y)
            .fold(f64::MIN, f64::max);
        Edge {
            side,
            requirement: e.requirement,
            pcb_edge,
            overhang: pcb_edge.map(|d| r4(reach - d)),
        }
    });

    let keepouts = package
        .map(|p| {
            p.keepouts
                .iter()
                .map(|k| Keepout {
                    purpose: k.purpose,
                    restricts: k.restricts.clone(),
                    polygon: k.rect.map(|r| {
                        let a = t.point(r.x - r.width / 2.0, r.y - r.length / 2.0);
                        let b = t.point(r.x + r.width / 2.0, r.y + r.length / 2.0);
                        rect_outline(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
                    }),
                    size: k.size,
                    side: k.side.map(|s| t.side(s)),
                })
                .collect()
        })
        .unwrap_or_default();

    let stated_tail = package.and_then(|p| p.dimensions.tail_length.as_ref());
    let tht = tht_pads.then(|| Tht {
        tail_length: r4(match (&g.details, stated_tail) {
            (Details::Header(f), _) => f.tail_length,
            (Details::UsbC(f), _) => f.tail_length,
            (_, Some(d)) => stated(d).unwrap_or(0.0),
            _ => lands
                .iter()
                .filter(|l| l.mounting == Mounting::ThroughHole)
                .map(|l| l.size.z)
                .fold(0.0, f64::max),
        }),
    });

    let (ox, oy) = turn(q, -c.x, -c.y);
    let model3d = Model3d {
        rotation: f64::from(q) * 90.0,
        offset: [r4(ox), r4(oy), 0.0],
    };

    let basis = package
        .map(|p| basis(p, g, tht_pads))
        .unwrap_or_else(|| Basis {
            courtyard: BasisEntry {
                kind: if tht_pads {
                    BasisKind::GeneratorDefault
                } else {
                    BasisKind::Ipc7351
                },
                ..Default::default()
            },
            ..Default::default()
        });

    let pin1_number = pin1.as_ref().map(|p| p.pad.clone());
    let (fam_row, convention) = zero_orientation_row(g.family);
    let mut model = PlacementModel {
        frame: Frame {
            schema_version: SCHEMA_VERSION.to_string(),
            units: "mm".into(),
            origin: "land_pattern_centroid".into(),
            axes: "top_view_x_right_y_up_z_up".into(),
            rotation: "ccw_from_top".into(),
            bottom_side: "board = T·R(θ)·M".into(),
            zero_orientation: ZeroOrientation {
                rule: "ipc7351b_level_a".into(),
                family: fam_row.into(),
                convention: convention.into(),
                pin1: pin1_number,
            },
            density,
        },
        package: package.map(|p| p.id.0.clone()),
        pitch: g.pitch.map(r4),
        min_pad_gap: min_pad_gap(&pads),
        pads,
        courtyard,
        body: Body {
            regions,
            standoff,
            height,
        },
        pin1,
        edge,
        keepouts,
        tht,
        model3d,
        basis,
        hash: String::new(),
    };
    model.hash = content_hash(&model);
    model
}

/// A dimension's value as the generators read it.
fn stated(d: &openparts_core::Dimension) -> Option<f64> {
    d.nominal.or(match (d.min, d.max) {
        (Some(a), Some(b)) => Some((a + b) / 2.0),
        (a, b) => a.or(b),
    })
}

fn rect_outline(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point2> {
    vec![
        Point2 { x: x0, y: y0 },
        Point2 { x: x1, y: y0 },
        Point2 { x: x1, y: y1 },
        Point2 { x: x0, y: y1 },
    ]
}

/// The main body's extent in the model frame.
fn body_bounds(g: &MechanicalGeometry, t: ToModel) -> (Point2, Point2) {
    let b = &g.body;
    let a = t.point(b.position.x - b.size.x / 2.0, b.position.y - b.size.y / 2.0);
    let z = t.point(b.position.x + b.size.x / 2.0, b.position.y + b.size.y / 2.0);
    (
        Point2 {
            x: a.x.min(z.x),
            y: a.y.min(z.y),
        },
        Point2 {
            x: a.x.max(z.x),
            y: a.y.max(z.y),
        },
    )
}

fn region(
    name: &str,
    shape: OutlineShape,
    center: Point2,
    size: Size2,
    z: (f64, f64),
) -> BodyRegion {
    let outline = match shape {
        OutlineShape::Rect => rect_outline(
            r4(center.x - size.x / 2.0),
            r4(center.y - size.y / 2.0),
            r4(center.x + size.x / 2.0),
            r4(center.y + size.y / 2.0),
        ),
        OutlineShape::Circle | OutlineShape::Stadium => stadium(center, size),
    };
    BodyRegion {
        name: name.into(),
        shape,
        center,
        size,
        outline,
        z_min: r4(z.0),
        z_max: r4(z.1),
    }
}

/// A stadium (a circle when square) as a CCW polygon.
fn stadium(center: Point2, size: Size2) -> Vec<Point2> {
    let along_x = size.x >= size.y;
    let r = size.x.min(size.y) / 2.0;
    let half = (size.x.max(size.y) / 2.0 - r).max(0.0);
    let n = CIRCLE_SEGMENTS / 2;
    let mut out = Vec::new();
    for (end, start) in [(1.0, -0.5), (-1.0, 0.5)] {
        for i in 0..=n {
            let a = (start + i as f64 / n as f64) * std::f64::consts::PI;
            let (u, v) = (end * half + r * a.cos(), r * a.sin());
            let (x, y) = if along_x { (u, v) } else { (-v, u) };
            out.push(Point2 {
                x: r4(center.x + x),
                y: r4(center.y + y),
            });
        }
    }
    if half < EPS {
        // A circle: drop the duplicated quarter points.
        out.dedup_by(|a, b| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9);
        if out.len() > 1 && out.first() == out.last() {
            out.pop();
        }
    }
    out
}

fn body_regions(g: &MechanicalGeometry, t: ToModel) -> Vec<BodyRegion> {
    let b = &g.body;
    let center = t.point(b.position.x, b.position.y);
    let size = t.size(b.size.x, b.size.y);
    let shape = match b.shape {
        BodyShape::Box | BodyShape::CylinderX => OutlineShape::Rect,
        BodyShape::Cylinder if (b.size.x - b.size.y).abs() < 1e-9 => OutlineShape::Circle,
        BodyShape::Cylinder => OutlineShape::Stadium,
    };
    let z = (b.position.z - b.size.z / 2.0, b.position.z + b.size.z / 2.0);
    let mut out = vec![region("body", shape, center, size, z)];
    if let Details::Header(f) = &g.details {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for l in &g.leads {
            x0 = x0.min(l.position.x);
            x1 = x1.max(l.position.x);
            y0 = y0.min(l.position.y);
            y1 = y1.max(l.position.y);
        }
        let w = f.pin_width / 2.0;
        let housing_top = f.standoff + f.body_height;
        let pins = match f.kind {
            HeaderKind::Pin if f.mating_length > 0.0 && !g.leads.is_empty() => Some((
                (x0 - w, y0 - w, x1 + w, y1 + w),
                (housing_top, housing_top + f.mating_length),
            )),
            HeaderKind::RightAngle if f.mating_length > 0.0 && !g.leads.is_empty() => {
                let front = b.position.x + b.size.x / 2.0;
                Some((
                    (front, y0 - w, front + f.mating_length, y1 + w),
                    (f.bend_height - w, f.bend_height + w),
                ))
            }
            _ => None,
        };
        if let Some(((x0, y0, x1, y1), z)) = pins {
            let a = t.point(x0, y0);
            let c = t.point(x1, y1);
            let center = Point2 {
                x: r4((a.x + c.x) / 2.0),
                y: r4((a.y + c.y) / 2.0),
            };
            let size = Size2 {
                x: r4((a.x - c.x).abs()),
                y: r4((a.y - c.y).abs()),
            };
            out.push(region("pins", OutlineShape::Rect, center, size, z));
        }
    }
    out
}

/// The lands and body grown by `excess`, corners snapped outward to the
/// courtyard grid.
fn courtyard(pads: &[Pad], regions: &[BodyRegion], excess: f64, density: Density) -> Courtyard {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut add = |lo: Point2, hi: Point2| {
        x0 = x0.min(lo.x);
        y0 = y0.min(lo.y);
        x1 = x1.max(hi.x);
        y1 = y1.max(hi.y);
    };
    for p in pads {
        let (lo, hi) = p.bounds();
        add(lo, hi);
    }
    if let Some(body) = regions.first() {
        for p in &body.outline {
            add(*p, *p);
        }
    }
    // Snap with a tolerance, so a value already on the grid stays.
    let down = |v: f64| r4(((v - excess) / COURTYARD_GRID + 1e-6).floor() * COURTYARD_GRID);
    let up = |v: f64| r4(((v + excess) / COURTYARD_GRID - 1e-6).ceil() * COURTYARD_GRID);
    Courtyard {
        density,
        excess,
        grid: COURTYARD_GRID,
        polygons: vec![rect_outline(down(x0), down(y0), up(x1), up(y1))],
    }
}

/// The smallest gap between the copper of two pads (axis-aligned
/// extents), pads of one number (a shared contact) excepted.
fn min_pad_gap(pads: &[Pad]) -> Option<f64> {
    let copper: Vec<_> = pads
        .iter()
        .filter(|p| p.has_copper() && p.shares_contact_with.is_none())
        .map(|p| (p.number.as_str(), p.bounds()))
        .collect();
    let mut best: Option<f64> = None;
    for (i, (na, (alo, ahi))) in copper.iter().enumerate() {
        for (nb, (blo, bhi)) in &copper[i + 1..] {
            if na == nb {
                continue;
            }
            let dx = (blo.x - ahi.x).max(alo.x - bhi.x).max(0.0);
            let dy = (blo.y - ahi.y).max(alo.y - bhi.y).max(0.0);
            let gap = dx.hypot(dy);
            best = Some(best.map_or(gap, |b: f64| b.min(gap)));
        }
    }
    best.map(r4)
}

/// The JSON Pointer paths a field group reads, with the provenance each
/// cites.
fn sources(package: &Package, pointers: &[&str]) -> Vec<SourceRef> {
    let mut out = Vec::new();
    for pointer in pointers {
        let entries = package.provenance.iter().filter(|(key, _)| {
            pointer
                .strip_prefix(key.as_str())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        });
        for (_, list) in entries {
            for e in list {
                out.push(SourceRef {
                    pointer: (*pointer).to_string(),
                    source: e.source.0.clone(),
                    page: e.page,
                });
            }
        }
    }
    out
}

fn basis(package: &Package, g: &MechanicalGeometry, tht: bool) -> Basis {
    let present: Vec<&str> = package
        .dimensions
        .entries()
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    let has = |p: &str| present.contains(&p);
    let pad_inputs: &[&str] = &[
        "/dimensions/lead_width",
        "/dimensions/lead_length",
        "/dimensions/overall_width",
        "/dimensions/overall_length",
        "/dimensions/body_width",
        "/dimensions/body_length",
        "/dimensions/exposed_pad/width",
        "/dimensions/exposed_pad/length",
        "/dimensions/hole_diameter",
        "/dimensions/pad_diameter",
        "/dimensions/row_spacing",
        "/dimensions/tab_width",
        "/dimensions/tab_length",
        "/dimensions/lead_inset",
        "/dimensions/aux_pad_width",
        "/dimensions/aux_pad_length",
        "/dimensions/aux_row_x",
        "/dimensions/aux_row_y",
    ];
    // What the family's terminals need, to say which were defaulted.
    let needed: &[&str] = match (g.lead_form, tht) {
        (_, true) => &["/dimensions/hole_diameter"],
        (Some(_), _) => &["/dimensions/lead_width", "/dimensions/lead_length"],
        (None, _) if g.family == F::Chip => &["/dimensions/lead_length"],
        _ => &[],
    };
    let pads = if !package.pads.is_empty() {
        BasisEntry {
            kind: BasisKind::Datasheet,
            sources: sources(package, &["/pads"]),
            defaults: vec![],
        }
    } else {
        let mut pointers: Vec<&str> = vec!["/pitch"];
        pointers.extend(pad_inputs.iter().copied().filter(|p| has(p)));
        BasisEntry {
            kind: if !g.lands.is_empty() {
                BasisKind::Ipc7351
            } else if needed.iter().all(|p| has(p)) {
                BasisKind::Datasheet
            } else {
                BasisKind::GeneratorDefault
            },
            sources: sources(package, &pointers),
            defaults: needed
                .iter()
                .filter(|p| !has(p))
                .map(|p| p.to_string())
                .collect(),
        }
    };
    let body_ptrs = [
        "/dimensions/body_width",
        "/dimensions/body_length",
        "/dimensions/body_height",
        "/dimensions/overall_height",
        "/dimensions/standoff",
        "/dimensions/body_offset",
    ];
    let mut body_defaults = vec![];
    if !has("/dimensions/body_height") && !has("/dimensions/overall_height") {
        body_defaults.push("/dimensions/body_height".to_string());
    }
    let body = BasisEntry {
        kind: if body_defaults.is_empty() {
            BasisKind::Datasheet
        } else {
            BasisKind::GeneratorDefault
        },
        sources: sources(
            package,
            &body_ptrs
                .iter()
                .copied()
                .filter(|p| has(p))
                .collect::<Vec<_>>(),
        ),
        defaults: body_defaults,
    };
    Basis {
        pads,
        courtyard: BasisEntry {
            kind: if tht {
                BasisKind::GeneratorDefault
            } else {
                BasisKind::Ipc7351
            },
            ..Default::default()
        },
        body,
        edge: package.edge.as_ref().map(|_| BasisEntry {
            kind: BasisKind::Datasheet,
            sources: sources(package, &["/edge"]),
            defaults: vec![],
        }),
        keepouts: (!package.keepouts.is_empty()).then(|| BasisEntry {
            kind: BasisKind::Datasheet,
            sources: sources(package, &["/keepouts"]),
            defaults: vec![],
        }),
        tht: tht.then(|| {
            if has("/dimensions/tail_length") {
                BasisEntry {
                    kind: BasisKind::Datasheet,
                    sources: sources(package, &["/dimensions/tail_length"]),
                    defaults: vec![],
                }
            } else {
                BasisEntry {
                    kind: BasisKind::GeneratorDefault,
                    sources: vec![],
                    defaults: vec!["/dimensions/tail_length".into()],
                }
            }
        }),
    }
}

/// The family's row of the zero-orientation table (spec section 3):
/// (families, convention).
pub(crate) fn zero_orientation_row(f: F) -> (&'static str, &'static str) {
    match f {
        F::Chip | F::Sod | F::SodFlat | F::Do214 | F::Axial | F::Radial => (
            "two-terminal",
            "body along X, pin 1 (cathode, +, pad 1) on the left",
        ),
        F::To92 | F::To220 | F::Sip => ("single row", "leads in a row along X, pin 1 on the left"),
        F::Sot | F::Sot223 | F::Dpak | F::D2pak => (
            "SOT / power tab",
            "leads down the left side, pin 1 at the top; the opposite side's lead or tab on the right",
        ),
        F::Soic | F::Tssop | F::Ssop | F::Msop | F::Son | F::Dfn | F::Dip => (
            "dual row",
            "rows along Y, pin 1 at the top of the left row, numbering counter-clockwise",
        ),
        F::Lqfp | F::Qfn | F::Lga => (
            "quad / LGA",
            "pin 1 at the top of the left side (or the upper-left corner), numbering counter-clockwise",
        ),
        F::Bga => ("BGA", "ball A1 upper left, rows A.. downward, columns 1.. to the right"),
        F::Module => ("module", "pin 1 upper left"),
        F::Header | F::Socket | F::ShroudedHeader => (
            "pin header / socket",
            "rows along Y, pin 1 upper left (1xN: pin 1 at the top)",
        ),
        F::RightAngleHeader => (
            "right-angle header",
            "pins in a column along Y, pin 1 at the top, mating side +X",
        ),
        F::TactSwitchSmd | F::TactSwitchTht => ("tactile switch", "pad 1 upper left"),
        F::UsbC => ("USB-C receptacle", "contact A1 upper left: the opening faces −Y"),
    }
}

#[cfg(test)]
mod tests;
