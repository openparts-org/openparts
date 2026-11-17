//! `Package::connector` (Canonical Data Specification section 20.6): the
//! facts a connector's source states about how it mates -- gender, which
//! way it mates, a card socket's eject mechanism, an FPC connector's
//! contact side, the mating interface, and whether it straddles the board
//! edge. Every field is optional and recorded only when the source says
//! so; the parts search facets (`Package::structure`) read them.

use crate::structure::{Gender, Orientation};
use crate::PackageFamily as F;
use serde::{Deserialize, Serialize};

/// A card socket's card insertion and ejection mechanism.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardMechanism {
    /// Push to lock, push again to eject (a latch spring).
    PushPush,
    /// Push in, pull out (friction, no eject spring).
    PushPull,
    /// A hinged lid closes over the card.
    Hinged,
}

/// Which face of an FPC/FFC cable the connector's contacts touch.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactSide {
    /// Contacts above the cable (they touch its upper face).
    Top,
    /// Contacts below the cable.
    Bottom,
    /// Contacts on both faces.
    Dual,
}

/// The standard mating interface of a card socket or an RF connector.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorInterface {
    /// microSD (TransFlash) card.
    MicroSd,
    /// Full-size SD card.
    Sd,
    Sma,
    /// Reverse-polarity SMA.
    RpSma,
    Smb,
    Mcx,
    Mmcx,
    /// Hirose U.FL and its intermateable equivalents (IPEX MHF I).
    UFl,
    Bnc,
}

impl ConnectorInterface {
    /// Card interfaces (the rest are RF).
    pub fn is_card(self) -> bool {
        matches!(self, ConnectorInterface::MicroSd | ConnectorInterface::Sd)
    }
}

/// `Package::connector`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageConnector {
    /// Jack/receptacle or plug, when the family doesn't fix it (`rf_coax`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<Gender>,
    /// Mating direction relative to the board: `vertical` (straight up)
    /// or `right_angle` (parallel to the board). For a terminal block
    /// it is the wire entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<Orientation>,
    /// `card_socket` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mechanism: Option<CardMechanism>,
    /// `fpc_connector` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_side: Option<ContactSide>,
    /// `card_socket` (card interfaces) and `rf_coax` (RF interfaces).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<ConnectorInterface>,
    /// The part straddles the board edge (an edge-launch RF connector):
    /// its body lies beyond the PCB edge line, so `Package::edge` must
    /// say where that line is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub edge_mount: bool,
}

impl PackageConnector {
    /// The fields `family` may not carry, by name: a mechanism outside
    /// card sockets, a contact side outside FPC connectors, an interface
    /// that doesn't belong to the family.
    pub fn misplaced(&self, family: F) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.mechanism.is_some() && family != F::CardSocket {
            out.push("mechanism");
        }
        if self.contact_side.is_some() && family != F::FpcConnector {
            out.push("contact_side");
        }
        if let Some(i) = self.interface {
            let ok = match family {
                F::CardSocket => i.is_card(),
                F::RfCoax => !i.is_card(),
                _ => false,
            };
            if !ok {
                out.push("interface");
            }
        }
        if self.edge_mount && family != F::RfCoax {
            out.push("edge_mount");
        }
        out
    }
}

/// The connector families: an explicit land pattern (`Package::pads`)
/// in a frame centred on the housing, mating face toward +Y.
pub fn is_connector_family(family: F) -> bool {
    matches!(
        family,
        F::CardSocket
            | F::BarrelJack
            | F::TerminalBlock
            | F::FpcConnector
            | F::RfCoax
            | F::MicroUsbB
            | F::MiniUsbB
            | F::UsbB
    )
}

/// The electromechanical families (relay, buzzer, trimmer, rotary
/// encoder): like a connector, a body over an explicit land pattern
/// (`Package::pads`) in a frame centred on the body, but no mating face
/// and no `connector` facts (Canonical Data Specification section 20.14).
pub fn is_electromechanical_family(family: F) -> bool {
    matches!(
        family,
        F::Relay
            | F::Buzzer
            | F::Trimmer
            | F::RotaryEncoder
            | F::Potentiometer
            | F::SlidePotentiometer
    )
}

/// The electromechanical families whose shaft may lie parallel to the
/// board (`actuation: side`): rotary encoders and shaft potentiometers.
pub fn is_shaft_family(family: F) -> bool {
    matches!(family, F::RotaryEncoder | F::Potentiometer)
}

/// The families drawn as a body over the stated land pattern, the
/// connector and the electromechanical ones.
pub fn is_pad_body_family(family: F) -> bool {
    is_connector_family(family) || is_electromechanical_family(family)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_rejects_fields_outside_their_family() {
        let c: PackageConnector =
            serde_yaml::from_str("{ mechanism: push_push, interface: micro_sd }").unwrap();
        assert!(c.misplaced(F::CardSocket).is_empty());
        assert_eq!(c.misplaced(F::RfCoax), vec!["mechanism", "interface"]);
        let c: PackageConnector = serde_yaml::from_str(
            "{ gender: female, orientation: right_angle, interface: sma, edge_mount: true }",
        )
        .unwrap();
        assert!(c.misplaced(F::RfCoax).is_empty());
        assert_eq!(c.misplaced(F::BarrelJack), vec!["interface", "edge_mount"]);
        assert!(serde_yaml::from_str::<PackageConnector>("{ bogus: 1 }").is_err());
    }
}
