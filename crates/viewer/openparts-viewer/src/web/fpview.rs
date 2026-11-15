//! The footprint view: the dimensioned footprint SVG and the measure
//! tool, snapping to pads and the courtyard and holding the second point
//! to X or Y when it is near that line (Shift: free, X/Y: lock).

use super::{button, element, listen, measure_text, set_pressed, show, typing, window, Listener};
use crate::footprint::{self, Drawing, FpSnap, MeasureDraw, P};
use crate::labels::Labels;
use crate::measure::{self, Constrained, Lock};
use crate::snap::SnapKind;
use glam::{Vec2, Vec3};
use openparts_core::Device;
use openparts_mcad::MechanicalGeometry;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::{Event, EventTarget, HtmlElement, KeyboardEvent};

pub(crate) struct FootprintView {
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
    /// First point, then the second (constrained) and its snap.
    first: Option<FpSnap>,
    second: Option<(Constrained, FpSnap)>,
    hover: Option<FpSnap>,
    /// Drawing units per CSS pixel, as of the last pointer event.
    per_px: f64,
    lock: Lock,
    shift: bool,
    active: bool,
    labels: Labels,
    listeners: Vec<Listener>,
}

pub(crate) fn mount_footprint(
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
        first: None,
        second: None,
        hover: None,
        per_px: 0.05,
        lock: Lock::Auto,
        shift: false,
        active: false,
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
        listen(window().as_ref(), "pointerdown", &weak, |v, e| {
            v.active = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                .is_some_and(|t| v.root.contains(Some(&t)));
        }),
        listen(window().as_ref(), "keydown", &weak, FootprintView::on_key),
        listen(window().as_ref(), "keyup", &weak, |v, e| {
            let e: KeyboardEvent = e.unchecked_into();
            if e.key() == "Shift" && v.shift {
                v.shift = false;
                v.redraw();
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

fn v3(p: P) -> Vec3 {
    Vec3::new(p.x as f32, p.y as f32, 0.0)
}

fn p2(v: Vec3) -> P {
    P {
        x: v.x as f64,
        y: v.y as f64,
    }
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

    fn at(&mut self, e: &Event) -> FpSnap {
        let e: &web_sys::MouseEvent = e.unchecked_ref();
        self.shift = e.shift_key();
        let (p, per_px) = self.to_drawing(e);
        self.per_px = per_px;
        footprint::pick(&self.drawing.snaps, p, per_px * 12.0).unwrap_or(FpSnap {
            at: p,
            kind: SnapKind::Surface,
        })
    }

    fn effective_lock(&self) -> Lock {
        match self.lock {
            Lock::Auto if self.shift => Lock::Free,
            l => l,
        }
    }

    /// The second point as it would land on `h`.
    fn constrain(&self, a: P, h: FpSnap) -> Constrained {
        let k = 1.0 / self.per_px.max(1e-9) as f32;
        measure::constrain(v3(a), v3(h.at), self.effective_lock(), 2, |p| {
            Some(Vec2::new(p.x * k, p.y * k))
        })
    }

    fn on_move(&mut self, e: Event) {
        if !self.measuring {
            return;
        }
        self.hover = Some(self.at(&e));
        self.redraw();
    }

    fn on_click(&mut self, e: Event) {
        if !self.measuring {
            return;
        }
        let s = self.at(&e);
        match (self.first, self.second) {
            (Some(a), None) => self.second = Some((self.constrain(a.at, s), s)),
            _ => {
                self.first = Some(s);
                self.second = None;
                self.lock = Lock::Auto;
            }
        }
        self.redraw();
    }

    fn on_key(&mut self, e: Event) {
        let e: KeyboardEvent = e.unchecked_into();
        if typing(&e) || !self.active || !self.measuring {
            return;
        }
        match e.key().as_str() {
            "Escape" => self.clear_measure(),
            "Shift" => {
                self.shift = true;
                self.redraw();
            }
            k @ ("x" | "X" | "y" | "Y") => {
                let i = if k.eq_ignore_ascii_case("x") { 0 } else { 1 };
                self.lock = if self.lock == Lock::Axis(i) {
                    Lock::Auto
                } else {
                    Lock::Axis(i)
                };
                self.redraw();
            }
            _ => {}
        }
    }

    pub(crate) fn set_dimensions(&mut self, on: bool) {
        self.show_dims = on;
        set_pressed(&self.dims_button, on);
        let _ = self
            .dims
            .set_attribute("display", if on { "inline" } else { "none" });
    }

    pub(crate) fn set_measure(&mut self, on: bool) {
        self.measuring = on;
        set_pressed(&self.measure_button, on);
        self.root
            .class_list()
            .toggle_with_force("opv-measuring", on)
            .ok();
        if !on {
            self.first = None;
            self.second = None;
            self.hover = None;
            self.lock = Lock::Auto;
        }
        self.redraw();
    }

    pub(crate) fn clear_measure(&mut self) {
        self.first = None;
        self.second = None;
        self.lock = Lock::Auto;
        self.redraw();
    }

    fn redraw(&mut self) {
        let a = self.first.map(|s| s.at);
        let second = match (a, self.second, self.hover) {
            (_, Some(s), _) => Some(s),
            (Some(a), None, Some(h)) => Some((self.constrain(a, h), h)),
            _ => None,
        };
        // Y down in the drawing: the board's Y is the negated one.
        let board = |p: P| Vec3::new(p.x as f32, -p.y as f32, 0.0);
        let text = match (a, second) {
            (Some(a), Some((c, _))) => Some(measure_text(
                &self.labels,
                board(a),
                board(p2(c.end)),
                c.axis,
                false,
            )),
            _ => None,
        };
        self.measure
            .set_inner_html(&footprint::measure_svg(&MeasureDraw {
                a,
                end: second.map(|s| p2(s.0.end)),
                from: second.map(|s| s.1.at),
                axis: second.and_then(|s| s.0.axis),
                hover: self.hover.map(|h| h.at),
                view: self.drawing.view,
                font: self.drawing.font,
                text: text.as_ref().map(|t| t.0.clone()).unwrap_or_default(),
            }));
        match (&text, self.measuring) {
            (Some(t), _) => {
                self.readout.set_inner_html(&t.1);
                show(&self.readout, true);
            }
            (None, true) => {
                self.readout.set_inner_html(&format!(
                    "{}<br><span class=\"opv-hint\">{}</span>",
                    crate::footprint::esc(&self.labels.measure_hint),
                    crate::footprint::esc(&self.labels.axis_hint)
                ));
                show(&self.readout, true);
            }
            (None, false) => show(&self.readout, false),
        }
    }

    pub(crate) fn teardown(&mut self) {
        self.listeners.clear();
        self.root.set_inner_html("");
    }
}
