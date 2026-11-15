//! The viewer's UI strings. The site passes them in its language
//! (web/cmd/app/i18n.mbt); English is the fallback for any it omits.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Labels {
    pub dimensions: String,
    pub measure: String,
    pub reset_view: String,
    pub fit: String,
    pub home: String,
    pub perspective: String,
    pub orthographic: String,
    pub view_front: String,
    pub view_back: String,
    pub view_left: String,
    pub view_right: String,
    pub view_top: String,
    pub view_bottom: String,
    pub view_iso: String,
    pub axis_hint: String,
    pub axis_locked: String,
    pub free: String,
    pub measure_hint: String,
    pub distance: String,
    pub source: String,
    pub page: String,
    pub locator: String,
    pub open_source: String,
    pub model_value: String,
    pub computed: String,
    pub close: String,
    pub model_failed: String,
    pub no_gpu: String,
    pub footprint_failed: String,
    pub snap_vertex: String,
    pub snap_edge: String,
    pub snap_pad: String,
    pub snap_surface: String,
    pub body_width: String,
    pub body_length: String,
    pub body_height: String,
    pub overall_width: String,
    pub overall_length: String,
    pub overall_height: String,
    pub pitch: String,
    pub lead_width: String,
    pub lead_length: String,
    pub pad_size: String,
    pub pad_span_x: String,
    pub pad_span_y: String,
    pub courtyard_x: String,
    pub courtyard_y: String,
}

impl Default for Labels {
    fn default() -> Labels {
        let s = |s: &str| s.to_string();
        Labels {
            dimensions: s("Dimensions"),
            measure: s("Measure"),
            reset_view: s("Reset view"),
            fit: s("Fit"),
            home: s("Home view"),
            perspective: s("Perspective"),
            orthographic: s("Orthographic"),
            view_front: s("Front"),
            view_back: s("Back"),
            view_left: s("Left"),
            view_right: s("Right"),
            view_top: s("Top"),
            view_bottom: s("Bottom"),
            view_iso: s("Isometric"),
            axis_hint: s(
                "Snaps to X/Y/Z when near an axis · Shift: free · X/Y/Z key: lock an axis",
            ),
            axis_locked: s("locked"),
            free: s("free"),
            measure_hint: s(
                "Click two points (snaps to vertices, edge midpoints and pad centres). Esc clears.",
            ),
            distance: s("Distance"),
            source: s("Source"),
            page: s("Page"),
            locator: s("Where"),
            open_source: s("Open the document"),
            model_value: s("Not stated by the datasheet: measured on the generated model."),
            computed: s("Computed by OpenParts (IPC-7351B land pattern), not a datasheet value."),
            close: s("Close"),
            model_failed: s("The 3D model could not be loaded."),
            no_gpu: s("3D preview needs WebGPU or WebGL2, which this browser doesn't offer."),
            footprint_failed: s("The footprint could not be drawn."),
            snap_vertex: s("vertex"),
            snap_edge: s("edge midpoint"),
            snap_pad: s("pad centre"),
            snap_surface: s("surface"),
            body_width: s("Body width"),
            body_length: s("Body length"),
            body_height: s("Height"),
            overall_width: s("Overall width"),
            overall_length: s("Overall length"),
            overall_height: s("Overall height"),
            pitch: s("Pitch"),
            lead_width: s("Lead width"),
            lead_length: s("Lead length"),
            pad_size: s("Pad"),
            pad_span_x: s("Pad span X"),
            pad_span_y: s("Pad span Y"),
            courtyard_x: s("Courtyard X"),
            courtyard_y: s("Courtyard Y"),
        }
    }
}

impl Labels {
    /// The string for an annotation or snap key.
    pub fn get(&self, key: &str) -> &str {
        match key {
            "body_width" => &self.body_width,
            "body_length" => &self.body_length,
            "body_height" => &self.body_height,
            "overall_width" => &self.overall_width,
            "overall_length" => &self.overall_length,
            "overall_height" => &self.overall_height,
            "pitch" => &self.pitch,
            "lead_width" => &self.lead_width,
            "lead_length" => &self.lead_length,
            "pad_size" => &self.pad_size,
            "pad_span_x" => &self.pad_span_x,
            "pad_span_y" => &self.pad_span_y,
            "courtyard_x" => &self.courtyard_x,
            "courtyard_y" => &self.courtyard_y,
            "view_front" => &self.view_front,
            "view_back" => &self.view_back,
            "view_left" => &self.view_left,
            "view_right" => &self.view_right,
            "view_top" => &self.view_top,
            "view_bottom" => &self.view_bottom,
            "view_iso" => &self.view_iso,
            "snap_vertex" => &self.snap_vertex,
            "snap_edge" => &self.snap_edge,
            "snap_pad" => &self.snap_pad,
            "snap_surface" => &self.snap_surface,
            _ => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_strings_fall_back_to_english() {
        let l: Labels = serde_json::from_str(r#"{"pitch": "ピッチ"}"#).unwrap();
        assert_eq!(l.get("pitch"), "ピッチ");
        assert_eq!(l.get("body_width"), "Body width");
        assert_eq!(l.get("nope"), "");
    }
}
