// Category icons: schematic-symbol inspired monoline SVGs (24x24,
// stroke = currentColor) shown in the catalog tiles' mark box.

///|
/// One icon per visual group; categories share them.
priv enum Icon {
  Chip
  Board
  Cap
  Xtal
  DiodeSym
  Bjt
  LedSym
  Opto
  FuseSym
  Plug
  RelaySym
  AndGate
  Zigzag
  Coil
  Spst
  Bolt
  MemChip
  Thermo
  OpAmpSym
  Bus
  Clock
  Motor
  Speaker
  Mic
  Antenna
  Pot
  Knob
  Grid
}

///|
fn category_icon_kind(c : @api.ComponentCategory) -> Icon {
  match c {
    Microcontroller | Other => Chip
    DevelopmentBoard => Board
    Capacitor => Cap
    Resistor => Zigzag
    Potentiometer => Pot
    Connector => Plug
    Crystal => Xtal
    Diode => DiodeSym
    Transistor => Bjt
    Led => LedSym
    Inductor | FerriteBead | CommonModeChoke => Coil
    Fuse => FuseSym
    Optocoupler => Opto
    Switch | TactSwitch => Spst
    RotaryEncoder => Knob
    Power
    | LinearRegulator
    | SwitchingRegulator
    | BatteryCharger
    | VoltageReference
    | PowerSwitch
    | Efuse => Bolt
    Memory | Flash | Eeprom | Sram => MemChip
    Sensor
    | PressureSensor
    | Imu
    | TemperatureSensor
    | LightSensor
    | GasSensor
    | MagneticSensor
    | TouchSensor => Thermo
    Analog
    | OpAmp
    | Comparator
    | CurrentSenseAmplifier
    | PowerMonitor
    | AnalogSwitch
    | Adc
    | Dac => OpAmpSym
    Interface
    | IoExpander
    | CanController
    | CanTransceiver
    | Rs485Transceiver
    | Rs232Transceiver
    | LevelShifter
    | UsbPdController
    | DigitalIsolator => Bus
    Logic => AndGate
    Timing | Oscillator | Timer | Rtc => Clock
    Driver | GateDriver | DarlingtonArray | MotorDriver | LedDriver | DisplayDriver =>
      Motor
    Relay => RelaySym
    Audio | AudioAmplifier | AudioCodec | Buzzer => Speaker
    Microphone => Mic
    Rf | WirelessModule | Antenna => Antenna
  }
}

///|
/// The inner SVG elements of an icon.
fn icon_body(i : Icon) -> String {
  let dot = (x : String, y : String, r : String) => {
    "<circle cx=\"\{x}\" cy=\"\{y}\" r=\"\{r}\" fill=\"currentColor\" stroke=\"none\"/>"
  }
  match i {
    Chip =>
      "<rect x=\"6\" y=\"6\" width=\"12\" height=\"12\" rx=\"1.5\"/>" +
      "<path d=\"M9 2.5V6M12 2.5V6M15 2.5V6M9 18v3.5M12 18v3.5M15 18v3.5" +
      "M2.5 9H6M2.5 12H6M2.5 15H6M18 9h3.5M18 12h3.5M18 15h3.5\"/>" +
      dot("9", "9", "1")
    Board =>
      "<rect x=\"2.5\" y=\"4.5\" width=\"19\" height=\"15\" rx=\"2\"/>" +
      "<rect x=\"10\" y=\"10.5\" width=\"7\" height=\"6\" rx=\"0.8\"/>" +
      "<path d=\"M2.5 11.5h3v4h-3\"/>" +
      dot("6.5", "7.5", "0.9") +
      dot("9.5", "7.5", "0.9") +
      dot("12.5", "7.5", "0.9") +
      dot("15.5", "7.5", "0.9") +
      dot("18.5", "7.5", "0.9")
    Cap => "<path d=\"M2.5 12H10M14 12h7.5M10 5.5v13M14 5.5v13\"/>"
    Xtal =>
      "<rect x=\"9.5\" y=\"6\" width=\"5\" height=\"12\" rx=\"0.6\"/>" +
      "<path d=\"M2.5 12H6.5M17.5 12h4M6.5 7.5v9M17.5 7.5v9\"/>"
    DiodeSym => "<path d=\"M2.5 12H8M16 12h5.5M8 6.5v11l8-5.5zM16 6.5v11\"/>"
    Bjt =>
      "<circle cx=\"12.5\" cy=\"12\" r=\"8.5\"/>" +
      "<path d=\"M3 12h6M9 7.5v9M9 10l5.5-4V2.5M9 14l5.5 4v3.5" +
      "M14.5 18l-1.2-2.8M14.5 18l-3-.3\"/>"
    LedSym =>
      "<path d=\"M2.5 15H6M13 15h3M6 10v10l7-5zM13 10v10\"/>" +
      "<path d=\"M13.5 8.5l4-4M17.5 4.5H15M17.5 4.5V7" +
      "M16.5 11.5l4-4M20.5 7.5H18M20.5 7.5V10\"/>"
    Opto =>
      "<rect x=\"2.5\" y=\"3.5\" width=\"19\" height=\"17\" rx=\"1.5\"/>" +
      "<path d=\"M7 3.5v6M4.5 9.5h5L7 13.5zM4.5 13.5h5M7 13.5v7\"/>" +
      "<path d=\"M10.5 11.5h3.5M12.3 9.8l1.7 1.7-1.7 1.7\"/>" +
      "<path d=\"M16.5 8v7M16.5 10l3-2.2V3.5M16.5 13l3 2.2v5.3\"/>"
    FuseSym =>
      "<rect x=\"6\" y=\"8.5\" width=\"12\" height=\"7\" rx=\"1\"/>" +
      "<path d=\"M2.5 12h19\"/>"
    Plug =>
      "<rect x=\"3\" y=\"5.5\" width=\"10\" height=\"13\" rx=\"1.5\"/>" +
      "<path d=\"M13 9.5h7.5M13 14.5h7.5M6.5 9.5h3M6.5 14.5h3\"/>"
    RelaySym =>
      "<rect x=\"2.5\" y=\"7\" width=\"6.5\" height=\"10\" rx=\"0.5\"/>" +
      "<path d=\"M2.5 17l6.5-10M18 2.5V8M18 21.5V16.5l-3.5-7\"/>" +
      "<path d=\"M9 13h6.5\" stroke-dasharray=\"1.5 2\"/>"
    AndGate =>
      "<path d=\"M5.5 5.5H11a6.5 6.5 0 0 1 0 13H5.5zM2.5 9h3M2.5 15h3M17.5 12h4\"/>"
    Zigzag => "<path d=\"M2.5 12H5l1.5-4 3 8 3-8 3 8 3-8L20 12h1.5\"/>"
    Coil =>
      "<path d=\"M2.5 13.5H4a2 3 0 0 1 4 0a2 3 0 0 1 4 0a2 3 0 0 1 4 0a2 3 0 0 1 4 0h1.5\"/>"
    Spst =>
      "<path d=\"M2.5 15H5.5M18.5 15h3M7.5 14l8.5-6\"/>" +
      dot("6.5", "15", "1.3") +
      dot("17.5", "15", "1.3")
    Bolt => "<path d=\"M13 2.5 4.5 14H11l-1 7.5L19 10h-6.5z\"/>"
    MemChip =>
      "<rect x=\"3\" y=\"5\" width=\"18\" height=\"12\" rx=\"1.5\"/>" +
      "<rect x=\"5.5\" y=\"8.5\" width=\"3.5\" height=\"5\" rx=\"0.5\"/>" +
      "<rect x=\"10.25\" y=\"8.5\" width=\"3.5\" height=\"5\" rx=\"0.5\"/>" +
      "<rect x=\"15\" y=\"8.5\" width=\"3.5\" height=\"5\" rx=\"0.5\"/>" +
      "<path d=\"M6 17v3M9 17v3M12 17v3M15 17v3M18 17v3\"/>"
    Thermo =>
      "<path d=\"M9.5 13.8V5a2.5 2.5 0 0 1 5 0v8.8a4.5 4.5 0 1 1-5 0z\"/>" +
      "<path d=\"M12 9.5v7M17.5 6h2.5M17.5 9h2.5M17.5 12h2.5\"/>" +
      dot("12", "17.5", "1.8")
    OpAmpSym =>
      "<path d=\"M6 4.5v15L18.5 12zM2.5 9H6M2.5 15H6M18.5 12h3\"/>"
    Bus =>
      "<path d=\"M3.5 8h16M16.5 5l3 3-3 3M20.5 16h-16M7.5 13l-3 3 3 3\"/>"
    Clock =>
      "<circle cx=\"12\" cy=\"12\" r=\"9\"/>" +
      "<path d=\"M12 6.5V12l3.5 2.5\"/>"
    Motor =>
      "<circle cx=\"12\" cy=\"12\" r=\"6.5\"/>" +
      "<path d=\"M12 2.5v3M12 18.5v3M9.3 14.8V9.2l2.7 3.6 2.7-3.6v5.6\"/>"
    Speaker =>
      "<path d=\"M3.5 9.5h3L11.5 5.5v13l-5-4h-3z\"/>" +
      "<path d=\"M15 9.2a4 4 0 0 1 0 5.6M17.8 6.4a8 8 0 0 1 0 11.2\"/>"
    Mic =>
      "<rect x=\"9\" y=\"2.5\" width=\"6\" height=\"11\" rx=\"3\"/>" +
      "<path d=\"M6 11a6 6 0 0 0 12 0M12 17v4M9 21.5h6\"/>"
    Antenna =>
      "<path d=\"M12 10v11.5M9 21.5h6M9.5 4.5a3.5 3.5 0 0 0 0 5M14.5 4.5a3.5 3.5 0 0 1 0 5" +
      "M6.5 2a7 7 0 0 0 0 10M17.5 2a7 7 0 0 1 0 10\"/>" +
      dot("12", "7", "1.5")
    Pot =>
      "<path d=\"M2.5 14H5l1.5-3 3 6 3-6 3 6 3-6L20 14h1.5\"/>" +
      "<path d=\"M12 3v6.5M9.8 7.3 12 9.5l2.2-2.2\"/>"
    Knob =>
      "<circle cx=\"12\" cy=\"12\" r=\"6.5\"/>" +
      "<path d=\"M12 12V8M12 2.5v1.5M12 20v1.5M2.5 12H4M20 12h1.5" +
      "M5.3 5.3l1 1M17.7 5.3l-1 1M5.3 18.7l1-1M17.7 18.7l-1-1\"/>"
    Grid =>
      "<rect x=\"4\" y=\"4\" width=\"7\" height=\"7\" rx=\"1.5\"/>" +
      "<rect x=\"13\" y=\"4\" width=\"7\" height=\"7\" rx=\"1.5\"/>" +
      "<rect x=\"4\" y=\"13\" width=\"7\" height=\"7\" rx=\"1.5\"/>" +
      "<rect x=\"13\" y=\"13\" width=\"7\" height=\"7\" rx=\"1.5\"/>"
  }
}

///|
fn icon_svg(i : Icon) -> String {
  "<svg class=\"cat-icon\" viewBox=\"0 0 24 24\" width=\"24\" height=\"24\" " +
  "fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.75\" " +
  "stroke-linecap=\"round\" stroke-linejoin=\"round\" " +
  "aria-hidden=\"true\" focusable=\"false\">" +
  icon_body(i) +
  "</svg>"
}

///|
/// The icon markup for a category tile (`None`: the chip fallback).
fn category_icon(c : @api.ComponentCategory?) -> String {
  icon_svg(
    match c {
      Some(c) => category_icon_kind(c)
      None => Chip
    },
  )
}

///|
/// The icon markup for the "all parts" tile.
fn all_parts_icon() -> String {
  icon_svg(Grid)
}

///|
/// Inline icon markup (our own constant SVG, no user text in it).
#warnings("-alert_xss_vulnerable")
fn icon_markup(svg : String) -> @html.Attrs {
  @html.Attrs::build().inner_html(svg)
}
