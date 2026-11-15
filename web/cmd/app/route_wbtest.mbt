///|
/// An MPN with `/` in it ("APT1608QBC/D") stays one value: the page link
/// carries it encoded in the query, the route decodes it back, and the
/// API path sends it as one `%2F` segment (the server's `:mpn`).
test "an MPN containing a slash round-trips through links and the API path" {
  for
    mpn in [
      "APT1608QBC/D", "19-217/R6C-P1Q2/6T", "LM1117MPX-3.3/NOPB", "RP2040",
    ] {
    let link = part_path("kingbright", mpn)
    let url = try! @url.parse("http://localhost/" + link)
    assert_true(route_of(url) is (Ja, Part("kingbright", m)) && m == mpn)
    let en = try! @url.parse("http://localhost/en/" + link)
    assert_true(route_of(en) is (En, Part("kingbright", m)) && m == mpn)
  }
  assert_eq(
    part_path("kingbright", "APT1608QBC/D"),
    "parts?manufacturer=kingbright&mpn=APT1608QBC%2FD",
  )
  assert_eq(
    part_api_path("kingbright", "APT1608QBC/D"),
    "/v1/parts/kingbright/APT1608QBC%2FD",
  )
  assert_eq(
    part_api_path("everlight", "19-217/R6C-P1Q2/6T"),
    "/v1/parts/everlight/19-217%2FR6C-P1Q2%2F6T",
  )
}
