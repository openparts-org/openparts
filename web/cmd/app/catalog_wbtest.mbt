///|
test "every results column is a labelled parameter key" {
  for
    category in [
      "power", "linear_regulator", "memory", "switch", "capacitor", "resistor", "inductor",
      "crystal", "diode", "led", "transistor", "optocoupler", "fuse",
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
