// The part page's 3D preview (three.js via mizchi/three).

///|
const VIEWER_ID : String = "model-viewer"

///|
/// Stops the running preview, if any (set while one is shown).
let stop_viewer : Ref[() -> Unit] = { val: fn() {  }, }

///|
/// "Nice" grid cell sizes (mm), so the floor grid reads like graph
/// paper; the one giving about 15 squares across three times the
/// model's footprint.
fn grid_step(footprint : Double) -> Double {
  let span = footprint * 3.0
  let mut best = 0.1
  let mut best_diff = 1.0e300
  for
    step in [
      0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0,
    ] {
    let diff = (span / step - 15.0).abs()
    if diff < best_diff {
      best_diff = diff
      best = step
    }
  }
  best
}

///|
/// The model: the 3MF (SolidCraft's colour per body) when it parses,
/// else the uncoloured STL in a neutral grey.
async fn load_model(
  manufacturer : String,
  mpn : String,
) -> Result[@three.Object3D, String] noraise {
  if get_bytes(artifact_url(manufacturer, mpn, "3mf")) is Ok(bytes) {
    let group = Some(@three.ThreeMFLoader().parse(bytes)) catch { _ => None }
    if group is Some(g) && g.children().length() > 0 {
      return Ok(g.as_object3d())
    }
  }
  match get_bytes(artifact_url(manufacturer, mpn, "stl")) {
    Ok(bytes) =>
      try @three.STLLoader().parse(bytes) catch {
        _ => Err("")
      } noraise {
        geometry => {
          let material = @three.MeshStandardMaterial(
            0x8f97a3,
            metalness=0.2,
            roughness=0.6,
          )
          Ok(@three.Mesh(geometry, material.as_material()).as_object3d())
        }
      }
    Err(e) => Err(e)
  }
}

///|
/// Shows the part's model in the element `VIEWER_ID` (once rendered);
/// returns an error message to show instead.
async fn show_model(manufacturer : String, mpn : String) -> String? noraise {
  (stop_viewer.val)()
  let loaded = load_model(manufacturer, mpn)
  let ready = wait_for_element(VIEWER_ID).wait() catch { _ => false }
  guard ready else { return None }
  let object = match loaded {
    Ok(o) => o
    Err(e) => return Some(e)
  }
  // The artifacts are Z-up (mechanical CAD); three.js is Y-up: stand
  // the model on the floor grid, centred on the origin.
  let model = @three.Group()
  model.add(object) |> ignore
  let root = model.as_object3d()
  root.rotation().set(-3.141592653589793 / 2.0, 0.0, 0.0) |> ignore
  root.update_matrix_world(true)
  let center = @three.Box3()
    .set_from_object(root)
    .get_center(@three.Vector3(0.0, 0.0, 0.0))
  root.position().sub(center) |> ignore
  root.update_matrix_world(true)
  let bbox = @three.Box3().set_from_object(root)
  let size = bbox.get_size(@three.Vector3(0.0, 0.0, 0.0))
  let radius = if bbox.is_empty() {
    1.0
  } else {
    let r = bbox
      .get_bounding_sphere(@three.Sphere(@three.Vector3(0.0, 0.0, 0.0), 0.0))
      .radius()
    if r > 0.0 {
      r
    } else {
      1.0
    }
  }
  let w = element_width(VIEWER_ID)
  let h = element_height(VIEWER_ID)
  let scene = @three.Scene()
  scene.set_background(Some(@three.Color::from_hex(0xffffff)))
  let camera = @three.PerspectiveCamera(
    45.0,
    w.to_double() / (if h > 0 { h } else { 1 }).to_double(),
    radius / 100.0,
    radius * 100.0,
  )
  camera.as_object3d().position().set(radius * 1.8, radius * 1.4, radius * 1.8)
  |> ignore
  let renderer = @three.WebGLRenderer(antialias=true)
  renderer.set_pixel_ratio(device_pixel_ratio())
  renderer.set_size(w, h)
  mount_canvas(VIEWER_ID, renderer.dom_element())
  scene.add(root) |> ignore
  // A real-scale (mm) floor grid sized off the model's footprint, flush
  // with its bottom face.
  let footprint = if size.x() > size.z() { size.x() } else { size.z() }
  let step = grid_step(footprint)
  let divisions = {
    let d = (footprint * 3.0 / step).round().to_int()
    if d < 2 {
      2
    } else {
      d
    }
  }
  let grid = @three.GridHelper(step * divisions.to_double(), divisions)
  grid.position().set_y(bbox.min().y()) |> ignore
  scene.add(grid.as_object3d()) |> ignore
  scene.add(@three.HemisphereLight(0xffffff, 0x888888, 1.2).as_object3d())
  |> ignore
  let key = @three.DirectionalLight(0xffffff, 1.2)
  key.as_object3d().position().set(radius * 2.0, radius * 3.0, radius * 2.0)
  |> ignore
  scene.add(key.as_object3d()) |> ignore
  let controls = @three.OrbitControls(
    camera.as_camera(),
    renderer.dom_element(),
  )
  controls.set_enable_damping(true)
  controls.set_min_distance(radius * 0.5)
  controls.set_max_distance(radius * 10.0)
  controls.update(0.0) |> ignore
  let mut last_w = w
  let mut last_h = h
  renderer.set_animation_loop(_ => {
    // Gone from the page (navigated away): stop.
    if !element_exists(VIEWER_ID) {
      (stop_viewer.val)()
      return
    }
    let w = element_width(VIEWER_ID)
    let h = element_height(VIEWER_ID)
    if (w != last_w || h != last_h) && w > 0 && h > 0 {
      last_w = w
      last_h = h
      camera.set_aspect(w.to_double() / h.to_double())
      camera.update_projection_matrix()
      renderer.set_size(w, h)
    }
    controls.update(0.0) |> ignore
    renderer.render(scene, camera.as_camera())
  })
  stop_viewer.val = () => {
    stop_viewer.val = () => ()
    renderer.stop_animation_loop()
    controls.dispose()
    grid.dispose()
    renderer.dispose()
    renderer.force_context_loss()
  }
  None
}
