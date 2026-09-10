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

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use tre_engine::{
    CornerRadii, LineCap, LineJoin, Path, PathCommand, Polygon, PrimitiveCommon, Rectangle,
    ShapeColor as Color, ShapeId, ShapePrimitive, ShapeRegistry, Transform2D,
};

/// A real cap on `Polygon::sides`/`star_points`, found necessary by this
/// project's own review process (REVIEW.md #196-198): `tre_engine`'s own
/// `generate_polygon_points` computes `star_points * 2` with no overflow
/// guard and allocates one `Vec2` per resulting vertex with no upper
/// bound -- both real, reachable DoS/panic surfaces for a value that
/// crosses straight from untrusted Python input into that code with no
/// validation anywhere in between. Comfortably above any real polygon a
/// UI would ever draw.
const MAX_POLYGON_SIDES: u32 = 4096;

/// Rejects a non-finite (`NaN`/`+-inf`) coordinate before it can cross
/// into `tre_engine`'s flatten path -- found necessary by this project's
/// own review process (REVIEW.md #202): none of `Rectangle`/`Circle`/
/// `Polygon`/`Path`'s own flatten code, nor `lyon`'s tessellator the
/// `Path` coordinates feed into, checks finiteness, and `lyon` documents
/// finite input as a caller obligation (non-finite points there are a
/// real panic/hang surface, not just a wrong render).
fn validate_finite(field: &str, value: f32) -> PyResult<()> {
    if !value.is_finite() {
        return Err(PyValueError::new_err(format!(
            "{field} must be finite, got {value}"
        )));
    }
    Ok(())
}

/// Rejects a negative size/radius -- distinct from [`validate_finite`]
/// since a negative-but-finite value passes that check yet is still
/// nonsensical geometry (REVIEW.md #202).
fn validate_non_negative(field: &str, value: f32) -> PyResult<()> {
    validate_finite(field, value)?;
    if value < 0.0 {
        return Err(PyValueError::new_err(format!(
            "{field} must be >= 0, got {value}"
        )));
    }
    Ok(())
}

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
    fn new(fill_color: u32) -> Self {
        Self {
            commands: Vec::new(),
            fill_color,
            border_color: 0,
            border_thickness: 0.0,
            opacity: 1.0,
        }
    }

    /// # Errors
    /// Raises `ValueError` if `x`/`y` is not finite -- a non-finite
    /// point reaching `lyon`'s tessellator is a real panic/hang surface,
    /// not just a wrong render (REVIEW.md #202).
    fn move_to(&mut self, x: f32, y: f32) -> PyResult<()> {
        validate_finite("x", x)?;
        validate_finite("y", y)?;
        self.commands.push(PathCommand::MoveTo([x, y]));
        Ok(())
    }

    /// # Errors
    /// See [`PyPath::move_to`].
    fn line_to(&mut self, x: f32, y: f32) -> PyResult<()> {
        validate_finite("x", x)?;
        validate_finite("y", y)?;
        self.commands.push(PathCommand::LineTo([x, y]));
        Ok(())
    }

    /// # Errors
    /// See [`PyPath::move_to`].
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) -> PyResult<()> {
        validate_finite("cx", cx)?;
        validate_finite("cy", cy)?;
        validate_finite("x", x)?;
        validate_finite("y", y)?;
        self.commands.push(PathCommand::QuadraticTo {
            control: [cx, cy],
            to: [x, y],
        });
        Ok(())
    }

    /// # Errors
    /// See [`PyPath::move_to`].
    fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) -> PyResult<()> {
        validate_finite("c1x", c1x)?;
        validate_finite("c1y", c1y)?;
        validate_finite("c2x", c2x)?;
        validate_finite("c2y", c2y)?;
        validate_finite("x", x)?;
        validate_finite("y", y)?;
        self.commands.push(PathCommand::CubicTo {
            control1: [c1x, c1y],
            control2: [c2x, c2y],
            to: [x, y],
        });
        Ok(())
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

    /// # Errors
    /// Raises `ValueError` if any of `rect`'s numeric fields is
    /// non-finite, or if `width`/`height` is negative (REVIEW.md #202).
    fn insert_rectangle(&mut self, rect: &PyRectangle) -> PyResult<PyShapeId> {
        validate_finite("x", rect.x)?;
        validate_finite("y", rect.y)?;
        validate_non_negative("width", rect.width)?;
        validate_non_negative("height", rect.height)?;
        validate_non_negative("border_thickness", rect.border_thickness)?;
        validate_non_negative("corner_radius", rect.corner_radius)?;
        validate_finite("opacity", rect.opacity)?;
        let mut rect = rect.clone();
        // Documented 0.0..=1.0 contract (gpu_style.rs), never enforced
        // before this fix -- clamped, not rejected, since it's a
        // cosmetic parameter an animation can briefly overshoot.
        rect.corner_smoothing = rect.corner_smoothing.clamp(0.0, 1.0);
        Ok(PyShapeId(
            self.inner
                .insert(ShapePrimitive::Rectangle(Rectangle::from(&rect))),
        ))
    }

    /// # Errors
    /// Raises `ValueError` if any of `circle`'s numeric fields is
    /// non-finite, or if `radius_x`/`radius_y` is negative
    /// (REVIEW.md #202).
    fn insert_circle(&mut self, circle: &PyCircle) -> PyResult<PyShapeId> {
        validate_finite("x", circle.x)?;
        validate_finite("y", circle.y)?;
        validate_non_negative("radius_x", circle.radius_x)?;
        validate_non_negative("radius_y", circle.radius_y)?;
        validate_non_negative("border_thickness", circle.border_thickness)?;
        validate_finite("arc_length", circle.arc_length)?;
        validate_finite("opacity", circle.opacity)?;
        Ok(PyShapeId(self.inner.insert(ShapePrimitive::Circle(
            tre_engine::Circle::from(circle),
        ))))
    }

    /// # Errors
    /// Raises `ValueError` if `polygon.sides` or `polygon.star_points`
    /// exceeds [`MAX_POLYGON_SIDES`], if any numeric field is
    /// non-finite, or if `radius`/`vertex_radius` is negative
    /// (REVIEW.md #202).
    fn insert_polygon(&mut self, polygon: &PyPolygon) -> PyResult<PyShapeId> {
        if polygon.sides > MAX_POLYGON_SIDES
            || polygon.star_points.is_some_and(|p| p > MAX_POLYGON_SIDES)
        {
            return Err(PyValueError::new_err(format!(
                "Polygon sides/star_points must be <= {MAX_POLYGON_SIDES}, got sides={}, \
                 star_points={:?}",
                polygon.sides, polygon.star_points
            )));
        }
        validate_finite("x", polygon.x)?;
        validate_finite("y", polygon.y)?;
        validate_non_negative("radius", polygon.radius)?;
        validate_non_negative("vertex_radius", polygon.vertex_radius)?;
        validate_non_negative("border_thickness", polygon.border_thickness)?;
        validate_finite("opacity", polygon.opacity)?;
        Ok(PyShapeId(
            self.inner
                .insert(ShapePrimitive::Polygon(Polygon::from(polygon))),
        ))
    }

    /// # Errors
    /// Raises `ValueError` if `path.border_thickness`/`path.opacity` is
    /// non-finite, or `border_thickness` is negative (REVIEW.md #202).
    /// Path command coordinates are already validated at `move_to`/
    /// `line_to`/`quad_to`/`cubic_to` call time.
    fn insert_path(&mut self, path: &PyPath) -> PyResult<PyShapeId> {
        validate_non_negative("border_thickness", path.border_thickness)?;
        validate_finite("opacity", path.opacity)?;
        Ok(PyShapeId(
            self.inner.insert(ShapePrimitive::Path(Path::from(path))),
        ))
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
