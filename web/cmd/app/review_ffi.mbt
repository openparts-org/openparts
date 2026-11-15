// Browser calls of the review workflow: the reviewer's name and token
// in localStorage, the token-carrying POST, and the datasheet viewer
// (src/datasheet.js).

///|
/// `localStorage[key]`, or "" (also when storage is unavailable).
extern "js" fn local_get(key : String) -> String =
  #|(k) => { try { return localStorage.getItem(k) ?? ""; } catch { return ""; } }

///|
extern "js" fn local_set(key : String, value : String) -> Unit =
  #|(k, v) => { try { localStorage.setItem(k, v); } catch {} }

///|
/// POSTs `body` (JSON) to `url` with `Authorization: Bearer <token>`.
/// Rejects only on a network error.
extern "js" fn post_json(
  url : String,
  token : String,
  body : String,
) -> @js_async.Promise[JsResponse] =
  #|async (url, token, body) => {
  #|  const r = await fetch(url, {
  #|    method: "POST",
  #|    headers: { "Content-Type": "application/json", "Authorization": "Bearer " + token },
  #|    body,
  #|  });
  #|  return { status: r.status, statusText: r.statusText, body_text: await r.text() };
  #|}

///|
/// The datasheet viewer's strings (JSON).
extern "js" fn datasheet_set_labels(json : String) -> Unit =
  #|(j) => globalThis.openpartsDatasheet?.setLabels(j)

///|
/// Shows a provenance entry (JSON options) in the element `id`.
extern "js" fn datasheet_inline(id : String, json : String) -> Unit =
  #|(id, j) => globalThis.openpartsDatasheet?.inline(id, j)

///|
/// Closes the datasheet side panel, if open.
extern "js" fn datasheet_close() -> Unit =
  #|() => globalThis.openpartsDatasheet?.close()

///|
/// Keyboard focus is in a text field (shortcuts are off there).
extern "js" fn typing_in_field() -> Bool =
  #|() => {
  #|  const e = document.activeElement;
  #|  return !!e && (e.tagName === "INPUT" || e.tagName === "TEXTAREA" || e.tagName === "SELECT" || e.isContentEditable);
  #|}

///|
extern "js" fn focus_element(id : String) -> Unit =
  #|(id) => { requestAnimationFrame(() => document.getElementById(id)?.focus()); }
