//! The browser side: `mount` fetches the part, its package and device
//! from the API, draws the 3D view (wgpu) and the footprint view (SVG),
//! and returns a `Viewer` handle (`destroy`, `setDimensions`,
//! `setMeasure`, `clearMeasure`, `resetView`).

mod fpview;
mod gpu;
mod model;

use crate::format::number;
use crate::labels::Labels;
use crate::measure::{AXIS_CSS, AXIS_NAMES};
use fpview::{mount_footprint, FootprintView};
use glam::Vec3;
use model::{mount_model, ModelView};
use openparts_core::{Device, Package};
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Event, EventTarget, HtmlElement, KeyboardEvent};

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[derive(Deserialize)]
pub(crate) struct Options {
    /// The API's base URL ("/api").
    api: String,
    manufacturer: String,
    mpn: String,
    #[serde(default)]
    pub(crate) labels: Labels,
}

pub(crate) fn window() -> web_sys::Window {
    web_sys::window().expect("a window")
}

pub(crate) fn document() -> web_sys::Document {
    window().document().expect("a document")
}

pub(crate) fn element(tag: &str, class: &str) -> HtmlElement {
    let e: HtmlElement = document().create_element(tag).unwrap().unchecked_into();
    e.set_class_name(class);
    e
}

pub(crate) fn set_style(e: &HtmlElement, name: &str, value: &str) {
    let _ = e.style().set_property(name, value);
}

pub(crate) fn show(e: &HtmlElement, visible: bool) {
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

pub(crate) async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let r = fetch(url).await?;
    let b = JsFuture::from(r.array_buffer().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?;
    Ok(js_sys::Uint8Array::new(&b).to_vec())
}

pub(crate) async fn fetch_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, String> {
    let text = fetch_text(url).await?;
    serde_json::from_str(&text).map_err(|e| format!("{url}: {e}"))
}

/// What the views need about the part.
pub(crate) struct PartInfo {
    pub(crate) package: Package,
    pub(crate) device: Option<Device>,
    /// Source id -> (label, URL).
    pub(crate) sources: HashMap<String, (String, String)>,
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

/// The 3D models' links from `GET .../artifacts`: content-addressed
/// (cacheable for good) where the server knows the content.
#[derive(Deserialize, Default)]
pub(crate) struct ModelLinks {
    #[serde(rename = "3mf")]
    pub(crate) three_mf: Option<String>,
    pub(crate) stl: Option<String>,
}

pub(crate) fn part_path(o: &Options) -> String {
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
pub(crate) struct Listener {
    pub(crate) target: EventTarget,
    pub(crate) kind: &'static str,
    pub(crate) f: Closure<dyn FnMut(Event)>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.kind, self.f.as_ref().unchecked_ref());
    }
}

/// Calls `f` on the view behind `weak` for `kind` events on `target`.
pub(crate) fn listen<T: 'static>(
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

pub(crate) fn button(label: &str, pressed: Option<bool>) -> HtmlElement {
    let b = element("button", "opv-button");
    let _ = b.set_attribute("type", "button");
    b.set_text_content(Some(label));
    if let Some(p) = pressed {
        let _ = b.set_attribute("aria-pressed", if p { "true" } else { "false" });
    }
    b
}

pub(crate) fn set_pressed(b: &HtmlElement, p: bool) {
    let _ = b.set_attribute("aria-pressed", if p { "true" } else { "false" });
}

/// The measurement's texts: the short label (on the line) and the
/// readout (HTML), for `a` to `b` held to `axis` (if any).
pub(crate) fn measure_text(
    labels: &Labels,
    a: Vec3,
    b: Vec3,
    axis: Option<usize>,
    three_d: bool,
) -> (String, String) {
    let d = b - a;
    let short = format!("{} mm", number(d.length() as f64));
    let mut long = match axis {
        Some(i) => format!(
            "<strong style=\"color:{}\">{} {}</strong>",
            AXIS_CSS[i], AXIS_NAMES[i], short
        ),
        None => format!(
            "{} <strong>{}</strong>",
            crate::footprint::esc(&labels.distance),
            short
        ),
    };
    long.push_str(&format!(
        " · ΔX {} · ΔY {}",
        number(d.x as f64),
        number(d.y as f64)
    ));
    if three_d {
        long.push_str(&format!(" · ΔZ {}", number(d.z as f64)));
    }
    (short, long)
}

/// Whether a key event comes from a text field (then it's typing).
pub(crate) fn typing(e: &KeyboardEvent) -> bool {
    e.target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        .is_some_and(|t| matches!(t.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"))
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

    /// The isometric home view, fitted.
    #[wasm_bindgen(js_name = resetView)]
    pub fn reset_view(&self) {
        if let Some(m) = &self.model {
            m.borrow_mut().home();
        }
    }

    /// Zooms to fit the model.
    pub fn fit(&self) {
        if let Some(m) = &self.model {
            m.borrow_mut().fit(true);
        }
    }

    /// Looks from a side: "front", "back", "left", "right", "top",
    /// "bottom" or "iso".
    #[wasm_bindgen(js_name = setView)]
    pub fn set_view(&self, name: &str) {
        use crate::camera::StandardView as V;
        let v = match name {
            "front" => V::Front,
            "back" => V::Back,
            "left" => V::Left,
            "right" => V::Right,
            "top" => V::Top,
            "bottom" => V::Bottom,
            _ => V::Iso,
        };
        if let Some(m) = &self.model {
            m.borrow_mut().view(v);
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
