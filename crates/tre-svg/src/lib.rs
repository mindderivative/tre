//! SVG ingestion and ear-clipping tessellation (IMPLEMENTATION.md Step
//! 3.3.1). Parses real SVG documents via the `usvg` crate -- which
//! resolves the DOM (`<use>`/`<g>`/CSS) and converts every shape to
//! absolute-coordinate path data but performs no rasterization itself --
//! then hand-rolls the actual tessellation this project owns: Bezier
//! curve flattening ([`flatten`]) and ear-clipping triangulation
//! ([`triangulate`]).
//!
//! Only simple (non-self-intersecting) polygons are handled here.
//! IMPLEMENTATION.md Step 3.3.3's stencil-and-cover fallback -- not yet
//! built -- is the correct tool for a path this crate's triangulator
//! rejects via [`SvgError::NotSimplePolygon`].
#![forbid(unsafe_code)]

mod flatten;
mod triangulate;

use tre_math::Affine2;

pub use triangulate::triangulate;

/// A single closed polygon contour: an ordered list of points with the
/// last point implicitly connected back to the first (not repeated in
/// `points` itself). One SVG subpath (between a `MoveTo` and its
/// matching `Close`/next `MoveTo`) becomes one `Polygon`.
#[derive(Debug, Clone, PartialEq)]
pub struct Polygon {
    pub points: Vec<[f32; 2]>,
}

/// Everything that can go wrong turning untrusted SVG input into
/// triangles -- every case is a `Result`, never a panic or an unbounded
/// loop (IMPLEMENTATION.md Step 3.3 task 4).
#[derive(Debug)]
pub enum SvgError {
    /// The raw input exceeded the caller-supplied byte-size ceiling,
    /// checked before `usvg` ever sees the data.
    TooLarge { size: usize, max: usize },
    /// The total resolved path point count, summed across every path in
    /// the document and checked incrementally while walking the parsed
    /// tree (not only after fully resolving a pathological document
    /// first), exceeded the caller-supplied ceiling. `usvg` itself does
    /// not enforce this -- see this crate's module docs and
    /// `planning/archive/PLAN_PHASE3_STEP3_3_1.md` for why a
    /// depth/element-count-bounded document can still resolve to an
    /// unbounded number of points.
    TooManyPoints { count: usize, max: usize },
    /// `usvg` itself rejected the document: malformed XML, or one of
    /// `usvg`'s own built-in hardening limits (a 1024-deep nesting/`<use>`
    /// -chain cap, a 1,000,000-element cap, `<use>` reference cycle
    /// detection -- verified by reading `usvg`'s source, not assumed from
    /// its reputation).
    Parse(String),
    /// A path's fill region could not be triangulated by ear-clipping --
    /// e.g. a self-intersecting contour. IMPLEMENTATION.md Step 3.3.3's
    /// stencil-and-cover fallback is the right tool for such a path, not
    /// a guess from this algorithm.
    NotSimplePolygon,
}

impl std::fmt::Display for SvgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { size, max } => {
                write!(f, "SVG source is {size} bytes, exceeding the {max}-byte limit")
            }
            Self::TooManyPoints { count, max } => write!(
                f,
                "SVG resolves to {count} path points, exceeding the {max}-point limit"
            ),
            Self::Parse(msg) => write!(f, "failed to parse SVG: {msg}"),
            Self::NotSimplePolygon => write!(
                f,
                "polygon is self-intersecting or otherwise not simple; ear-clipping cannot triangulate it"
            ),
        }
    }
}

impl std::error::Error for SvgError {}

/// Converts `usvg`'s own 2D affine transform into this workspace's
/// canonical [`Affine2`] (`tre-math`, Phase 3 Step 3.1) -- the first real
/// consumer of `Affine2::transform_point` outside its own test suite.
/// Field correspondence confirmed by reading both crates' actual
/// `map_point`/`transform_point` formulas: `x' = sx*x + kx*y + tx`,
/// `y' = ky*x + sy*y + ty` (`tiny_skia_path::Transform`) is the same
/// formula as `x' = a*x + b*y + tx`, `y' = c*x + d*y + ty` (`Affine2`)
/// under `a=sx, b=kx, c=ky, d=sy`.
fn to_affine2(t: usvg::tiny_skia_path::Transform) -> Affine2 {
    Affine2 {
        a: t.sx,
        b: t.kx,
        tx: t.tx,
        c: t.ky,
        d: t.sy,
        ty: t.ty,
    }
}

fn flush_subpath(polygons: &mut Vec<Polygon>, current: &mut Vec<[f32; 2]>) {
    if current.len() >= 3 {
        polygons.push(Polygon {
            points: std::mem::take(current),
        });
    } else {
        current.clear();
    }
}

fn push_point(
    current: &mut Vec<[f32; 2]>,
    point_budget: &mut usize,
    max_points: usize,
    transform: &Affine2,
    raw: usvg::tiny_skia_path::Point,
) -> Result<[f32; 2], SvgError> {
    let p = transform.transform_point([raw.x, raw.y]);
    *point_budget += 1;
    if *point_budget > max_points {
        return Err(SvgError::TooManyPoints {
            count: *point_budget,
            max: max_points,
        });
    }
    current.push(p);
    Ok(p)
}

fn append_flattened(
    current: &mut Vec<[f32; 2]>,
    point_budget: &mut usize,
    max_points: usize,
    flattened: &[[f32; 2]],
) -> Result<(), SvgError> {
    *point_budget += flattened.len();
    if *point_budget > max_points {
        return Err(SvgError::TooManyPoints {
            count: *point_budget,
            max: max_points,
        });
    }
    current.extend_from_slice(flattened);
    Ok(())
}

/// Converts one `usvg::Path`'s (already-resolved, but still curved and
/// local-space) geometry into absolute-coordinate, curve-flattened
/// [`Polygon`]s -- one per subpath. `point_budget` is threaded through
/// (rather than owned locally) so the cap in [`parse_svg`] applies across
/// the *whole document*, not per path.
fn path_to_polygons(
    path: &usvg::Path,
    point_budget: &mut usize,
    max_points: usize,
) -> Result<Vec<Polygon>, SvgError> {
    use usvg::tiny_skia_path::PathSegment;

    let transform = to_affine2(path.abs_transform());
    let mut polygons = Vec::new();
    let mut current: Vec<[f32; 2]> = Vec::new();
    let mut last = [0.0f32, 0.0];
    let mut start = [0.0f32, 0.0];

    for seg in path.data().segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                flush_subpath(&mut polygons, &mut current);
                let pt = push_point(&mut current, point_budget, max_points, &transform, p)?;
                last = pt;
                start = pt;
            }
            PathSegment::LineTo(p) => {
                last = push_point(&mut current, point_budget, max_points, &transform, p)?;
            }
            PathSegment::QuadTo(control, p) => {
                let control = transform.transform_point([control.x, control.y]);
                let end = transform.transform_point([p.x, p.y]);
                let mut flattened = Vec::new();
                flatten::flatten_quad(last, control, end, &mut flattened);
                append_flattened(&mut current, point_budget, max_points, &flattened)?;
                last = end;
            }
            PathSegment::CubicTo(control1, control2, p) => {
                let control1 = transform.transform_point([control1.x, control1.y]);
                let control2 = transform.transform_point([control2.x, control2.y]);
                let end = transform.transform_point([p.x, p.y]);
                let mut flattened = Vec::new();
                flatten::flatten_cubic(last, control1, control2, end, &mut flattened);
                append_flattened(&mut current, point_budget, max_points, &flattened)?;
                last = end;
            }
            PathSegment::Close => {
                flush_subpath(&mut polygons, &mut current);
                last = start;
            }
        }
    }
    flush_subpath(&mut polygons, &mut current);
    Ok(polygons)
}

fn collect_polygons(
    group: &usvg::Group,
    out: &mut Vec<Polygon>,
    point_budget: &mut usize,
    max_points: usize,
) -> Result<(), SvgError> {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => collect_polygons(g, out, point_budget, max_points)?,
            usvg::Node::Path(p) => {
                // Stroke-only paths have no fill region to tessellate this
                // step (stroke rendering is explicitly out of scope, see
                // PLAN_PHASE3_STEP3_3_1.md).
                if p.fill().is_some() {
                    let mut polys = path_to_polygons(p, point_budget, max_points)?;
                    out.append(&mut polys);
                }
            }
            usvg::Node::Image(_) | usvg::Node::Text(_) => {}
        }
    }
    Ok(())
}

/// Parses `source` as an SVG document and returns every filled path's
/// geometry as absolute-coordinate, curve-flattened [`Polygon`]s, ready
/// for [`triangulate`].
///
/// # Errors
/// Returns [`SvgError::TooLarge`] if `source.len()` exceeds `max_bytes`
/// (checked before `usvg` ever sees the data). Returns
/// [`SvgError::Parse`] if `usvg` itself rejects the document -- which
/// already includes its own hardening against malformed XML, a
/// 1024-deep nesting/`<use>`-chain cap, a 1,000,000-element cap, and
/// `<use>` reference cycle detection. Returns [`SvgError::TooManyPoints`]
/// if the total point count across every path -- checked incrementally
/// while walking the tree -- exceeds `max_points`, a cap `usvg` does not
/// itself enforce.
pub fn parse_svg(
    source: &[u8],
    max_bytes: usize,
    max_points: usize,
) -> Result<Vec<Polygon>, SvgError> {
    if source.len() > max_bytes {
        return Err(SvgError::TooLarge {
            size: source.len(),
            max: max_bytes,
        });
    }

    let tree = usvg::Tree::from_data(source, &usvg::Options::default())
        .map_err(|e| SvgError::Parse(e.to_string()))?;

    let mut polygons = Vec::new();
    let mut point_budget = 0usize;
    collect_polygons(tree.root(), &mut polygons, &mut point_budget, max_points)?;
    Ok(polygons)
}

/// Converts a triangulated polygon into flat-colored `UiVertex`/index
/// buffers, ready for the exact same `upload_buffer`/`draw_indexed` path
/// every pre-Step-3.2 flat-color example already uses. `uv`/`params` are
/// zeroed, matching that convention -- a plain triangle soup has no SDF
/// to evaluate.
#[must_use]
pub fn to_ui_vertices(
    polygon: &Polygon,
    triangles: &[[u32; 3]],
    rgba: u32,
) -> (Vec<tre_engine::UiVertex>, Vec<u32>) {
    let vertices = polygon
        .points
        .iter()
        .map(|&position| tre_engine::UiVertex {
            position,
            uv: [0.0, 0.0],
            color: rgba,
            params: [0.0; 3],
        })
        .collect();
    let indices = triangles.iter().flat_map(|&t| t).collect();
    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_SQUARE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">
             <path d="M 0 0 L 10 0 L 10 10 L 0 10 Z" fill="white"/>
           </svg>"#;

    #[test]
    fn parse_svg_extracts_a_single_square_polygon() {
        let polygons =
            parse_svg(SIMPLE_SQUARE_SVG.as_bytes(), 1_000_000, 10_000).expect("valid SVG");
        assert_eq!(polygons.len(), 1);
        assert_eq!(polygons[0].points.len(), 4);
    }

    #[test]
    fn parse_svg_rejects_oversized_input_before_touching_usvg() {
        let result = parse_svg(SIMPLE_SQUARE_SVG.as_bytes(), 4, 10_000);
        assert!(matches!(result, Err(SvgError::TooLarge { .. })));
    }

    #[test]
    fn parse_svg_rejects_a_document_with_too_many_resolved_points() {
        // A path with far more than 4 points, well under any of usvg's own
        // depth/element caps -- this must be caught by this crate's own
        // point-count ceiling, not usvg's.
        use std::fmt::Write as _;
        let mut d = String::from("M 0 0 ");
        for i in 1..2000 {
            let _ = write!(d, "L {i} {i} ");
        }
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">
                 <path d="{d}Z" fill="white"/>
               </svg>"#
        );
        let result = parse_svg(svg.as_bytes(), 1_000_000, 100);
        assert!(matches!(result, Err(SvgError::TooManyPoints { .. })));
    }

    #[test]
    fn parse_svg_applies_a_group_transform_to_its_children() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">
                        <g transform="translate(5, 5)">
                          <path d="M 0 0 L 10 0 L 10 10 L 0 10 Z" fill="white"/>
                        </g>
                      </svg>"#;
        let polygons = parse_svg(svg.as_bytes(), 1_000_000, 10_000).expect("valid SVG");
        assert_eq!(polygons.len(), 1);
        // The translate(5, 5) must have been applied -- the polygon's
        // points should be shifted, not the raw (0,0)-(10,10) local coords.
        assert!(polygons[0]
            .points
            .iter()
            .any(|&[x, y]| x >= 4.9 && y >= 4.9));
        assert!(!polygons[0].points.iter().any(|&[x, y]| x < 0.1 && y < 0.1));
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "position/uv/params pass through UiVertex construction unchanged (a plain \
                   struct-literal copy in to_ui_vertices), not a rounded computed value"
    )]
    fn to_ui_vertices_zeroes_uv_and_params_and_preserves_position_and_color() {
        let square = Polygon {
            points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
        };
        let triangles = triangulate(&square).expect("a square is a simple polygon");
        let (vertices, indices) = to_ui_vertices(&square, &triangles, 0xFF00_FFFF);

        assert_eq!(vertices.len(), 4);
        assert_eq!(indices.len(), 6);
        for (vertex, &position) in vertices.iter().zip(&square.points) {
            assert_eq!(vertex.position, position);
            assert_eq!(vertex.uv, [0.0, 0.0]);
            assert_eq!(vertex.params, [0.0; 3]);
            assert_eq!(vertex.color, 0xFF00_FFFF);
        }
    }
}
