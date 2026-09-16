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

fn polyline(points: Vec<(f64, f64)>) -> PyResult<BezPath> {
    if points.len() < 2 {
        return Err(PyValueError::new_err(
            "stroke_path/set_hit_test_path needs at least 2 points",
        ));
    }
    let mut path = BezPath::new();
    path.move_to(points[0]);
    for &p in &points[1..] {
        path.line_to(p);
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

    /// `points` builds a real `kurbo::BezPath` (a straight-line polyline
    /// via `move_to`/`line_to`) -- not a general curve-authoring API
    /// this phase; see `engine_core::canvas`'s own module doc comment
    /// for why the underlying distance-to-path hit-test math already
    /// generalizes to true bezier curves regardless.
    fn stroke_path(
        &mut self,
        points: Vec<(f64, f64)>,
        color: (u8, u8, u8, u8),
        width: f64,
    ) -> PyResult<()> {
        let path = polyline(points)?;
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
    /// example (a straight polyline here, same underlying real
    /// distance-to-path math a true curve would use).
    fn set_hit_test_path(&mut self, points: Vec<(f64, f64)>, tolerance: f64) -> PyResult<()> {
        let path = polyline(points)?;
        self.hit_test = Some(CustomHitTest::Path { path, tolerance });
        Ok(())
    }
}
