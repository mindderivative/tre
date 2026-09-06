//! CPU-side geometry for the stencil-and-cover fallback rendering
//! technique (IMPLEMENTATION.md Step 3.3.3) -- the correct tool for a
//! polygon [`crate::triangulate`] rejects via
//! [`crate::SvgError::NotSimplePolygon`]. Unlike ear-clipping, neither
//! function here makes any validity assumption about the input: overlap
//! and self-intersection are exactly what the GPU's stencil-buffer
//! accumulation (`tre-rhi-vulkan`'s `create_stencil_and_cover_pipelines`)
//! is designed to resolve correctly, not something the CPU side needs to
//! reject or work around.

use crate::Polygon;

/// Fans triangles from `polygon.points[0]` to every edge -- always
/// succeeds, for any polygon with at least 3 points, including
/// self-intersecting ones. The anchor point does not need to be inside
/// the polygon, and individual fan triangles are not expected to stay
/// inside it either: correctness comes entirely from how the GPU
/// accumulates per-pixel stencil values across all of them, not from any
/// property of an individual triangle. Returns an empty `Vec` for fewer
/// than 3 points.
#[must_use]
pub fn fan_triangles(polygon: &Polygon) -> Vec<[u32; 3]> {
    let n = polygon.points.len();
    if n < 3 {
        return Vec::new();
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a single tessellated path's point count stays far below u32::MAX, the same \
                   headroom reasoning ARCHITECTURE.md Section 4.1 applies to Depth ID"
    )]
    (1..n as u32 - 1).map(|i| [0, i, i + 1]).collect()
}

/// The axis-aligned bounding box (`min`, `max`) of `polygon`'s points --
/// the extent the stencil-and-cover cover pass's quad is sized to. Panics
/// if `polygon.points` is empty (a programmer error: every caller of this
/// function already has a real polygon in hand, e.g. straight from
/// `crate::parse_svg`, which never returns an empty `Polygon`).
///
/// # Panics
/// Panics if `polygon.points` is empty.
#[must_use]
pub fn bounding_box(polygon: &Polygon) -> ([f32; 2], [f32; 2]) {
    let first = polygon.points[0];
    polygon
        .points
        .iter()
        .fold((first, first), |(min, max), &[x, y]| {
            (
                [min[0].min(x), min[1].min(y)],
                [max[0].max(x), max[1].max(y)],
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_triangles_of_a_square_gives_two_triangles_of_the_correct_area() {
        let square = Polygon {
            points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
        };
        let triangles = fan_triangles(&square);
        assert_eq!(triangles, vec![[0, 1, 2], [0, 2, 3]]);
    }

    #[test]
    fn fan_triangles_makes_no_validity_assumption_about_a_self_intersecting_pentagram() {
        // Connect five circle points in 0,2,4,1,3 order -- the classic
        // pentagram construction, genuinely self-intersecting. Unlike
        // `triangulate`, this must succeed anyway: the point of this
        // function is that it never needs to check.
        let mut points = Vec::with_capacity(5);
        for i in 0_u8..5 {
            let angle =
                std::f32::consts::FRAC_PI_2 + f32::from(i) * 4.0 * std::f32::consts::PI / 5.0;
            points.push([100.0 * angle.cos(), -100.0 * angle.sin()]);
        }
        let pentagram = Polygon { points };
        let triangles = fan_triangles(&pentagram);
        assert_eq!(triangles.len(), 3);
    }

    #[test]
    fn fan_triangles_of_fewer_than_three_points_is_empty() {
        let degenerate = Polygon {
            points: vec![[0.0, 0.0], [1.0, 1.0]],
        };
        assert_eq!(fan_triangles(&degenerate), Vec::<[u32; 3]>::new());
    }

    #[test]
    fn bounding_box_of_a_square_matches_its_corners() {
        let square = Polygon {
            points: vec![[2.0, 3.0], [12.0, 3.0], [12.0, 13.0], [2.0, 13.0]],
        };
        assert_eq!(bounding_box(&square), ([2.0, 3.0], [12.0, 13.0]));
    }

    #[test]
    fn bounding_box_of_an_off_center_triangle() {
        let triangle = Polygon {
            points: vec![[-5.0, 10.0], [5.0, -2.0], [1.0, 1.0]],
        };
        assert_eq!(bounding_box(&triangle), ([-5.0, -2.0], [5.0, 10.0]));
    }
}
