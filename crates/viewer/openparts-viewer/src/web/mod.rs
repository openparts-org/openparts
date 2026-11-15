//! The browser side: `mount` fetches the part, its package and device
//! from the API, draws the 3D view (wgpu) and the footprint view (SVG),
//! and returns a `Viewer` handle (`destroy`, `setDimensions`,
//! `setMeasure`, `clearMeasure`, `resetView`).

mod gpu;

use crate::camera::OrbitCamera;
use crate::dims::{self, Annotation};
use crate::footprint::{self, esc, Drawing, FpSnap, P};
use crate::format::number;
use crate::labels::Labels;
use crate::mesh::{self, Mesh};
use crate::snap::{self, Candidate, SnapKind};
use glam::{Vec2, Vec3};
use gpu::{Gpu, Line};
use openparts_core::{Device, Package, ProvenanceMap};
use openparts_mcad::MechanicalGeometry;
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Event, EventTarget, HtmlCanvasElement, HtmlElement, KeyboardEvent, PointerEvent};

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[derive(Deserialize)]
struct Options {
    /// The API's base URL ("/api").
    api: String,
    manufacturer: String,
    mpn: String,
    #[serde(default)]
    labels: Labels,
}

fn window() -> web_sys::Window {
    web_sys::window().expect("a window")
}

fn document() -> web_sys::Document {
    window().document().expect("a document")
}

fn element(tag: &str, class: &str) -> HtmlElement {
    let e: HtmlElement = document().create_element(tag).unwrap().unchecked_into();
    e.set_class_name(class);
    e
}

fn set_style(e: &HtmlElement, name: &str, value: &str) {
    let _ = e.style().set_property(name, value);
}

fn show(e: &HtmlElement, visible: bool) {
    set_style(e, "display", if visible { "" } else { "none" });
}

async fn fetch(url: &str) -> Result<web_sys::Response, String> {
    let r = JsFuture::from(window().fetch_with_str(url))
        .await
        .map_err(|e| format!("{e:?}"))?;
    let r: web_sys::Response = r.unchecked_into();
    if r.ok() {
        Ok(r)
    } else {
        Err(format!("{} {}: {}", r.status(), r.status_text(), url))
    }
}

async fn fetch_text(url: &str) -> Result<String, String> {
    let r = fetch(url).await?;
    let t = JsFuture::from(r.text().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?;
    t.as_string().ok_or_else(|| "not text".to_string())
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let r = fetch(url).await?;
    let b = JsFuture::from(r.array_buffer().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?;
    Ok(js_sys::Uint8Array::new(&b).to_vec())
}

async fn fetch_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, String> {
    let text = fetch_text(url).await?;
    serde_json::from_str(&text).map_err(|e| format!("{url}: {e}"))
}

/// What the views need about the part.
struct PartInfo {
    package: Package,
    device: Option<Device>,
    /// Source id -> (label, URL).
    sources: HashMap<String, (String, String)>,
}

#[derive(Deserialize)]
struct PartRefs {
    device: String,
    package: String,
}

#[derive(Deserialize)]
struct SourceDoc {
    id: String,
    publisher: String,
    document_id: String,
    #[serde(default)]
    revision: Option<String>,
    url: String,
}

#[derive(Deserialize)]
struct PartSource {
    source: SourceDoc,
}

fn part_path(o: &Options) -> String {
    format!(
        "{}/v1/parts/{}/{}",
        o.api.trim_end_matches('/'),
        String::from(js_sys::encode_uri_component(&o.manufacturer)),
        String::from(js_sys::encode_uri_component(&o.mpn))
    )
}

async fn load_part(o: &Options) -> Result<PartInfo, String> {
    let base = part_path(o);
    let api = o.api.trim_end_matches('/');
    let refs: PartRefs = fetch_json(&base).await?;
    let package: Package = fetch_json(&format!("{api}/v1/packages/{}", refs.package)).await?;
    let device: Option<Device> = fetch_json(&format!("{api}/v1/devices/{}", refs.device))
        .await
        .ok();
    let sources: Vec<PartSource> = fetch_json(&format!("{base}/sources"))
        .await
        .unwrap_or_default();
    let sources = sources
        .into_iter()
        .map(|s| {
            let s = s.source;
            let mut label = format!("{} {}", s.publisher.to_uppercase(), s.document_id);
            if let Some(r) = s.revision {
                label = format!("{label} rev {r}");
            }
            (s.id, (label, s.url))
        })
        .collect();
    Ok(PartInfo {
        package,
        device,
        sources,
    })
}

/// An event listener, removed when dropped.
struct Listener {
    target: EventTarget,
    kind: &'static str,
    f: Closure<dyn FnMut(Event)>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.kind, self.f.as_ref().unchecked_ref());
    }
}

/// Calls `f` on the view behind `weak` for `kind` events on `target`.
fn listen<T: 'static>(
    target: &EventTarget,
    kind: &'static str,
    weak: &Weak<RefCell<T>>,
    f: fn(&mut T, Event),
) -> Listener {
    let weak = weak.clone();
    let closure = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
        if let Some(s) = weak.upgrade() {
            if let Ok(mut s) = s.try_borrow_mut() {
                f(&mut s, e);
            }
        }
    });
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_passive(false);
    let _ = target.add_event_listener_with_callback_and_add_event_listener_options(
        kind,
        closure.as_ref().unchecked_ref(),
        &opts,
    );
    Listener {
        target: target.clone(),
        kind,
        f: closure,
    }
}

fn button(label: &str, pressed: Option<bool>) -> HtmlElement {
    let b = element("button", "opv-button");
    let _ = b.set_attribute("type", "button");
    b.set_text_content(Some(label));
    if let Some(p) = pressed {
        let _ = b.set_attribute("aria-pressed", if p { "true" } else { "false" });
    }
    b
}

fn set_pressed(b: &HtmlElement, p: bool) {
    let _ = b.set_attribute("aria-pressed", if p { "true" } else { "false" });
}

/// The measurement's text: distance, then the axis deltas.
fn measure_text(labels: &Labels, a: Vec3, b: Vec3, three_d: bool) -> (String, String) {
    let d = b - a;
    let short = format!("{} mm", number(d.length() as f64));
    let mut long = format!(
        "{} {} · ΔX {} · ΔY {}",
        labels.distance,
        short,
        number(d.x as f64),
        number(d.y as f64)
    );
    if three_d {
        long.push_str(&format!(" · ΔZ {}", number(d.z as f64)));
    }
    (short, long)
}

// ---------------------------------------------------------------- 3D view

const DIM_COLOR: [f32; 4] = [0.012, 0.16, 0.65, 1.0];
const MEASURE_COLOR: [f32; 4] = [0.85, 0.12, 0.005, 1.0];

struct ModelView {
    root: HtmlElement,
    canvas: HtmlCanvasElement,
    gpu: Option<Gpu>,
    camera: OrbitCamera,
    home: OrbitCamera,
    mesh: Mesh,
    snaps: Vec<Candidate>,
    dims: Vec<(Annotation, HtmlElement)>,
    dims_button: HtmlElement,
    measure_button: HtmlElement,
    readout: HtmlElement,
    popup: HtmlElement,
    popup_for: Option<usize>,
    marker: HtmlElement,
    point_marks: [HtmlElement; 2],
    measure_label: HtmlElement,
    grid_center: Vec2,
    grid_footprint: f32,
    show_dims: bool,
    measuring: bool,
    points: Vec<Candidate>,
    hover: Option<Candidate>,
    cursor: Option<Vec2>,
    cursor_moved: bool,
    pointers: Vec<(i32, Vec2)>,
    press: Option<(Vec2, i16, bool)>,
    moved: bool,
    size: Vec2,
    dpr: f32,
    dirty: bool,
    alive: bool,
    raf: Option<i32>,
    raf_closure: Option<Closure<dyn FnMut()>>,
    labels: Labels,
    provenance: ProvenanceMap,
    sources: HashMap<String, (String, String)>,
    listeners: Vec<Listener>,
}

/// Lays out the 3D view in `root`: canvas, label layer, toolbar,
/// readout and provenance popup.
async fn mount_model(
    root: HtmlElement,
    o: &Options,
    info: &PartInfo,
    geometry: Option<&MechanicalGeometry>,
) -> Result<Rc<RefCell<ModelView>>, String> {
    let labels = o.labels.clone();
    root.set_inner_html("");
    root.class_list().add_1("opv-root").ok();
    let base = part_path(o);
    let mesh = match fetch_bytes(&format!("{base}/artifacts/3mf"))
        .await
        .and_then(|b| mesh::parse_3mf(&b))
    {
        Ok(m) => m,
        Err(_) => fetch_bytes(&format!("{base}/artifacts/stl"))
            .await
            .and_then(|b| mesh::parse_stl(&b))
            .map_err(|_| labels.model_failed.clone())?,
    };
    let make_canvas = || {
        let c: HtmlCanvasElement = element("canvas", "opv-canvas").unchecked_into();
        let old = root.query_selector("canvas").ok().flatten();
        if let Some(old) = old {
            old.remove();
        }
        root.prepend_with_node_1(&c).ok();
        let dpr = window().device_pixel_ratio();
        c.set_width(((root.client_width().max(1) as f64) * dpr) as u32);
        c.set_height(((root.client_height().max(1) as f64) * dpr) as u32);
        c
    };
    let (mut gpu, canvas) = gpu::init(&make_canvas)
        .await
        .map_err(|e| format!("{} ({e})", labels.no_gpu))?;
    canvas.set_attribute("data-backend", gpu.backend).ok();
    gpu.set_mesh(&mesh);

    let bounds = mesh.bounds();
    let annotations = geometry
        .map(|g| dims::annotations(&info.package, g, bounds))
        .unwrap_or_default();
    let (mut lo, mut hi) = bounds.unwrap_or((Vec3::splat(-1.0), Vec3::splat(1.0)));
    let grid_center = ((lo + hi) * 0.5).truncate();
    let grid_footprint = (hi - lo).truncate().max_element().max(0.5);
    for a in &annotations {
        for p in a.line {
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    let camera = OrbitCamera::fit(lo, hi);
    let snaps = snap::candidates(&mesh, geometry);

    let layer = element("div", "opv-labels");
    let dims: Vec<(Annotation, HtmlElement)> = annotations
        .into_iter()
        .map(|a| {
            let b = element("button", "opv-dim");
            b.set_attribute("type", "button").ok();
            if a.pointer.is_none() {
                b.class_list().add_1("opv-dim-model").ok();
            }
            b.set_inner_html(&format!(
                "<span class=\"opv-dim-name\">{}</span> {}",
                esc(labels.get(a.key)),
                esc(&a.text)
            ));
            layer.append_child(&b).ok();
            (a, b)
        })
        .collect();
    let marker = element("div", "opv-snap");
    let point_marks = [element("div", "opv-point"), element("div", "opv-point")];
    let measure_label = element("div", "opv-measure-label");
    for e in [&marker, &point_marks[0], &point_marks[1], &measure_label] {
        show(e, false);
        layer.append_child(e).ok();
    }
    let toolbar = element("div", "opv-toolbar");
    let dims_button = button(&labels.dimensions, Some(true));
    let measure_button = button(&labels.measure, Some(false));
    let reset_button = button(&labels.reset_view, None);
    for b in [&dims_button, &measure_button, &reset_button] {
        toolbar.append_child(b).ok();
    }
    let readout = element("div", "opv-readout");
    show(&readout, false);
    let popup = element("div", "opv-popup");
    show(&popup, false);
    for e in [&layer, &toolbar, &readout, &popup] {
        root.append_child(e).ok();
    }

    let view = Rc::new(RefCell::new(ModelView {
        root: root.clone(),
        canvas: canvas.clone(),
        gpu: Some(gpu),
        camera,
        home: camera,
        mesh,
        snaps,
        dims,
        dims_button: dims_button.clone(),
        measure_button: measure_button.clone(),
        readout,
        popup: popup.clone(),
        popup_for: None,
        marker,
        point_marks,
        measure_label,
        grid_center,
        grid_footprint,
        show_dims: true,
        measuring: false,
        points: Vec::new(),
        hover: None,
        cursor: None,
        cursor_moved: false,
        pointers: Vec::new(),
        press: None,
        moved: false,
        size: Vec2::ZERO,
        dpr: 0.0,
        dirty: true,
        alive: true,
        raf: None,
        raf_closure: None,
        labels,
        provenance: info.package.provenance.clone(),
        sources: info.sources.clone(),
        listeners: Vec::new(),
    }));
    let weak = Rc::downgrade(&view);
    let c: &EventTarget = canvas.as_ref();
    let mut listeners = vec![
        listen(c, "pointerdown", &weak, ModelView::on_pointer_down),
        listen(c, "pointermove", &weak, ModelView::on_pointer_move),
        listen(c, "pointerup", &weak, ModelView::on_pointer_up),
        listen(c, "pointercancel", &weak, ModelView::on_pointer_up),
        listen(c, "pointerleave", &weak, |v, _| {
            v.cursor = None;
            v.cursor_moved = true;
        }),
        listen(c, "wheel", &weak, ModelView::on_wheel),
        listen(c, "contextmenu", &weak, |_, e| e.prevent_default()),
        listen(window().as_ref(), "keydown", &weak, ModelView::on_key),
        listen(dims_button.as_ref(), "click", &weak, |v, _| {
            v.set_dimensions(!v.show_dims)
        }),
        listen(measure_button.as_ref(), "click", &weak, |v, _| {
            v.set_measure(!v.measuring)
        }),
        listen(reset_button.as_ref(), "click", &weak, |v, _| v.reset_view()),
        listen(popup.as_ref(), "click", &weak, |v, e| {
            let close = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                .is_some_and(|t| t.class_list().contains("opv-popup-close"));
            if close {
                v.close_popup();
            }
        }),
    ];
    {
        let mut v = view.borrow_mut();
        for (i, (_, el)) in v.dims.iter().enumerate() {
            let weak = weak.clone();
            let f = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
                e.stop_propagation();
                if let Some(s) = weak.upgrade() {
                    if let Ok(mut s) = s.try_borrow_mut() {
                        s.toggle_popup(i);
                    }
                }
            });
            el.add_event_listener_with_callback("click", f.as_ref().unchecked_ref())
                .ok();
            listeners.push(Listener {
                target: el.clone().into(),
                kind: "click",
                f,
            });
        }
        v.listeners = listeners;
        let w = weak.clone();
        v.raf_closure = Some(Closure::<dyn FnMut()>::new(move || {
            let Some(s) = w.upgrade() else { return };
            let Ok(mut s) = s.try_borrow_mut() else {
                return;
            };
            s.raf = None;
            s.frame();
            if s.alive {
                s.schedule();
            }
        }));
        v.schedule();
    }
    Ok(view)
}

impl ModelView {
    fn schedule(&mut self) {
        if let Some(f) = &self.raf_closure {
            self.raf = window()
                .request_animation_frame(f.as_ref().unchecked_ref())
                .ok();
        }
    }

    fn local(&self, e: &PointerEvent) -> Vec2 {
        let r = self.canvas.get_bounding_client_rect();
        Vec2::new(
            (e.client_x() as f64 - r.left()) as f32,
            (e.client_y() as f64 - r.top()) as f32,
        )
    }

    fn on_pointer_down(&mut self, e: Event) {
        let e: PointerEvent = e.unchecked_into();
        let p = self.local(&e);
        self.canvas.set_pointer_capture(e.pointer_id()).ok();
        self.pointers.retain(|(id, _)| *id != e.pointer_id());
        self.pointers.push((e.pointer_id(), p));
        if self.pointers.len() == 1 {
            self.press = Some((p, e.button(), e.shift_key() || e.ctrl_key()));
            self.moved = false;
        } else {
            self.moved = true;
        }
        e.prevent_default();
    }

    fn on_pointer_move(&mut self, e: Event) {
        let e: PointerEvent = e.unchecked_into();
        let p = self.local(&e);
        self.cursor = Some(p);
        self.cursor_moved = true;
        let Some(i) = self
            .pointers
            .iter()
            .position(|(id, _)| *id == e.pointer_id())
        else {
            return;
        };
        let before: Vec<Vec2> = self.pointers.iter().map(|x| x.1).collect();
        self.pointers[i].1 = p;
        if self.pointers.len() >= 2 {
            // Pinch: zoom by the spread, pan by the centroid.
            let after: Vec<Vec2> = self.pointers.iter().map(|x| x.1).collect();
            let spread = |v: &[Vec2]| (v[0] - v[1]).length().max(1.0);
            let mid = |v: &[Vec2]| (v[0] + v[1]) * 0.5;
            self.camera.zoom(spread(&before) / spread(&after));
            self.camera.pan(mid(&after) - mid(&before), self.size.y);
            self.dirty = true;
            return;
        }
        let d = p - before[i];
        if let Some((start, button, modifier)) = self.press {
            if (p - start).length() > 3.0 {
                self.moved = true;
            }
            if self.moved {
                if button == 0 && !modifier {
                    self.camera.orbit(d);
                } else {
                    self.camera.pan(d, self.size.y);
                }
                self.dirty = true;
            }
        }
    }

    fn on_pointer_up(&mut self, e: Event) {
        let e: PointerEvent = e.unchecked_into();
        let p = self.local(&e);
        let was_single = self.pointers.len() == 1;
        self.pointers.retain(|(id, _)| *id != e.pointer_id());
        if was_single && e.type_() == "pointerup" && !self.moved && self.measuring {
            if let Some((_, 0, _)) = self.press {
                self.cursor = Some(p);
                self.update_hover();
                if let Some(c) = self.hover {
                    if self.points.len() >= 2 {
                        self.points.clear();
                    }
                    self.points.push(c);
                    self.dirty = true;
                }
            }
        }
        if self.pointers.is_empty() {
            self.press = None;
        }
    }

    fn on_wheel(&mut self, e: Event) {
        let e: web_sys::WheelEvent = e.unchecked_into();
        e.prevent_default();
        let scale = match e.delta_mode() {
            1 => 16.0,
            2 => 400.0,
            _ => 1.0,
        };
        self.camera
            .zoom((e.delta_y() * scale * 0.0015).exp() as f32);
        self.dirty = true;
    }

    fn on_key(&mut self, e: Event) {
        let e: KeyboardEvent = e.unchecked_into();
        if e.key() == "Escape" {
            if !self.points.is_empty() {
                self.points.clear();
                self.dirty = true;
            }
            if self.popup_for.is_some() {
                self.close_popup();
            }
        }
    }

    fn set_dimensions(&mut self, on: bool) {
        self.show_dims = on;
        set_pressed(&self.dims_button, on);
        if !on {
            self.close_popup();
        }
        self.dirty = true;
    }

    fn set_measure(&mut self, on: bool) {
        self.measuring = on;
        set_pressed(&self.measure_button, on);
        self.root
            .class_list()
            .toggle_with_force("opv-measuring", on)
            .ok();
        if !on {
            self.points.clear();
            self.hover = None;
        }
        self.dirty = true;
    }

    fn clear_measure(&mut self) {
        self.points.clear();
        self.dirty = true;
    }

    fn reset_view(&mut self) {
        self.camera = self.home;
        self.dirty = true;
    }

    fn close_popup(&mut self) {
        self.popup_for = None;
        show(&self.popup, false);
    }

    fn toggle_popup(&mut self, i: usize) {
        if self.popup_for == Some(i) {
            self.close_popup();
            return;
        }
        let (a, _) = &self.dims[i];
        let l = &self.labels;
        let mut html = format!(
            "<button type=\"button\" class=\"opv-popup-close\" aria-label=\"{}\">×</button>\
             <div class=\"opv-popup-title\">{} <strong>{}</strong></div>",
            esc(&l.close),
            esc(l.get(a.key)),
            esc(&a.text)
        );
        let entries = a.pointer.as_ref().and_then(|p| self.provenance.get(p));
        match entries {
            Some(entries) if !entries.is_empty() => {
                for entry in entries {
                    let id = entry.source.0.as_str();
                    let (name, url) = self
                        .sources
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| (id.to_string(), String::new()));
                    let link = if url.is_empty() {
                        esc(&name)
                    } else {
                        let href = match entry.page {
                            Some(p) => format!("{url}#page={p}"),
                            None => url,
                        };
                        format!(
                            "<a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\" title=\"{}\">{}</a>",
                            esc(&href),
                            esc(&l.open_source),
                            esc(&name)
                        )
                    };
                    html.push_str(&format!("<dl><dt>{}</dt><dd>{link}</dd>", esc(&l.source)));
                    if let Some(p) = entry.page {
                        html.push_str(&format!("<dt>{}</dt><dd>{p}</dd>", esc(&l.page)));
                    }
                    if let Some(loc) = &entry.locator {
                        html.push_str(&format!(
                            "<dt>{}</dt><dd>{}</dd>",
                            esc(&l.locator),
                            esc(loc)
                        ));
                    }
                    html.push_str("</dl>");
                }
            }
            _ => {
                let note = if a.pointer.is_none() {
                    &l.model_value
                } else {
                    &l.source
                };
                html.push_str(&format!("<p>{}</p>", esc(note)));
            }
        }
        self.popup.set_inner_html(&html);
        self.popup_for = Some(i);
        show(&self.popup, true);
    }

    /// The snap target (or surface point) under the cursor.
    fn update_hover(&mut self) {
        self.cursor_moved = false;
        let Some(cursor) = self.cursor.filter(|_| self.measuring) else {
            self.hover = None;
            return;
        };
        let cam = self.camera;
        let size = self.size;
        let mesh = &self.mesh;
        let eye = cam.eye();
        let tol = cam.radius * 0.01;
        let visible = |p: Vec3| {
            let to = p - eye;
            let dist = to.length();
            match snap::raycast(mesh, eye, to / dist) {
                Some(t) => t >= dist - tol - cam.world_per_pixel(p, size.y) * 3.0,
                None => true,
            }
        };
        let picked = snap::pick(
            &self.snaps,
            cursor,
            12.0,
            |p| cam.project(p, size).map(|x| x.0),
            visible,
        );
        self.hover = picked.or_else(|| {
            let (o, d) = cam.ray(cursor, size);
            snap::raycast(mesh, o, d).map(|t| Candidate {
                point: o + d * t,
                kind: SnapKind::Surface,
            })
        });
    }

    fn place(&self, e: &HtmlElement, p: Vec3) -> bool {
        match self.camera.project(p, self.size) {
            Some((px, _)) => {
                set_style(
                    e,
                    "transform",
                    &format!("translate({}px, {}px)", px.x, px.y),
                );
                true
            }
            None => false,
        }
    }

    fn frame(&mut self) {
        if !self.alive {
            return;
        }
        if !self.root.is_connected() {
            self.teardown();
            return;
        }
        let size = Vec2::new(
            self.canvas.client_width() as f32,
            self.canvas.client_height() as f32,
        );
        let dpr = window().device_pixel_ratio() as f32;
        if size.x < 1.0 || size.y < 1.0 {
            return;
        }
        if size != self.size || dpr != self.dpr {
            let (w, h) = ((size.x * dpr) as u32, (size.y * dpr) as u32);
            self.canvas.set_width(w);
            self.canvas.set_height(h);
            if let Some(g) = &mut self.gpu {
                g.resize(w, h);
                g.set_grid(&gpu::grid_lines(self.grid_center, self.grid_footprint), dpr);
            }
            self.size = size;
            self.dpr = dpr;
            self.dirty = true;
        }
        if self.cursor_moved && self.measuring {
            self.update_hover();
            self.dirty = true;
        }
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let cam = self.camera;
        let mut lines = Vec::new();
        for (a, el) in &self.dims {
            show(el, self.show_dims);
            if !self.show_dims {
                continue;
            }
            let mid = (a.line[0] + a.line[1]) * 0.5;
            let k = cam.world_per_pixel(mid, size.y);
            for [p, q] in a.segments(k * 9.0) {
                lines.push(Line {
                    a: p,
                    b: q,
                    color: DIM_COLOR,
                    width: 1.4,
                });
            }
        }
        // The measurement: chosen points, the line, the rubber band.
        let end = self.points.get(1).copied().or(self.hover);
        if let (Some(a), Some(b)) = (self.points.first(), end) {
            lines.push(Line {
                a: a.point,
                b: b.point,
                color: MEASURE_COLOR,
                width: 2.0,
            });
        }
        if let Some(g) = &mut self.gpu {
            g.set_overlay(&lines, dpr);
            g.render(cam.view_projection(size), cam.eye(), [1.0, 1.0, 1.0]);
        }
        self.layout_labels();
        self.layout_measure();
    }

    /// Labels at their anchors, nudged apart where they would overlap.
    fn layout_labels(&self) {
        if !self.show_dims {
            return;
        }
        let cam = self.camera;
        let mut placed: Vec<(Vec2, Vec2)> = Vec::new();
        for (a, el) in &self.dims {
            let mid = (a.line[0] + a.line[1]) * 0.5;
            let k = cam.world_per_pixel(mid, self.size.y);
            let Some((mut px, _)) = cam.project(a.label_anchor(k * 11.0), self.size) else {
                show(el, false);
                continue;
            };
            let half = Vec2::new(el.offset_width() as f32, el.offset_height() as f32) * 0.5;
            for _ in 0..6 {
                let hit = placed.iter().find(|(c, h)| {
                    (px.x - c.x).abs() < half.x + h.x && (px.y - c.y).abs() < half.y + h.y
                });
                match hit {
                    Some((c, h)) => {
                        let down = px.y >= c.y;
                        px.y = if down {
                            c.y + h.y + half.y + 2.0
                        } else {
                            c.y - h.y - half.y - 2.0
                        };
                    }
                    None => break,
                }
            }
            placed.push((px, half));
            set_style(
                el,
                "transform",
                &format!("translate({}px, {}px) translate(-50%, -50%)", px.x, px.y),
            );
        }
    }

    fn layout_measure(&self) {
        let hover = self.hover.filter(|_| self.measuring);
        show(&self.marker, hover.is_some());
        if let Some(h) = hover {
            self.place(&self.marker, h.point);
            self.marker
                .set_attribute("data-kind", self.labels.get(h.kind.key()))
                .ok();
        }
        for (i, m) in self.point_marks.iter().enumerate() {
            let p = self.points.get(i);
            show(m, p.is_some());
            if let Some(p) = p {
                self.place(m, p.point);
            }
        }
        let end = self.points.get(1).copied().or(hover);
        match (self.points.first(), end) {
            (Some(a), Some(b)) => {
                let (short, long) = measure_text(&self.labels, a.point, b.point, true);
                self.measure_label.set_text_content(Some(&short));
                show(&self.measure_label, true);
                self.place(&self.measure_label, (a.point + b.point) * 0.5);
                self.readout.set_text_content(Some(&long));
                show(&self.readout, true);
            }
            _ => {
                show(&self.measure_label, false);
                if self.measuring {
                    self.readout
                        .set_text_content(Some(&self.labels.measure_hint));
                }
                show(&self.readout, self.measuring);
            }
        }
    }

    /// Stops drawing and releases the canvas, listeners and GPU objects.
    fn teardown(&mut self) {
        self.alive = false;
        if let Some(id) = self.raf.take() {
            window().cancel_animation_frame(id).ok();
        }
        self.listeners.clear();
        self.gpu = None;
        self.mesh = Mesh::default();
        self.snaps.clear();
        self.root.set_inner_html("");
    }
}

// --------------------------------------------------------- footprint view

struct FootprintView {
    root: HtmlElement,
    svg: web_sys::Element,
    dims: web_sys::Element,
    measure: web_sys::Element,
    drawing: Drawing,
    dims_button: HtmlElement,
    measure_button: HtmlElement,
    readout: HtmlElement,
    show_dims: bool,
    measuring: bool,
    points: Vec<FpSnap>,
    hover: Option<FpSnap>,
    labels: Labels,
    listeners: Vec<Listener>,
}

fn mount_footprint(
    root: HtmlElement,
    labels: &Labels,
    geometry: &MechanicalGeometry,
    device: Option<&Device>,
) -> Result<Rc<RefCell<FootprintView>>, String> {
    let mut fp = openparts_pcbcad::build_footprint("", geometry);
    if let Some(d) = device {
        openparts_pcbcad::add_shared_pads(&mut fp, d);
    }
    let drawing = footprint::drawing(&fp, labels);
    root.set_inner_html("");
    root.class_list().add_1("opv-fp").ok();
    let toolbar = element("div", "opv-toolbar");
    let dims_button = button(&labels.dimensions, Some(true));
    let measure_button = button(&labels.measure, Some(false));
    toolbar.append_child(&dims_button).ok();
    toolbar.append_child(&measure_button).ok();
    let holder = element("div", "opv-fp-holder");
    holder.set_inner_html(&drawing.svg);
    let readout = element("div", "opv-readout opv-fp-readout");
    show(&readout, false);
    for e in [&toolbar, &holder, &readout] {
        root.append_child(e).ok();
    }
    let find = |sel: &str| {
        holder
            .query_selector(sel)
            .ok()
            .flatten()
            .ok_or_else(|| labels.footprint_failed.clone())
    };
    let svg = find("svg")?;
    let dims = find(".opv-fp-dims")?;
    let measure = find(".opv-fp-measure")?;
    let view = Rc::new(RefCell::new(FootprintView {
        root,
        svg: svg.clone(),
        dims,
        measure,
        drawing,
        dims_button: dims_button.clone(),
        measure_button: measure_button.clone(),
        readout,
        show_dims: true,
        measuring: false,
        points: Vec::new(),
        hover: None,
        labels: labels.clone(),
        listeners: Vec::new(),
    }));
    let weak = Rc::downgrade(&view);
    let s: &EventTarget = svg.as_ref();
    let listeners = vec![
        listen(s, "pointermove", &weak, FootprintView::on_move),
        listen(s, "pointerleave", &weak, |v, _| {
            v.hover = None;
            v.redraw();
        }),
        listen(s, "click", &weak, FootprintView::on_click),
        listen(window().as_ref(), "keydown", &weak, |v, e| {
            let e: KeyboardEvent = e.unchecked_into();
            if e.key() == "Escape" {
                v.clear_measure();
            }
        }),
        listen(dims_button.as_ref(), "click", &weak, |v, _| {
            v.set_dimensions(!v.show_dims)
        }),
        listen(measure_button.as_ref(), "click", &weak, |v, _| {
            v.set_measure(!v.measuring)
        }),
    ];
    view.borrow_mut().listeners = listeners;
    Ok(view)
}

impl FootprintView {
    /// The event's position in drawing units (mm, Y down) and how many
    /// of those one CSS pixel is.
    fn to_drawing(&self, e: &web_sys::MouseEvent) -> (P, f64) {
        let r = self.svg.get_bounding_client_rect();
        let [vx, vy, vw, vh] = self.drawing.view;
        let scale = (r.width() / vw).min(r.height() / vh).max(1e-9);
        let ox = (r.width() - vw * scale) * 0.5;
        let oy = (r.height() - vh * scale) * 0.5;
        let x = vx + (e.client_x() as f64 - r.left() - ox) / scale;
        let y = vy + (e.client_y() as f64 - r.top() - oy) / scale;
        (P { x, y }, 1.0 / scale)
    }

    fn at(&self, e: &Event) -> Option<FpSnap> {
        let e: &web_sys::MouseEvent = e.unchecked_ref();
        let (p, per_px) = self.to_drawing(e);
        Some(
            footprint::pick(&self.drawing.snaps, p, per_px * 12.0).unwrap_or(FpSnap {
                at: p,
                kind: SnapKind::Surface,
            }),
        )
    }

    fn on_move(&mut self, e: Event) {
        if !self.measuring {
            return;
        }
        self.hover = self.at(&e);
        self.redraw();
    }

    fn on_click(&mut self, e: Event) {
        if !self.measuring {
            return;
        }
        if let Some(s) = self.at(&e) {
            if self.points.len() >= 2 {
                self.points.clear();
            }
            self.points.push(s);
            self.redraw();
        }
    }

    fn set_dimensions(&mut self, on: bool) {
        self.show_dims = on;
        set_pressed(&self.dims_button, on);
        let _ = self
            .dims
            .set_attribute("display", if on { "inline" } else { "none" });
    }

    fn set_measure(&mut self, on: bool) {
        self.measuring = on;
        set_pressed(&self.measure_button, on);
        self.root
            .class_list()
            .toggle_with_force("opv-measuring", on)
            .ok();
        if !on {
            self.points.clear();
            self.hover = None;
        }
        self.redraw();
    }

    fn clear_measure(&mut self) {
        if !self.points.is_empty() {
            self.points.clear();
            self.redraw();
        }
    }

    fn redraw(&mut self) {
        let a = self.points.first().map(|s| s.at);
        let b = self
            .points
            .get(1)
            .map(|s| s.at)
            .or(self.hover.map(|h| h.at));
        let hover = self.hover.map(|h| h.at);
        // Y down in the drawing: the board's ΔY is the negated one.
        let text = match (a, b) {
            (Some(a), Some(b)) => Some(measure_text(
                &self.labels,
                Vec3::new(a.x as f32, -a.y as f32, 0.0),
                Vec3::new(b.x as f32, -b.y as f32, 0.0),
                false,
            )),
            _ => None,
        };
        let short = text.as_ref().map(|t| t.0.clone()).unwrap_or_default();
        let b_drawn = if a.is_some() { b } else { None };
        self.measure.set_inner_html(&footprint::measure_svg(
            a,
            b_drawn,
            hover,
            self.drawing.font,
            &short,
        ));
        match (&text, self.measuring) {
            (Some(t), _) => {
                self.readout.set_text_content(Some(&t.1));
                show(&self.readout, true);
            }
            (None, true) => {
                self.readout
                    .set_text_content(Some(&self.labels.measure_hint));
                show(&self.readout, true);
            }
            (None, false) => show(&self.readout, false),
        }
    }

    fn teardown(&mut self) {
        self.listeners.clear();
        self.root.set_inner_html("");
    }
}

// ---------------------------------------------------------------- handle

/// The mounted views. `destroy()` tears both down (also done on its own
/// once the 3D view's element leaves the page).
#[wasm_bindgen]
pub struct Viewer {
    model: Option<Rc<RefCell<ModelView>>>,
    footprint: Option<Rc<RefCell<FootprintView>>>,
}

#[wasm_bindgen]
impl Viewer {
    pub fn destroy(&mut self) {
        if let Some(m) = self.model.take() {
            let closure = {
                let mut m = m.borrow_mut();
                m.teardown();
                m.raf_closure.take()
            };
            drop(closure);
        }
        if let Some(f) = self.footprint.take() {
            f.borrow_mut().teardown();
        }
    }

    /// Which graphics API the 3D view runs on ("WebGPU", "WebGL2"), or
    /// "" when it isn't shown.
    #[wasm_bindgen(getter)]
    pub fn backend(&self) -> String {
        self.model
            .as_ref()
            .and_then(|m| m.borrow().gpu.as_ref().map(|g| g.backend.to_string()))
            .unwrap_or_default()
    }

    #[wasm_bindgen(js_name = setDimensions)]
    pub fn set_dimensions(&self, on: bool) {
        if let Some(m) = &self.model {
            m.borrow_mut().set_dimensions(on);
        }
        if let Some(f) = &self.footprint {
            f.borrow_mut().set_dimensions(on);
        }
    }

    #[wasm_bindgen(js_name = setMeasure)]
    pub fn set_measure(&self, on: bool) {
        if let Some(m) = &self.model {
            m.borrow_mut().set_measure(on);
        }
        if let Some(f) = &self.footprint {
            f.borrow_mut().set_measure(on);
        }
    }

    #[wasm_bindgen(js_name = clearMeasure)]
    pub fn clear_measure(&self) {
        if let Some(m) = &self.model {
            m.borrow_mut().clear_measure();
        }
        if let Some(f) = &self.footprint {
            f.borrow_mut().clear_measure();
        }
    }

    #[wasm_bindgen(js_name = resetView)]
    pub fn reset_view(&self) {
        if let Some(m) = &self.model {
            m.borrow_mut().reset_view();
        }
    }

    /// The dimensions on the 3D view, as JSON (key, text, pointer): for
    /// tests and tooling.
    #[wasm_bindgen(js_name = dimensionsJson)]
    pub fn dimensions_json(&self) -> String {
        let Some(m) = &self.model else {
            return "[]".into();
        };
        let items: Vec<String> = m
            .borrow()
            .dims
            .iter()
            .map(|(a, _)| {
                format!(
                    "{{\"key\":{:?},\"text\":{:?},\"pointer\":{}}}",
                    a.key,
                    a.text,
                    a.pointer
                        .as_ref()
                        .map(|p| format!("{p:?}"))
                        .unwrap_or_else(|| "null".into())
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }
}

/// Shows `options.manufacturer`/`options.mpn` in `model` (the 3D view)
/// and `footprint` (the footprint view, optional). `options` is JSON:
/// `{"api": "/api", "manufacturer", "mpn", "labels": {...}}`. A view
/// that can't be shown says why in its element; the promise rejects
/// only when the part itself can't be loaded.
#[wasm_bindgen]
pub async fn mount(
    model: HtmlElement,
    footprint: Option<HtmlElement>,
    options: String,
) -> Result<Viewer, JsError> {
    let o: Options = serde_json::from_str(&options).map_err(|e| JsError::new(&e.to_string()))?;
    let info = load_part(&o).await.map_err(|e| JsError::new(&e))?;
    let geometry = openparts_mcad::generate(&info.package).ok();
    let message = |el: &HtmlElement, text: &str| {
        el.set_inner_html("");
        let p = element("p", "opv-message");
        p.set_text_content(Some(text));
        el.append_child(&p).ok();
    };
    let footprint = match (footprint, &geometry) {
        (Some(el), Some(g)) => {
            match mount_footprint(el.clone(), &o.labels, g, info.device.as_ref()) {
                Ok(v) => Some(v),
                Err(e) => {
                    message(&el, &e);
                    None
                }
            }
        }
        (Some(el), None) => {
            message(&el, &o.labels.footprint_failed);
            None
        }
        _ => None,
    };
    let model = match mount_model(model.clone(), &o, &info, geometry.as_ref()).await {
        Ok(v) => Some(v),
        Err(e) => {
            message(&model, &e);
            None
        }
    };
    Ok(Viewer { model, footprint })
}
