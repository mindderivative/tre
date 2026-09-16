//! `NodeKind::Canvas` (§11.10, §11.11, M5 Phase 3): custom-drawn content
//! plus an optional custom hit-test override. Both fields here are
//! plain, inert Rust data -- no `Py<PyAny>` anywhere, deliberately, so
//! `engine-render::paint_node`/`Tree::hit_test_at` never need to touch
//! Python on their hot paths. See `PLAN.md` for the full reasoning: the
//! actual Python "draw callback" is invoked exactly once by
//! `engine-py::Window.redraw_canvas`, at an app-triggered sync point
//! outside both paint and hit-testing, and only its *result* -- this
//! module's types -- ever reaches `Tree`.

use peniko::Color;
use peniko::kurbo::BezPath;

/// A `Canvas` node's real, current content -- everything `paint_node`
/// needs to draw it and everything `hit_test_at` needs to test it,
/// resolved ahead of time by `Tree::set_canvas_content` rather than
/// computed live during either walk.
pub struct CanvasState {
    pub commands: Vec<DrawCommand>,
    /// `None` means "no override" -- `hit_test_at` falls back to the
    /// ordinary transform-aware rect test every other node already
    /// uses (§11.10's own "nodes with no custom hit-test use the rect
    /// default").
    pub hit_test: Option<CustomHitTest>,
}

impl CanvasState {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            hit_test: None,
        }
    }
}

impl Default for CanvasState {
    fn default() -> Self {
        Self::new()
    }
}

/// Deliberately narrow -- exactly what M5 Phase 4's own node-graph/
/// chart validation example needs: circles for nodes/data points,
/// strokes for edges/chart lines, rects for simple backgrounds/bars.
/// Not a general vector-drawing API; additive whenever a real future
/// need asks for more (fill paths, gradients, images), matching this
/// codebase's own "don't build ahead of need" discipline throughout
/// (`ItemExtent`, `MotionCurve`, ... same reasoning every time).
///
/// All coordinates are node-local (M5 Phase 1's own local-space paint
/// convention -- `(0, 0)` is this `Canvas` node's own top-left corner),
/// not canvas-absolute.
#[derive(Clone)]
pub enum DrawCommand {
    FillRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: Color,
    },
    FillCircle {
        cx: f64,
        cy: f64,
        radius: f64,
        color: Color,
    },
    StrokePath {
        path: BezPath,
        color: Color,
        width: f64,
    },
}

/// §11.10's own two named examples, verbatim: "a bezier curve within N
/// pixels of the point" (`Path`) and "a specific plotted data point"
/// (`Circle`). Coordinates are node-local, matching `DrawCommand`.
#[derive(Clone)]
pub enum CustomHitTest {
    Circle {
        cx: f64,
        cy: f64,
        radius: f64,
    },
    /// `tolerance` is the maximum allowed distance from `path`, in the
    /// same node-local units as `path`'s own coordinates.
    Path {
        path: BezPath,
        tolerance: f64,
    },
}
