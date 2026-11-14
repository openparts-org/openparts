// Field decoders the generated FromJson impls (types.mbt) use: serde's
// rules for required, optional (missing or null) and defaulted fields.

///|
fn[T : @json.FromJson] field(
  obj : Map[String, Json],
  key : String,
  path : @json.JsonPath,
) -> T raise @json.JsonDecodeError {
  match obj.get(key) {
    Some(v) => @json.from_json(v, path=path.add_key(key))
    None => raise @json.JsonDecodeError((path, "missing field \{key}"))
  }
}

///|
fn[T : @json.FromJson] opt_field(
  obj : Map[String, Json],
  key : String,
  path : @json.JsonPath,
) -> T? raise @json.JsonDecodeError {
  match obj.get(key) {
    None | Some(Null) => None
    Some(v) => Some(@json.from_json(v, path=path.add_key(key)))
  }
}

///|
fn[T : @json.FromJson] field_or(
  obj : Map[String, Json],
  key : String,
  path : @json.JsonPath,
  default : T,
) -> T raise @json.JsonDecodeError {
  match obj.get(key) {
    None | Some(Null) => default
    Some(v) => @json.from_json(v, path=path.add_key(key))
  }
}
