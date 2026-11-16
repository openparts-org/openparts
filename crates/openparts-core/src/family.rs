//! The closed set of package families (Canonical Data Specification
//! section 20): every `family:` a Package may name. Each family has one
//! canonical name and may accept aliases on input. The geometry and 3D
//! generators match on [`PackageFamily`] exhaustively, so a family added
//! here must be given a generator before the workspace compiles.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Declares [`PackageFamily`]: `Variant: "name" | "alias"...,` -- one line
/// per family, generating the enum, its names and aliases, parsing and
/// serde, so a family is defined in exactly one place.
macro_rules! families {
    ($( $(#[$doc:meta])* $variant:ident : $name:literal $(| $alias:literal)* ),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum PackageFamily {
            $( $(#[$doc])* $variant, )*
        }

        impl PackageFamily {
            pub const ALL: &'static [PackageFamily] = &[$(PackageFamily::$variant),*];

            /// The canonical name, as written in Canonical Data.
            pub fn name(self) -> &'static str {
                match self { $(PackageFamily::$variant => $name),* }
            }

            /// Other names accepted on input for the same family.
            pub fn aliases(self) -> &'static [&'static str] {
                match self { $(PackageFamily::$variant => &[$($alias),*]),* }
            }
        }
    };
}

families! {
    /// LQFP/TQFP (JEDEC MS-026).
    Lqfp: "lqfp" | "tqfp",
    Qfn: "qfn",
    /// QFN-style terminals on two sides only.
    Dfn: "dfn",
    Son: "son",
    /// Sensor LGA, per-side terminal counts.
    Lga: "lga",
    Soic: "soic",
    Tssop: "tssop",
    Ssop: "ssop",
    Msop: "msop",
    Sot: "sot",
    Sot223: "sot223",
    Dpak: "dpak" | "to252",
    D2pak: "d2pak" | "to263",
    Sod: "sod",
    SodFlat: "sod_flat",
    Do214: "do214",
    To220: "to220",
    Chip: "chip",
    Radial: "radial",
    Axial: "axial",
    To92: "to92",
    Dip: "dip",
    Sip: "sip",
    Bga: "bga",
    Module: "module",
    Header: "header",
    Socket: "socket",
    ShroudedHeader: "shrouded_header",
    RightAngleHeader: "right_angle_header",
    TactSwitchSmd: "tact_switch_smd",
    TactSwitchTht: "tact_switch_tht",
    UsbC: "usb_c",
    /// Memory card sockets (microSD, SD), push-push or push-pull.
    CardSocket: "card_socket",
    /// DC power barrel jacks (THT or SMT).
    BarrelJack: "barrel_jack" | "dc_jack",
    /// PCB terminal blocks (screw, spring or lever clamps).
    TerminalBlock: "terminal_block",
    /// FPC/FFC connectors.
    FpcConnector: "fpc_connector" | "ffc_connector",
    /// RF coaxial connectors (SMA, MCX, MMCX, U.FL...).
    RfCoax: "rf_coax",
    /// Micro-USB B receptacles.
    MicroUsbB: "micro_usb_b",
    /// Top-view SMD LEDs with terminals on two opposite sides (5050 and
    /// 3535 PLCC, 2020 chip-scale), addressable RGB LEDs among them; the
    /// footprint is the source's land pattern (`pads`).
    SmdLed: "smd_led" | "plcc_led",
    /// SMD aluminium electrolytic capacitors ("V-chip"): a round can
    /// standing on a square insulating base plate, two flat terminals
    /// bent under the plate; the footprint is the source's land pattern
    /// (`pads`).
    SmdElectrolytic: "smd_electrolytic" | "v_chip",
}

impl PackageFamily {
    /// The family named `s` (canonical name or alias, any case).
    pub fn parse(s: &str) -> Option<PackageFamily> {
        PackageFamily::ALL.iter().copied().find(|f| {
            f.name().eq_ignore_ascii_case(s)
                || f.aliases().iter().any(|a| a.eq_ignore_ascii_case(s))
        })
    }

    /// Whether `s` names this family.
    pub fn is_named(self, s: &str) -> bool {
        PackageFamily::parse(s) == Some(self)
    }

    /// The pin-header families, which share one generator.
    pub fn is_header(self) -> bool {
        matches!(
            self,
            PackageFamily::Header
                | PackageFamily::Socket
                | PackageFamily::ShroudedHeader
                | PackageFamily::RightAngleHeader
        )
    }
}

impl fmt::Display for PackageFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl std::str::FromStr for PackageFamily {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        PackageFamily::parse(s).ok_or_else(|| {
            let names: Vec<&str> = PackageFamily::ALL.iter().map(|f| f.name()).collect();
            format!(
                "unknown package family \"{s}\" (known: {})",
                names.join(", ")
            )
        })
    }
}

impl Serialize for PackageFamily {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for PackageFamily {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for PackageFamily {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "PackageFamily".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let names: Vec<&str> = PackageFamily::ALL.iter().map(|f| f.name()).collect();
        schemars::json_schema!({ "type": "string", "enum": names })
    }
}

/// Standard chip body sizes (length, width in mm) with their imperial
/// and metric codes, smallest first. A metric code that reads like an
/// imperial one carries an `m` suffix (`0603m` is imperial 0201).
pub const CHIP_SIZES: &[(f64, f64, &[&str])] = &[
    (0.4, 0.2, &["01005", "0402m"]),
    (0.6, 0.3, &["0201", "0603m"]),
    (1.0, 0.5, &["0402", "1005"]),
    (1.6, 0.8, &["0603", "1608"]),
    (2.0, 1.25, &["0805", "2012"]),
    (3.2, 1.6, &["1206", "3216"]),
    (3.2, 2.5, &["1210", "3225"]),
    (4.5, 3.2, &["1812", "4532"]),
    (5.0, 2.5, &["2010", "5025"]),
    (6.4, 3.2, &["2512", "6432"]),
];

/// Imperial and metric size codes of a chip body (length x width, mm).
pub fn chip_codes(l: f64, w: f64) -> &'static [&'static str] {
    CHIP_SIZES
        .iter()
        .find(|(sl, sw, _)| (l - sl).abs() <= 0.15 && (w - sw).abs() <= 0.15)
        .map_or(&[], |s| s.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_aliases_parse_to_one_family_each() {
        let mut seen = std::collections::BTreeSet::new();
        for f in PackageFamily::ALL {
            assert_eq!(PackageFamily::parse(f.name()), Some(*f));
            for a in f.aliases() {
                assert_eq!(PackageFamily::parse(a), Some(*f));
            }
            for n in std::iter::once(f.name()).chain(f.aliases().iter().copied()) {
                assert!(seen.insert(n), "{n} names two families");
            }
        }
        assert_eq!(PackageFamily::parse("TO252"), Some(PackageFamily::Dpak));
        assert!("bogus".parse::<PackageFamily>().is_err());
    }

    #[test]
    fn serializes_by_canonical_name() {
        let f: PackageFamily = serde_yaml::from_str("tqfp").unwrap();
        assert_eq!(f, PackageFamily::Lqfp);
        assert_eq!(serde_yaml::to_string(&f).unwrap().trim(), "lqfp");
    }
}
