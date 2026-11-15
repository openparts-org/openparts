//! A package's structure as a parts search narrows it: pin rows × pins
//! per row, connector gender and orientation, a shroud, an exposed pad.
//! Each is derived from what the Package states (its family, whose
//! definition fixes these facts, `lead_layout`, `exposed_pad`), and is
//! `None` where the package doesn't decide it -- never guessed.

use crate::{Package, PackageFamily as F};
use serde::Serialize;

/// Connector gender.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Gender {
    /// Pins: a pin header, a plug.
    Male,
    /// Contacts in a housing: a socket, a receptacle.
    Female,
}

/// Which way a connector mates.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    /// Straight up from the board.
    Vertical,
    /// Parallel to the board (right angle, horizontal).
    RightAngle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackageStructure {
    /// (rows, pins per row): headers and sockets (`lead_layout`), DIP
    /// (two rows), SIP (one).
    pub rows: Option<(u32, u32)>,
    pub gender: Option<Gender>,
    pub orientation: Option<Orientation>,
    /// Pin headers and sockets: a shroud (box header) or not.
    pub shrouded: Option<bool>,
    /// IC packages that can have one: whether this one has an exposed
    /// pad (`dimensions.exposed_pad`).
    pub exposed_pad: Option<bool>,
}

impl Package {
    /// The package's structure (see [`PackageStructure`]).
    pub fn structure(&self) -> PackageStructure {
        let n = self.lead_count;
        let layout = self.lead_layout.as_deref().and_then(|l| match *l {
            [rows, per_row] if rows >= 1 && rows * per_row == n => Some((rows, per_row)),
            _ => None,
        });
        let rows = match self.family {
            F::Header | F::Socket | F::ShroudedHeader => layout,
            F::RightAngleHeader => layout.or(Some((1, n))),
            F::Dip if n.is_multiple_of(2) => Some((2, n / 2)),
            F::Sip => Some((1, n)),
            _ => None,
        };
        let gender = match self.family {
            F::Header | F::ShroudedHeader | F::RightAngleHeader => Some(Gender::Male),
            // `usb_c` is the receptacle family.
            F::Socket | F::UsbC => Some(Gender::Female),
            _ => None,
        };
        let orientation = match self.family {
            F::Header | F::Socket | F::ShroudedHeader => Some(Orientation::Vertical),
            // Its generator draws the horizontal (mid/top-mount)
            // receptacle: opening parallel to the board.
            F::RightAngleHeader | F::UsbC => Some(Orientation::RightAngle),
            _ => None,
        };
        let shrouded = match self.family {
            F::ShroudedHeader => Some(true),
            F::Header | F::Socket | F::RightAngleHeader => Some(false),
            _ => None,
        };
        let exposed_pad = matches!(
            self.family,
            F::Lqfp | F::Qfn | F::Dfn | F::Son | F::Lga | F::Soic | F::Tssop | F::Ssop | F::Msop
        )
        .then(|| self.dimensions.exposed_pad.is_some());
        PackageStructure {
            rows,
            gender,
            orientation,
            shrouded,
            exposed_pad,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(yaml: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\npitch: {{nominal: 2.54, unit: mm}}\n\
             dimensions: {{body_width: {{nominal: 1, unit: mm}}, body_length: {{nominal: 1, unit: mm}}}}\n{yaml}"
        ))
        .unwrap()
    }

    #[test]
    fn headers_and_sockets() {
        let s = pkg("family: header\nlead_count: 20\nlead_layout: [1, 20]").structure();
        assert_eq!(s.rows, Some((1, 20)));
        assert_eq!(s.gender, Some(Gender::Male));
        assert_eq!(s.orientation, Some(Orientation::Vertical));
        assert_eq!(s.shrouded, Some(false));
        assert_eq!(s.exposed_pad, None);
        let s = pkg("family: header\nlead_count: 20\nlead_layout: [2, 10]").structure();
        assert_eq!(s.rows, Some((2, 10)));
        let s = pkg("family: socket\nlead_count: 4\nlead_layout: [1, 4]").structure();
        assert_eq!(s.gender, Some(Gender::Female));
        let s = pkg("family: right_angle_header\nlead_count: 3").structure();
        assert_eq!(
            (s.rows, s.orientation),
            (Some((1, 3)), Some(Orientation::RightAngle))
        );
        let s = pkg("family: shrouded_header\nlead_count: 2\nlead_layout: [1, 2]").structure();
        assert_eq!(s.shrouded, Some(true));
        // A layout that doesn't multiply out decides nothing.
        let s = pkg("family: header\nlead_count: 20\nlead_layout: [2, 9]").structure();
        assert_eq!(s.rows, None);
    }

    #[test]
    fn ics_and_others() {
        let s = pkg("family: qfn\nlead_count: 16").structure();
        assert_eq!(s.exposed_pad, Some(false));
        let s = pkg("family: qfn\nlead_count: 16\n");
        let mut s = s;
        s.dimensions.exposed_pad = Some(crate::ExposedPadDimensions {
            width: s.dimensions.body_width.clone(),
            length: s.dimensions.body_length.clone(),
        });
        assert_eq!(s.structure().exposed_pad, Some(true));
        assert_eq!(
            pkg("family: dip\nlead_count: 8").structure().rows,
            Some((2, 4))
        );
        let s = pkg("family: chip\nlead_count: 2").structure();
        assert_eq!(s, PackageStructure::default());
        let s = pkg("family: usb_c\nlead_count: 16").structure();
        assert_eq!(
            (s.gender, s.orientation),
            (Some(Gender::Female), Some(Orientation::RightAngle))
        );
    }
}
