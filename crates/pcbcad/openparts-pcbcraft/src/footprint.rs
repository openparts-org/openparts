//! IR footprint -> `pcbcraft-footprint/1` (or `/4` with non-plated
//! holes).
//!
//! Lengths become integer nanometres and Y flips (PCBCraft's footprint
//! frame points Y down, the IR's up). Pads keep their shape where
//! PCBCraft has it (rectangle, circle, oval); everything PCBCraft
//! cannot represent refuses the footprint -- and so every part using it
//! -- with the reason, instead of approximating copper: chamfered,
//! polygonal and annular pads, slotted holes, bottom-side surface pads,
//! a drill not smaller than its land, more than 1024 pads.
//!
//! PCBCraft pad numbers are distinct. A number the IR uses for several
//! lands (a switch's paired legs) keeps it on its first land; the
//! others become `<n>_2`, `<n>_3`..., and the package map binds them all
//! to the same terminal.

use crate::record::{
    Artwork, Footprint, Geometry, Graphic, Hole, Pad, Point, Source, FOOTPRINT_FORMAT_V1,
    FOOTPRINT_FORMAT_V4,
};
use crate::{check_terminal_id, nm, Refuse};
use openparts_mcad::Mounting;
use openparts_pcbcad::{
    FpLayer, FpShape, PadShape, PadSide, PcbFootprint, Point2, COURTYARD_WIDTH,
};
use std::collections::BTreeMap;

const MAX_COORD_NM: i64 = 100_000_000;
const MAX_PADS: usize = 1024;
const MAX_HOLES: usize = 128;
const MAX_ARTWORK: usize = 256;
const MAX_POINTS: usize = 64;

#[derive(Debug, Clone)]
pub(crate) struct FootprintDraft {
    /// Library, id, revision, license and source still unset.
    pub record: Footprint,
    /// IR pad number -> its PCBCraft pad numbers, in land order.
    pub pads: BTreeMap<String, Vec<String>>,
}

fn coord(v: f64) -> Result<i64, Refuse> {
    let v = nm(v)?;
    if v.abs() > MAX_COORD_NM {
        return Err(format!(
            "a coordinate {v} nm lies beyond PCBCraft's ±100 mm"
        ));
    }
    Ok(v)
}

fn point(p: Point2) -> Result<Point, Refuse> {
    Ok(Point {
        x_nm: coord(p.x)?,
        y_nm: -coord(p.y)?,
    })
}

fn size(what: &str, v: f64) -> Result<i64, Refuse> {
    let v = nm(v)?;
    if !(1..=MAX_COORD_NM).contains(&v) {
        return Err(format!("{what} {v} nm is outside PCBCraft's 1 nm..100 mm"));
    }
    Ok(v)
}

pub(crate) fn convert(fp: &PcbFootprint, name: &str) -> Result<FootprintDraft, Refuse> {
    let mut pads = Vec::new();
    let mut holes = Vec::new();
    let mut numbers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for pad in &fp.pads {
        let label = if pad.number.is_empty() {
            "an unnumbered pad".to_string()
        } else {
            format!("pad {}", pad.number)
        };
        if pad.slot.is_some() {
            return Err(format!(
                "{label} has a slotted hole (PCBCraft has round holes only)"
            ));
        }
        let center = point(pad.position)?;
        let (w, h) = (
            size("pad width", pad.size.0)?,
            size("pad height", pad.size.1)?,
        );
        if pad.mounting == Mounting::NonPlated {
            let diameter = match pad.drill {
                Some(d) => size("hole diameter", d)?,
                None if w == h => w,
                None => return Err(format!("{label}: a non-round non-plated hole")),
            };
            holes.push(Hole {
                center,
                diameter_nm: diameter,
            });
            continue;
        }
        let shape = match &pad.shape {
            PadShape::Rect => "rectangle",
            PadShape::Circle if w == h => "circle",
            PadShape::Circle => {
                return Err(format!(
                    "{label}: a circular pad of unequal width and height"
                ))
            }
            PadShape::Oval => "oval",
            PadShape::Chamfered { .. } => {
                return Err(format!("{label} is chamfered (no PCBCraft pad shape)"))
            }
            PadShape::Polygon(_) => {
                return Err(format!("{label} is a polygon (no PCBCraft pad shape)"))
            }
            PadShape::Annulus { .. } => {
                return Err(format!("{label} is an annulus (no PCBCraft pad shape)"))
            }
        };
        let (copper, drill_nm) = match pad.mounting {
            Mounting::Smd if pad.side == PadSide::Bottom => {
                return Err(format!("{label} is a bottom-side surface pad (PCBCraft places surface copper on the component side only)"))
            }
            Mounting::Smd => ("component_side", None),
            _ => {
                let drill = size(
                    "drill",
                    pad.drill
                        .ok_or_else(|| format!("{label}: a through-hole pad without a drill"))?,
                )?;
                if drill >= w || drill >= h {
                    return Err(format!(
                        "{label}: the drill ({drill} nm) is not smaller than its land ({w} x {h} nm)"
                    ));
                }
                ("all_copper", Some(drill))
            }
        };
        check_terminal_id(&pad.number).map_err(|e| format!("pad number {e}"))?;
        let names = numbers.entry(pad.number.clone()).or_default();
        let number = if names.is_empty() {
            pad.number.clone()
        } else {
            format!("{}_{}", pad.number, names.len() + 1)
        };
        names.push(number.clone());
        pads.push(Pad {
            number,
            center,
            width_nm: w,
            height_nm: h,
            rotation: 0,
            shape,
            copper,
            drill_nm,
        });
    }
    // Renamed duplicates must still be valid and distinct.
    let mut seen = std::collections::BTreeSet::new();
    for p in &pads {
        check_terminal_id(&p.number)
            .map_err(|e| format!("pad {} needs a distinct number: {e}", p.number))?;
        if !seen.insert(p.number.clone()) {
            return Err(format!("pad number {} would be used twice", p.number));
        }
    }
    if pads.len() > MAX_PADS {
        return Err(format!(
            "{} pads: PCBCraft footprints hold at most {MAX_PADS}",
            pads.len()
        ));
    }
    if holes.len() > MAX_HOLES {
        return Err(format!(
            "{} non-plated holes: PCBCraft allows at most {MAX_HOLES}",
            holes.len()
        ));
    }
    if pads.is_empty() && holes.is_empty() {
        return Err("the footprint has no pads".into());
    }

    let mut artwork = Vec::new();
    for g in &fp.graphics {
        let purpose = match g.layer {
            FpLayer::Silk => "silkscreen",
            FpLayer::Fab => "fabrication",
        };
        let stroke = stroke(g.width)?;
        match &g.shape {
            FpShape::Line(a, b) => {
                let (from, to) = (point(*a)?, point(*b)?);
                if from != to {
                    artwork.push(art(purpose, stroke, Geometry::Line { from, to }));
                }
            }
            FpShape::Circle { center, radius } => artwork.push(art(
                purpose,
                stroke,
                Geometry::Circle {
                    center: point(*center)?,
                    radius_nm: size("circle radius", *radius)?,
                },
            )),
            FpShape::Polygon(points) => polygon(&mut artwork, purpose, stroke, points)?,
        }
    }
    if let Some(courtyard) = &fp.courtyard {
        polygon(
            &mut artwork,
            "courtyard",
            stroke(COURTYARD_WIDTH)?,
            &courtyard.outline,
        )?;
    }
    if artwork.len() > MAX_ARTWORK {
        return Err(format!(
            "{} drawing objects: PCBCraft footprints hold at most {MAX_ARTWORK}",
            artwork.len()
        ));
    }
    Ok(FootprintDraft {
        record: Footprint {
            format: if holes.is_empty() {
                FOOTPRINT_FORMAT_V1
            } else {
                FOOTPRINT_FORMAT_V4
            },
            library: String::new(),
            id: String::new(),
            revision: 0,
            name: crate::clean_text("footprint name", name, 200)?,
            license: String::new(),
            source: Source {
                kind: String::new(),
                reference: String::new(),
                locator: String::new(),
            },
            pads,
            non_plated_holes: holes,
            graphics: vec![],
            artwork,
            reference: "pcbcraft-technical-1",
        },
        pads: numbers,
    })
}

fn stroke(width: f64) -> Result<i64, Refuse> {
    let v = nm(width)?;
    if !(1..=2_000_000).contains(&v) {
        return Err(format!(
            "a stroke of {v} nm is outside PCBCraft's 1 nm..2 mm"
        ));
    }
    Ok(v)
}

fn art(purpose: &'static str, stroke_nm: i64, geometry: Geometry) -> Artwork {
    Artwork {
        purpose,
        graphic: Graphic {
            geometry,
            stroke_nm,
            fill: "none",
        },
    }
}

/// A closed outline: one closed polyline, or -- beyond PCBCraft's 64
/// points -- consecutive open polylines tracing the same path.
fn polygon(
    out: &mut Vec<Artwork>,
    purpose: &'static str,
    stroke: i64,
    points: &[Point2],
) -> Result<(), Refuse> {
    let mut pts: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        let p = point(*p)?;
        if pts.last() != Some(&p) {
            pts.push(p);
        }
    }
    while pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    match pts.len() {
        0 | 1 => Ok(()),
        2 => {
            out.push(art(
                purpose,
                stroke,
                Geometry::Line {
                    from: pts[0],
                    to: pts[1],
                },
            ));
            Ok(())
        }
        n if n <= MAX_POINTS => {
            out.push(art(
                purpose,
                stroke,
                Geometry::Polyline {
                    points: pts,
                    closed: true,
                },
            ));
            Ok(())
        }
        _ => {
            pts.push(pts[0]);
            let mut start = 0;
            while start + 1 < pts.len() {
                let end = (start + MAX_POINTS).min(pts.len());
                out.push(art(
                    purpose,
                    stroke,
                    Geometry::Polyline {
                        points: pts[start..end].to_vec(),
                        closed: false,
                    },
                ));
                start = end - 1;
            }
            Ok(())
        }
    }
}
