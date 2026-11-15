//! A package's structure as a parts search narrows it: pin rows × pins
//! per row, connector gender and orientation, a shroud, an exposed pad.
//! Each is derived from what the Package states (its family, whose
//! definition fixes these facts, `lead_layout`, `exposed_pad`), and is
//! `None` where the package doesn't decide it -- never guessed.

use crate::{Package, PackageFamily as F};
use serde::{Deserialize, Serialize};

/// Connector gender.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gender {
    /// Pins: a pin header, a plug.
    Male,
    /// Contacts in a housing: a socket, a receptacle.
    Female,
}

/// Which way a connector mates.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
    /// Card sockets: push-push, push-pull, hinged (`connector.mechanism`).
    pub mechanism: Option<crate::CardMechanism>,
    /// FPC connectors: the contact side (`connector.contact_side`).
    pub contact_side: Option<crate::ContactSide>,
    /// Card sockets and RF connectors: the mating interface
    /// (`connector.interface`).
    pub interface: Option<crate::ConnectorInterface>,
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
            // `usb_c` is the receptacle family, and so are the
            // connector families that take a card, a plug or a cable.
            F::Socket
            | F::UsbC
            | F::CardSocket
            | F::BarrelJack
            | F::FpcConnector
            | F::MicroUsbB => Some(Gender::Female),
            _ => None,
        };
        let connector = self.connector.as_ref();
        let gender = gender.or(connector.and_then(|c| c.gender));
        let orientation = match self.family {
            F::Header | F::Socket | F::ShroudedHeader => Some(Orientation::Vertical),
            // Its generator draws the horizontal (mid/top-mount)
            // receptacle: opening parallel to the board.
            F::RightAngleHeader | F::UsbC => Some(Orientation::RightAngle),
            // Connector families: as the source states it.
            _ => connector.and_then(|c| c.orientation),
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
            mechanism: connector.and_then(|c| c.mechanism),
            contact_side: connector.and_then(|c| c.contact_side),
            interface: connector.and_then(|c| c.interface),
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
            land_width: None,
            land_length: None,
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

    #[test]
    fn connector_families_read_their_connector_record() {
        let s = pkg("family: card_socket\nlead_count: 9\n\
                     connector: { mechanism: push_push, interface: micro_sd }")
        .structure();
        assert_eq!(s.gender, Some(Gender::Female));
        assert_eq!(s.mechanism, Some(crate::CardMechanism::PushPush));
        assert_eq!(s.interface, Some(crate::ConnectorInterface::MicroSd));
        assert_eq!(s.orientation, None);
        let s = pkg("family: rf_coax\nlead_count: 3\n\
                     connector: { gender: female, orientation: vertical, interface: sma }")
        .structure();
        assert_eq!(
            (s.gender, s.orientation),
            (Some(Gender::Female), Some(Orientation::Vertical))
        );
        let s = pkg("family: fpc_connector\nlead_count: 6\n\
                     connector: { contact_side: bottom, orientation: right_angle }")
        .structure();
        assert_eq!(s.contact_side, Some(crate::ContactSide::Bottom));
        // Without a record the search knows only what the family fixes.
        let s = pkg("family: terminal_block\nlead_count: 2").structure();
        assert_eq!(s, PackageStructure::default());
    }
}
