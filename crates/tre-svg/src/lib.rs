//! SVG ingestion, tessellation, and morphing (IMPLEMENTATION.md Steps
//! 3.3.1-3.3.2; Step 3.3.3's stencil-and-cover fallback is retired, see
//! below). Parses real SVG documents via the `usvg` crate -- which
//! resolves the DOM (`<use>`/`<g>`/CSS) and converts every shape to
//! absolute-coordinate path data but performs no rasterization itself --
//! then flattens curves ([`flatten_cubic`]/[`flatten_quad`], also reused
//! by `tre-text` for glyph outline geometry) and tessellates the result
//! ([`tessellate_fill`]) for the existing IR/sort/batch/RHI pipeline to
//! render, plus SIMD keyframe interpolation ([`morph`]) between two
//! already-flattened keyframe shapes.
//!
//! **Phase 10 Step 10.2 follow-up: real fill tessellation now goes
//! through `lyon` (`nical/lyon`), the industry-standard Rust 2D
//! tessellation library, not a hand-rolled ear-clipping triangulator.**
//! The original ear-clipper could only ever handle a single simple
//! (non-self-intersecting) contour, rejecting everything else
//! (`SvgError::NotSimplePolygon`) -- which is why Step 3.3.3 built a
//! separate stencil-and-cover GPU fallback technique for exactly that
//! rejected case. `lyon`'s real sweep-line fill tessellator handles
//! self-intersection, compound shapes with holes, and both winding rules
//! directly, as one algorithm -- there is no longer a case ear-clipping
//! could handle that this can't, and no case needing a fallback
//! technique at all. See REVIEW.md for the full account of this
//! retirement (the ear-clipping triangulator and the CPU-side
//! stencil/fan-triangle geometry it needed are both deleted; the GPU-side
//! stencil-and-cover pipeline in `tre-rhi-vulkan` is unused but not yet
//! removed).
#![forbid(unsafe_code)]

mod flatten;
mod morph;
mod tessellate;

use tre_math::Affine2;

pub use flatten::{flatten_cubic, flatten_quad};
pub use morph::{morph, morph_into};
pub use tessellate::{tessellate_fill, FillRule};

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
    /// [`tessellate_fill`]'s own lyon `FillTessellator` reported an
    /// internal failure -- rare in practice (Phase 10 Step 10.2
    /// follow-up: lyon's real sweep-line fill tessellator is designed to
    /// succeed on self-intersecting and multi-contour input, unlike the
    /// ear-clipping algorithm it replaced, which rejected such input
    /// outright via the now-retired `NotSimplePolygon` variant this one
    /// replaces).
    TessellationFailed,
    /// [`morph`]'s two keyframe `Polygon`s have different vertex counts --
    /// IMPLEMENTATION.md Step 3.3 task 2's "topological equivalence"
    /// requirement, for already-flattened polygons, means equal vertex
    /// counts. Not automatically resampled to match; see
    /// `planning/archive/PLAN_PHASE3_STEP3_3_2.md` for why.
    TopologyMismatch {
        from_points: usize,
        to_points: usize,
    },
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
            Self::TessellationFailed => write!(f, "lyon's fill tessellator reported an internal failure"),
            Self::TopologyMismatch { from_points, to_points } => write!(
                f,
                "cannot morph: keyframes have different vertex counts ({from_points} vs {to_points})"
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
/// for [`tessellate_fill`].
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
///
/// `max_points` bounds peak memory (and, transitively, [`tessellate_fill`]'s
/// input size) across the *whole document*, but NOT worst-case CPU time:
/// it says nothing about how those points are distributed across
/// individual paths, and a single adversarially-shaped path even within
/// this budget can still be far more expensive to tessellate than a
/// well-behaved one of the same point count.
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
}

/// Phase 9 Step 9.1's own "fuzz-test the SVG parser and tessellator...
/// with malformed and adversarial documents, asserting bounded
/// tessellation time and memory regardless of input" -- via `proptest`
/// (this crate's `Cargo.toml` has the full account of why, not
/// `cargo-fuzz`). Every property here generates hundreds of randomized
/// inputs per run and asserts two things regardless of what comes out:
/// the real function never panics (a `proptest!` assertion failure
/// inside the closure -- including a Rust panic -- fails the specific
/// case and, on first failure, proptest automatically *shrinks* the
/// input to the smallest one that still reproduces it, so a real
/// failure here comes with a minimal repro, not just "some 4KB input
/// broke it somewhere"), and it completes within a generous, fixed wall
/// -clock bound -- a real, if coarse, stand-in for "bounded time" that
/// doesn't need instrumenting the tessellator's own internals.
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::time::{Duration, Instant};

    /// Generous on purpose: this exists to catch a real hang/blowup
    /// (unbounded, not just slow), not to enforce a tight performance
    /// budget -- TECHNICAL.md Section 9.2's own criterion benchmark
    /// suite owns real performance budgets.
    const BOUNDED_TIME: Duration = Duration::from_secs(2);

    proptest! {
        /// Pure random bytes, no SVG/XML structure at all -- the
        /// adversarial case most likely to exercise usvg's own error
        /// paths (malformed XML) rather than this crate's own logic,
        /// but still a real, direct test that garbage input can never
        /// panic or hang `parse_svg` before it even reaches XML
        /// parsing (the `max_bytes` check) or after (usvg's own parse
        /// failure, surfaced as `Err(SvgError::Parse(..))`).
        #[test]
        fn parse_svg_never_panics_or_hangs_on_arbitrary_bytes(
            bytes in proptest::collection::vec(any::<u8>(), 0..4096)
        ) {
            let start = Instant::now();
            let _ = parse_svg(&bytes, 1_000_000, 10_000);
            prop_assert!(
                start.elapsed() < BOUNDED_TIME,
                "parse_svg took {:?} on {} bytes of pure random input -- expected bounded, not \
                 unbounded, worst-case time regardless of how malformed the input is",
                start.elapsed(),
                bytes.len()
            );
        }

        /// A real SVG/XML skeleton (so usvg's own parser actually
        /// engages, not just its unicode/XML-level guard rails), but
        /// with a randomly-generated `d` path attribute -- extreme,
        /// negative, huge, or malformed-looking coordinate sequences
        /// are exactly the "adversarial document" shape this task
        /// names, more targeted than pure random bytes at stressing
        /// this crate's own path-resolution and point-budget logic.
        #[test]
        fn parse_svg_never_panics_or_hangs_on_random_path_data(
            path_data in "[MLZmlz0-9.,\\- ]{0,500}"
        ) {
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path d="{path_data}" fill="white"/></svg>"#
            );
            let start = Instant::now();
            let _ = parse_svg(svg.as_bytes(), 1_000_000, 10_000);
            prop_assert!(
                start.elapsed() < BOUNDED_TIME,
                "parse_svg took {:?} on a random path data string of length {} -- expected \
                 bounded, not unbounded, worst-case time",
                start.elapsed(),
                path_data.len()
            );
        }

        /// Random point sets fed directly to `tessellate_fill`, bypassing
        /// SVG parsing entirely -- degenerate (0-2 points), duplicate/
        /// coincident points, and self-intersecting orderings are all
        /// real, reachable shapes a resolved SVG path can produce, so
        /// this property is real adversarial coverage for the
        /// tessellator specifically, independent of whatever the
        /// parser's own point-budget already bounds.
        #[test]
        fn tessellate_fill_never_panics_or_hangs_on_arbitrary_point_sets(
            points in proptest::collection::vec(
                (-1.0e6f32..1.0e6f32, -1.0e6f32..1.0e6f32),
                0..500
            )
        ) {
            let polygon = Polygon {
                points: points.into_iter().map(|(x, y)| [x, y]).collect(),
            };
            let start = Instant::now();
            let _ = tessellate_fill(std::slice::from_ref(&polygon), FillRule::NonZero, 0);
            prop_assert!(
                start.elapsed() < BOUNDED_TIME,
                "tessellate_fill took {:?} on {} arbitrary points -- expected bounded, not \
                 unbounded, worst-case time regardless of how degenerate or self-intersecting \
                 the polygon is",
                start.elapsed(),
                polygon.points.len()
            );
        }
    }
}
