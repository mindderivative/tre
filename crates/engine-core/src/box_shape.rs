//! #269 (TRE-REQ-013 step 1, part B): the outline of a box whose corners may curve
//! inward. A concave corner is the box with a quarter disc centred on the box's own
//! corner cut away, so a small box with one concave corner is exactly the fillet that fills
//! the inside corner where two shapes meet.

use peniko::kurbo::{Arc, BezPath, Point, Vec2};
use std::f64::consts::{PI, TAU};

/// Flattening tolerance of the arcs, as every other box outline in the engine.
const TOLERANCE: f64 = 0.1;

/// The outline of a `w` by `h` box, drawn `inset` inside its edges (a border's path is
/// inset by half its width), with corner `radii` in the order top-left, top-right,
/// bottom-right, bottom-left. A `concave` corner curves inward about the box's corner
/// instead of outward; a radius of 0 or less is square either way.
///
/// Used only when a corner is concave; an all-convex box keeps `RoundedRect`.
pub fn box_outline(w: f64, h: f64, radii: [f64; 4], concave: [bool; 4], inset: f64) -> BezPath {
    let (x0, y0, x1, y1) = (inset, inset, w - inset, h - inset);
    let max_convex = ((x1 - x0).min(y1 - y0) / 2.0).max(0.0);
    let max_concave = w.min(h).max(0.0);
    // Per corner: the box corner, the direction the path arrives along, and the point
    // where it leaves the corner (a unit step toward the next corner).
    let corners = [
        (Point::new(0.0, 0.0), Point::new(x0, y0), Vec2::new(1.0, 1.0)),
        (Point::new(w, 0.0), Point::new(x1, y0), Vec2::new(-1.0, 1.0)),
        (Point::new(w, h), Point::new(x1, y1), Vec2::new(-1.0, -1.0)),
        (Point::new(0.0, h), Point::new(x0, y1), Vec2::new(1.0, -1.0)),
    ];
    let mut path = BezPath::new();
    for (i, (box_corner, edge_corner, into)) in corners.into_iter().enumerate() {
        let r = radii[i];
        // Where the path comes into this corner, and where it leaves, along the two edges.
        let (arrive, leave, center, from_angle, to_angle, radius);
        if r <= 0.0 {
            arrive = edge_corner;
            leave = edge_corner;
            center = edge_corner;
            from_angle = 0.0;
            to_angle = 0.0;
            radius = 0.0;
        } else if concave[i] {
            let big = r.min(max_concave) + inset;
            let d = (big * big - inset * inset).max(0.0).sqrt();
            // The arc meets the two edges `d` along from the box's corner.
            arrive = if i % 2 == 0 {
                Point::new(edge_corner.x, box_corner.y + into.y * d)
            } else {
                Point::new(box_corner.x + into.x * d, edge_corner.y)
            };
            leave = if i % 2 == 0 {
                Point::new(box_corner.x + into.x * d, edge_corner.y)
            } else {
                Point::new(edge_corner.x, box_corner.y + into.y * d)
            };
            center = box_corner;
            from_angle = (arrive - box_corner).atan2();
            to_angle = (leave - box_corner).atan2();
            radius = big;
        } else {
            let rc = (r - inset).max(0.0).min(max_convex);
            let c = Point::new(edge_corner.x + into.x * rc, edge_corner.y + into.y * rc);
            arrive = if i % 2 == 0 {
                Point::new(edge_corner.x, edge_corner.y + into.y * rc)
            } else {
                Point::new(edge_corner.x + into.x * rc, edge_corner.y)
            };
            leave = if i % 2 == 0 {
                Point::new(edge_corner.x + into.x * rc, edge_corner.y)
            } else {
                Point::new(edge_corner.x, edge_corner.y + into.y * rc)
            };
            center = c;
            from_angle = (arrive - c).atan2();
            to_angle = (leave - c).atan2();
            radius = rc;
        }
        if i == 0 {
            path.move_to(arrive);
        } else {
            path.line_to(arrive);
        }
        if radius > 0.0 {
            let sweep = shortest(to_angle - from_angle);
            let arc = Arc::new(center, Vec2::new(radius, radius), from_angle, sweep, 0.0);
            path.extend(arc.append_iter(TOLERANCE));
        }
        path.line_to(leave);
    }
    path.close_path();
    path
}

/// An angle difference folded into `(-PI, PI]`.
fn shortest(angle: f64) -> f64 {
    let a = angle.rem_euclid(TAU);
    if a > PI {
        a - TAU
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peniko::kurbo::Shape;

    fn covers(path: &BezPath, x: f64, y: f64) -> bool {
        path.contains(Point::new(x, y))
    }

    #[test]
    fn a_concave_corner_removes_a_quarter_disc_about_the_box_corner() {
        // A 12 by 12 fillet: concave bottom-right of radius 12 keeps the top-left area.
        let p = box_outline(12.0, 12.0, [0.0, 0.0, 12.0, 0.0], [false, false, true, false], 0.0);
        assert!(covers(&p, 1.0, 1.0));
        assert!(covers(&p, 2.0, 1.0));
        assert!(covers(&p, 1.0, 2.0));
        // Inside the quarter disc about (12, 12) of radius 12 (distance < 12): cut away.
        assert!(!covers(&p, 8.0, 8.0));
        assert!(!covers(&p, 11.0, 11.0));
        // Area is the square minus a quarter disc.
        let want = 144.0 - 0.25 * PI * 144.0;
        assert!((p.area().abs() - want).abs() < 0.3, "{} vs {want}", p.area());
    }

    #[test]
    fn every_corner_can_curve_inward() {
        let p = box_outline(40.0, 40.0, [8.0; 4], [true; 4], 0.0);
        for (x, y) in [(1.0, 1.0), (39.0, 1.0), (39.0, 39.0), (1.0, 39.0)] {
            assert!(!covers(&p, x, y), "({x}, {y})");
        }
        assert!(covers(&p, 20.0, 20.0) && covers(&p, 20.0, 1.0) && covers(&p, 1.0, 20.0));
    }

    #[test]
    fn a_mix_of_round_and_concave_corners_keeps_the_round_ones_round() {
        let p = box_outline(40.0, 40.0, [10.0; 4], [true, false, false, false], 0.0);
        assert!(!covers(&p, 2.0, 2.0)); // concave top-left
        assert!(!covers(&p, 39.0, 1.0)); // round top-right is cut
        assert!(covers(&p, 36.0, 4.0)); // but inside its arc
    }

    #[test]
    fn square_corners_are_square_without_a_radius() {
        let p = box_outline(20.0, 10.0, [0.0; 4], [true; 4], 0.0);
        assert!(covers(&p, 0.5, 0.5) && covers(&p, 19.5, 9.5));
        assert!((p.area().abs() - 200.0).abs() < 1e-6);
    }

    #[test]
    fn an_inset_outline_stays_inside_and_grows_the_notch() {
        let p = box_outline(30.0, 30.0, [10.0; 4], [false, false, true, false], 1.0);
        let b = p.bounding_box();
        assert!((b.x0 - 1.0).abs() < 1e-9 && (b.x1 - 29.0).abs() < 1e-9);
        // The notch about (30, 30) is 11 wide on the inset path: (19, 29.x) is on its rim.
        assert!(covers(&p, 18.0, 28.0) && !covers(&p, 21.0, 28.0));
    }
}
