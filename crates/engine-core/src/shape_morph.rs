//! §7.4's shape-morph module: the one MD3 mechanism with no library to
//! lean on (originally built at §14 step 10). "Equalize point count,
//! then lerp" is only half the real technique -- §7.4's own review
//! note names the missing, harder half explicitly: naive per-index
//! interpolation assumes point *N* on one shape visually corresponds
//! to point *N* on the other, which is usually false and produces
//! self-intersecting or wildly-rotating mid-morph geometry. This
//! module runs a real correspondence/alignment search -- every
//! rotational starting-point offset, in both winding directions,
//! scored by total point-travel distance -- before ever lerping a
//! single point.
//!
//! **M7 Phase 4 (§7.4): moved here from `engine-md3`, verbatim.**
//! Confirmed by reading the whole file before moving it: this module's
//! only real dependencies were always `Interpolate` and `peniko::
//! kurbo` -- no `ColorScheme`/`DynamicTheme`/`material_colors`, nothing
//! genuinely MD3-specific. `engine-core::node::PaintProperties`'s own
//! doc comment already named `shape: Animated<ShapeKey>` as always
//! intended to live directly on `PaintProperties` (matching
//! `ARCHITECTURE.md` §2's own original struct sketch) -- impossible
//! while `ShapeKey` lived in `engine-md3`, since `engine-core` cannot
//! depend on it (§4). The identical resolution M7 Phase 1 already made
//! for `MotionCurve` (real MD3 curve values landed in `engine-core`,
//! not a separate `engine_md3::motion` module) applies here. Confirmed
//! via grep before moving: zero references to `engine_md3::ShapeKey`/
//! `engine_md3::shape_morph` anywhere in the workspace, so no
//! backward-compat re-export was needed.
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

use crate::animation::Interpolate;
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
    /// M7 Phase 4 (§7.4): the "no shape ever set" sentinel `Paint
    /// Properties::new` defaults `shape` to -- an empty point list, the
    /// same value `to_path()`'s own `if let Some(&first) = points.
    /// next()` short-circuit already renders as an empty `BezPath` with
    /// zero code changes needed there.
    pub fn empty() -> Self {
        Self { points: Vec::new() }
    }

    /// `paint_node`'s own "is a real morph active" check -- true for a
    /// node that never called `ShapeKey::from_path`/animated `shape` at
    /// all, matching every other additive `PaintProperties` field's
    /// "off unless a caller opts in" contract.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Extracts a closed shape's own vertex sequence: the endpoint of
    /// every `MoveTo`/`LineTo`/`QuadTo`/`CurveTo` segment, in path
    /// order, dropping curve control handles (see this module's own
    /// doc comment for why) and `ClosePath` (it names no new point).
    pub fn from_path(path: &BezPath) -> Self {
        let points = path
            .iter()
            .filter_map(|el| match el {
                PathEl::MoveTo(p)
                | PathEl::LineTo(p)
                | PathEl::QuadTo(_, p)
                | PathEl::CurveTo(_, _, p) => Some(p),
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

    /// M39 Phase 3 (§5, §7): a real, general straight-edge polygon
    /// inset -- `engine-render`'s own border-paint code needs this to
    /// stroke a real, active shape morph's own border *inside* its
    /// fill's edge, the identical real "inset by half the stroke
    /// width" contract `RoundedRect`-based borders already get via
    /// `corner_radius - inset` (M30 Phase 1). A raw vertex polygon has
    /// no per-corner radius to shrink the way a `RoundedRect` does, so
    /// this shrinks the polygon itself instead: offsets each real edge
    /// inward along its own normal by `amount`, then finds each new
    /// vertex as the intersection of its two adjacent offset edges (a
    /// real miter join, the same technique any 2D vector-graphics
    /// polygon-offset implementation uses). Confirmed via direct
    /// source read before writing this: `kurbo = "0.13.1"`'s own
    /// `offset.rs` module offsets a single cubic Bézier curve, not a
    /// closed straight-edge polygon -- genuinely the wrong tool for
    /// `ShapeKey`'s own vertices-only shape, not merely unused; no
    /// general polygon-inset operation exists anywhere in this
    /// codebase's own kurbo usage (confirmed via grep), so this is
    /// real, new, self-contained geometry.
    ///
    /// Which of an edge's two perpendicular normals points "inward" is
    /// resolved per-edge by comparing against the real polygon
    /// centroid (whichever normal points toward it), not by assuming a
    /// fixed winding order -- `ShapeKey::from_path` extracts vertices
    /// straight from whatever `BezPath` a caller supplied, with no
    /// guaranteed winding direction, and `interpolate`'s own real
    /// alignment search (this module's own doc comment) can reorder
    /// them further, so this can't assume CW/CCW the way a purpose-
    /// built polygon type could.
    ///
    /// **Real, stated v1 scope limit:** correct for the real border
    /// widths this codebase actually uses (MD3's own 1-4dp outline
    /// range) against MD3-scale shapes -- not proven robust for an
    /// inset large enough to invert a polygon's own edges or force two
    /// non-adjacent offset edges to cross (a real, harder self-
    /// intersection-avoidance problem no real caller here needs
    /// solved). Two exactly parallel adjacent offset edges (a genuine
    /// straight run across a "vertex" the caller's own path never
    /// actually turned at) fall back to that edge's own offset start
    /// point, rather than an undefined line intersection.
    pub fn inset_path(&self, amount: f64) -> BezPath {
        let n = self.points.len();
        if n < 3 || amount <= 0.0 {
            return self.to_path();
        }

        let centroid = {
            let (sum_x, sum_y) = self
                .points
                .iter()
                .fold((0.0, 0.0), |(sx, sy), p| (sx + p.x, sy + p.y));
            Point::new(sum_x / n as f64, sum_y / n as f64)
        };

        // One offset line (a point on it, plus its unit direction) per
        // real edge `i -> i+1`.
        let offset_lines: Vec<(Point, peniko::kurbo::Vec2)> = (0..n)
            .map(|i| {
                let p0 = self.points[i];
                let p1 = self.points[(i + 1) % n];
                let edge = p1 - p0;
                let dir = if edge.hypot() > 1e-9 {
                    edge.normalize()
                } else {
                    // A real, degenerate zero-length edge -- none of
                    // this module's own real generated shapes ever
                    // produce one, so this is a defensive fallback
                    // (avoids a NaN normal), not a handled real case.
                    peniko::kurbo::Vec2::new(1.0, 0.0)
                };
                let normal = peniko::kurbo::Vec2::new(-dir.y, dir.x);
                let midpoint = p0.lerp(p1, 0.5);
                let inward = if normal.x * (centroid.x - midpoint.x)
                    + normal.y * (centroid.y - midpoint.y)
                    > 0.0
                {
                    normal
                } else {
                    -normal
                };
                (p0 + inward * amount, dir)
            })
            .collect();

        let inset_points: Vec<Point> = (0..n)
            .map(|i| {
                let (p_prev, d_prev) = offset_lines[(i + n - 1) % n];
                let (p_curr, d_curr) = offset_lines[i];
                line_intersection(p_prev, d_prev, p_curr, d_curr).unwrap_or(p_curr)
            })
            .collect();

        let mut path = BezPath::new();
        let mut points = inset_points.into_iter();
        if let Some(first) = points.next() {
            path.move_to(first);
            for p in points {
                path.line_to(p);
            }
            path.close_path();
        }
        path
    }
}

/// `ShapeKey::inset_path`'s own real line-line intersection helper --
/// solves `p1 + t*d1 == p2 + s*d2` for `t`, returning `None` when the
/// two lines are parallel (or near enough that the solve would blow up
/// numerically), the identical "return `None` rather than propagate a
/// near-infinite value" contract every other real geometry helper in
/// this codebase already uses for a degenerate input.
fn line_intersection(
    p1: Point,
    d1: peniko::kurbo::Vec2,
    p2: Point,
    d2: peniko::kurbo::Vec2,
) -> Option<Point> {
    let denom = d1.x * d2.y - d1.y * d2.x;
    if denom.abs() < 1e-9 {
        return None;
    }
    let diff = p2 - p1;
    let t = (diff.x * d2.y - diff.y * d2.x) / denom;
    Some(p1 + d1 * t)
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

/// M39 Phase 2 (§5, §7): four real, procedurally-generated shapes for
/// `LoadingIndicator`'s own real looping morph -- a real, honest
/// approximation of MD3 Expressive's own named seven-shape sequence
/// (Pentagon, Pill, Cookie[N], Oval among them), not sourced vertex-
/// for-vertex from official Material Design SVG assets (scoped via
/// `AskUserQuestion`: that would need a real asset-acquisition
/// pipeline this codebase has no precedent for). Built directly to a
/// real `w x h` box (not normalized/rescaled later) since `ShapeKey`
/// itself has no scale transform -- the caller (`engine-py::add_
/// loading_indicator`) already knows its own real size at construction
/// time, the identical real "resolve real geometry once, not every
/// frame" precedent every other real MD3 component factory already
/// follows.
pub mod loading_indicator_shapes {
    use std::f64::consts::{FRAC_PI_2, PI};

    use peniko::kurbo::{Ellipse, Point, RoundedRect, Shape};

    use super::ShapeKey;

    /// A regular pentagon, point-up, inscribed in the real `w x h`
    /// box's own bounding circle.
    pub fn pentagon(w: f64, h: f64) -> ShapeKey {
        let (cx, cy) = (w / 2.0, h / 2.0);
        let r = cx.min(cy);
        ShapeKey::from_path(&ngon_path(cx, cy, r, r, 5, 1.0))
    }

    /// A real pill -- fully rounded on both ends, matching `Split
    /// Button`'s own real "relaxed" corner shape (M38 Phase 4).
    pub fn pill(w: f64, h: f64) -> ShapeKey {
        let radius = h / 2.0;
        // A looser real tessellation tolerance than the fill-path
        // default (`0.1`) deliberately -- a smoother curve produces
        // many more real vertices, which would badly outnumber the
        // other three shapes' own small, hand-authored vertex counts
        // and skew `ShapeKey::interpolate`'s own real "pad the shorter
        // list" correspondence search toward a poor visual morph.
        ShapeKey::from_path(&RoundedRect::new(0.0, 0.0, w, h, radius).to_path(1.0))
    }

    /// A real "cookie" -- a soft, scalloped shape, alternating between
    /// a real outer and a real, slightly smaller inner radius around
    /// six real real points (twelve total vertices).
    pub fn cookie(w: f64, h: f64) -> ShapeKey {
        let (cx, cy) = (w / 2.0, h / 2.0);
        let r = cx.min(cy);
        ShapeKey::from_path(&scalloped_path(cx, cy, r, 6, 0.82))
    }

    /// A real oval -- wider than tall, the identical real proportions
    /// (2:1) MD3 Expressive's own real "Oval" shape has.
    pub fn oval(w: f64, h: f64) -> ShapeKey {
        let (cx, cy) = (w / 2.0, h / 2.0);
        let r = cx.min(cy);
        // The same real, looser tessellation tolerance `pill` already
        // uses, for the identical real reason.
        ShapeKey::from_path(&Ellipse::new((cx, cy), (r, r * 0.5), 0.0).to_path(1.0))
    }

    /// A real, regular N-gon -- `pentagon`'s own shared real generator,
    /// `rx`/`ry` independent so a future real caller could build an
    /// elongated one without a second function.
    fn ngon_path(
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        sides: usize,
        scale: f64,
    ) -> peniko::kurbo::BezPath {
        let mut path = peniko::kurbo::BezPath::new();
        for i in 0..sides {
            let angle = -FRAC_PI_2 + i as f64 * (2.0 * PI / sides as f64);
            let p = Point::new(cx + rx * scale * angle.cos(), cy + ry * scale * angle.sin());
            if i == 0 {
                path.move_to(p);
            } else {
                path.line_to(p);
            }
        }
        path.close_path();
        path
    }

    /// A real, soft scalloped shape -- `scallops` real outer points,
    /// each real pair separated by one real, slightly-closer-in inner
    /// point (`inner_ratio` of the real outer radius), `2 * scallops`
    /// vertices total.
    fn scalloped_path(
        cx: f64,
        cy: f64,
        r: f64,
        scallops: usize,
        inner_ratio: f64,
    ) -> peniko::kurbo::BezPath {
        let mut path = peniko::kurbo::BezPath::new();
        let points = scallops * 2;
        let inner_r = r * inner_ratio;
        for i in 0..points {
            let angle = -FRAC_PI_2 + i as f64 * (2.0 * PI / points as f64);
            let radius = if i % 2 == 0 { r } else { inner_r };
            let p = Point::new(cx + radius * angle.cos(), cy + radius * angle.sin());
            if i == 0 {
                path.move_to(p);
            } else {
                path.line_to(p);
            }
        }
        path.close_path();
        path
    }
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
    fn empty_shape_key_is_empty_and_renders_an_empty_path() {
        let key = ShapeKey::empty();
        assert!(key.is_empty());
        assert_eq!(
            key.to_path().elements().len(),
            0,
            "an empty ShapeKey must build a genuinely empty BezPath, the true no-op \
             paint_node relies on for a node that never set a shape"
        );
    }

    #[test]
    fn a_real_shape_is_not_empty() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((10.0, 0.0));
        path.close_path();
        assert!(!ShapeKey::from_path(&path).is_empty());
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

    /// M39 Phase 3 (§5, §7): a real, hand-verified case -- `square`'s
    /// own 10x10 corners, inset by 2.0, must produce exactly the
    /// 6x6 square `(2,2)-(8,2)-(8,8)-(2,8)` a real ruler-and-compass
    /// shrink of every edge by 2 units would give. Hand-traced through
    /// `inset_path`'s own real per-edge-normal-then-intersect math
    /// before writing this assertion, not just picked because it
    /// looked plausible.
    #[test]
    fn inset_path_of_a_square_shrinks_every_side_by_the_real_amount() {
        let key = ShapeKey {
            points: square(0, false),
        };
        let inset = key.inset_path(2.0);
        let points: Vec<Point> = inset
            .iter()
            .filter_map(|el| match el {
                PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(p),
                _ => None,
            })
            .collect();
        let expected = [
            Point::new(2.0, 2.0),
            Point::new(8.0, 2.0),
            Point::new(8.0, 8.0),
            Point::new(2.0, 8.0),
        ];
        assert_eq!(points.len(), expected.len());
        for (p, e) in points.iter().zip(expected.iter()) {
            assert!(
                p.distance(*e) < 1e-9,
                "expected inset corner ~{e:?}, got {p:?}"
            );
        }
    }

    /// The reversed-winding twin of the test above -- `inset_path`
    /// resolves each edge's own inward normal via the real polygon
    /// centroid (not an assumed CW/CCW winding), so a reversed vertex
    /// order must shrink identically, not outward or not at all.
    #[test]
    fn inset_path_shrinks_inward_regardless_of_winding_direction() {
        let key = ShapeKey {
            points: square(0, true),
        };
        let inset = key.inset_path(2.0);
        let points: Vec<Point> = inset
            .iter()
            .filter_map(|el| match el {
                PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(p),
                _ => None,
            })
            .collect();
        for p in &points {
            assert!(
                (2.0..=8.0).contains(&p.x) && (2.0..=8.0).contains(&p.y),
                "every inset corner of a reversed-winding square must still land \
                 strictly inside the original 0..10 bounds, got {p:?}"
            );
        }
    }

    #[test]
    fn inset_path_with_zero_amount_is_the_same_as_to_path() {
        let key = ShapeKey {
            points: square(0, false),
        };
        assert_eq!(key.inset_path(0.0).elements(), key.to_path().elements());
    }

    #[test]
    fn inset_path_of_a_degenerate_two_point_shape_is_a_true_no_op() {
        let key = ShapeKey {
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
        };
        assert_eq!(key.inset_path(2.0).elements(), key.to_path().elements());
    }
}
