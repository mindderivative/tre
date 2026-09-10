//! Real fill tessellation via lyon's sweep-line `FillTessellator` --
//! Phase 10 Step 10.2 follow-up, replacing this crate's original
//! hand-rolled ear-clipping triangulator (`triangulate.rs`, retired) and
//! its stencil-and-cover GPU fallback (`stencil.rs`, retired -- Phase 3
//! Step 3.3.3) for everything ear-clipping couldn't handle: compound
//! shapes with holes, and genuinely self-intersecting contours.
//! `nical/lyon` is the industry-standard Rust 2D tessellation library
//! (real, actively maintained, used across the Rust graphics ecosystem);
//! see REVIEW.md for the full account of why this project moved to it.

use crate::{Polygon, SvgError};
use lyon::math::point;
use lyon::path::Path;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillRule as LyonFillRule, FillTessellator, FillVertex,
    FillVertexConstructor, VertexBuffers,
};

/// The tessellation tolerance every call here uses -- the same value
/// `flatten.rs`'s own curve flattening already used, kept as one shared
/// constant now that both live in the same crate for the same reason
/// (bounding polyline deviation from the true curve).
const TOLERANCE: f32 = 0.25;

/// SVG's own two fill rules (`fill-rule: nonzero | evenodd`) -- a plain
/// re-statement of `lyon::tessellation::FillRule`'s own two variants so
/// this crate's public API doesn't force every caller to name a lyon
/// type directly just to pick a fill rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

impl From<FillRule> for LyonFillRule {
    fn from(rule: FillRule) -> Self {
        match rule {
            FillRule::NonZero => Self::NonZero,
            FillRule::EvenOdd => Self::EvenOdd,
        }
    }
}

/// Emits `UiVertex`s carrying one flat, caller-supplied color and no
/// UV/params -- `uv`/`params` zeroed, matching this crate's own
/// pre-existing `to_ui_vertices` convention: a plain triangle soup has
/// no SDF to evaluate.
struct SolidColorVertex(u32);

impl FillVertexConstructor<tre_engine::UiVertex> for SolidColorVertex {
    fn new_vertex(&mut self, vertex: FillVertex<'_>) -> tre_engine::UiVertex {
        let p = vertex.position();
        tre_engine::UiVertex {
            position: [p.x, p.y],
            uv: [0.0, 0.0],
            color: self.0,
            params: [0.0; 3],
        }
    }
}

/// Builds one lyon `Path` from one or more already-flattened boundary
/// contours -- each contour becomes its own `begin`/`line_to`.../`end`
/// sequence within the SAME path, so a fill rule correctly resolves a
/// compound shape (e.g. a ring: an outer contour plus an inner "hole"
/// contour) across all of them together, not each tessellated in
/// isolation. A contour with fewer than 3 points is skipped (degenerate,
/// contributes no area).
fn build_path(contours: &[Polygon]) -> Path {
    let mut builder = Path::builder();
    for contour in contours {
        if contour.points.len() < 3 {
            continue;
        }
        let mut points = contour.points.iter();
        let &first = points.next().expect("length checked above");
        builder.begin(point(first[0], first[1]));
        for &p in points {
            builder.line_to(point(p[0], p[1]));
        }
        builder.end(true);
    }
    builder.build()
}

/// Tessellates `contours` (one or more already-flattened boundary
/// polygons, together forming one compound fill region under
/// `fill_rule`) directly into flat-colored `UiVertex`/index buffers,
/// ready for the exact same `upload_buffer`/`draw_indexed` path every
/// flat-color example already uses.
///
/// Unlike this crate's original `triangulate` + `to_ui_vertices` pair,
/// this is ONE real tessellation pass, not "triangulate one contour's
/// own point list, then separately convert the result": lyon's fill
/// tessellator can introduce genuinely new vertices (e.g. at a real
/// self-intersection it resolves) that don't correspond to any single
/// input point, so a result shaped as "indices into the caller's own
/// point array" -- the old `triangulate`'s own contract -- is not
/// something a general tessellator can honestly promise. Producing the
/// final vertex buffer directly is the correct contract for what this
/// function actually does.
///
/// # Errors
/// Returns [`SvgError::TessellationFailed`] if lyon's own tessellator
/// reports an internal failure -- rare in practice: lyon's fill
/// tessellator is designed to succeed on self-intersecting and
/// multi-contour input, unlike the ear-clipping algorithm it replaces,
/// which rejected such input outright ([`SvgError::NotSimplePolygon`],
/// now retired -- see this module's own top-level doc comment).
pub fn tessellate_fill(
    contours: &[Polygon],
    fill_rule: FillRule,
    rgba: u32,
) -> Result<(Vec<tre_engine::UiVertex>, Vec<u32>), SvgError> {
    let path = build_path(contours);
    let mut geometry: VertexBuffers<tre_engine::UiVertex, u32> = VertexBuffers::new();
    let mut tessellator = FillTessellator::new();
    tessellator
        .tessellate_path(
            &path,
            &FillOptions::tolerance(TOLERANCE).with_fill_rule(fill_rule.into()),
            &mut BuffersBuilder::new(&mut geometry, SolidColorVertex(rgba)),
        )
        .map_err(|_| SvgError::TessellationFailed)?;
    Ok((geometry.vertices, geometry.indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle_area(vertices: &[tre_engine::UiVertex], indices: &[u32]) -> f32 {
        indices
            .chunks_exact(3)
            .map(|tri| {
                let [a, b, c] = [
                    vertices[tri[0] as usize].position,
                    vertices[tri[1] as usize].position,
                    vertices[tri[2] as usize].position,
                ];
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0
            })
            .sum()
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "uv is zeroed by construction (SolidColorVertex::new_vertex's own literal \
                   [0.0, 0.0]), not a rounded computed value"
    )]
    fn tessellates_a_square_into_the_correct_total_area() {
        let square = Polygon {
            points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
        };
        let (vertices, indices) = tessellate_fill(&[square], FillRule::NonZero, 0xFF00_FFFF)
            .expect("a square tessellates");
        assert!((triangle_area(&vertices, &indices) - 100.0).abs() < 1e-3);
        for v in &vertices {
            assert_eq!(v.color, 0xFF00_FFFF);
            assert_eq!(v.uv, [0.0, 0.0]);
        }
    }

    #[test]
    fn tessellates_a_non_convex_l_shape_correctly() {
        // A 10x10 square with its top-right 5x5 quadrant removed. True
        // area = 100 - 25 = 75.
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
        let (vertices, indices) = tessellate_fill(&[l_shape], FillRule::NonZero, 0xFFFF_FFFF)
            .expect("an L-shape tessellates");
        assert!((triangle_area(&vertices, &indices) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn tessellates_a_classic_self_intersecting_pentagram_without_rejecting_it() {
        // Five circle points connected in 0,2,4,1,3 order -- the classic
        // pentagram construction. The old hand-rolled ear-clipper
        // rejected this outright (SvgError::NotSimplePolygon); lyon's
        // real sweep-line fill tessellator resolves it directly instead
        // -- the whole reason this project adopted it.
        let mut raw = Vec::with_capacity(5);
        for i in 0_u8..5 {
            let angle =
                std::f32::consts::FRAC_PI_2 + f32::from(i) * 2.0 * std::f32::consts::PI / 5.0;
            raw.push([100.0 * angle.cos(), -100.0 * angle.sin()]);
        }
        let pentagram = Polygon {
            points: vec![raw[0], raw[2], raw[4], raw[1], raw[3]],
        };
        let (vertices, indices) = tessellate_fill(&[pentagram], FillRule::NonZero, 0xFFFF_FFFF)
            .expect("lyon's real fill tessellator must succeed on a self-intersecting contour");
        assert!(
            !indices.is_empty(),
            "a real pentagram has real area; the tessellation must not be empty"
        );
        assert!(triangle_area(&vertices, &indices) > 0.0);
    }

    #[test]
    fn tessellates_a_ring_with_a_real_hole_using_two_contours_and_even_odd() {
        // An outer 20x20 square with an inner 10x10 square "hole,"
        // wound the SAME direction (even-odd doesn't care about
        // relative winding, unlike non-zero) -- real area = 400 - 100 =
        // 300. This is exactly the compound-shape case the old
        // single-contour ear-clipper could never express at all.
        let outer = Polygon {
            points: vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]],
        };
        let hole = Polygon {
            points: vec![[5.0, 5.0], [15.0, 5.0], [15.0, 15.0], [5.0, 15.0]],
        };
        let (vertices, indices) = tessellate_fill(&[outer, hole], FillRule::EvenOdd, 0xFFFF_FFFF)
            .expect("a ring with a hole tessellates under the even-odd rule");
        assert!(
            (triangle_area(&vertices, &indices) - 300.0).abs() < 1e-2,
            "expected area 300 (outer 400 minus hole 100), got {}",
            triangle_area(&vertices, &indices)
        );
    }

    #[test]
    fn fewer_than_three_points_tessellates_to_nothing_without_erroring() {
        let degenerate = Polygon {
            points: vec![[0.0, 0.0], [1.0, 1.0]],
        };
        let (vertices, indices) = tessellate_fill(&[degenerate], FillRule::NonZero, 0)
            .expect("a degenerate contour must not error, just contribute no geometry");
        assert!(vertices.is_empty());
        assert!(indices.is_empty());
    }
}
