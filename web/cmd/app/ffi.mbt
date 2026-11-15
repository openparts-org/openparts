// The few browser calls the site makes outside rabbita (and the wasm
// part viewer, src/viewer.js).

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
/// A mounted part viewer (crates/viewer/openparts-viewer), or why it
/// couldn't be mounted.
#external
type ViewerMount

///|
/// Mounts the wasm part viewer: the 3D view into the element `model_id`,
/// the footprint view into `footprint_id`. `options` is its JSON options
/// (API base, part, UI strings). Never rejects: a failure (the wasm
/// not loading, the part not found) comes back as `error`.
extern "js" fn viewer_mount(
  model_id : String,
  footprint_id : String,
  options : String,
) -> @js_async.Promise[ViewerMount] =
  #|async (m, f, o) => {
  #|  try {
  #|    const lib = await globalThis.openpartsViewer();
  #|    const model = document.getElementById(m);
  #|    if (!model) return { error: "" };
  #|    const fp = document.getElementById(f) ?? undefined;
  #|    return { handle: await lib.mount(model, fp, o) };
  #|  } catch (e) {
  #|    return { error: String(e?.message ?? e) };
  #|  }
  #|}

///|
extern "js" fn ViewerMount::mounted(self : ViewerMount) -> Bool =
  #|(v) => v.handle !== undefined

///|
/// Why it didn't mount ("" when there's no message).
extern "js" fn ViewerMount::error(self : ViewerMount) -> String =
  #|(v) => v.error ?? ""

///|
/// Tears the viewer down (idempotent).
extern "js" fn ViewerMount::destroy(self : ViewerMount) -> Unit =
  #|(v) => { if (v.handle) { v.handle.destroy(); v.handle.free(); v.handle = undefined; } }

///|
extern "js" fn scroll_top() -> Unit =
  #|() => window.scrollTo(0, 0)
