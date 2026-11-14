// The few browser calls the site makes outside rabbita and three.js.

///|
/// Where the site is served from, with a trailing slash ("/openparts/"
/// on GitHub Pages, "/" behind openparts-server): index.html sets it
/// from Vite's base.
extern "js" fn base_path() -> String =
  #|() => {
  #|  const b = globalThis.OPENPARTS_BASE ?? "/";
  #|  return b.endsWith("/") ? b : b + "/";
  #|}

///|
/// The API's base URL without a trailing slash ("/api" by default).
extern "js" fn api_base() -> String =
  #|() => (globalThis.OPENPARTS_API ?? "/api").replace(/\/$/, "")

///|
extern "js" fn current_href() -> String =
  #|() => window.location.href

///|
extern "js" fn encode_uri_component(s : String) -> String =
  #|(s) => encodeURIComponent(s)

///|
extern "js" fn decode_uri_component(s : String) -> String =
  #|(s) => { try { return decodeURIComponent(s.replace(/\+/g, " ")); } catch { return s; } }

///|
extern "js" fn set_document_title(title : String) -> Unit =
  #|(t) => { document.title = t; }

///|
extern "js" fn set_document_lang(lang : String) -> Unit =
  #|(l) => { document.documentElement.lang = l; }

///|
#external
type JsResponse

///|
extern "js" fn JsResponse::status(self : JsResponse) -> Int32 =
  #|(r) => r.status

///|
extern "js" fn JsResponse::status_text(self : JsResponse) -> String =
  #|(r) => r.statusText

///|
extern "js" fn JsResponse::body_text(self : JsResponse) -> String =
  #|(r) => r.body_text

///|
extern "js" fn JsResponse::body_bytes(self : JsResponse) -> Bytes =
  #|(r) => r.body_bytes

///|
/// `fetch(url)`, with the whole body read: as text, or as bytes when
/// `binary`. Rejects only on a network error.
extern "js" fn fetch_raw(
  url : String,
  binary : Bool,
) -> @js_async.Promise[JsResponse] =
  #|async (url, binary) => {
  #|  const r = await fetch(url);
  #|  const out = { status: r.status, statusText: r.statusText };
  #|  if (binary && r.ok) out.body_bytes = new Uint8Array(await r.arrayBuffer());
  #|  else out.body_text = await r.text();
  #|  return out;
  #|}

///|
/// Hands `bytes` to the browser as a file download.
extern "js" fn save_file(
  bytes : Bytes,
  filename : String,
  mime : String,
) -> Unit =
  #|(bytes, name, mime) => {
  #|  const url = URL.createObjectURL(new Blob([bytes], { type: mime }));
  #|  const a = document.createElement("a");
  #|  a.href = url;
  #|  a.download = name;
  #|  a.click();
  #|  setTimeout(() => URL.revokeObjectURL(url), 0);
  #|}

///|
/// Resolves with the element `id` once rabbita has rendered it (or
/// false after ~5 s).
extern "js" fn wait_for_element(id : String) -> @js_async.Promise[Bool] =
  #|(id) => new Promise((resolve) => {
  #|  let n = 0;
  #|  const tick = () => {
  #|    if (document.getElementById(id)) resolve(true);
  #|    else if (++n > 300) resolve(false);
  #|    else requestAnimationFrame(tick);
  #|  };
  #|  tick();
  #|})

///|
extern "js" fn element_exists(id : String) -> Bool =
  #|(id) => document.getElementById(id) !== null

///|
/// Puts `canvas` into the element `id`, replacing what is there.
extern "js" fn mount_canvas(id : String, canvas : @three.Canvas) -> Unit =
  #|(id, c) => { const el = document.getElementById(id); el.replaceChildren(c); }

///|
extern "js" fn element_width(id : String) -> Int32 =
  #|(id) => document.getElementById(id)?.clientWidth ?? 0

///|
extern "js" fn element_height(id : String) -> Int32 =
  #|(id) => document.getElementById(id)?.clientHeight ?? 0

///|
extern "js" fn device_pixel_ratio() -> Double =
  #|() => window.devicePixelRatio || 1

///|
extern "js" fn scroll_top() -> Unit =
  #|() => window.scrollTo(0, 0)
