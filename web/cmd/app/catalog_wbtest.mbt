///|
test "every results column is a labelled parameter key" {
  for
    category in [
      "power", "linear_regulator", "memory", "switch", "capacitor", "resistor", "inductor",
      "crystal", "diode", "led", "transistor", "optocoupler", "fuse", "op_amp", "comparator",
      "oscillator", "can_transceiver", "rs485_transceiver", "level_shifter", "analog",
      "interface", "logic", "timing", "motor_driver", "led_driver", "driver", "potentiometer",
      "ferrite_bead", "common_mode_choke", "rotary_encoder", "relay", "light_sensor",
      "gas_sensor", "magnetic_sensor", "sensor", "audio_amplifier", "audio_codec",
      "microphone", "buzzer", "audio", "wireless_module", "antenna", "rf",
      "touch_sensor", "display_driver", "usb_pd_controller", "digital_isolator", "efuse",
      "development_board",
    ] {
    let columns = param_columns(category)
    assert_true(!columns.is_empty())
    for key in columns {
      assert_true(is_param_key(key))
      assert_true(param_name(En, key) != key)
      assert_true(param_name(Ja, key) != key)
    }
  }
  assert_eq(param_name(En, "zener_voltage"), "Zener voltage (Vz)")
  assert_eq(
    param_name(En, "current_transfer_ratio"),
    "Current transfer ratio (CTR)",
  )
}

///|
test "disjoint ranges show each range" {
  let r = (lo : Double, hi : Double) => @api.ParameterRange::{
    min: Some(lo),
    typ: None,
    max: Some(hi),
    unit: Some("V"),
  }
  let v = @api.ParameterValue::{
    at: None,
    max: None,
    min: None,
    ranges: Some([r(1.65, 1.95), r(3.0, 3.6)]),
    typ: None,
    unit: None,
    value: None,
    values: None,
  }
  assert_eq(
    format_parameter(En, "supply_voltage", v),
    "1.65–1.95 V / 3–3.6 V",
  )
}
