//! Ear-clipping triangulation of a single, simple (non-self-intersecting)
//! polygon contour -- IMPLEMENTATION.md Step 3.3's task 1 half; the
//! harder self-intersecting case is Step 3.3.3's stencil-and-cover
//! fallback, not this module's job.

use crate::{Polygon, SvgError};

/// Points within this distance of each other are treated as coincident --
/// guards against zero-length edges (e.g. a repeated `MoveTo`/`LineTo`
/// pair, or a flattened curve's endpoint exactly matching the next
/// segment's start) producing degenerate, numerically unstable triangles.
const COINCIDENT_EPSILON: f32 = 1e-4;

/// A signed area at or below this (near-zero or negative) is treated as
/// reflex/degenerate, not a valid convex ear tip -- a distinct constant
/// from `COINCIDENT_EPSILON` since this compares an *area*
/// (length-squared), not a *distance*.
const AREA_EPSILON: f32 = 1e-5;

fn signed_area2(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn polygon_signed_area(points: &[[f32; 2]]) -> f32 {
    let n = points.len();
    let mut sum = 0.0;
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        sum += a[0] * b[1] - b[0] * a[1];
    }
    sum * 0.5
}

fn is_coincident(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < COINCIDENT_EPSILON && (a[1] - b[1]).abs() < COINCIDENT_EPSILON
}

/// Strictly inside `(a, b, c)`, assuming `(a, b, c)` is wound
/// counter-clockwise (positive signed area) -- callers ensure this via
/// `polygon_signed_area` before calling.
fn point_in_triangle(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    signed_area2(a, b, p) > AREA_EPSILON
        && signed_area2(b, c, p) > AREA_EPSILON
        && signed_area2(c, a, p) > AREA_EPSILON
}

/// Whether segment `a`-`b` properly crosses segment `c`-`d` -- each
/// segment's endpoints strictly straddle the other segment's line.
/// Touching at a shared endpoint, or one segment merely grazing the
/// other's line without crossing it, is deliberately NOT a proper
/// intersection: adjacent polygon edges sharing a vertex with the
/// candidate diagonal are expected to touch it and must not disqualify
/// an otherwise-valid ear.
fn segments_properly_intersect(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> bool {
    let d1 = signed_area2(c, d, a);
    let d2 = signed_area2(c, d, b);
    let d3 = signed_area2(a, b, c);
    let d4 = signed_area2(a, b, d);
    ((d1 > AREA_EPSILON && d2 < -AREA_EPSILON) || (d1 < -AREA_EPSILON && d2 > AREA_EPSILON))
        && ((d3 > AREA_EPSILON && d4 < -AREA_EPSILON) || (d3 < -AREA_EPSILON && d4 > AREA_EPSILON))
}

/// Whether the polygon described by `points` (its edges, in order, with
/// wraparound) has any two non-adjacent edges that properly cross each
/// other. A real, global check -- independent of ear-clipping's own
/// per-candidate-diagonal checks, which do not by themselves guarantee
/// detecting this (see `triangulate`'s own comment on the bug this
/// closed).
fn has_self_intersection(points: &[[f32; 2]]) -> bool {
    let n = points.len();
    (0..n).any(|i| {
        let (a1, a2) = (points[i], points[(i + 1) % n]);
        // `j` starts at `i + 2` (skip the edge sharing vertex `points[i+1]`
        // with this one) and stops before the edge sharing vertex
        // `points[i]` via wraparound (`i == 0 && j == n - 1`) -- both are
        // adjacent edges expected to touch at their shared vertex, not a
        // crossing.
        (i + 2..n).any(|j| {
            if i == 0 && j == n - 1 {
                return false;
            }
            let (b1, b2) = (points[j], points[(j + 1) % n]);
            segments_properly_intersect(a1, a2, b1, b2)
        })
    })
}

/// Triangulates `polygon`'s single contour via ear-clipping, returning
/// triangles as index triples into `polygon.points`.
///
/// Consecutive coincident points are dropped first (a defensive pre-pass,
/// not an assumption the caller already did this), and the contour is
/// normalized to counter-clockwise winding before clipping -- the input's
/// original winding order does not affect the result.
///
/// # Errors
/// Returns [`SvgError::NotSimplePolygon`] if `has_self_intersection`
/// finds two non-adjacent edges of the original contour crossing each
/// other, or if no valid ear can be found before every remaining vertex
/// is exhausted -- the contour is self-intersecting or otherwise not
/// simple, and [`crate::stencil`]'s stencil-and-cover fallback
/// (IMPLEMENTATION.md Step 3.3.3) is the correct tool for it, not a guess
/// from this algorithm.
#[allow(
    clippy::similar_names,
    reason = "edge_a_idx/edge_b_idx are literally the two endpoints of one edge -- an 'a'/'b' \
               pairing convention, not an accidental near-collision"
)]
pub fn triangulate(polygon: &Polygon) -> Result<Vec<[u32; 3]>, SvgError> {
    // `points`/`remaining` below are a WORKING copy (deduped, possibly
    // reversed) -- deliberately not the same indexing as the caller's
    // `polygon.points`. `original_index[k]` records which index into
    // `polygon.points` working-copy position `k` actually came from, so
    // every returned triangle can be translated back to indices valid
    // against the caller's own array before this function returns them.
    // A real bug this exact split fixed: an earlier version emitted
    // indices straight from the (possibly-reversed) working copy, which
    // silently produced a corrupted mesh for any polygon whose original
    // winding needed reversing -- caught by `svg_tessellation_demo`'s
    // pixel readback, not by this module's own area-only unit tests
    // (a symmetric shape's *total* area is often unchanged by exactly
    // this class of index-relabeling bug, even though individual
    // triangles are wrong).
    let mut points: Vec<[f32; 2]> = Vec::with_capacity(polygon.points.len());
    let mut original_index: Vec<u32> = Vec::with_capacity(polygon.points.len());
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a single tessellated path's point count stays far below u32::MAX, the same \
                   headroom reasoning ARCHITECTURE.md Section 4.1 applies to Depth ID"
    )]
    for (idx, &p) in polygon.points.iter().enumerate() {
        if points.last().is_some_and(|&last| is_coincident(last, p)) {
            continue;
        }
        points.push(p);
        original_index.push(idx as u32);
    }
    if let Some(&last) = points.last() {
        if points.len() > 1 && is_coincident(points[0], last) {
            points.pop();
            original_index.pop();
        }
    }

    if points.len() < 3 {
        return Ok(Vec::new());
    }

    // A real gap found via IMPLEMENTATION.md Step 3.3.3's pentagram demo:
    // the ear-validity checks below (vertex-inside / edge-crosses-diagonal)
    // only ever examine a candidate diagonal against the CURRENTLY
    // REMAINING boundary during clipping -- they do not, by themselves,
    // guarantee catching every self-intersecting ORIGINAL polygon. A
    // classic pentagram (five points connected in `0,2,4,1,3` order) has
    // real, non-adjacent edges that cross each other, yet clipped cleanly
    // with no diagonal ever conflicting with a remaining edge along the
    // way, silently producing a plausible-looking but geometrically wrong
    // triangulation instead of being rejected. This explicit, global
    // check -- independent of the clipping process -- runs once up front
    // instead.
    if has_self_intersection(&points) {
        return Err(SvgError::NotSimplePolygon);
    }

    if polygon_signed_area(&points) < 0.0 {
        points.reverse();
        original_index.reverse();
    }

    let n = points.len();
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a single tessellated path's point count stays far below u32::MAX, the same \
                   headroom reasoning ARCHITECTURE.md Section 4.1 applies to Depth ID"
    )]
    let mut remaining: Vec<u32> = (0..n as u32).collect();
    let mut triangles = Vec::with_capacity(n.saturating_sub(2));

    while remaining.len() > 3 {
        let m = remaining.len();
        let mut found_ear = false;

        for i in 0..m {
            let prev_idx = remaining[(i + m - 1) % m];
            let cur_idx = remaining[i];
            let next_idx = remaining[(i + 1) % m];
            let (prev, cur, next) = (
                points[prev_idx as usize],
                points[cur_idx as usize],
                points[next_idx as usize],
            );

            if signed_area2(prev, cur, next) <= AREA_EPSILON {
                continue; // reflex or degenerate vertex, not a valid ear tip
            }

            // A valid ear needs BOTH of the following, checked against
            // every OTHER remaining vertex/edge (not just the ear's own
            // three corners):
            //
            // 1. No remaining vertex strictly inside the candidate
            //    triangle. Necessary but NOT sufficient on its own: a
            //    vertex can sit exactly on one of the ear's own edges
            //    (e.g. on the new diagonal itself, as this crate's
            //    L-shape unit test demonstrated) without ever being
            //    strictly "inside" the triangle, while an edge through it
            //    still crosses the triangle boundary.
            // 2. No remaining edge properly crosses the diagonal
            //    (`prev`, `next`). Necessary but NOT sufficient on its
            //    own either: a remaining vertex can lie fully inside the
            //    triangle while BOTH of its own edges terminate exactly
            //    at two of the triangle's own corners -- such an edge
            //    never "properly crosses" anything (it shares an
            //    endpoint with the diagonal by construction), yet the
            //    vertex it reaches is a real intrusion. This crate's own
            //    star-polygon demo (`svg_tessellation_demo`) caught
            //    exactly this case after check 1 alone was replaced with
            //    check 2 alone instead of keeping both.
            //
            // Only the two edges actually consumed by this cut (prev-cur
            // and cur-next) are excluded from check 2 -- not every edge
            // that merely touches prev/cur/next at one shared endpoint,
            // since `segments_properly_intersect`'s strict inequalities
            // already treat a shared-endpoint touch as "not a proper
            // crossing" on their own.
            let no_vertex_inside = remaining.iter().all(|&k| {
                k == prev_idx
                    || k == cur_idx
                    || k == next_idx
                    || !point_in_triangle(points[k as usize], prev, cur, next)
            });
            let no_edge_crosses = (0..m).all(|j| {
                let edge_a_idx = remaining[j];
                let edge_b_idx = remaining[(j + 1) % m];
                let is_consumed_edge = (edge_a_idx == prev_idx && edge_b_idx == cur_idx)
                    || (edge_a_idx == cur_idx && edge_b_idx == next_idx);
                is_consumed_edge
                    || !segments_properly_intersect(
                        prev,
                        next,
                        points[edge_a_idx as usize],
                        points[edge_b_idx as usize],
                    )
            });

            if no_vertex_inside && no_edge_crosses {
                triangles.push([
                    original_index[prev_idx as usize],
                    original_index[cur_idx as usize],
                    original_index[next_idx as usize],
                ]);
                remaining.remove(i);
                found_ear = true;
                break;
            }
        }

        if !found_ear {
            return Err(SvgError::NotSimplePolygon);
        }
    }

    triangles.push([
        original_index[remaining[0] as usize],
        original_index[remaining[1] as usize],
        original_index[remaining[2] as usize],
    ]);
    Ok(triangles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area_of_triangles(points: &[[f32; 2]], triangles: &[[u32; 3]]) -> f32 {
        triangles
            .iter()
            .map(|&[a, b, c]| {
                signed_area2(points[a as usize], points[b as usize], points[c as usize]).abs() / 2.0
            })
            .sum()
    }

    /// Orientation-agnostic point-in-triangle check for test assertions --
    /// unlike the module's own `point_in_triangle`, this doesn't assume
    /// `(a, b, c)` is wound any particular way, since a triangle returned
    /// from `triangulate` (translated back through `original_index`) can
    /// carry either winding depending on the caller's own input order.
    fn point_in_triangle_any_winding(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
        let d1 = signed_area2(p, a, b);
        let d2 = signed_area2(p, b, c);
        let d3 = signed_area2(p, c, a);
        let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(has_neg && has_pos)
    }

    #[test]
    fn triangulates_a_square_into_two_triangles_of_the_correct_total_area() {
        let square = Polygon {
            points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
        };
        let triangles = triangulate(&square).expect("a square is a simple polygon");
        assert_eq!(triangles.len(), 2);
        assert!((area_of_triangles(&square.points, &triangles) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn triangulates_a_non_convex_l_shape_correctly() {
        // An L-shape: a 10x10 square with its top-right 5x5 quadrant
        // removed. True area = 100 - 25 = 75.
        let l_shape = Polygon {
            points: vec![
                [0.0, 0.0],
                [10.0, 0.0],
                [10.0, 5.0],
                [5.0, 5.0],
                [5.0, 10.0],
                [0.0, 10.0],
            ],
        };
        let triangles = triangulate(&l_shape).expect("an L-shape is a simple polygon");
        assert_eq!(triangles.len(), l_shape.points.len() - 2);
        assert!((area_of_triangles(&l_shape.points, &triangles) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn triangulates_a_clockwise_wound_polygon_identically_to_counter_clockwise() {
        let mut square = Polygon {
            points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
        };
        square.points.reverse(); // now clockwise
        let triangles = triangulate(&square).expect("winding order must not affect correctness");
        assert_eq!(triangles.len(), 2);
        assert!((area_of_triangles(&square.points, &triangles) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn triangulates_a_five_pointed_star_excluding_its_concave_notches() {
        // 10-vertex star (5 outer points, 5 inner notches) -- the same
        // shape `svg_tessellation_demo` renders. A triangle *count* and
        // *total area* check alone is not sufficient to catch every
        // triangulation bug: a real regression here emitted triangles
        // that summed to a total area matching this test's old, weaker
        // form while one of them actually covered a concave notch and
        // missed part of a spike (caught only by the demo's pixel
        // readback at the time). This version also checks that a known
        // concave-notch point is covered by NO emitted triangle.
        let (outer_r, inner_r) = (100.0f32, 40.0f32);
        let mut points = Vec::with_capacity(10);
        for i in 0_u8..10 {
            let angle = std::f32::consts::FRAC_PI_2 + f32::from(i) * std::f32::consts::PI / 5.0;
            let r = if i % 2 == 0 { outer_r } else { inner_r };
            points.push([r * angle.cos(), -r * angle.sin()]);
        }
        let star = Polygon { points };

        let triangles = triangulate(&star).expect("a simple (non-self-intersecting) star polygon");
        assert_eq!(triangles.len(), star.points.len() - 2);

        let true_area = polygon_signed_area(&star.points).abs();
        let triangulated_area = area_of_triangles(&star.points, &triangles);
        assert!(
            (triangulated_area - true_area).abs() < 1e-2,
            "triangulated area {triangulated_area} does not match the true polygon area {true_area}"
        );

        // Vertex index 1's own angle, pushed 5 units past its radius --
        // inside the star's bounding box, but in the concave notch,
        // outside the actual polygon.
        let notch_angle = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI / 5.0;
        let notch_point = [
            (inner_r + 5.0) * notch_angle.cos(),
            -(inner_r + 5.0) * notch_angle.sin(),
        ];
        for &[a, b, c] in &triangles {
            assert!(
                !point_in_triangle_any_winding(
                    notch_point,
                    star.points[a as usize],
                    star.points[b as usize],
                    star.points[c as usize]
                ),
                "a concave-notch point must not be covered by any triangle, but triangle [{a}, {b}, {c}] covers it"
            );
        }
    }

    #[test]
    fn rejects_a_classic_self_intersecting_pentagram() {
        // Five circle points connected in 0,2,4,1,3 order -- the classic
        // pentagram construction, with real, non-adjacent edges that
        // cross each other. A real bug this exact test was written to
        // pin down: the ear-validity checks alone (vertex-inside /
        // edge-crosses-diagonal) clip this cleanly with no diagonal ever
        // conflicting with a remaining edge, silently producing a
        // plausible-looking but wrong triangulation instead of being
        // rejected -- caught only by IMPLEMENTATION.md Step 3.3.3's own
        // demo, not by this crate's own unit tests, until this test was
        // added alongside the fix (`has_self_intersection`).
        let mut raw = Vec::with_capacity(5);
        for i in 0_u8..5 {
            let angle =
                std::f32::consts::FRAC_PI_2 + f32::from(i) * 2.0 * std::f32::consts::PI / 5.0;
            raw.push([100.0 * angle.cos(), -100.0 * angle.sin()]);
        }
        let pentagram = Polygon {
            points: vec![raw[0], raw[2], raw[4], raw[1], raw[3]],
        };
        assert!(matches!(
            triangulate(&pentagram),
            Err(SvgError::NotSimplePolygon)
        ));
    }

    #[test]
    fn fewer_than_three_points_triangulates_to_nothing_without_erroring() {
        let degenerate = Polygon {
            points: vec![[0.0, 0.0], [1.0, 1.0]],
        };
        assert_eq!(triangulate(&degenerate).unwrap(), Vec::<[u32; 3]>::new());
    }

    #[test]
    fn drops_consecutive_coincident_points_before_triangulating() {
        let square_with_dup = Polygon {
            points: vec![
                [0.0, 0.0],
                [0.0, 0.0], // exact duplicate of the previous point
                [10.0, 0.0],
                [10.0, 10.0],
                [0.0, 10.0],
            ],
        };
        let triangles =
            triangulate(&square_with_dup).expect("duplicate points must be dropped, not rejected");
        assert_eq!(triangles.len(), 2);
    }
}
