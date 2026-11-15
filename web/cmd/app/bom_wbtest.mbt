///|
test "csv cells are quoted only when needed" {
  assert_eq(csv_cell("C1"), "C1")
  assert_eq(csv_cell("C1, C2"), "\"C1, C2\"")
  assert_eq(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"")
  assert_eq(strip_extension("pico.kicad_bom.csv"), "pico.kicad_bom")
  assert_eq(strip_extension("bom"), "bom")
}

///|
test "the annotated BOM keeps every line" {
  let text =
    #|{"format":"generic_csv","columns":[{"role":"mpn","header":"MPN"}],
    #| "summary":{"lines":1,"components":2,"ok":0,"review":0,"warning":0,"not_found":1,
    #|   "exact":0,"normalized":0,"variant":0,"value_footprint":0},
    #| "lines":[{"row":2,"references":["R1","R2"],"quantity":2,"value":"10k","footprint":"",
    #|   "mpn":"NOPE, 1","manufacturer":"","dnp":false,"status":"not_found","kind":"none",
    #|   "confidence":0.0,"matches":[],"issues":[{"code":"not_found","severity":"warning","detail":null}],
    #|   "alternatives":[]}]}
  let json = @json.parse(text)
  let r : @api.BomReport = @json.from_json(json)
  let csv = annotated_csv(r)
  let rows = csv.split("\r\n").collect()
  assert_eq(rows.length(), 3) // header, line, trailing ""
  assert_true(
    rows[1].to_owned().has_prefix("R1 R2,2,10k,,\"NOPE, 1\",,not_found,none,0"),
  )
}
