//! The 3D view: the model drawn with wgpu, the CAD-style navigation
//! (orbit about the point under the cursor, zoom toward it, pan, standard
//! views, fit, a navigation gizmo, smooth transitions), the datasheet
//! dimensions and the axis-snapping measure tool.

use super::gpu::{self, Gpu, Layer, Line};
use super::{
    button, element, listen, measure_text, part_path, set_pressed, set_style, show, typing, window,
    Listener, Options, PartInfo,
};
use crate::camera::{OrbitCamera, StandardView};
use crate::dims::{self, Annotation};
use crate::footprint::esc;
use crate::format::number;
use crate::gizmo;
use crate::labels::Labels;
use crate::measure::{self, Constrained, Lock, AXES, AXIS_COLORS, AXIS_CSS};
use crate::mesh::{self, Mesh};
use crate::snap::{self, Candidate, SnapKind};
use glam::{Vec2, Vec3};
use openparts_core::ProvenanceMap;
use openparts_mcad::MechanicalGeometry;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Event, EventTarget, HtmlCanvasElement, HtmlElement, KeyboardEvent, PointerEvent};

const DIM_COLOR: [f32; 4] = [0.012, 0.16, 0.65, 1.0];
const GHOST_ALPHA: f32 = 0.22;
const FREE_COLOR: [f32; 4] = [0.85, 0.12, 0.005, 1.0];
const GUIDE_GREY: [f32; 4] = [0.25, 0.25, 0.28, 0.8];
const ANIMATION_MS: f64 = 380.0;

fn now() -> f64 {
    window().performance().map(|p| p.now()).unwrap_or(0.0)
}

fn rgba(c: [f32; 3], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], a]
}

/// What a pointer drag does.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    Orbit(Vec3),
    Pan,
}

/// A press on the gizmo: where, the ball (axis, positive) it began on,
/// and whether it has moved (then it orbits).
type GizmoPress = (Vec2, Option<(usize, bool)>, bool);

/// The measurement: the first point, then the (constrained) second.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Measurement {
    a: Candidate,
    b: Option<(Constrained, Candidate)>,
}

pub(crate) struct ModelView {
    root: HtmlElement,
    canvas: HtmlCanvasElement,
    pub(crate) gpu: Option<Gpu>,
    camera: OrbitCamera,
    animation: Option<(OrbitCamera, OrbitCamera, f64)>,
    mesh: Mesh,
    bounds: (Vec3, Vec3),
    snaps: Vec<Candidate>,
    pub(crate) dims: Vec<(Annotation, HtmlElement)>,
    leaders: web_sys::Element,
    dims_button: HtmlElement,
    measure_button: HtmlElement,
    projection_button: HtmlElement,
    toolbar: HtmlElement,
    nav: HtmlElement,
    gizmo: web_sys::Element,
    gizmo_press: Option<GizmoPress>,
    readout: HtmlElement,
    coords: HtmlElement,
    popup: HtmlElement,
    popup_for: Option<usize>,
    marker: HtmlElement,
    pivot_mark: HtmlElement,
    point_marks: [HtmlElement; 2],
    measure_label: HtmlElement,
    grid_center: Vec2,
    grid_footprint: f32,
    show_dims: bool,
    measuring: bool,
    measurement: Option<Measurement>,
    lock: Lock,
    shift: bool,
    hover: Option<Candidate>,
    cursor: Option<Vec2>,
    cursor_moved: bool,
    pointers: Vec<(i32, Vec2)>,
    press: Option<(Vec2, Drag)>,
    moved: bool,
    active: bool,
    size: Vec2,
    dpr: f32,
    fitted: bool,
    dirty: bool,
    alive: bool,
    raf: Option<i32>,
    pub(crate) raf_closure: Option<Closure<dyn FnMut()>>,
    labels: Labels,
    provenance: ProvenanceMap,
    sources: HashMap<String, (String, String)>,
    listeners: Vec<Listener>,
}

/// Lays out the 3D view in `root`: canvas, label layer, toolbar,
/// navigation gizmo, readouts and the provenance popup.
pub(crate) async fn mount_model(
    root: HtmlElement,
    o: &Options,
    info: &PartInfo,
    geometry: Option<&MechanicalGeometry>,
) -> Result<Rc<RefCell<ModelView>>, String> {
    let labels = o.labels.clone();
    root.set_inner_html("");
    root.class_list().add_1("opv-root").ok();
    let base = part_path(o);
    let mesh = match super::fetch_bytes(&format!("{base}/artifacts/3mf"))
        .await
        .and_then(|b| mesh::parse_3mf(&b))
    {
        Ok(m) => m,
        Err(_) => super::fetch_bytes(&format!("{base}/artifacts/stl"))
            .await
            .and_then(|b| mesh::parse_stl(&b))
            .map_err(|_| labels.model_failed.clone())?,
    };
    let make_canvas = || {
        let c: HtmlCanvasElement = element("canvas", "opv-canvas").unchecked_into();
        if let Some(old) = root.query_selector("canvas").ok().flatten() {
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
    canvas.set_attribute("tabindex", "0").ok();
    gpu.set_mesh(&mesh);

    let bounds = mesh
        .bounds()
        .unwrap_or((Vec3::splat(-1.0), Vec3::splat(1.0)));
    let annotations = geometry
        .map(|g| dims::annotations(&info.package, g, Some(bounds)))
        .unwrap_or_default();
    let grid_center = ((bounds.0 + bounds.1) * 0.5).truncate();
    let grid_footprint = (bounds.1 - bounds.0).truncate().max_element().max(0.5);
    let camera = OrbitCamera::fit(bounds.0, bounds.1);
    let snaps = snap::candidates(&mesh, geometry);

    let layer = element("div", "opv-labels");
    let leaders = super::document()
        .create_element_ns(Some("http://www.w3.org/2000/svg"), "svg")
        .map_err(|_| labels.model_failed.clone())?;
    leaders.set_attribute("class", "opv-leaders").ok();
    layer.append_child(&leaders).ok();
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
    let pivot_mark = element("div", "opv-pivot");
    let point_marks = [element("div", "opv-point"), element("div", "opv-point")];
    let measure_label = element("div", "opv-measure-label");
    for e in [
        &marker,
        &pivot_mark,
        &point_marks[0],
        &point_marks[1],
        &measure_label,
    ] {
        show(e, false);
        layer.append_child(e).ok();
    }
    let toolbar = element("div", "opv-toolbar");
    let dims_button = button(&labels.dimensions, Some(true));
    let measure_button = button(&labels.measure, Some(false));
    let fit_button = button(&labels.fit, None);
    fit_button.set_title(&format!("{} (F)", labels.fit));
    let projection_button = button(&labels.orthographic, Some(false));
    projection_button.set_title("P");
    for b in [
        &dims_button,
        &measure_button,
        &fit_button,
        &projection_button,
    ] {
        toolbar.append_child(b).ok();
    }
    // The navigation gizmo and the home button, top right.
    let nav = element("div", "opv-nav");
    let home_button = element("button", "opv-button opv-home");
    home_button.set_attribute("type", "button").ok();
    home_button.set_title(&format!("{} (0)", labels.home));
    home_button.set_attribute("aria-label", &labels.home).ok();
    home_button.set_inner_html(
        "<svg viewBox=\"0 0 24 24\" width=\"18\" height=\"18\" aria-hidden=\"true\"><path d=\"M3 11 12 3l9 8M5 9.5V21h5v-6h4v6h5V9.5\" \
         fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linejoin=\"round\"/></svg>",
    );
    let gizmo = super::document()
        .create_element_ns(Some("http://www.w3.org/2000/svg"), "svg")
        .map_err(|_| labels.model_failed.clone())?;
    let half = gizmo::HALF;
    gizmo.set_attribute("class", "opv-gizmo").ok();
    gizmo
        .set_attribute(
            "viewBox",
            &format!("{} {} {} {}", -half, -half, 2.0 * half, 2.0 * half),
        )
        .ok();
    nav.append_child(&home_button).ok();
    nav.append_child(&gizmo).ok();
    let readout = element("div", "opv-readout");
    show(&readout, false);
    let coords = element("div", "opv-coords");
    show(&coords, false);
    let popup = element("div", "opv-popup");
    show(&popup, false);
    for e in [&layer, &toolbar, &nav, &readout, &coords, &popup] {
        root.append_child(e).ok();
    }

    let view = Rc::new(RefCell::new(ModelView {
        root: root.clone(),
        canvas: canvas.clone(),
        gpu: Some(gpu),
        camera,
        animation: None,
        mesh,
        bounds,
        snaps,
        dims,
        leaders,
        dims_button: dims_button.clone(),
        measure_button: measure_button.clone(),
        projection_button: projection_button.clone(),
        toolbar,
        nav,
        gizmo: gizmo.clone(),
        gizmo_press: None,
        readout,
        coords,
        popup: popup.clone(),
        popup_for: None,
        marker,
        pivot_mark,
        point_marks,
        measure_label,
        grid_center,
        grid_footprint,
        show_dims: true,
        measuring: false,
        measurement: None,
        lock: Lock::Auto,
        shift: false,
        hover: None,
        cursor: None,
        cursor_moved: false,
        pointers: Vec::new(),
        press: None,
        moved: false,
        active: true,
        size: Vec2::ZERO,
        dpr: 0.0,
        fitted: false,
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
    let g: &EventTarget = gizmo.as_ref();
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
        listen(c, "dblclick", &weak, ModelView::on_double_click),
        listen(c, "contextmenu", &weak, |_, e| e.prevent_default()),
        listen(g, "pointerdown", &weak, ModelView::on_gizmo_down),
        listen(g, "pointermove", &weak, ModelView::on_gizmo_move),
        listen(g, "pointerup", &weak, ModelView::on_gizmo_up),
        listen(window().as_ref(), "keydown", &weak, ModelView::on_key),
        listen(window().as_ref(), "keyup", &weak, |v, e| {
            let e: KeyboardEvent = e.unchecked_into();
            if e.key() == "Shift" {
                v.set_shift(false);
            }
        }),
        listen(window().as_ref(), "pointerdown", &weak, |v, e| {
            v.active = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                .is_some_and(|t| v.root.contains(Some(&t)));
        }),
        listen(dims_button.as_ref(), "click", &weak, |v, _| {
            v.set_dimensions(!v.show_dims)
        }),
        listen(measure_button.as_ref(), "click", &weak, |v, _| {
            v.set_measure(!v.measuring)
        }),
        listen(fit_button.as_ref(), "click", &weak, |v, _| v.fit(true)),
        listen(projection_button.as_ref(), "click", &weak, |v, _| {
            v.toggle_projection()
        }),
        listen(home_button.as_ref(), "click", &weak, |v, _| v.home()),
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

    fn local(&self, e: &web_sys::MouseEvent) -> Vec2 {
        let r = self.canvas.get_bounding_client_rect();
        Vec2::new(
            (e.client_x() as f64 - r.left()) as f32,
            (e.client_y() as f64 - r.top()) as f32,
        )
    }

    /// The model point under `px`, if any.
    fn hit(&self, px: Vec2) -> Option<Vec3> {
        let (o, d) = self.camera.ray(px, self.size);
        snap::raycast(&self.mesh, o, d).map(|t| o + d * t)
    }

    /// The point a zoom or orbit at `px` centres on: the model under the
    /// cursor, else the target plane.
    fn anchor(&self, px: Vec2) -> Vec3 {
        self.hit(px)
            .unwrap_or_else(|| self.camera.on_target_plane(px, self.size))
    }

    fn stop_animation(&mut self) {
        if let Some((_, to, _)) = self.animation.take() {
            // Land where the transition was going, keeping what the user
            // started from close to it.
            self.camera.orthographic = to.orthographic;
        }
    }

    fn animate_to(&mut self, to: OrbitCamera) {
        self.animation = Some((self.camera, to, now()));
        self.dirty = true;
    }

    /// The box fit frames: the model, and the dimension lines when shown.
    fn fit_bounds(&self) -> (Vec3, Vec3) {
        let (mut lo, mut hi) = self.bounds;
        if self.show_dims {
            for (a, _) in &self.dims {
                for p in a.line {
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
            }
        }
        (lo, hi)
    }

    fn fitted(&self, mut cam: OrbitCamera) -> OrbitCamera {
        let (lo, hi) = self.fit_bounds();
        let aspect = if self.size.y > 0.0 {
            self.size.x / self.size.y
        } else {
            1.6
        };
        cam.fit_box_aspect(lo, hi, 1.15, aspect);
        cam
    }

    pub(crate) fn fit(&mut self, animate: bool) {
        let to = self.fitted(self.target_camera());
        if animate {
            self.animate_to(to);
        } else {
            self.camera = to;
            self.dirty = true;
        }
    }

    /// Where the camera is (or is heading, mid-transition).
    fn target_camera(&self) -> OrbitCamera {
        self.animation.map(|a| a.1).unwrap_or(self.camera)
    }

    pub(crate) fn view(&mut self, v: StandardView) {
        let mut to = self.target_camera();
        to.set_view(v);
        let to = self.fitted(to);
        self.animate_to(to);
    }

    pub(crate) fn home(&mut self) {
        self.view(StandardView::Iso);
    }

    fn toggle_projection(&mut self) {
        self.stop_animation();
        // Keep the model about as large: a perspective eye backs off by
        // the model's depth, which orthographic doesn't need.
        let depth = (self.bounds.1 - self.bounds.0).length() * 0.5;
        if self.camera.orthographic {
            self.camera.distance += depth;
        } else {
            self.camera.distance = (self.camera.distance - depth).max(self.camera.distance * 0.2);
        }
        self.camera.orthographic = !self.camera.orthographic;
        set_pressed(&self.projection_button, !self.camera.orthographic);
        self.projection_button
            .set_text_content(Some(if self.camera.orthographic {
                &self.labels.orthographic
            } else {
                &self.labels.perspective
            }));
        self.dirty = true;
    }

    fn on_pointer_down(&mut self, e: Event) {
        let e: PointerEvent = e.unchecked_into();
        let p = self.local(&e);
        self.canvas.set_pointer_capture(e.pointer_id()).ok();
        self.pointers.retain(|(id, _)| *id != e.pointer_id());
        self.pointers.push((e.pointer_id(), p));
        self.stop_animation();
        if self.pointers.len() == 1 {
            let pan = e.button() == 1 || e.button() == 2 || (e.button() == 0 && e.shift_key());
            let drag = if pan {
                Drag::Pan
            } else {
                Drag::Orbit(self.anchor(p))
            };
            self.press = Some((p, drag));
            self.moved = false;
        } else {
            self.moved = true;
            self.press = None;
            show(&self.pivot_mark, false);
        }
        e.prevent_default();
    }

    fn on_pointer_move(&mut self, e: Event) {
        let e: PointerEvent = e.unchecked_into();
        let p = self.local(&e);
        self.cursor = Some(p);
        self.cursor_moved = true;
        if e.shift_key() != self.shift {
            self.set_shift(e.shift_key());
        }
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
            // Two fingers: pinch zooms about their middle, moving pans.
            let after: Vec<Vec2> = self.pointers.iter().map(|x| x.1).collect();
            let spread = |v: &[Vec2]| (v[0] - v[1]).length().max(1.0);
            let mid = |v: &[Vec2]| (v[0] + v[1]) * 0.5;
            let at = self.camera.on_target_plane(mid(&after), self.size);
            self.camera.zoom_at(spread(&before) / spread(&after), at);
            self.camera.pan(mid(&after) - mid(&before), self.size.y);
            self.dirty = true;
            return;
        }
        let d = p - before[i];
        if let Some((start, drag)) = self.press {
            if (p - start).length() > 3.0 {
                self.moved = true;
            }
            if self.moved {
                match drag {
                    Drag::Orbit(pivot) => {
                        self.camera.orbit_about(d, pivot);
                        show(&self.pivot_mark, true);
                    }
                    Drag::Pan => self.camera.pan(d, self.size.y),
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
        let clicked = was_single && e.type_() == "pointerup" && !self.moved;
        if clicked && self.measuring && e.button() == 0 {
            self.cursor = Some(p);
            self.shift = e.shift_key();
            self.update_hover();
            self.click_measure();
        }
        if self.pointers.is_empty() {
            self.press = None;
            show(&self.pivot_mark, false);
            self.dirty = true;
        }
    }

    fn on_wheel(&mut self, e: Event) {
        let e: web_sys::WheelEvent = e.unchecked_into();
        e.prevent_default();
        self.stop_animation();
        let scale = match e.delta_mode() {
            1 => 16.0,
            2 => 400.0,
            _ => 1.0,
        };
        let at = self.anchor(self.local(&e));
        self.camera
            .zoom_at((e.delta_y() * scale * 0.0015).exp() as f32, at);
        self.dirty = true;
    }

    /// Double click: turn about and close in on the point clicked.
    fn on_double_click(&mut self, e: Event) {
        if self.measuring {
            return;
        }
        let e: web_sys::MouseEvent = e.unchecked_into();
        if let Some(p) = self.hit(self.local(&e)) {
            let mut to = self.camera;
            to.target = p;
            to.distance *= 0.6;
            self.animate_to(to);
        }
    }

    fn gizmo_point(&self, e: &web_sys::MouseEvent) -> Vec2 {
        Vec2::new(e.client_x() as f32, e.client_y() as f32)
    }

    fn on_gizmo_down(&mut self, e: Event) {
        let e: PointerEvent = e.unchecked_into();
        e.prevent_default();
        e.stop_propagation();
        self.gizmo.set_pointer_capture(e.pointer_id()).ok();
        let ball = e
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|t| t.closest(".opv-gizmo-ball").ok().flatten())
            .and_then(|b| {
                let axis = b.get_attribute("data-axis")?.parse().ok()?;
                Some((axis, b.get_attribute("data-positive")? == "1"))
            });
        self.stop_animation();
        self.gizmo_press = Some((self.gizmo_point(&e), ball, false));
    }

    fn on_gizmo_move(&mut self, e: Event) {
        let e: PointerEvent = e.unchecked_into();
        let p = self.gizmo_point(&e);
        if let Some((last, ball, moved)) = self.gizmo_press {
            let d = p - last;
            if moved || d.length() > 3.0 {
                self.camera.orbit_about(d, self.camera.target);
                self.gizmo_press = Some((p, ball, true));
                self.dirty = true;
            }
        }
    }

    fn on_gizmo_up(&mut self, e: Event) {
        let _ = e;
        if let Some((_, Some((axis, positive)), false)) = self.gizmo_press {
            self.view(StandardView::from_axis(axis, positive));
        }
        self.gizmo_press = None;
    }

    fn on_key(&mut self, e: Event) {
        let e: KeyboardEvent = e.unchecked_into();
        if typing(&e) {
            return;
        }
        let key = e.key();
        if key == "Shift" {
            self.set_shift(true);
            return;
        }
        if key == "Escape" {
            if self.measurement.is_some() || self.lock != Lock::Auto {
                self.measurement = None;
                self.lock = Lock::Auto;
                self.dirty = true;
            }
            if self.popup_for.is_some() {
                self.close_popup();
            }
            return;
        }
        if !self.active || e.meta_key() || e.alt_key() {
            return;
        }
        if e.ctrl_key() && !matches!(key.as_str(), "1" | "3" | "7") {
            return;
        }
        let other = e.ctrl_key();
        match key.as_str() {
            "f" | "F" => self.fit(true),
            "p" | "P" => self.toggle_projection(),
            "0" => self.home(),
            "1" => self.view(if other {
                StandardView::Back
            } else {
                StandardView::Front
            }),
            "3" => self.view(if other {
                StandardView::Left
            } else {
                StandardView::Right
            }),
            "7" => self.view(if other {
                StandardView::Bottom
            } else {
                StandardView::Top
            }),
            "x" | "X" | "y" | "Y" | "z" | "Z" if self.measuring => {
                let i = match key.to_ascii_lowercase().as_str() {
                    "x" => 0,
                    "y" => 1,
                    _ => 2,
                };
                self.lock = if self.lock == Lock::Axis(i) {
                    Lock::Auto
                } else {
                    Lock::Axis(i)
                };
                self.cursor_moved = true;
            }
            _ => return,
        }
        e.prevent_default();
    }

    fn set_shift(&mut self, on: bool) {
        self.shift = on;
        self.cursor_moved = true;
    }

    pub(crate) fn set_dimensions(&mut self, on: bool) {
        self.show_dims = on;
        set_pressed(&self.dims_button, on);
        if !on {
            self.close_popup();
        }
        self.dirty = true;
    }

    pub(crate) fn set_measure(&mut self, on: bool) {
        self.measuring = on;
        set_pressed(&self.measure_button, on);
        self.root
            .class_list()
            .toggle_with_force("opv-measuring", on)
            .ok();
        if !on {
            self.measurement = None;
            self.lock = Lock::Auto;
        }
        self.cursor_moved = true;
        self.dirty = true;
    }

    pub(crate) fn clear_measure(&mut self) {
        self.measurement = None;
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
                            None => url.clone(),
                        };
                        // data-op-*: the site's datasheet viewer
                        // (web/src/datasheet.js) opens the cited page
                        // with the locator highlighted.
                        let page = entry.page.map_or(String::new(), |p| p.to_string());
                        format!(
                            "<a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\" title=\"{}\" \
                             data-op-source=\"{}\" data-op-page=\"{}\" data-op-locator=\"{}\" \
                             data-op-url=\"{}\" data-op-title=\"{}\" data-op-label=\"{}\" data-op-value=\"{}\">{}</a>",
                            esc(&href),
                            esc(&l.open_source),
                            esc(id),
                            esc(&page),
                            esc(entry.locator.as_deref().unwrap_or("")),
                            esc(&url),
                            esc(&name),
                            esc(&format!("{} {}", l.get(a.key), a.text)),
                            esc(&a.text),
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

    fn effective_lock(&self) -> Lock {
        match self.lock {
            Lock::Auto if self.shift => Lock::Free,
            l => l,
        }
    }

    /// The snap target (or surface point) under the cursor.
    fn update_hover(&mut self) {
        self.cursor_moved = false;
        let Some(cursor) = self.cursor else {
            self.hover = None;
            return;
        };
        if !self.measuring {
            self.hover = self.hit(cursor).map(|p| Candidate {
                point: p,
                kind: SnapKind::Surface,
            });
            return;
        }
        let cam = self.camera;
        let size = self.size;
        let mesh = &self.mesh;
        let eye_ray = |p: Vec3| -> (Vec3, Vec3) {
            if cam.orthographic {
                let f = cam.basis().2;
                (p - f * cam.radius * 50.0, f)
            } else {
                let e = cam.eye();
                (e, (p - e).normalize())
            }
        };
        let tol = cam.radius * 0.01;
        let visible = |p: Vec3| {
            let (o, d) = eye_ray(p);
            let dist = (p - o).dot(d);
            match snap::raycast(mesh, o, d) {
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
            self.hit(cursor).map(|p| Candidate {
                point: p,
                kind: SnapKind::Surface,
            })
        });
    }

    /// The second point as it would land now: the hover point held to an
    /// axis (or the axis line under the cursor when locked and off the
    /// model).
    fn pending_end(&self, a: Vec3) -> Option<(Constrained, Candidate)> {
        let lock = self.effective_lock();
        let cam = self.camera;
        let size = self.size;
        let project = |p: Vec3| cam.project(p, size).map(|x| x.0);
        let snapped = self.hover.filter(|h| h.kind != SnapKind::Surface);
        match (self.hover, lock) {
            // A lock follows the cursor along the axis, unless it is on a
            // feature (then that feature's projection onto the axis).
            (_, Lock::Axis(i)) if snapped.is_none() => {
                let (o, d) = cam.ray(self.cursor?, size);
                let end = measure::closest_on_line(a, AXES[i], o, d);
                let c = Candidate {
                    point: end,
                    kind: SnapKind::Surface,
                };
                Some((Constrained { end, axis: Some(i) }, c))
            }
            (Some(h), _) => Some((measure::constrain(a, h.point, lock, 3, project), h)),
            _ => None,
        }
    }

    fn click_measure(&mut self) {
        match self.measurement {
            Some(Measurement { a, b: None }) => {
                if let Some(end) = self.pending_end(a.point) {
                    self.measurement = Some(Measurement { a, b: Some(end) });
                }
            }
            _ => {
                if let Some(h) = self.hover {
                    self.measurement = Some(Measurement { a: h, b: None });
                    self.lock = Lock::Auto;
                }
            }
        }
        self.dirty = true;
    }

    /// The measurement to draw: first point, second (constrained) and
    /// the point it came from.
    fn shown_measurement(&self) -> Option<(Vec3, Option<(Constrained, Vec3)>)> {
        let m = self.measurement?;
        let b = match m.b {
            Some((c, h)) => Some((c, h.point)),
            None if self.measuring => self.pending_end(m.a.point).map(|(c, h)| (c, h.point)),
            None => None,
        };
        Some((m.a.point, b))
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
                g.set_lines(
                    Layer::Grid,
                    &gpu::grid_lines(self.grid_center, self.grid_footprint),
                    dpr,
                );
            }
            self.size = size;
            self.dpr = dpr;
            self.dirty = true;
        }
        if !self.fitted {
            // Fit on load, now the viewport's shape is known.
            self.fitted = true;
            self.fit(false);
        }
        if let Some((from, to, start)) = self.animation {
            let t = ((now() - start) / ANIMATION_MS).min(1.0) as f32;
            self.camera = from.lerp(&to, t);
            if t >= 1.0 {
                self.camera = to;
                self.animation = None;
            }
            self.dirty = true;
            self.cursor_moved = true;
        }
        if self.cursor_moved {
            self.update_hover();
            self.dirty = true;
        }
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let cam = self.camera;
        let mut dims = Vec::new();
        let mut ghost = Vec::new();
        for (a, _) in &self.dims {
            if !self.show_dims {
                break;
            }
            let mid = (a.line[0] + a.line[1]) * 0.5;
            let k = cam.world_per_pixel(mid, size.y);
            for [p, q] in a.segments(k * 9.0) {
                let line = Line {
                    a: p,
                    b: q,
                    color: DIM_COLOR,
                    width: 1.4,
                };
                ghost.push(Line {
                    color: [DIM_COLOR[0], DIM_COLOR[1], DIM_COLOR[2], GHOST_ALPHA],
                    ..line
                });
                dims.push(line);
            }
        }
        let overlay = self.measure_lines();
        if let Some(g) = &mut self.gpu {
            g.set_lines(Layer::Ghost, &ghost, dpr);
            g.set_lines(Layer::Dims, &dims, dpr);
            g.set_lines(Layer::Overlay, &overlay, dpr);
            g.render(cam.view_projection(size), cam.eye(), [1.0, 1.0, 1.0]);
        }
        self.gizmo
            .set_inner_html(&gizmo::svg(&cam, &self.gizmo_titles()));
        self.layout_labels();
        self.layout_measure();
    }

    fn gizmo_titles(&self) -> [[String; 2]; 3] {
        let t = |axis: usize, positive: bool| {
            let v = StandardView::from_axis(axis, positive);
            format!(
                "{}{} — {}",
                if positive { "+" } else { "−" },
                ["X", "Y", "Z"][axis],
                self.labels.get(v.key())
            )
        };
        [
            [t(0, false), t(0, true)],
            [t(1, false), t(1, true)],
            [t(2, false), t(2, true)],
        ]
    }

    /// The measurement's lines: the distance in the axis colour (or the
    /// free colour), a dashed guide along the axis through the first
    /// point, and a dashed tie to the snapped point it was projected from.
    fn measure_lines(&self) -> Vec<Line> {
        let mut lines = Vec::new();
        let Some((a, b)) = self.shown_measurement() else {
            return lines;
        };
        let cam = self.camera;
        let k = cam.world_per_pixel(a, self.size.y);
        if let Some((c, from)) = b {
            let color = c
                .axis
                .map(|i| rgba(AXIS_COLORS[i], 1.0))
                .unwrap_or(FREE_COLOR);
            if let Some(i) = c.axis {
                let reach = cam.radius * 3.0;
                for [p, q] in
                    measure::dashes(a - AXES[i] * reach, a + AXES[i] * reach, k * 7.0, k * 5.0)
                {
                    lines.push(Line {
                        a: p,
                        b: q,
                        color: rgba(AXIS_COLORS[i], 0.55),
                        width: 1.0,
                    });
                }
                if (from - c.end).length() > k * 2.0 {
                    for [p, q] in measure::dashes(c.end, from, k * 4.0, k * 3.0) {
                        lines.push(Line {
                            a: p,
                            b: q,
                            color: GUIDE_GREY,
                            width: 1.0,
                        });
                    }
                }
            }
            lines.push(Line {
                a,
                b: c.end,
                color,
                width: 2.4,
            });
        }
        lines
    }

    /// Labels at their anchors; one that would overlap another (or the
    /// toolbar/gizmo) moves out along its extension direction, with a
    /// leader back to its line.
    fn layout_labels(&self) {
        let mut leaders = String::new();
        if self.show_dims {
            let cam = self.camera;
            let rect = |e: &HtmlElement| {
                let half = Vec2::new(e.offset_width() as f32, e.offset_height() as f32) * 0.5;
                (
                    Vec2::new(e.offset_left() as f32, e.offset_top() as f32) + half,
                    half,
                )
            };
            let mut placed: Vec<(Vec2, Vec2)> = vec![rect(&self.toolbar), rect(&self.nav)];
            for (a, el) in &self.dims {
                let mid = (a.line[0] + a.line[1]) * 0.5;
                let k = cam.world_per_pixel(mid, self.size.y);
                let ends = (
                    cam.project(a.line[0], self.size),
                    cam.project(a.line[1], self.size),
                    cam.project(mid, self.size),
                    cam.project(a.label_anchor(k * 11.0), self.size),
                );
                let (Some((p0, _)), Some((p1, _)), Some((pm, _)), Some((base, _))) = ends else {
                    show(el, false);
                    continue;
                };
                // Seen end on (the height from above): nothing to label.
                if (p1 - p0).length() < 4.0 {
                    show(el, false);
                    continue;
                }
                show(el, true);
                let half = Vec2::new(el.offset_width() as f32, el.offset_height() as f32) * 0.5;
                let out = (base - pm).normalize_or(Vec2::Y);
                let step = half.y * 2.0 + 4.0;
                let size = self.size;
                let free = |c: Vec2| {
                    c.x >= half.x
                        && c.y >= half.y
                        && c.x <= size.x - half.x
                        && c.y <= size.y - half.y
                        && !placed.iter().any(|(q, h)| {
                            (c.x - q.x).abs() < half.x + h.x + 2.0
                                && (c.y - q.y).abs() < half.y + h.y + 2.0
                        })
                };
                let candidates =
                    (0..6)
                        .map(|n| base + out * step * n as f32)
                        .chain((1..4).flat_map(|n| {
                            let side = Vec2::new(half.x + 6.0, 0.0) * n as f32;
                            [base + side, base - side]
                        }));
                let mut chosen = None;
                for c in candidates {
                    if free(c) {
                        chosen = Some(c);
                        break;
                    }
                }
                let crowded = chosen.is_none();
                let px = chosen
                    .unwrap_or(base)
                    .clamp(half, (self.size - half).max(half));
                el.class_list()
                    .toggle_with_force("opv-dim-crowded", crowded)
                    .ok();
                placed.push((px, half));
                if (px - base).length() > 6.0 {
                    leaders.push_str(&format!(
                        "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>",
                        pm.x, pm.y, px.x, px.y
                    ));
                }
                set_style(
                    el,
                    "transform",
                    &format!("translate({}px, {}px) translate(-50%, -50%)", px.x, px.y),
                );
            }
        } else {
            for (_, el) in &self.dims {
                show(el, false);
            }
        }
        self.leaders.set_inner_html(&leaders);
    }

    fn layout_measure(&self) {
        let snapping = self.measuring;
        let hover = self.hover.filter(|_| snapping);
        show(&self.marker, hover.is_some());
        if let Some(h) = hover {
            self.place(&self.marker, h.point);
            self.marker
                .set_attribute("data-kind", self.labels.get(h.kind.key()))
                .ok();
            let class = match h.kind {
                SnapKind::Vertex => "opv-snap opv-snap-vertex",
                SnapKind::EdgeMidpoint => "opv-snap opv-snap-edge",
                SnapKind::PadCenter => "opv-snap opv-snap-pad",
                SnapKind::Surface => "opv-snap opv-snap-surface",
            };
            self.marker.set_class_name(class);
        }
        if let Some((_, Drag::Orbit(p))) = self.press {
            self.place(&self.pivot_mark, p);
        }
        // Cursor coordinates.
        let at = self.hover.map(|h| h.point);
        show(&self.coords, at.is_some());
        if let Some(p) = at {
            self.coords.set_inner_html(&format!(
                "<span style=\"color:{}\">X</span> {} <span style=\"color:{}\">Y</span> {} \
                 <span style=\"color:{}\">Z</span> {}",
                AXIS_CSS[0],
                number(p.x as f64),
                AXIS_CSS[1],
                number(p.y as f64),
                AXIS_CSS[2],
                number(p.z as f64)
            ));
        }
        let shown = self.shown_measurement();
        let a = shown.map(|s| s.0);
        let end = shown.and_then(|s| s.1);
        for (i, m) in self.point_marks.iter().enumerate() {
            let p = if i == 0 { a } else { end.map(|e| e.0.end) };
            show(m, p.is_some());
            if let Some(p) = p {
                self.place(m, p);
            }
        }
        match (a, end) {
            (Some(a), Some((c, _))) => {
                let (short, long) = measure_text(&self.labels, a, c.end, c.axis, true);
                self.measure_label.set_text_content(Some(&short));
                let bg = c.axis.map(|i| AXIS_CSS[i]).unwrap_or("#e8590c");
                set_style(&self.measure_label, "background", bg);
                show(&self.measure_label, true);
                self.place(&self.measure_label, (a + c.end) * 0.5);
                let lock = match self.effective_lock() {
                    Lock::Axis(_) => format!(" · {}", esc(&self.labels.axis_locked)),
                    Lock::Free => format!(" · {}", esc(&self.labels.free)),
                    Lock::Auto => String::new(),
                };
                self.readout.set_inner_html(&format!("{long}{lock}"));
                show(&self.readout, true);
            }
            _ => {
                show(&self.measure_label, false);
                if self.measuring {
                    self.readout.set_inner_html(&format!(
                        "{}<br><span class=\"opv-hint\">{}</span>",
                        esc(&self.labels.measure_hint),
                        esc(&self.labels.axis_hint)
                    ));
                }
                show(&self.readout, self.measuring);
            }
        }
    }

    /// Stops drawing and releases the canvas, listeners and GPU objects.
    pub(crate) fn teardown(&mut self) {
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
