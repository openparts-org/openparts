//! openparts-pcbcad: the CAD-independent PCB Symbol/Footprint
//! Intermediate Representation (Architecture Specification section 12).
//! EDA-specific concepts stay in the adapter crates (openparts-kicad,
//! openparts-librepcb, openparts-altium); this crate only knows about
//! Device pins and Mechanical Geometry leads.

use openparts_core::{Device, PinType};
use openparts_mcad::MechanicalGeometry;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinElectricalType {
    Input,
    Output,
    Bidirectional,
    PowerIn,
    Passive,
    NoConnect,
    Unspecified,
}

impl From<PinType> for PinElectricalType {
    fn from(t: PinType) -> Self {
        match t {
            PinType::Power | PinType::Ground => PinElectricalType::PowerIn,
            PinType::Input | PinType::Clock | PinType::Reset => PinElectricalType::Input,
            PinType::Output => PinElectricalType::Output,
            PinType::Io => PinElectricalType::Bidirectional,
            PinType::Analog | PinType::Reserved | PinType::Passive => PinElectricalType::Passive,
            PinType::Nc => PinElectricalType::NoConnect,
            PinType::Unknown => PinElectricalType::Unspecified,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinOrientation {
    /// Pin stub points toward +X, pin sits on the symbol's left edge.
    Left,
    /// Pin stub points toward -X, pin sits on the symbol's right edge.
    Right,
    /// Pin stub points toward +Y, pin sits on the symbol's bottom edge.
    Bottom,
    /// Pin stub points toward -Y, pin sits on the symbol's top edge.
    Top,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolPin {
    pub number: String,
    pub name: String,
    pub electrical_type: PinElectricalType,
    pub position: Point2,
    pub length: f64,
    pub orientation: PinOrientation,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphicKind {
    Rectangle { start: Point2, end: Point2 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Graphic {
    pub kind: GraphicKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolUnit {
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcbSymbol {
    pub name: String,
    pub units: Vec<SymbolUnit>,
    pub pins: Vec<SymbolPin>,
    pub graphics: Vec<Graphic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadShape {
    Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pad {
    pub number: String,
    pub position: Point2,
    pub size: (f64, f64),
    pub shape: PadShape,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Courtyard {
    pub outline: Vec<Point2>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcbFootprint {
    pub name: String,
    pub pads: Vec<Pad>,
    pub graphics: Vec<Graphic>,
    pub courtyard: Option<Courtyard>,
}

const GRID_MM: f64 = 2.54;
const PIN_LENGTH_MM: f64 = 2.54;

/// Builds a Symbol from a Device's pins: one rectangular body, pins
/// distributed left/bottom/right/top in ascending pin-number order on a
/// 2.54mm grid (same left->bottom->right->top ordering as the LQFP
/// Mechanical Geometry generator, for visual consistency between the
/// symbol and the footprint).
pub fn build_symbol(name: &str, device: &Device) -> PcbSymbol {
    let mut numbers: Vec<&String> = device.pins.keys().collect();
    numbers.sort_by_key(|n| n.parse::<u32>().unwrap_or(u32::MAX));

    let n = numbers.len().max(1);
    let per_side = n.div_ceil(4);
    let half_extent = ((per_side as f64 - 1.0) * GRID_MM / 2.0).max(GRID_MM);

    let mut pins = Vec::with_capacity(numbers.len());
    for (i, number) in numbers.iter().enumerate() {
        let side = i / per_side;
        let index_on_side = (i % per_side) as f64;
        let pin = &device.pins[*number];

        let (position, orientation) = match side {
            0 => (
                Point2 { x: -half_extent - PIN_LENGTH_MM, y: half_extent - index_on_side * GRID_MM },
                PinOrientation::Left,
            ),
            1 => (
                Point2 { x: -half_extent + index_on_side * GRID_MM, y: -half_extent - PIN_LENGTH_MM },
                PinOrientation::Bottom,
            ),
            2 => (
                Point2 { x: half_extent + PIN_LENGTH_MM, y: -half_extent + index_on_side * GRID_MM },
                PinOrientation::Right,
            ),
            _ => (
                Point2 { x: half_extent - index_on_side * GRID_MM, y: half_extent + PIN_LENGTH_MM },
                PinOrientation::Top,
            ),
        };

        pins.push(SymbolPin {
            number: (*number).clone(),
            name: pin.name.clone(),
            electrical_type: pin.pin_type.into(),
            position,
            length: PIN_LENGTH_MM,
            orientation,
        });
    }

    PcbSymbol {
        name: name.to_string(),
        units: vec![SymbolUnit { index: 1 }],
        pins,
        graphics: vec![Graphic {
            kind: GraphicKind::Rectangle {
                start: Point2 { x: -half_extent, y: half_extent },
                end: Point2 { x: half_extent, y: -half_extent },
            },
        }],
    }
}

/// Builds a Footprint by projecting Mechanical Geometry leads onto the
/// XY plane. Lead size/position from the generator already encode the
/// correct per-side orientation, so no rotation is needed here.
pub fn build_footprint(name: &str, geometry: &MechanicalGeometry) -> PcbFootprint {
    let pads = geometry
        .leads
        .iter()
        .map(|lead| Pad {
            number: lead.number.clone(),
            position: Point2 { x: lead.position.x, y: lead.position.y },
            size: (lead.size.x, lead.size.y),
            shape: PadShape::Rect,
        })
        .collect();

    PcbFootprint {
        name: name.to_string(),
        pads,
        graphics: vec![],
        courtyard: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Device, DeviceId, Kind, ManufacturerId, Pin};
    use std::collections::BTreeMap;

    fn device_8_pins() -> Device {
        let mut pins = BTreeMap::new();
        for i in 1..=8u32 {
            pins.insert(
                i.to_string(),
                Pin {
                    name: format!("P{i}"),
                    pin_type: PinType::Io,
                    alternate_functions: vec![],
                },
            );
        }
        Device {
            schema_version: "0.1".into(),
            kind: Kind::Device,
            id: DeviceId::from("ex/DEV8"),
            manufacturer: ManufacturerId::from("ex"),
            family: None,
            pins,
            revisions: BTreeMap::new(),
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn symbol_has_one_pin_per_device_pin() {
        let symbol = build_symbol("EX8", &device_8_pins());
        assert_eq!(symbol.pins.len(), 8);
        let numbers: std::collections::BTreeSet<&str> =
            symbol.pins.iter().map(|p| p.number.as_str()).collect();
        for i in 1..=8 {
            assert!(numbers.contains(i.to_string().as_str()));
        }
    }

    #[test]
    fn footprint_pad_count_matches_lead_count() {
        use openparts_mcad::{Body, Lead, MechanicalGeometry, Point3, Size3};
        let geometry = MechanicalGeometry {
            body: Body {
                position: Point3 { x: 0.0, y: 0.0, z: 0.0 },
                size: Size3 { x: 1.0, y: 1.0, z: 1.0 },
            },
            leads: vec![Lead {
                number: "1".into(),
                position: Point3 { x: 1.0, y: 2.0, z: 0.0 },
                size: Size3 { x: 0.3, y: 0.2, z: 0.1 },
            }],
            markers: vec![],
        };
        let footprint = build_footprint("EX1", &geometry);
        assert_eq!(footprint.pads.len(), 1);
        assert_eq!(footprint.pads[0].number, "1");
        assert_eq!(footprint.pads[0].position, Point2 { x: 1.0, y: 2.0 });
        assert_eq!(footprint.pads[0].size, (0.3, 0.2));
    }
}
