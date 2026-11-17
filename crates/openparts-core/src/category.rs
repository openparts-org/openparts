use serde::{Deserialize, Serialize};

/// Canonical Data Specification section 14.1 (Device): a component-type
/// classification for catalog browsing rather than engineering
/// decisions.
///
/// The values form a two-level tree (see [`ComponentCategory::parent`]):
/// a *group* such as `power` and its *subcategories* such as
/// `linear_regulator`. Every node is a valid value; a Device states the
/// most specific node its datasheet supports, and the tree places it
/// (a `linear_regulator` is also `power`). Data never states the
/// parent, so the two can't disagree. A group is itself used only when
/// no subcategory fits (a sensor of a kind not listed is `sensor`).
///
/// The vocabulary grows like `PinType`'s (section 15): add a node here
/// and in the spec when a new component type enters the dataset.
/// `Other` covers a real component type not yet in the tree; it must
/// still be set explicitly, never left absent (Unknown Values, section
/// 7 -- this field has no default).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentCategory {
    Microcontroller,
    Capacitor,
    Resistor,
    Inductor,
    Diode,
    Transistor,
    Led,
    Crystal,
    Fuse,
    Connector,
    /// Electromechanical, reed and solid-state relays.
    Relay,
    Other,
    /// Photocouplers (an LED and a photodetector in one package).
    Optocoupler,
    // Power
    Power,
    LinearRegulator,
    SwitchingRegulator,
    BatteryCharger,
    VoltageReference,
    /// Load switches and power multiplexers.
    PowerSwitch,
    // Memory
    Memory,
    Flash,
    Eeprom,
    Sram,
    // Sensors
    Sensor,
    PressureSensor,
    /// Accelerometers, gyroscopes and IMUs.
    Imu,
    TemperatureSensor,
    // Analog
    Analog,
    OpAmp,
    Comparator,
    CurrentSenseAmplifier,
    /// Current/voltage/power monitors with a digital interface.
    PowerMonitor,
    /// Analog switches and multiplexers.
    AnalogSwitch,
    Adc,
    Dac,
    // Interface
    Interface,
    IoExpander,
    CanController,
    CanTransceiver,
    Rs485Transceiver,
    Rs232Transceiver,
    /// Voltage-level translators.
    LevelShifter,
    /// Gates, buffers, flip-flops, shift registers, decoders.
    Logic,
    // Timing
    Timing,
    /// Clock oscillators (XO, TCXO, VCXO, MEMS): a resonator with its
    /// drive circuit, giving a clock output. A bare resonator is
    /// `crystal`.
    Oscillator,
    Timer,
    Rtc,
    // Switches
    Switch,
    TactSwitch,
    /// Mechanical and optical incremental rotary encoders.
    RotaryEncoder,
    // Drivers
    Driver,
    /// MOSFET/IGBT gate drivers.
    GateDriver,
    DarlingtonArray,
    /// Brushed DC, stepper and BLDC motor drivers (bridges and
    /// pre-drivers with their control logic).
    MotorDriver,
    /// LED drivers: constant-current sinks, LED matrix and PWM drivers,
    /// switching LED drivers.
    LedDriver,
    // Resistors
    /// Potentiometers and trimmers (a resistor with a wiper).
    Potentiometer,
    // Inductors
    /// Ferrite beads (chip EMI suppressors rated by impedance).
    FerriteBead,
    /// Common-mode chokes and filters.
    CommonModeChoke,
    // Sensors
    /// Ambient-light, proximity, colour and UV sensors.
    LightSensor,
    /// Gas and air-quality sensors (VOC, CO2, ...).
    GasSensor,
    /// Hall switches and latches, linear Hall sensors, magnetometers.
    MagneticSensor,
    // Audio
    Audio,
    /// Audio power amplifiers (class D, AB...).
    AudioAmplifier,
    /// Audio codecs (ADC + DAC with an audio interface).
    AudioCodec,
    /// Microphones (MEMS, electret).
    Microphone,
    /// Buzzers and sounders (magnetic or piezo, with or without a
    /// driver circuit).
    Buzzer,
    // RF
    Rf,
    /// Radio modules (Wi-Fi, Bluetooth, 802.15.4, LoRa...): a radio
    /// with its antenna or antenna connector on one module.
    WirelessModule,
    /// Antennas (chip, PCB, patch...).
    Antenna,
}

use ComponentCategory as C;

impl ComponentCategory {
    /// Every value, groups before their subcategories, in catalog order.
    pub const ALL: &'static [ComponentCategory] = &[
        C::Microcontroller,
        C::Capacitor,
        C::Resistor,
        C::Potentiometer,
        C::Inductor,
        C::FerriteBead,
        C::CommonModeChoke,
        C::Crystal,
        C::Diode,
        C::Transistor,
        C::Led,
        C::Optocoupler,
        C::Fuse,
        C::Connector,
        C::Relay,
        C::Switch,
        C::TactSwitch,
        C::RotaryEncoder,
        C::Power,
        C::LinearRegulator,
        C::SwitchingRegulator,
        C::BatteryCharger,
        C::VoltageReference,
        C::PowerSwitch,
        C::Memory,
        C::Flash,
        C::Eeprom,
        C::Sram,
        C::Sensor,
        C::PressureSensor,
        C::Imu,
        C::TemperatureSensor,
        C::LightSensor,
        C::GasSensor,
        C::MagneticSensor,
        C::Analog,
        C::OpAmp,
        C::Comparator,
        C::CurrentSenseAmplifier,
        C::PowerMonitor,
        C::AnalogSwitch,
        C::Adc,
        C::Dac,
        C::Interface,
        C::IoExpander,
        C::CanController,
        C::CanTransceiver,
        C::Rs485Transceiver,
        C::Rs232Transceiver,
        C::LevelShifter,
        C::Logic,
        C::Timing,
        C::Oscillator,
        C::Timer,
        C::Rtc,
        C::Driver,
        C::GateDriver,
        C::DarlingtonArray,
        C::MotorDriver,
        C::LedDriver,
        C::Audio,
        C::AudioAmplifier,
        C::AudioCodec,
        C::Microphone,
        C::Buzzer,
        C::Rf,
        C::WirelessModule,
        C::Antenna,
        C::Other,
    ];

    /// The group a subcategory belongs to; `None` for a top-level value.
    pub fn parent(self) -> Option<ComponentCategory> {
        Some(match self {
            C::TactSwitch | C::RotaryEncoder => C::Switch,
            C::Potentiometer => C::Resistor,
            C::FerriteBead | C::CommonModeChoke => C::Inductor,
            C::LinearRegulator
            | C::SwitchingRegulator
            | C::BatteryCharger
            | C::VoltageReference
            | C::PowerSwitch => C::Power,
            C::Flash | C::Eeprom | C::Sram => C::Memory,
            C::PressureSensor
            | C::Imu
            | C::TemperatureSensor
            | C::LightSensor
            | C::GasSensor
            | C::MagneticSensor => C::Sensor,
            C::OpAmp
            | C::Comparator
            | C::CurrentSenseAmplifier
            | C::PowerMonitor
            | C::AnalogSwitch
            | C::Adc
            | C::Dac => C::Analog,
            C::IoExpander
            | C::CanController
            | C::CanTransceiver
            | C::Rs485Transceiver
            | C::Rs232Transceiver
            | C::LevelShifter => C::Interface,
            C::Oscillator | C::Timer | C::Rtc => C::Timing,
            C::GateDriver | C::DarlingtonArray | C::MotorDriver | C::LedDriver => C::Driver,
            C::AudioAmplifier | C::AudioCodec | C::Microphone | C::Buzzer => C::Audio,
            C::WirelessModule | C::Antenna => C::Rf,
            _ => return None,
        })
    }

    /// The top-level value this one is under (itself if top-level).
    pub fn root(self) -> ComponentCategory {
        self.parent().unwrap_or(self)
    }

    /// Whether `self` is `other` or one of its subcategories.
    pub fn is_within(self, other: ComponentCategory) -> bool {
        self == other || self.parent() == Some(other)
    }

    /// The subcategories of a group, in catalog order.
    pub fn children(self) -> impl Iterator<Item = ComponentCategory> {
        Self::ALL
            .iter()
            .copied()
            .filter(move |c| c.parent() == Some(self))
    }

    /// The snake_case value as written in data (`linear_regulator`).
    pub fn name(self) -> &'static str {
        match self {
            C::Microcontroller => "microcontroller",
            C::Capacitor => "capacitor",
            C::Resistor => "resistor",
            C::Inductor => "inductor",
            C::Diode => "diode",
            C::Transistor => "transistor",
            C::Led => "led",
            C::Crystal => "crystal",
            C::Fuse => "fuse",
            C::Connector => "connector",
            C::Other => "other",
            C::Optocoupler => "optocoupler",
            C::Power => "power",
            C::LinearRegulator => "linear_regulator",
            C::SwitchingRegulator => "switching_regulator",
            C::BatteryCharger => "battery_charger",
            C::VoltageReference => "voltage_reference",
            C::PowerSwitch => "power_switch",
            C::Memory => "memory",
            C::Flash => "flash",
            C::Eeprom => "eeprom",
            C::Sram => "sram",
            C::Sensor => "sensor",
            C::PressureSensor => "pressure_sensor",
            C::Imu => "imu",
            C::TemperatureSensor => "temperature_sensor",
            C::Analog => "analog",
            C::OpAmp => "op_amp",
            C::Comparator => "comparator",
            C::CurrentSenseAmplifier => "current_sense_amplifier",
            C::PowerMonitor => "power_monitor",
            C::AnalogSwitch => "analog_switch",
            C::Adc => "adc",
            C::Dac => "dac",
            C::Interface => "interface",
            C::IoExpander => "io_expander",
            C::CanController => "can_controller",
            C::CanTransceiver => "can_transceiver",
            C::Rs485Transceiver => "rs485_transceiver",
            C::Rs232Transceiver => "rs232_transceiver",
            C::LevelShifter => "level_shifter",
            C::Logic => "logic",
            C::Timing => "timing",
            C::Oscillator => "oscillator",
            C::Timer => "timer",
            C::Rtc => "rtc",
            C::Switch => "switch",
            C::TactSwitch => "tact_switch",
            C::Driver => "driver",
            C::GateDriver => "gate_driver",
            C::DarlingtonArray => "darlington_array",
            C::Relay => "relay",
            C::RotaryEncoder => "rotary_encoder",
            C::MotorDriver => "motor_driver",
            C::LedDriver => "led_driver",
            C::Potentiometer => "potentiometer",
            C::FerriteBead => "ferrite_bead",
            C::CommonModeChoke => "common_mode_choke",
            C::LightSensor => "light_sensor",
            C::GasSensor => "gas_sensor",
            C::MagneticSensor => "magnetic_sensor",
            C::Audio => "audio",
            C::AudioAmplifier => "audio_amplifier",
            C::AudioCodec => "audio_codec",
            C::Microphone => "microphone",
            C::Buzzer => "buzzer",
            C::Rf => "rf",
            C::WirelessModule => "wireless_module",
            C::Antenna => "antenna",
        }
    }

    /// The value for its data spelling.
    pub fn parse(s: &str) -> Option<ComponentCategory> {
        Self::ALL.iter().copied().find(|c| c.name() == s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_value_is_listed_once_with_its_group_first() {
        let mut seen = std::collections::BTreeSet::new();
        for (i, c) in C::ALL.iter().enumerate() {
            assert!(seen.insert(*c), "{c:?} listed twice");
            if let Some(p) = c.parent() {
                let at = C::ALL.iter().position(|x| *x == p).unwrap();
                assert!(at < i, "{p:?} must precede {c:?}");
                assert_eq!(p.parent(), None, "two levels only");
            }
            assert_eq!(C::parse(c.name()), Some(*c));
        }
        // Every variant is in ALL (the match in `name` lists them all).
        assert_eq!(seen.len(), 69);
    }

    #[test]
    fn subtree_membership() {
        assert!(C::LinearRegulator.is_within(C::Power));
        assert!(C::Power.is_within(C::Power));
        assert!(!C::Power.is_within(C::LinearRegulator));
        assert!(!C::Capacitor.is_within(C::Power));
        assert_eq!(C::Imu.root(), C::Sensor);
        assert!(C::Potentiometer.is_within(C::Resistor));
        assert!(C::FerriteBead.is_within(C::Inductor));
        assert_eq!(C::Buzzer.root(), C::Audio);
        assert_eq!(
            C::Rf.children().collect::<Vec<_>>(),
            [C::WirelessModule, C::Antenna]
        );
        assert_eq!(
            C::Memory.children().collect::<Vec<_>>(),
            [C::Flash, C::Eeprom, C::Sram]
        );
    }
}
