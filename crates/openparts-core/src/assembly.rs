//! Module assemblies (Canonical Data Specification section 20.12): the
//! components mounted on a module board, so the module's 3D model can
//! show them. The footprint never changes: the carrier board sees only
//! the module package's pads.

use crate::ids::PartId;
use serde::{Deserialize, Serialize};

/// The board side a component is mounted on.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssemblySide {
    #[default]
    Top,
    Bottom,
}

impl AssemblySide {
    pub fn is_top(&self) -> bool {
        *self == AssemblySide::Top
    }
}

/// A placeholder box for a component without an OpenParts part: its
/// size from the component's own datasheet (else the design file), in
/// the component's frame before rotation -- the axes of a package's
/// `body_width` (X), `body_length` (Y) and `body_height` (Z). Lengths in
/// mm.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyBody {
    pub width: f64,
    pub length: f64,
    /// Above the board surface (below it on the bottom side).
    pub height: f64,
    /// `#rrggbb`; absent: a neutral grey.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// One component on a module board: an OpenParts `part` (its own
/// generated 3D model) or a `body` box, exactly one of the two.
///
/// `x`/`y` (mm) place the component's origin -- its Placement Model
/// origin, the land-pattern centroid -- in the module package's frame
/// (for `module`: the board centre, +X right, +Y up, top view);
/// `rotation` (degrees) turns it counter-clockwise seen from the top
/// from its zero orientation. A `bottom` component is first turned over
/// about its own Y axis (x -> -x), as a bottom-side part in the
/// Placement Model (`board = T · R(θ) · M`).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyComponent {
    /// The reference designator (design file / BOM).
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<PartId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<AssemblyBody>,
    /// When `part` is an equivalent standing in for the real component
    /// (its maker's datasheet is not available, or the design names no
    /// MPN): what is really mounted -- its MPN, or a short description
    /// such as "micro-USB B receptacle, MPN not stated". Only with `part`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stand_in_for: Option<String>,
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: f64,
    #[serde(default, skip_serializing_if = "AssemblySide::is_top")]
    pub side: AssemblySide,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl AssemblyComponent {
    /// `(x, y)` of a component-frame point `(px, py)` on the board.
    pub fn place(&self, px: f64, py: f64) -> (f64, f64) {
        let px = if self.side == AssemblySide::Bottom {
            -px
        } else {
            px
        };
        let (s, c) = self.rotation.to_radians().sin_cos();
        (self.x + c * px - s * py, self.y + s * px + c * py)
    }

    /// The rigid 3x4 row-major transform (rotation, then translation)
    /// taking the component's frame (seating plane z = 0) onto a board
    /// whose top surface is at `board_top` and bottom surface at
    /// `board_bottom`: on top it stands on the top surface, on the
    /// bottom it hangs, turned over, under the bottom surface.
    pub fn matrix(&self, board_top: f64, board_bottom: f64) -> [f64; 12] {
        let (s, c) = self.rotation.to_radians().sin_cos();
        match self.side {
            AssemblySide::Top => [
                c, -s, 0.0, self.x, s, c, 0.0, self.y, 0.0, 0.0, 1.0, board_top,
            ],
            // R(θ) · diag(-1, 1, -1): a half turn about Y, then θ.
            AssemblySide::Bottom => [
                -c,
                -s,
                0.0,
                self.x,
                -s,
                c,
                0.0,
                self.y,
                0.0,
                0.0,
                -1.0,
                board_bottom,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f64, y: f64, rotation: f64, side: AssemblySide) -> AssemblyComponent {
        AssemblyComponent {
            reference: "U1".into(),
            part: None,
            body: None,
            stand_in_for: None,
            x,
            y,
            rotation,
            side,
        }
    }

    fn apply(m: &[f64; 12], p: [f64; 3]) -> [f64; 3] {
        let row =
            |r: usize| m[r * 4] * p[0] + m[r * 4 + 1] * p[1] + m[r * 4 + 2] * p[2] + m[r * 4 + 3];
        [row(0), row(1), row(2)]
    }

    #[test]
    fn the_3d_transform_agrees_with_the_board_placement() {
        for side in [AssemblySide::Top, AssemblySide::Bottom] {
            for rot in [0.0, 30.0, 90.0, 225.0] {
                let c = at(3.0, -2.0, rot, side);
                let m = c.matrix(1.0, 0.0);
                for p in [[1.0, 0.0], [0.0, 1.0], [2.5, -0.7]] {
                    let q = apply(&m, [p[0], p[1], 0.0]);
                    let (x, y) = c.place(p[0], p[1]);
                    assert!((q[0] - x).abs() < 1e-12 && (q[1] - y).abs() < 1e-12);
                }
                // Up from the seating plane is away from the board.
                let up = apply(&m, [0.0, 0.0, 1.0])[2];
                match side {
                    AssemblySide::Top => assert!((up - 2.0).abs() < 1e-12),
                    AssemblySide::Bottom => assert!((up + 1.0).abs() < 1e-12),
                }
            }
        }
    }

    #[test]
    fn a_bottom_part_is_mirrored_in_the_top_view() {
        let c = at(0.0, 0.0, 0.0, AssemblySide::Bottom);
        assert_eq!(c.place(1.0, 2.0), (-1.0, 2.0));
    }
}
