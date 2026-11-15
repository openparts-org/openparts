// The part page's 3D and footprint views: the wasm part viewer
// (crates/viewer/openparts-viewer, wgpu) mounted into two empty elements
// view.mbt renders.

///|
const VIEWER_ID : String = "model-viewer"

///|
const FOOTPRINT_ID : String = "footprint-viewer"

///|
/// Stops the running viewer, if any (set while one is shown).
let stop_viewer : Ref[() -> Unit] = { val: fn() {  }, }

///|
/// The viewer's options: where the API is, which part, and its UI
/// strings in `lang`.
fn viewer_options(lang : Lang, manufacturer : String, mpn : String) -> String {
  let options : Json = {
    "api": Json::string(api_base()),
    "manufacturer": Json::string(manufacturer),
    "mpn": Json::string(mpn),
    "labels": viewer_labels(lang),
  }
  options.stringify()
}

///|
/// Shows the part in `VIEWER_ID` and `FOOTPRINT_ID` (once rendered);
/// returns an error message to show instead.
async fn show_model(
  lang : Lang,
  manufacturer : String,
  mpn : String,
) -> String? noraise {
  (stop_viewer.val)()
  let ready = wait_for_element(VIEWER_ID).wait() catch { _ => false }
  guard ready else { return None }
  let mounted = viewer_mount(
    VIEWER_ID,
    FOOTPRINT_ID,
    viewer_options(lang, manufacturer, mpn),
  ).wait() catch {
    e => return Some(e.to_string())
  }
  guard mounted.mounted() else { Some(mounted.error()) }
  stop_viewer.val = () => {
    stop_viewer.val = () => ()
    mounted.destroy()
  }
  // Navigated away while it loaded: nothing is left to show it in.
  if !element_exists(VIEWER_ID) {
    (stop_viewer.val)()
  }
  None
}
