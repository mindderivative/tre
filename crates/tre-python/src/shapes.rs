//! Pythonic shape classes (IMPLEMENTATION.md Phase 10 Step 10.4 task 1/2)
//! and the `ShapeRegistry` binding they insert into.
//!
//! **Scope of this first pass**: solid fill only (`FillStyle::Gradient`/
//! `Texture` are real in `tre-engine` but not yet exposed to Python --
//! a real, bounded follow-up, matching this project's own established
//! precedent of shipping one fill kind before the next, e.g. Step
//! 10.2.1/10.2.2). Every shape's `common.transform` beyond position
//! (rotation, non-uniform scale) and `blend_mode`/`visibility` also stay
//! at their Rust-side defaults for now -- `opacity` is the one
//! `PrimitiveCommon` field exposed here, since it's the one a real caller
//! reaches for immediately (fades) and costs nothing extra to wire.

use pyo3::prelude::*;
use tre_engine::{
    CornerRadii, LineCap, LineJoin, Path, PathCommand, Polygon, PrimitiveCommon, Rectangle,
    ShapeColor as Color, ShapeId, ShapePrimitive, ShapeRegistry, Transform2D,
};

fn common(x: f32, y: f32, opacity: f32) -> PrimitiveCommon {
    PrimitiveCommon {
        transform: Transform2D {
            position: [x, y],
            ..Transform2D::IDENTITY
        },
        opacity,
        ..PrimitiveCommon::new()
    }
}

/// A stable handle to a shape already inserted into a [`PyShapeRegistry`].
#[pyclass(name = "ShapeId", frozen)]
#[derive(Clone, Copy)]
pub struct PyShapeId(pub ShapeId);

#[pymethods]
impl PyShapeId {
    fn __repr__(&self) -> String {
        format!("{:?}", self.0)
    }
}

/// A corner-radius-free-by-default rectangle. Mirrors `tre_engine::Rectangle`.
#[pyclass(name = "Rectangle")]
#[derive(Clone)]
pub struct PyRectangle {
    #[pyo3(get, set)]
    pub x: f32,
    #[pyo3(get, set)]
    pub y: f32,
    #[pyo3(get, set)]
    pub width: f32,
    #[pyo3(get, set)]
    pub height: f32,
    #[pyo3(get, set)]
    pub fill_color: u32,
    #[pyo3(get, set)]
    pub border_color: u32,
    #[pyo3(get, set)]
    pub border_thickness: f32,
    #[pyo3(get, set)]
    pub corner_radius: f32,
    #[pyo3(get, set)]
    pub corner_smoothing: f32,
    #[pyo3(get, set)]
    pub opacity: f32,
}

#[pymethods]
impl PyRectangle {
    #[new]
    #[pyo3(signature = (x, y, width, height, fill_color))]
    fn new(x: f32, y: f32, width: f32, height: f32, fill_color: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            fill_color,
            border_color: 0,
            border_thickness: 0.0,
            corner_radius: 0.0,
            corner_smoothing: 0.0,
            opacity: 1.0,
        }
    }
}

impl From<&PyRectangle> for Rectangle {
    fn from(r: &PyRectangle) -> Self {
        Self {
            common: common(r.x, r.y, r.opacity),
            size: [r.width, r.height],
            fill: tre_engine::FillStyle::Solid(r.fill_color as Color),
            border_color: r.border_color as Color,
            border_thickness: r.border_thickness,
            corner_radius: CornerRadii::uniform(r.corner_radius),
            corner_smoothing: r.corner_smoothing,
        }
    }
}

/// A circle (`radius_x == radius_y`) or ellipse. Mirrors `tre_engine::Circle`.
///
/// `x`/`y` are the top-left of the shape's bounding box, matching
/// `Rectangle`'s own convention (`tre_engine::shapes`'s own
/// `flatten_circle` doc comment) -- NOT the circle's center. For a
/// circle centered at `(cx, cy)` with `radius`, pass
/// `x=cx-radius, y=cy-radius`.
#[pyclass(name = "Circle")]
#[derive(Clone)]
pub struct PyCircle {
    #[pyo3(get, set)]
    pub x: f32,
    #[pyo3(get, set)]
    pub y: f32,
    #[pyo3(get, set)]
    pub radius_x: f32,
    #[pyo3(get, set)]
    pub radius_y: f32,
    #[pyo3(get, set)]
    pub fill_color: u32,
    #[pyo3(get, set)]
    pub border_color: u32,
    #[pyo3(get, set)]
    pub border_thickness: f32,
    /// Degrees, `0.0..=360.0` -- a partial sweep starting at 12 o'clock, clockwise.
    #[pyo3(get, set)]
    pub arc_length: f32,
    #[pyo3(get, set)]
    pub opacity: f32,
}

#[pymethods]
impl PyCircle {
    #[new]
    #[pyo3(signature = (x, y, radius, fill_color))]
    fn new(x: f32, y: f32, radius: f32, fill_color: u32) -> Self {
        Self {
            x,
            y,
            radius_x: radius,
            radius_y: radius,
            fill_color,
            border_color: 0,
            border_thickness: 0.0,
            arc_length: 360.0,
            opacity: 1.0,
        }
    }
}

impl From<&PyCircle> for tre_engine::Circle {
    fn from(c: &PyCircle) -> Self {
        Self {
            common: common(c.x, c.y, c.opacity),
            radius: [c.radius_x, c.radius_y],
            fill: tre_engine::FillStyle::Solid(c.fill_color as Color),
            border_color: c.border_color as Color,
            border_thickness: c.border_thickness,
            arc_length: c.arc_length,
        }
    }
}

/// A regular polygon or, with `star_points` set, a star. Mirrors `tre_engine::Polygon`.
#[pyclass(name = "Polygon")]
#[derive(Clone)]
pub struct PyPolygon {
    #[pyo3(get, set)]
    pub x: f32,
    #[pyo3(get, set)]
    pub y: f32,
    #[pyo3(get, set)]
    pub sides: u32,
    #[pyo3(get, set)]
    pub radius: f32,
    /// Only used when `star_points` is not `None`.
    #[pyo3(get, set)]
    pub vertex_radius: f32,
    /// `None`: a regular `sides`-gon. `Some(k)`: a `2*k`-vertex star.
    #[pyo3(get, set)]
    pub star_points: Option<u32>,
    #[pyo3(get, set)]
    pub fill_color: u32,
    #[pyo3(get, set)]
    pub border_color: u32,
    #[pyo3(get, set)]
    pub border_thickness: f32,
    #[pyo3(get, set)]
    pub opacity: f32,
}

#[pymethods]
impl PyPolygon {
    #[new]
    #[pyo3(signature = (x, y, sides, radius, fill_color))]
    fn new(x: f32, y: f32, sides: u32, radius: f32, fill_color: u32) -> Self {
        Self {
            x,
            y,
            sides,
            radius,
            vertex_radius: 0.0,
            star_points: None,
            fill_color,
            border_color: 0,
            border_thickness: 0.0,
            opacity: 1.0,
        }
    }
}

impl From<&PyPolygon> for Polygon {
    fn from(p: &PyPolygon) -> Self {
        Self {
            common: common(p.x, p.y, p.opacity),
            sides: p.sides,
            radius: p.radius,
            vertex_radius: p.vertex_radius,
            star_points: p.star_points,
            fill: tre_engine::FillStyle::Solid(p.fill_color as Color),
            border_color: p.border_color as Color,
            border_thickness: p.border_thickness,
        }
    }
}

/// A path built from `move_to`/`line_to`/`quad_to`/`cubic_to`/`close` calls,
/// the same shape HTML5 Canvas's own path API uses. Mirrors `tre_engine::Path`.
#[pyclass(name = "Path")]
#[derive(Clone)]
pub struct PyPath {
    commands: Vec<PathCommand>,
    #[pyo3(get, set)]
    pub fill_color: u32,
    #[pyo3(get, set)]
    pub border_color: u32,
    #[pyo3(get, set)]
    pub border_thickness: f32,
    #[pyo3(get, set)]
    pub opacity: f32,
}

#[pymethods]
impl PyPath {
    #[new]
    #[pyo3(signature = (fill_color))]
    fn new(fill_color: u32) -> Self {
        Self {
            commands: Vec::new(),
            fill_color,
            border_color: 0,
            border_thickness: 0.0,
            opacity: 1.0,
        }
    }

    fn move_to(&mut self, x: f32, y: f32) {
        self.commands.push(PathCommand::MoveTo([x, y]));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.commands.push(PathCommand::LineTo([x, y]));
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.commands.push(PathCommand::QuadraticTo {
            control: [cx, cy],
            to: [x, y],
        });
    }

    fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.commands.push(PathCommand::CubicTo {
            control1: [c1x, c1y],
            control2: [c2x, c2y],
            to: [x, y],
        });
    }

    fn close(&mut self) {
        self.commands.push(PathCommand::Close);
    }
}

impl From<&PyPath> for Path {
    fn from(p: &PyPath) -> Self {
        Self {
            common: common(0.0, 0.0, p.opacity),
            commands: p.commands.clone(),
            fill: tre_engine::FillStyle::Solid(p.fill_color as Color),
            border_color: p.border_color as Color,
            border_thickness: p.border_thickness,
            stroke_line_cap: LineCap::Butt,
            stroke_line_join: LineJoin::Miter,
        }
    }
}

/// The generational shape store (`tre_engine::ShapeRegistry`). Insert
/// shapes, get back a stable [`PyShapeId`], then render via
/// [`crate::renderer::PyHeadlessRenderer`].
#[pyclass(name = "ShapeRegistry")]
pub struct PyShapeRegistry {
    pub(crate) inner: ShapeRegistry,
}

#[pymethods]
impl PyShapeRegistry {
    #[new]
    fn new() -> Self {
        Self {
            inner: ShapeRegistry::new(),
        }
    }

    fn insert_rectangle(&mut self, rect: &PyRectangle) -> PyShapeId {
        PyShapeId(
            self.inner
                .insert(ShapePrimitive::Rectangle(Rectangle::from(rect))),
        )
    }

    fn insert_circle(&mut self, circle: &PyCircle) -> PyShapeId {
        PyShapeId(
            self.inner
                .insert(ShapePrimitive::Circle(tre_engine::Circle::from(circle))),
        )
    }

    fn insert_polygon(&mut self, polygon: &PyPolygon) -> PyShapeId {
        PyShapeId(
            self.inner
                .insert(ShapePrimitive::Polygon(Polygon::from(polygon))),
        )
    }

    fn insert_path(&mut self, path: &PyPath) -> PyShapeId {
        PyShapeId(self.inner.insert(ShapePrimitive::Path(Path::from(path))))
    }

    fn remove(&mut self, id: &PyShapeId) -> bool {
        self.inner.remove(id.0)
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}
