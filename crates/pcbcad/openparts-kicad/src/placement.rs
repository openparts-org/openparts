//! A Placement Model placement (`board = T · R(θ) · M`, Y up) in KiCad's
//! board terms, and back (Placement Model spec section 2.4).
//!
//! KiCad's board frame is Y-down; a footprint's orientation is
//! counter-clockwise as seen on screen (from the top). Flipping a
//! footprint to the back (`FOOTPRINT::Flip`, either direction) mirrors
//! its pads in its own Y (KiCad's Y-down local frame) and leaves an
//! orientation φ; a back-side footprint's pad at local `f` (Y-down)
//! lands at `C + Rd(φ) · (fx, −fy)`. In the model's Y-up terms that is
//! `R(φ) · diag(1, −1)`, and `R(θ) · M = R(θ) · diag(−1, 1) =
//! R(θ + 180°) · diag(1, −1)`, so
//!
//! ```text
//! top:     φ = θ            KiCad (x, y) = (x, −y)
//! bottom:  φ = θ + 180°     flipped
//! ```
//!
//! pcbnew's `Flip(pos, LEFT_RIGHT)` on a footprint at orientation α
//! leaves φ = 180° − α (it flips top-bottom, negating α, then turns
//! 180°), i.e. the model's θ = −α: placing a bottom part by setting α
//! and then flipping turns it the other way -- set φ after the flip.

use openparts_pcbcad::openparts_placement::{normalize, BoardSide, Placement, Point2};

/// A footprint's placement as KiCad stores it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KicadPlacement {
    /// mm, KiCad's Y-down board frame.
    pub x: f64,
    pub y: f64,
    /// Degrees, KiCad's (counter-clockwise on screen).
    pub orientation: f64,
    /// On the back (B.Cu).
    pub flipped: bool,
}

pub fn to_kicad(p: &Placement) -> KicadPlacement {
    let flipped = p.side == BoardSide::Bottom;
    KicadPlacement {
        x: p.x + 0.0,
        y: -p.y + 0.0,
        orientation: normalize(p.rotation + if flipped { 180.0 } else { 0.0 }),
        flipped,
    }
}

pub fn from_kicad(k: &KicadPlacement) -> Placement {
    Placement {
        x: k.x + 0.0,
        y: -k.y + 0.0,
        rotation: normalize(k.orientation - if k.flipped { 180.0 } else { 0.0 }),
        side: if k.flipped {
            BoardSide::Bottom
        } else {
            BoardSide::Top
        },
    }
}

/// Where KiCad puts a footprint pad, Y-up board mm: `local` is the pad
/// in the model frame (Y-up, as the IR; the `.kicad_mod` stores
/// `(x, −y)`). Follows KiCad's own transform (`PAD::GetPosition` of a
/// flipped, rotated footprint), for checking [`to_kicad`].
pub fn kicad_pad_position(k: &KicadPlacement, local: Point2) -> Point2 {
    // KiCad local (Y-down) pad position, mirrored in Y when flipped.
    let (fx, fy) = (local.x, -local.y);
    let fy = if k.flipped { -fy } else { fy };
    // A counter-clockwise turn on screen in a Y-down frame.
    let (s, c) = k.orientation.to_radians().sin_cos();
    let (rx, ry) = (c * fx + s * fy, -s * fx + c * fy);
    Point2 {
        x: k.x + rx,
        y: -(k.y + ry),
    }
}

/// [`kicad_pad_position`] of a footprint first set to orientation
/// `alpha` and then flipped with `Flip(pos, LEFT_RIGHT)`, as KiCad's
/// `FOOTPRINT::Flip` does it: mirror top-bottom (negating the
/// orientation), then turn 180° about the position.
pub fn kicad_flip_left_right(x: f64, y_down: f64, alpha: f64) -> KicadPlacement {
    KicadPlacement {
        x,
        y: y_down,
        orientation: normalize(180.0 - alpha),
        flipped: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point2, b: Point2) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    const PADS: [Point2; 4] = [
        Point2 { x: -1.2, y: 0.7 },
        Point2 { x: 1.3, y: -0.4 },
        Point2 { x: 0.0, y: 2.0 },
        Point2 { x: -3.0, y: -1.0 },
    ];

    #[test]
    fn kicad_puts_every_pad_where_the_model_does() {
        for side in [BoardSide::Top, BoardSide::Bottom] {
            for rotation in [0.0, 90.0, -90.0, 180.0, 37.5] {
                let p = Placement {
                    x: 12.5,
                    y: -3.25,
                    rotation,
                    side,
                };
                let k = to_kicad(&p);
                for pad in PADS {
                    assert!(
                        close(kicad_pad_position(&k, pad), p.apply(pad)),
                        "{p:?} {pad:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn round_trips_through_kicad() {
        for side in [BoardSide::Top, BoardSide::Bottom] {
            for rotation in [0.0, 90.0, -90.0, 180.0, 12.0, -170.0] {
                let p = Placement {
                    x: -4.0,
                    y: 8.5,
                    rotation,
                    side,
                };
                let back = from_kicad(&to_kicad(&p));
                assert!((back.x - p.x).abs() < 1e-12 && (back.y - p.y).abs() < 1e-12);
                assert_eq!(back.side, p.side);
                assert!(
                    (normalize(back.rotation - p.rotation)).abs() < 1e-9,
                    "{p:?}"
                );
            }
        }
    }

    #[test]
    fn flip_left_right_after_rotating_negates_the_model_angle() {
        // pcbnew: SetOrientation(α) then Flip(LEFT_RIGHT) == the model's
        // bottom side at θ = −α.
        for alpha in [0.0, 90.0, -90.0, 180.0, 30.0] {
            let k = kicad_flip_left_right(5.0, -2.0, alpha);
            let p = Placement {
                x: 5.0,
                y: 2.0,
                rotation: -alpha,
                side: BoardSide::Bottom,
            };
            for pad in PADS {
                assert!(close(kicad_pad_position(&k, pad), p.apply(pad)));
            }
            // The KiCad mirror: x mirrored about the position at α = 0.
            if alpha == 0.0 {
                let q = kicad_pad_position(&k, Point2 { x: -1.0, y: 1.0 });
                assert!(close(q, Point2 { x: 6.0, y: 3.0 }));
            }
        }
    }

    #[test]
    fn kicad_coordinates_are_y_down() {
        let k = to_kicad(&Placement {
            x: 1.0,
            y: 2.0,
            rotation: 90.0,
            side: BoardSide::Top,
        });
        assert_eq!(
            (k.x, k.y, k.orientation, k.flipped),
            (1.0, -2.0, 90.0, false)
        );
        let k = to_kicad(&Placement {
            side: BoardSide::Bottom,
            rotation: 0.0,
            ..Default::default()
        });
        assert_eq!((k.orientation, k.flipped), (180.0, true));
    }
}
