//! A SolidCraft command script under construction.
//!
//! Every dimension that more than one feature depends on is declared once
//! as a design parameter ([`Script::param`]) and referenced by name in
//! expressions (`"body_w / 2 + lead_reach"`), so the generated design stays
//! parametric when opened in SolidCraft: changing `pitch` or
//! `leads_per_side` in its Parameters panel rebuilds every lead, the body
//! outline and the pin-1 mark together.

use serde_json::{json, Value};

/// A length in millimetres as a SolidCraft expression (`"0.5 mm"`).
pub fn mm(v: f64) -> String {
    format!("{} mm", num(v))
}

/// A plain number, rounded to 1 nm and without trailing zeros, so the
/// same input always prints the same text (reproducible scripts).
pub fn num(v: f64) -> String {
    let rounded = (v * 1e6).round() / 1e6;
    let rounded = if rounded == 0.0 { 0.0 } else { rounded }; // no "-0"
    let mut s = format!("{rounded:.6}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

#[derive(Debug, Clone, Default)]
pub struct Script {
    commands: Vec<Value>,
    params: Vec<String>,
}

impl Script {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares a design parameter once. Declaring the same name again is
    /// a no-op, so shared builders can each declare what they rely on.
    pub fn param(&mut self, name: &str, expression: impl Into<String>, comment: &str) {
        self.param_with_unit(name, expression, None, comment);
    }

    /// A unitless parameter (a count).
    pub fn count(&mut self, name: &str, n: u32, comment: &str) {
        self.param_with_unit(name, n.to_string(), Some(""), comment);
    }

    /// Declares a parameter from a template (see [`Self::param`]).
    pub(crate) fn declare(
        &mut self,
        name: &str,
        expression: String,
        unit: Option<String>,
        comment: &str,
    ) {
        self.param_with_unit(name, expression, unit.as_deref(), comment);
    }

    fn param_with_unit(
        &mut self,
        name: &str,
        expression: impl Into<String>,
        unit: Option<&str>,
        comment: &str,
    ) {
        if self.params.iter().any(|p| p == name) {
            return;
        }
        self.params.push(name.to_string());
        let mut params = json!({"name": name, "expression": expression.into()});
        if let Some(unit) = unit {
            params["unit"] = json!(unit);
        }
        if !comment.is_empty() {
            params["comment"] = json!(comment);
        }
        self.cmd("parameters.add", params);
    }

    pub fn cmd(&mut self, command: &str, params: Value) {
        self.commands
            .push(json!({"command": command, "params": params}));
    }

    pub fn paint(&mut self, bodies: &[String], look: Look) {
        if bodies.is_empty() {
            return;
        }
        let mut params = json!({"bodies": bodies, "color": look.color});
        if look.opacity < 1.0 {
            params["opacity"] = json!(look.opacity);
        } else {
            params["pbr"] = json!({
                "metallic": look.metallic,
                "roughness": look.roughness,
                "dielectric_ior": 1.5,
            });
        }
        self.cmd("appearance.assign", params);
    }

    pub fn commands(&self) -> &[Value] {
        &self.commands
    }

    /// The script document `solidcraft-cli run` takes.
    pub fn to_json(&self) -> Value {
        json!({"commands": self.commands})
    }

    pub fn to_string_pretty(&self) -> String {
        serde_json::to_string_pretty(&self.to_json()).expect("a JSON value always serializes")
    }
}

/// A surface finish: a colour plus the physically based shading
/// SolidCraft carries into its renders (STEP keeps the colour only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub color: &'static str,
    pub metallic: f64,
    pub roughness: f64,
    pub opacity: f64,
}

impl Look {
    const fn dielectric(color: &'static str, roughness: f64) -> Self {
        Look {
            color,
            metallic: 0.0,
            roughness,
            opacity: 1.0,
        }
    }
    const fn metal(color: &'static str, roughness: f64) -> Self {
        Look {
            color,
            metallic: 1.0,
            roughness,
            opacity: 1.0,
        }
    }
    const fn clear(color: &'static str, opacity: f64) -> Self {
        Look {
            color,
            metallic: 0.0,
            roughness: 0.1,
            opacity,
        }
    }
}

/// The finishes shared by every family.
pub mod looks {
    use super::Look;

    /// Epoxy mold compound (IC and discrete bodies).
    pub const MOLD: Look = Look::dielectric("#1c1c1e", 0.75);
    /// Matte tin plating (leads, terminations).
    pub const TIN: Look = Look::metal("#cfd1d4", 0.35);
    /// Bright nickel barrier / end caps.
    pub const NICKEL: Look = Look::metal("#b9bbbe", 0.25);
    /// Gold flash (LED and crystal pads).
    pub const GOLD: Look = Look::metal("#d4af37", 0.25);
    /// Copper-coloured wire.
    pub const COPPER: Look = Look::metal("#c47a45", 0.3);
    /// Aluminium (electrolytic can top, crystal lids).
    pub const ALUMINIUM: Look = Look::metal("#d9dbde", 0.3);
    /// Alumina substrate of thick-film resistors and crystal bases.
    pub const ALUMINA: Look = Look::dielectric("#ede9df", 0.6);
    /// Resistive overcoat of a thick-film chip resistor.
    pub const OVERCOAT: Look = Look::dielectric("#141414", 0.55);
    /// MLCC ceramic body.
    pub const CERAMIC_TAN: Look = Look::dielectric("#b5916a", 0.65);
    /// Ferrite / multilayer inductor body.
    pub const FERRITE: Look = Look::dielectric("#3b3d40", 0.7);
    /// Chip-fuse ceramic body.
    pub const FUSE_BODY: Look = Look::dielectric("#e8e2cf", 0.6);
    /// Laser-marked cathode band / polarity stripe on a dark body.
    pub const MARK_LIGHT: Look = Look::dielectric("#bfc2c6", 0.6);
    /// Electrolytic capacitor PVC sleeve.
    pub const SLEEVE: Look = Look::dielectric("#20345c", 0.45);
    /// Rubber bung under an electrolytic can.
    pub const RUBBER: Look = Look::dielectric("#2a2a2a", 0.9);
    /// BGA laminate substrate.
    pub const LAMINATE: Look = Look::dielectric("#3d5a3a", 0.6);
    /// Natural (unpigmented, white) polyamide connector housing.
    pub const NYLON_NATURAL: Look = Look::dielectric("#ebe5d3", 0.55);
    /// White LED housing.
    pub const LED_HOUSING: Look = Look::dielectric("#f4f4f2", 0.5);
    /// Water-clear epoxy lens.
    pub const LENS: Look = Look::clear("#e8f1f6", 0.55);
    /// Solid tantalum capacitor molding (KEMET-style yellow).
    pub const TANTALUM: Look = Look::dielectric("#d9a21b", 0.6);
    /// Anode band printed on a tantalum body.
    pub const TANTALUM_BAND: Look = Look::dielectric("#6b3f12", 0.6);
    /// Multilayer chip inductor ceramic.
    pub const CHIP_INDUCTOR: Look = Look::dielectric("#c9c4b8", 0.6);
    /// An LED die seen through the lens.
    pub const DIE: Look = Look::dielectric("#3a3f4a", 0.4);
    /// Green polyamide terminal-block housing.
    pub const TERMINAL_BLOCK: Look = Look::dielectric("#2f7d4a", 0.6);
    /// Generic grey body for anything without a better guess.
    pub const NEUTRAL: Look = Look::dielectric("#6d7178", 0.6);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_reproducibly() {
        assert_eq!(mm(0.5), "0.5 mm");
        assert_eq!(mm(7.0), "7 mm");
        assert_eq!(mm(-0.0), "0 mm");
        assert_eq!(num(0.1 + 0.2), "0.3");
    }

    #[test]
    fn a_parameter_is_declared_once() {
        let mut s = Script::new();
        s.param("pitch", mm(0.5), "");
        s.param("pitch", mm(0.8), "");
        assert_eq!(s.commands().len(), 1);
        assert_eq!(s.commands()[0]["params"]["expression"], "0.5 mm");
    }
}
