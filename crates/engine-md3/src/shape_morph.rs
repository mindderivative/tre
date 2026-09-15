//! §7.4's shape-morph module: the one MD3 mechanism with no library to
//! lean on (§14 step 10). "Equalize point count, then lerp" is only
//! half the real technique -- §7.4's own review note names the missing,
//! harder half explicitly: naive per-index interpolation assumes point
//! *N* on one shape visually corresponds to point *N* on the other,
//! which is usually false and produces self-intersecting or
//! wildly-rotating mid-morph geometry. This module runs a real
//! correspondence/alignment search -- every rotational starting-point
//! offset, in both winding directions, scored by total point-travel
//! distance -- before ever lerping a single point.
//!
//! Deliberately scoped to a path's own vertices (the endpoint of each
//! `PathEl`), not full curve-type-aware control-point morphing: a
//! curved segment's own control handles are dropped, and the morph
//! result is always straight-line segments between the interpolated
//! vertices. MD3's own shapes are close enough to polygons (rounded
//! corners aside) that this is the correct v1 scope -- true
//! curve-to-curve correspondence (matching bezier segment types, not
//! just endpoint positions) is a substantially larger problem than
//! "budget real implementation time" (§7.4) asks for, and nothing in
//! §14's build order calls for it. Also assumes a single closed
//! subpath: every MD3 shape is one contour, and no planned component
//! needs a shape with a hole.

use engine_core::Interpolate;
use peniko::kurbo::{BezPath, PathEl, Point};

/// A shape's own vertex sequence -- the value type an `Animated<
/// ShapeKey>` (§5) interpolates between. Two `ShapeKey`s of different
/// point counts, or built from paths that happen to start at different
/// corners or wind in different directions, are still valid
/// `interpolate` inputs: the equalize-then-align pipeline runs fresh
/// inside `interpolate` itself, not as a separate setup step a caller
/// must remember -- matching every other `Animated<T>`'s "just call
/// `animate_to`" ergonomics (§5, step 2).
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeKey {
    points: Vec<Point>,
}

impl ShapeKey {
    /// Extracts a closed shape's own vertex sequence: the endpoint of
    /// every `MoveTo`/`LineTo`/`QuadTo`/`CurveTo` segment, in path
    /// order, dropping curve control handles (see this module's own
    /// doc comment for why) and `ClosePath` (it names no new point).
    pub fn from_path(path: &BezPath) -> Self {
        let points = path
            .iter()
            .filter_map(|el| match el {
                PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(p),
                PathEl::QuadTo(_, p) | PathEl::CurveTo(_, _, p) => Some(p),
                PathEl::ClosePath => None,
            })
            .collect();
        Self { points }
    }

    /// Rebuilds a closed `BezPath` from the current vertex sequence --
    /// straight-line segments between each point, matching this
    /// module's "vertices only" scope.
    pub fn to_path(&self) -> BezPath {
        let mut path = BezPath::new();
        let mut points = self.points.iter();
        if let Some(&first) = points.next() {
            path.move_to(first);
            for &p in points {
                path.line_to(p);
            }
            path.close_path();
        }
        path
    }
}

impl Interpolate for ShapeKey {
    /// The full correspondence-then-lerp pipeline (§7.4), run fresh on
    /// every call: pad whichever point list is shorter up to the
    /// other's length, search for the lowest-total-travel alignment
    /// between them, then lerp each corresponding pair. Recomputing the
    /// (cheap, O(n^2) over a shape's own small vertex count) search on
    /// every tick rather than caching it once per animation is the same
    /// "naive until profiling says otherwise" call `Tree::tick_all`
    /// already makes for its own whole-tree walk -- correct, and the
    /// simplest thing that could work, with the frame-time CI benchmark
    /// standing ready to catch it if a real shape ever makes this the
    /// actual cost.
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        if self.points.is_empty() || other.points.is_empty() {
            // A degenerate (pointless) shape has no sensible morph --
            // snap rather than lerp against nothing.
            return if t < 0.5 { self.clone() } else { other.clone() };
        }

        let (from, to) = equalize_point_counts(self.points.clone(), other.points.clone());
        let to_aligned = best_aligned(&from, &to);

        let points = from
            .iter()
            .zip(to_aligned.iter())
            .map(|(&p1, &p2)| p1.lerp(p2, t))
            .collect();
        Self { points }
    }
}

/// Pads whichever of `a`/`b` has fewer points, by repeatedly
/// subdividing its own longest edge, until both have the same length.
/// §7.4's text permits either "zero-length or subdivided segments";
/// this always subdivides -- a real midpoint keeps the padded shape's
/// outline visually identical to its un-padded original, where a
/// zero-length duplicate would add a vertex with no geometric effect
/// that still has to participate in the correspondence search below.
fn equalize_point_counts(mut a: Vec<Point>, mut b: Vec<Point>) -> (Vec<Point>, Vec<Point>) {
    while a.len() < b.len() {
        subdivide_longest_edge(&mut a);
    }
    while b.len() < a.len() {
        subdivide_longest_edge(&mut b);
    }
    (a, b)
}

/// Inserts the midpoint of `points`' own longest edge (treating the
/// list as a closed polygon, wrapping from the last point back to the
/// first), growing its length by one.
fn subdivide_longest_edge(points: &mut Vec<Point>) {
    let n = points.len();
    if n < 2 {
        return; // a single point has no edge to subdivide
    }
    let mut longest_i = 0;
    let mut longest_len = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        let len = points[i].distance(points[j]);
        if len > longest_len {
            longest_len = len;
            longest_i = i;
        }
    }
    let j = (longest_i + 1) % n;
    let midpoint = points[longest_i].lerp(points[j], 0.5);
    points.insert(longest_i + 1, midpoint);
}

/// The correspondence/alignment search §7.4's own review note calls
/// the harder half: tries every rotational starting-point offset of
/// `b` against `a`'s fixed order, in both `b`'s own and its reversed
/// winding direction, and returns whichever reordering of `b` (still
/// length `n`) minimizes total point-travel distance. Naive per-index
/// pairing (offset 0, no reversal) is just one of the `2n` candidates
/// considered here, not assumed to be the winner.
fn best_aligned(a: &[Point], b: &[Point]) -> Vec<Point> {
    let n = a.len();
    debug_assert_eq!(
        n,
        b.len(),
        "best_aligned requires equalize_point_counts to have run first"
    );

    let mut best_total = f64::INFINITY;
    let mut best_points = b.to_vec();

    for reversed in [false, true] {
        let candidate_base: Vec<Point> = if reversed {
            b.iter().rev().copied().collect()
        } else {
            b.to_vec()
        };
        for offset in 0..n {
            let rotated: Vec<Point> = (0..n).map(|i| candidate_base[(i + offset) % n]).collect();
            let total: f64 = a
                .iter()
                .zip(&rotated)
                .map(|(p1, p2)| p1.distance(*p2))
                .sum();
            if total < best_total {
                best_total = total;
                best_points = rotated;
            }
        }
    }
    best_points
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(offset_start: usize, reversed: bool) -> Vec<Point> {
        // A 10x10 square's four corners, in a fixed winding order.
        let corners = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        let mut points: Vec<Point> = if reversed {
            corners.iter().rev().copied().collect()
        } else {
            corners.to_vec()
        };
        points.rotate_left(offset_start);
        points
    }

    #[test]
    fn from_path_and_to_path_round_trip_a_triangle() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((10.0, 0.0));
        path.line_to((5.0, 10.0));
        path.close_path();

        let key = ShapeKey::from_path(&path);
        assert_eq!(
            key.points,
            vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(5.0, 10.0),
            ]
        );

        let rebuilt = key.to_path();
        assert_eq!(ShapeKey::from_path(&rebuilt), key);
    }

    #[test]
    fn interpolate_at_t_zero_and_one_returns_the_equalized_endpoints_exactly() {
        // A triangle (3 points) morphing into a square (4 points) --
        // equalize_point_counts must pad the triangle to 4 points, and
        // t=0.0/t=1.0 must be exact snaps to each (equalized/aligned)
        // end, not an approximation.
        let from = ShapeKey {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(5.0, 10.0),
            ],
        };
        let to = ShapeKey {
            points: square(0, false),
        };

        let at_zero = from.interpolate(&to, 0.0);
        assert_eq!(at_zero.points.len(), 4, "should be padded up to 4 points");
        let (padded_from, _) = equalize_point_counts(from.points.clone(), to.points.clone());
        assert_eq!(at_zero.points, padded_from);

        let at_one = from.interpolate(&to, 1.0);
        let aligned_to = best_aligned(&padded_from, &to.points);
        assert_eq!(at_one.points, aligned_to);
    }

    /// The actual claim §7.4's review note exists to make: two point
    /// lists describing the *same* physical square -- one just starting
    /// at a different corner -- must morph into themselves (near-zero
    /// travel), not collapse. Naive index-0 pairing between `[A,B,C,D]`
    /// and its rotation `[C,D,A,B]` would lerp opposite corners at
    /// t=0.5 (e.g. midpoint(A,C) and midpoint(C,A) both land on the
    /// square's own center), collapsing all four points onto one spot
    /// -- an unambiguous, easy-to-detect failure if the alignment
    /// search isn't actually finding the zero-cost rotation.
    #[test]
    fn same_shape_started_at_a_different_corner_morphs_to_itself_not_a_collapsed_point() {
        let a = ShapeKey {
            points: square(0, false),
        };
        let b = ShapeKey {
            points: square(2, false), // same square, started 2 corners around
        };

        let mid = a.interpolate(&b, 0.5);
        for (p, expected) in mid.points.iter().zip(square(0, false).iter()) {
            assert!(
                p.distance(*expected) < 1e-9,
                "point {p:?} should be ~exactly {expected:?} (the unmoved square) -- \
                 got real movement, meaning the alignment search didn't find the \
                 zero-cost rotation and instead paired mismatched corners"
            );
        }
    }

    /// Same claim, for a winding-direction flip instead of a rotation:
    /// `[A,B,C,D]` (clockwise) against `[A,D,C,B]` (the same square,
    /// counter-clockwise, same starting corner). Only the `reversed`
    /// branch of the search can find this alignment.
    #[test]
    fn same_shape_with_opposite_winding_morphs_to_itself() {
        let a = ShapeKey {
            points: square(0, false),
        };
        let b = ShapeKey {
            points: square(0, true),
        };

        let mid = a.interpolate(&b, 0.5);
        for (p, expected) in mid.points.iter().zip(square(0, false).iter()) {
            assert!(
                p.distance(*expected) < 1e-9,
                "point {p:?} should be ~exactly {expected:?} -- the reversed-winding \
                 branch of the alignment search should have found the zero-cost match"
            );
        }
    }

    #[test]
    fn subdivide_longest_edge_splits_the_actual_longest_edge() {
        // A thin rectangle: the two long edges (length 100) are far
        // longer than the two short ones (length 1) -- subdivision must
        // pick one of those, not an arbitrary edge.
        let mut points = vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        subdivide_longest_edge(&mut points);

        assert_eq!(points.len(), 5);
        // The new point must be the midpoint of a length-100 edge
        // (y ~= 0 or y ~= 1, x ~= 50), not the short edges (x ~= 0/100).
        let inserted_on_long_edge = points
            .iter()
            .any(|p| (p.x - 50.0).abs() < 1e-9 && (p.y - 0.0).abs() < 1e-9)
            || points
                .iter()
                .any(|p| (p.x - 50.0).abs() < 1e-9 && (p.y - 1.0).abs() < 1e-9);
        assert!(
            inserted_on_long_edge,
            "expected the new point at the midpoint of a long (length-100) edge, \
             got points {points:?}"
        );
    }
}
