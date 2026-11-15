//! Placing a part on a board (spec section 2.3): `board = T · R(θ) · M`.

use crate::Point2;
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardSide {
    #[default]
    Top,
    Bottom,
}

/// Where a part sits on a board: its origin at (`x`, `y`) in the board's
/// frame (top view, Y up), turned `rotation` degrees CCW (viewed from
/// the top), on `side`. A part frame point `p` lands at
///
/// ```text
/// board = T · R(θ) · M · p      M = diag(−1, 1) on the bottom side, else I
/// ```
///
/// i.e. a bottom-side part is first mirrored x → −x in its own frame
/// (seen through the board from the top), then turned and moved.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    /// Degrees CCW viewed from the top.
    pub rotation: f64,
    pub side: BoardSide,
}

impl Placement {
    /// A part frame point in the board frame.
    pub fn apply(&self, p: Point2) -> Point2 {
        let px = if self.side == BoardSide::Bottom {
            -p.x
        } else {
            p.x
        };
        let (s, c) = self.rotation.to_radians().sin_cos();
        Point2 {
            x: self.x + c * px - s * p.y,
            y: self.y + s * px + c * p.y,
        }
    }

    /// A board point in the part frame (the inverse of [`apply`]).
    ///
    /// [`apply`]: Placement::apply
    pub fn invert(&self, b: Point2) -> Point2 {
        let (dx, dy) = (b.x - self.x, b.y - self.y);
        let (s, c) = self.rotation.to_radians().sin_cos();
        let x = c * dx + s * dy;
        let y = -s * dx + c * dy;
        Point2 {
            x: if self.side == BoardSide::Bottom {
                -x
            } else {
                x
            },
            y,
        }
    }

    /// A direction (no translation) in the board frame.
    pub fn apply_vector(&self, v: Point2) -> Point2 {
        let at = self.apply(v);
        Point2 {
            x: at.x - self.x,
            y: at.y - self.y,
        }
    }

    /// The board-frame rotation (degrees CCW) of something turned
    /// `local` degrees in the part frame (a pad).
    pub fn apply_angle(&self, local: f64) -> f64 {
        let local = if self.side == BoardSide::Bottom {
            -local
        } else {
            local
        };
        normalize(self.rotation + local)
    }
}

/// Degrees into (−180, 180].
pub fn normalize(deg: f64) -> f64 {
    let mut d = deg % 360.0;
    if d <= -180.0 {
        d += 360.0;
    } else if d > 180.0 {
        d -= 360.0;
    }
    d + 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point2, b: Point2) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    fn pt(x: f64, y: f64) -> Point2 {
        Point2 { x, y }
    }

    #[test]
    fn rotation_is_counter_clockwise_viewed_from_the_top() {
        let p = Placement {
            x: 10.0,
            y: 5.0,
            rotation: 90.0,
            side: BoardSide::Top,
        };
        // +X of the part points up (+Y) on the board.
        assert!(close(p.apply(pt(1.0, 0.0)), pt(10.0, 6.0)));
        assert!(close(p.apply(pt(0.0, 1.0)), pt(9.0, 5.0)));
    }

    #[test]
    fn the_bottom_side_mirrors_x_before_turning() {
        let p = Placement {
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            side: BoardSide::Bottom,
        };
        // Pin 1 top-left seen from the top is top-right seen through
        // the board.
        assert!(close(p.apply(pt(-1.0, 1.0)), pt(1.0, 1.0)));
        let p = Placement {
            rotation: 90.0,
            ..p
        };
        // M first, then R: (-1, 1) -> (1, 1) -> (-1, 1).
        assert!(close(p.apply(pt(-1.0, 1.0)), pt(-1.0, 1.0)));
        assert!(close(p.apply(pt(1.0, 0.0)), pt(0.0, -1.0)));
    }

    #[test]
    fn invert_undoes_apply_on_both_sides() {
        for side in [BoardSide::Top, BoardSide::Bottom] {
            for rotation in [0.0, 30.0, 90.0, -90.0, 180.0, 271.5] {
                let p = Placement {
                    x: 3.25,
                    y: -7.5,
                    rotation,
                    side,
                };
                let q = pt(1.2, -0.7);
                assert!(close(p.invert(p.apply(q)), q), "{p:?}");
            }
        }
    }

    #[test]
    fn angles_compose_and_mirror() {
        let top = Placement {
            rotation: 90.0,
            ..Default::default()
        };
        assert_eq!(top.apply_angle(45.0), 135.0);
        let bottom = Placement {
            side: BoardSide::Bottom,
            ..top
        };
        assert_eq!(bottom.apply_angle(45.0), 45.0);
        assert_eq!(normalize(270.0), -90.0);
        assert_eq!(normalize(-180.0), 180.0);
    }
}
