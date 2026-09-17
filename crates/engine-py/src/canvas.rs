//! `CanvasContext` (M5 Phase 3, §11.10/§11.11): the Python-facing
//! imperative drawing API a `Window.add_canvas` `draw` callback receives
//! once, real per `Window.redraw_canvas` call. Deliberately holds no
//! `Py<PyAny>` -- every field is plain Rust data (`engine_core::
//! DrawCommand`/`CustomHitTest`), so this pyclass needs no
//! `__traverse__`/`__clear__` at all, the same reasoning `context_menus`/
//! `dock` already established in `window.rs`. See `engine_core::canvas`'s
//! own module doc comment for why the callback that populates this
//! never reaches `engine-core`/`engine-render`'s hot paths.

use engine_core::{CustomHitTest, DrawCommand};
use peniko::Color;
use peniko::kurbo::BezPath;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

#[pyclass(name = "CanvasContext")]
#[derive(Default)]
pub struct CanvasContext {
    pub(crate) commands: Vec<DrawCommand>,
    pub(crate) hit_test: Option<CustomHitTest>,
}

fn to_color(rgba: (u8, u8, u8, u8)) -> Color {
    Color::from_rgba8(rgba.0, rgba.1, rgba.2, rgba.3)
}

/// M11 Phase 1 (§11.10, §11.11): each entry in `points` is a real
/// bezier segment, not just a line-to point -- 2 numbers (`x, y`) is a
/// line-to (a plain point, the only shape this ever accepted before
/// this phase), 4 (`cx, cy, x, y`) is a quadratic `quad_to`, 6 (`c1x,
/// c1y, c2x, c2y, x, y`) is a cubic `curve_to`. The *first* entry must
/// be 2 numbers -- a `move_to` has no control points of its own, so a
/// curve there would be meaningless. `kurbo::BezPath::quad_to`/
/// `curve_to` already exist and already accept `(f64, f64)` tuples via
/// `Into<Point>`, the identical mechanism `line_to` already relied on
/// -- nothing new needed from `kurbo` itself, only this authoring
/// surface, confirmed via direct read before this phase.
fn build_path(points: Vec<Vec<f64>>) -> PyResult<BezPath> {
    if points.len() < 2 {
        return Err(PyValueError::new_err(
            "stroke_path/set_hit_test_path needs at least 2 points",
        ));
    }
    let mut path = BezPath::new();
    match points[0].as_slice() {
        &[x, y] => path.move_to((x, y)),
        other => {
            return Err(PyValueError::new_err(format!(
                "stroke_path/set_hit_test_path: the first point must be a plain (x, y) pair, \
                 got {} numbers",
                other.len()
            )));
        }
    }
    for p in &points[1..] {
        match p.as_slice() {
            &[x, y] => path.line_to((x, y)),
            &[cx, cy, x, y] => path.quad_to((cx, cy), (x, y)),
            &[c1x, c1y, c2x, c2y, x, y] => path.curve_to((c1x, c1y), (c2x, c2y), (x, y)),
            other => {
                return Err(PyValueError::new_err(format!(
                    "stroke_path/set_hit_test_path: each point must be 2 numbers (line), 4 \
                     (quadratic), or 6 (cubic), got {} numbers",
                    other.len()
                )));
            }
        }
    }
    Ok(path)
}

#[pymethods]
impl CanvasContext {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    /// Node-local coordinates, matching every other `DrawCommand`
    /// (M5 Phase 1's own local-space paint convention).
    fn fill_rect(&mut self, x: f64, y: f64, width: f64, height: f64, color: (u8, u8, u8, u8)) {
        self.commands.push(DrawCommand::FillRect {
            x,
            y,
            width,
            height,
            color: to_color(color),
        });
    }

    fn fill_circle(&mut self, cx: f64, cy: f64, radius: f64, color: (u8, u8, u8, u8)) {
        self.commands.push(DrawCommand::FillCircle {
            cx,
            cy,
            radius,
            color: to_color(color),
        });
    }

    /// `points` builds a real `kurbo::BezPath` -- each entry a line-to
    /// (2 numbers), quadratic (4), or cubic (6) segment; see
    /// `build_path`'s own doc comment for the exact vocabulary. M11
    /// Phase 1 (§11.10, §11.11): real curve authoring, closing the gap
    /// this doc comment used to name here ("a straight-line polyline
    /// ... not a general curve-authoring API").
    fn stroke_path(
        &mut self,
        points: Vec<Vec<f64>>,
        color: (u8, u8, u8, u8),
        width: f64,
    ) -> PyResult<()> {
        let path = build_path(points)?;
        self.commands.push(DrawCommand::StrokePath {
            path,
            color: to_color(color),
            width,
        });
        Ok(())
    }

    /// §11.10's own "a specific plotted data point" example -- overrides
    /// the default rect hit-test for this `Canvas` node entirely (not
    /// just narrows it: a point inside the node's own bounding box but
    /// outside this circle misses).
    fn set_hit_test_circle(&mut self, cx: f64, cy: f64, radius: f64) {
        self.hit_test = Some(CustomHitTest::Circle { cx, cy, radius });
    }

    /// §11.10's own "a bezier curve within N pixels of the point"
    /// example -- M11 Phase 1 (§11.10, §11.11): `points` can now
    /// author a real curve (see `build_path`'s own doc comment), so
    /// this hit-tests against the true curve, not a straight-line
    /// approximation of it. The underlying distance-to-path math
    /// (`ParamCurveNearest`, `tree.rs`) already generalized to real
    /// curves before this phase -- only the authoring surface changed.
    fn set_hit_test_path(&mut self, points: Vec<Vec<f64>>, tolerance: f64) -> PyResult<()> {
        let path = build_path(points)?;
        self.hit_test = Some(CustomHitTest::Path { path, tolerance });
        Ok(())
    }
}
