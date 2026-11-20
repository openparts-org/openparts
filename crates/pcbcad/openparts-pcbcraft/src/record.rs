//! The PCBCraft native record shapes this adapter writes, field for
//! field as PCBCraft's published JSON Schemas (`docs/schemas/*` in
//! tangency-labs/pcbcraft) define them. Structs, not `serde_json::Value`:
//! their field order is fixed here, so the bytes (and the ledger's
//! digests) never depend on a serde_json feature another crate enables.
//! Maps are `BTreeMap`s for the same reason.

use serde::Serialize;
use std::collections::BTreeMap;

pub const LIBRARY_FORMAT: &str = "pcbcraft-library/1";
pub const BUNDLE_FORMAT: &str = "pcbcraft-library-bundle/1";
pub const SYMBOL_FORMAT_V1: &str = "pcbcraft-symbol/1";
pub const SYMBOL_FORMAT_V4: &str = "pcbcraft-symbol/4";
pub const FOOTPRINT_FORMAT_V1: &str = "pcbcraft-footprint/1";
pub const FOOTPRINT_FORMAT_V4: &str = "pcbcraft-footprint/4";
pub const PART_FORMAT: &str = "pcbcraft-part/2";
pub const MANUFACTURER_PART_FORMAT: &str = "pcbcraft-manufacturer-part/1";
pub const CATEGORY_FORMAT: &str = "pcbcraft-category/1";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Manifest {
    pub format: &'static str,
    pub id: String,
    pub name: String,
    pub license: String,
}

/// `library-common-v1#/$defs/source`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Source {
    pub kind: String,
    pub reference: String,
    pub locator: String,
}

/// `library-common-v1#/$defs/reference`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reference {
    pub library: String,
    pub id: String,
    pub revision: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CategoryBinding {
    pub definition: Reference,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_value: Option<String>,
}

// ---- category ---------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Category {
    pub format: &'static str,
    pub library: String,
    pub id: String,
    pub revision: u32,
    pub name: String,
    pub license: String,
    pub fields: BTreeMap<String, Field>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Field {
    pub name: String,
    pub value_type: FieldType,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldType {
    Quantity {
        quantity: &'static str,
        unit: &'static str,
    },
    Choice {
        choices: Vec<String>,
    },
    Text,
}

/// `library-common-v1#/$defs/parameter_value` (the kinds written here).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParameterValue {
    Quantity { value: String, unit: &'static str },
    Choice { value: String },
    Text { value: String },
}

// ---- symbol -----------------------------------------------------------

/// A standalone symbol: its library and license come from the manifest.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Symbol {
    pub format: &'static str,
    pub id: String,
    pub revision: u32,
    pub name: String,
    pub reference_prefix: String,
    pub default_value: String,
    pub terminals: Vec<Terminal>,
    pub units: Vec<Unit>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Terminal {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub electrical_type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Unit {
    pub id: String,
    pub drawing: Vec<Shape>,
    pub pins: Vec<Pin>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Shape {
    Rect {
        min_nm: [i64; 2],
        max_nm: [i64; 2],
    },
    /// A drawing item with PCBCraft's shared graphic geometry: here a
    /// rotated pin name (the plain `text` shape has no rotation).
    Graphic {
        graphic: Graphic,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pin {
    pub terminal: String,
    pub at_nm: [i64; 2],
    pub appearance: PinAppearance,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PinAppearance {
    pub orientation: &'static str,
    pub length_nm: i64,
    pub style: &'static str,
    pub show_name: bool,
    pub show_number: bool,
    pub name_height_nm: i64,
    pub number_height_nm: i64,
}

// ---- footprint --------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Footprint {
    pub format: &'static str,
    pub library: String,
    pub id: String,
    pub revision: u32,
    pub name: String,
    pub license: String,
    pub source: Source,
    pub pads: Vec<Pad>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub non_plated_holes: Vec<Hole>,
    /// Legacy rectangle graphics: required, always empty here (the
    /// drawings are `artwork`).
    pub graphics: Vec<()>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub artwork: Vec<Artwork>,
    pub reference: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Point {
    pub x_nm: i64,
    pub y_nm: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pad {
    pub number: String,
    pub center: Point,
    pub width_nm: i64,
    pub height_nm: i64,
    pub rotation: u16,
    pub shape: &'static str,
    pub copper: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drill_nm: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hole {
    pub center: Point,
    pub diameter_nm: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Artwork {
    pub purpose: &'static str,
    pub graphic: Graphic,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Graphic {
    pub geometry: Geometry,
    pub stroke_nm: i64,
    pub fill: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Geometry {
    Line {
        from: Point,
        to: Point,
    },
    Circle {
        center: Point,
        radius_nm: i64,
    },
    Polyline {
        points: Vec<Point>,
        closed: bool,
    },
    /// Symbol drawings only: 1-128 characters, `rotation` in degrees
    /// from +X toward +Y (Y points down, so 90 reads top to bottom and
    /// 270 bottom to top), `align` along the text (`start`, `middle`,
    /// `end`); the anchor is the text's vertical centre.
    Text {
        at: Point,
        text: String,
        height_nm: i64,
        rotation: u16,
        align: &'static str,
    },
}

// ---- parts ------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Part {
    pub format: &'static str,
    pub library: String,
    pub id: String,
    pub revision: u32,
    pub name: String,
    pub license: String,
    pub source: Source,
    pub symbol: Reference,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<CategoryBinding>,
    pub packages: BTreeMap<String, PackageVariant>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PackageVariant {
    pub name: String,
    pub footprint: Reference,
    pub mapping: BTreeMap<String, Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unconnected_pads: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ManufacturerPart {
    pub format: &'static str,
    pub library: String,
    pub id: String,
    pub revision: u32,
    pub name: String,
    pub license: String,
    pub source: Source,
    pub manufacturer: String,
    pub mpn: String,
    pub part: Reference,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datasheet: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub specifications: BTreeMap<String, ParameterValue>,
}

/// A record with an immutable revision: the ledger digests it with the
/// revision (and its source reference, which names the export run)
/// blanked, so the same content keeps its revision across runs.
pub trait Revisioned: Serialize + Clone {
    fn set_revision(&mut self, revision: u32);
    /// The record's `source.reference`, when it has a source.
    fn reference_mut(&mut self) -> Option<&mut String>;
}

macro_rules! revisioned {
    ($t:ty, source) => {
        impl Revisioned for $t {
            fn set_revision(&mut self, revision: u32) {
                self.revision = revision;
            }
            fn reference_mut(&mut self) -> Option<&mut String> {
                Some(&mut self.source.reference)
            }
        }
    };
    ($t:ty) => {
        impl Revisioned for $t {
            fn set_revision(&mut self, revision: u32) {
                self.revision = revision;
            }
            fn reference_mut(&mut self) -> Option<&mut String> {
                None
            }
        }
    };
}

revisioned!(Category);
revisioned!(Symbol);
revisioned!(Footprint, source);
revisioned!(Part, source);
revisioned!(ManufacturerPart, source);

/// The document text: two-space indented JSON and a final newline, as
/// PCBCraft's own library files.
pub fn render<T: Serialize>(value: &T) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("records serialise");
    text.push('\n');
    text
}
