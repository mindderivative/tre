//! `tre.Svg` -- real SVG parsing and fill tessellation (Phase 12 Step
//! 12.8), backed directly by `tre_svg::parse_svg`/`tessellate_fill` --
//! the exact same real pipeline `svg_tessellation_demo.rs`/
//! `svg_morph_demo.rs` already prove works, verified on real GPU
//! hardware. The result renders through `tre_engine::Svg`, a new,
//! deliberately minimal "pre-tessellated flat-color mesh" primitive
//! (`tre-engine` cannot depend on `tre-svg` itself -- a real dependency
//! cycle, see `tre_engine::Svg`'s own doc comment).
//!
//! **Real, disclosed scope, inherited from `tre-svg` itself, not
//! introduced here**: only `<path>` fill geometry is extracted --
//! strokes, `<image>`/`<text>` nodes, gradients, and each path's own
//! individual fill color are all discarded during parsing (confirmed by
//! reading `tre-svg::collect_polygons`'s own real source: it checks only
//! `path.fill().is_some()`, never reads the fill's actual paint/color).
//! A caller therefore supplies ONE solid `fill_color` for the whole
//! parsed document -- the same real "solid fill only" constraint
//! `tre_engine::Text` already discloses, here for the identical reason
//! (`RenderingCanvas::draw_flat_polygon`, the real method this renders
//! through, takes one flat `rgba`, not a gradient/texture fill).
//! Multi-color SVG icons will render as a single flat color, not their
//! original per-path colors -- a real limitation of today's `tre-svg`,
//! not a choice made in this binding.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use tre_engine::ShapeColor as Color;

use crate::error::TreError;

/// Comfortably above any real icon/illustration SVG; still a real,
/// bounded ceiling before untrusted bytes ever reach `usvg`'s own
/// parser (`tre_svg::parse_svg`'s own `max_bytes` contract).
const DEFAULT_MAX_BYTES: usize = 10_000_000;
/// Comfortably above any real icon/illustration SVG's own resolved path
/// point count (`tre_svg::parse_svg`'s own `max_points` contract, which
/// bounds a real, otherwise-unbounded blowup from deeply nested
/// `<use>`/`<g>` references `usvg`'s own hardening limits don't catch).
const DEFAULT_MAX_POINTS: usize = 1_000_000;

/// `tre_svg::tessellate::FillRule`, bound directly -- `NonZero` (the
/// SVG/CSS default) and `EvenOdd` (`fill-rule="evenodd"`), both real and
/// correctly handled by `tre-svg`'s own real `lyon`-based tessellator.
#[pyclass(name = "FillRule", eq)]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum PyFillRule {
    #[default]
    NonZero,
    EvenOdd,
}

impl From<PyFillRule> for tre_svg::FillRule {
    fn from(rule: PyFillRule) -> Self {
        match rule {
            PyFillRule::NonZero => Self::NonZero,
            PyFillRule::EvenOdd => Self::EvenOdd,
        }
    }
}

/// Maps a real `tre_svg::SvgError` to the matching Python exception --
/// `ValueError` for a real caller mistake (oversized input, a point
/// count over budget, malformed XML), `TreError` for a genuine internal
/// tessellation failure (rare: `lyon`'s real sweep-line fill tessellator
/// is designed to succeed on self-intersecting and multi-contour input).
fn svg_err(e: tre_svg::SvgError) -> PyErr {
    match e {
        tre_svg::SvgError::TooLarge { .. }
        | tre_svg::SvgError::TooManyPoints { .. }
        | tre_svg::SvgError::Parse(_) => PyValueError::new_err(e.to_string()),
        tre_svg::SvgError::TessellationFailed | tre_svg::SvgError::TopologyMismatch { .. } => {
            TreError::new_err(e.to_string())
        }
    }
}

/// A parsed, real-tessellated SVG document's fill geometry, ready for
/// `registry.insert_svg(...)`. See this module's own doc comment for
/// the real, disclosed "solid fill only, geometry only" scope.
#[pyclass(name = "Svg")]
pub struct PySvg {
    pub(crate) positions: Vec<[f32; 2]>,
    pub(crate) triangles: Vec<[u32; 3]>,
    #[pyo3(get, set)]
    pub x: f32,
    #[pyo3(get, set)]
    pub y: f32,
    /// The one flat solid color the whole parsed document renders in --
    /// see this module's own doc comment for why this can't yet be a
    /// `GradientId`/`Texture` (matching `Rectangle`/`Circle`/`Polygon`/
    /// `Path`'s own real `int | GradientId | Texture` union) or each
    /// path's own original SVG color.
    #[pyo3(get, set)]
    pub fill_color: u32,
    #[pyo3(get, set)]
    pub opacity: f32,
}

#[pymethods]
impl PySvg {
    /// Parses and tessellates `data` (a real SVG document's own bytes).
    ///
    /// # Errors
    /// Raises `ValueError` if `data` exceeds `max_bytes`, resolves to
    /// more than `max_points` path points, or fails to parse as SVG.
    /// Raises `TreError` if the real fill tessellator fails internally.
    #[staticmethod]
    #[pyo3(signature = (
        data,
        fill_color,
        fill_rule = PyFillRule::NonZero,
        x = 0.0,
        y = 0.0,
        opacity = 1.0,
        max_bytes = DEFAULT_MAX_BYTES,
        max_points = DEFAULT_MAX_POINTS,
    ))]
    #[allow(
        clippy::too_many_arguments,
        reason = "every parameter has a real default; a real \
             caller almost always writes tre.Svg.parse(data, fill_color) and nothing else"
    )]
    fn parse(
        data: Vec<u8>,
        fill_color: u32,
        fill_rule: PyFillRule,
        x: f32,
        y: f32,
        opacity: f32,
        max_bytes: usize,
        max_points: usize,
    ) -> PyResult<Self> {
        let contours = tre_svg::parse_svg(&data, max_bytes, max_points).map_err(svg_err)?;
        // A placeholder color: `tessellate_fill` bakes one `rgba` into
        // every output vertex, but this binding keeps geometry and fill
        // separate (matching every other shape's own insert-time fill
        // choice) -- only `.position` survives below; this color value
        // itself is discarded entirely, never the real `fill_color`.
        let (vertices, flat_indices) =
            tre_svg::tessellate_fill(&contours, fill_rule.into(), 0xFFFF_FFFF).map_err(svg_err)?;
        let positions = vertices.iter().map(|v| v.position).collect();
        let triangles = flat_indices
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        Ok(Self {
            positions,
            triangles,
            x,
            y,
            fill_color,
            opacity,
        })
    }

    /// The number of real triangles this document tessellated into --
    /// useful for a caller deciding whether a document is cheap enough
    /// to render every frame versus something to cache.
    #[getter]
    fn triangle_count(&self) -> usize {
        self.triangles.len()
    }
}

impl PySvg {
    pub(crate) fn to_engine(&self) -> tre_engine::Svg {
        let mut shape = tre_engine::Svg::new(
            self.positions.clone(),
            self.triangles.clone(),
            self.fill_color as Color,
        );
        shape.common = crate::shapes::common(self.x, self.y, self.opacity);
        shape
    }
}
