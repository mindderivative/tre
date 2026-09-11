//! Pythonic shape classes (IMPLEMENTATION.md Phase 10 Step 10.4 task 1/2)
//! and the `ShapeRegistry` binding they insert into.
//!
//! Every shape's `fill_color` accepts a real `int | GradientId | Texture`
//! union (Phase 12 Step 12.4) -- `PyShapeRegistry::resolve_fill` resolves
//! whichever was passed into the matching `FillStyle` variant at
//! `insert_*` time. Every shape exposes its full `common.transform`
//! (position, non-uniform `scale_x`/`scale_y`, `rotation` in radians --
//! GUI-framework gap remediation, Added 2026-09-10) plus `opacity`;
//! `blend_mode`/`visibility` still stay at their Rust-side defaults.

use std::collections::HashMap;
use std::sync::Arc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use tre_engine::{
    CornerRadii, CustomShaded, FillStyle, FontId, FontRegistry, GradientError, LineCap, LineJoin,
    Path, PathCommand, Polygon, PrimitiveCommon, Rectangle, RhiTexture, ShapeColor as Color,
    ShapeId, ShapePrimitive, ShapeRegistry, Text as EngineText, Transform2D,
};

use crate::custom_shader::PyCustomShaderId;
use crate::font::PyFont;
use crate::gradient::{PyGradient, PyGradientId};
use crate::texture::PyTexture;

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

/// Maps a real `GradientError` (`create_gradient`'s own validation
/// failure) to a Python `ValueError` -- a real caller mistake (bad
/// stops, non-positive radius), not an engine-internal failure, so
/// `ValueError` matches every other shape-field validation error in this
/// module rather than `TreError`.
fn gradient_err(e: GradientError) -> PyErr {
    PyValueError::new_err(e.to_string())
}

#[allow(
    clippy::too_many_arguments,
    reason = "one field per real Transform2D component; a real \
         caller almost always leaves scale_x/scale_y/rotation at their defaults"
)]
pub(crate) fn common(
    x: f32,
    y: f32,
    opacity: f32,
    scale_x: f32,
    scale_y: f32,
    rotation: f32,
) -> PrimitiveCommon {
    PrimitiveCommon {
        transform: Transform2D {
            position: [x, y],
            scale: [scale_x, scale_y],
            rotation,
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
///
/// `fill_color` accepts an `int` (solid RGBA8, via `tre.rgba8`), a
/// [`crate::gradient::PyGradientId`] (from `registry.create_gradient`),
/// or a [`PyTexture`] (from a renderer's `create_texture`) -- resolved
/// into the matching real `FillStyle` variant at `insert_rectangle` time
/// (Phase 12 Step 12.4).
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
    pub fill_color: Py<PyAny>,
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
    /// A real on/off switch, independent of `border_thickness` -- see
    /// `tre_engine::Rectangle::border_enabled`'s own doc comment.
    #[pyo3(get, set)]
    pub border_enabled: bool,
    #[pyo3(get, set)]
    pub scale_x: f32,
    #[pyo3(get, set)]
    pub scale_y: f32,
    /// Radians, matching `tre_engine::Transform2D::rotation`'s own convention.
    #[pyo3(get, set)]
    pub rotation: f32,
}

#[pymethods]
impl PyRectangle {
    #[new]
    #[pyo3(signature = (x, y, width, height, fill_color, scale_x = 1.0, scale_y = 1.0, rotation = 0.0))]
    #[allow(
        clippy::too_many_arguments,
        reason = "every trailing parameter has a real default; a \
             real caller almost always writes tre.Rectangle(x, y, w, h, color)"
    )]
    fn new(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        fill_color: Py<PyAny>,
        scale_x: f32,
        scale_y: f32,
        rotation: f32,
    ) -> Self {
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
            border_enabled: true,
            scale_x,
            scale_y,
            rotation,
        }
    }
}

impl PyRectangle {
    fn to_engine(&self, fill: FillStyle) -> Rectangle {
        Rectangle {
            common: common(
                self.x,
                self.y,
                self.opacity,
                self.scale_x,
                self.scale_y,
                self.rotation,
            ),
            size: [self.width, self.height],
            fill,
            border_color: self.border_color as Color,
            border_thickness: self.border_thickness,
            border_enabled: self.border_enabled,
            corner_radius: CornerRadii::uniform(self.corner_radius),
            corner_smoothing: self.corner_smoothing,
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
    /// See [`PyRectangle::fill_color`]'s own doc comment for the real
    /// `int | GradientId | Texture` contract.
    #[pyo3(get, set)]
    pub fill_color: Py<PyAny>,
    #[pyo3(get, set)]
    pub border_color: u32,
    #[pyo3(get, set)]
    pub border_thickness: f32,
    /// Degrees, `0.0..=360.0` -- a partial sweep starting at 12 o'clock, clockwise.
    #[pyo3(get, set)]
    pub arc_length: f32,
    #[pyo3(get, set)]
    pub opacity: f32,
    /// See `tre_engine::Rectangle::border_enabled`'s own doc comment.
    #[pyo3(get, set)]
    pub border_enabled: bool,
    #[pyo3(get, set)]
    pub scale_x: f32,
    #[pyo3(get, set)]
    pub scale_y: f32,
    /// Radians, matching `tre_engine::Transform2D::rotation`'s own convention.
    #[pyo3(get, set)]
    pub rotation: f32,
}

#[pymethods]
impl PyCircle {
    #[new]
    #[pyo3(signature = (x, y, radius, fill_color, scale_x = 1.0, scale_y = 1.0, rotation = 0.0))]
    #[allow(
        clippy::too_many_arguments,
        reason = "every trailing parameter has a real default; a \
             real caller almost always writes tre.Circle(x, y, r, color)"
    )]
    fn new(
        x: f32,
        y: f32,
        radius: f32,
        fill_color: Py<PyAny>,
        scale_x: f32,
        scale_y: f32,
        rotation: f32,
    ) -> Self {
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
            border_enabled: true,
            scale_x,
            scale_y,
            rotation,
        }
    }
}

impl PyCircle {
    fn to_engine(&self, fill: FillStyle) -> tre_engine::Circle {
        tre_engine::Circle {
            common: common(
                self.x,
                self.y,
                self.opacity,
                self.scale_x,
                self.scale_y,
                self.rotation,
            ),
            radius: [self.radius_x, self.radius_y],
            fill,
            border_color: self.border_color as Color,
            border_thickness: self.border_thickness,
            border_enabled: self.border_enabled,
            arc_length: self.arc_length,
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
    /// See [`PyRectangle::fill_color`]'s own doc comment for the real
    /// `int | GradientId | Texture` contract.
    #[pyo3(get, set)]
    pub fill_color: Py<PyAny>,
    #[pyo3(get, set)]
    pub border_color: u32,
    #[pyo3(get, set)]
    pub border_thickness: f32,
    #[pyo3(get, set)]
    pub opacity: f32,
    /// See `tre_engine::Rectangle::border_enabled`'s own doc comment.
    #[pyo3(get, set)]
    pub border_enabled: bool,
    #[pyo3(get, set)]
    pub scale_x: f32,
    #[pyo3(get, set)]
    pub scale_y: f32,
    /// Radians, matching `tre_engine::Transform2D::rotation`'s own convention.
    #[pyo3(get, set)]
    pub rotation: f32,
}

#[pymethods]
impl PyPolygon {
    #[new]
    #[pyo3(signature = (x, y, sides, radius, fill_color, scale_x = 1.0, scale_y = 1.0, rotation = 0.0))]
    #[allow(
        clippy::too_many_arguments,
        reason = "every trailing parameter has a real default; a \
             real caller almost always writes tre.Polygon(x, y, sides, r, color)"
    )]
    fn new(
        x: f32,
        y: f32,
        sides: u32,
        radius: f32,
        fill_color: Py<PyAny>,
        scale_x: f32,
        scale_y: f32,
        rotation: f32,
    ) -> Self {
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
            border_enabled: true,
            scale_x,
            scale_y,
            rotation,
        }
    }
}

impl PyPolygon {
    fn to_engine(&self, fill: FillStyle) -> Polygon {
        Polygon {
            common: common(
                self.x,
                self.y,
                self.opacity,
                self.scale_x,
                self.scale_y,
                self.rotation,
            ),
            sides: self.sides,
            radius: self.radius,
            vertex_radius: self.vertex_radius,
            star_points: self.star_points,
            fill,
            border_color: self.border_color as Color,
            border_thickness: self.border_thickness,
            border_enabled: self.border_enabled,
        }
    }
}

/// A path built from `move_to`/`line_to`/`quad_to`/`cubic_to`/`close` calls,
/// the same shape HTML5 Canvas's own path API uses. Mirrors `tre_engine::Path`.
#[pyclass(name = "Path")]
#[derive(Clone)]
pub struct PyPath {
    commands: Vec<PathCommand>,
    /// See [`PyRectangle::fill_color`]'s own doc comment for the real
    /// `int | GradientId | Texture` contract.
    #[pyo3(get, set)]
    pub fill_color: Py<PyAny>,
    #[pyo3(get, set)]
    pub border_color: u32,
    #[pyo3(get, set)]
    pub border_thickness: f32,
    #[pyo3(get, set)]
    pub opacity: f32,
    /// See `tre_engine::Rectangle::border_enabled`'s own doc comment.
    #[pyo3(get, set)]
    pub border_enabled: bool,
    #[pyo3(get, set)]
    pub scale_x: f32,
    #[pyo3(get, set)]
    pub scale_y: f32,
    /// Radians, matching `tre_engine::Transform2D::rotation`'s own
    /// convention. `Path` has no `x`/`y` (its own commands are already
    /// authored in absolute coordinates), so scale/rotation apply about
    /// the local origin `(0, 0)` those commands are drawn relative to.
    #[pyo3(get, set)]
    pub rotation: f32,
}

#[pymethods]
impl PyPath {
    #[new]
    #[pyo3(signature = (fill_color, scale_x = 1.0, scale_y = 1.0, rotation = 0.0))]
    fn new(fill_color: Py<PyAny>, scale_x: f32, scale_y: f32, rotation: f32) -> Self {
        Self {
            commands: Vec::new(),
            fill_color,
            border_color: 0,
            border_thickness: 0.0,
            opacity: 1.0,
            border_enabled: true,
            scale_x,
            scale_y,
            rotation,
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

impl PyPath {
    fn to_engine(&self, fill: FillStyle) -> Path {
        Path {
            common: common(
                0.0,
                0.0,
                self.opacity,
                self.scale_x,
                self.scale_y,
                self.rotation,
            ),
            commands: self.commands.clone(),
            fill,
            border_color: self.border_color as Color,
            border_thickness: self.border_thickness,
            border_enabled: self.border_enabled,
            stroke_line_cap: LineCap::Butt,
            stroke_line_join: LineJoin::Miter,
        }
    }
}

/// A single retained-mode text shape. Mirrors `tre_engine::Text` --
/// `x`/`y` are the top-left of the text block, matching `Rectangle`'s/
/// `Circle`'s own convention, NOT the text's baseline (`tre_engine::
/// text::flatten_text` offsets the real baseline internally using the
/// font's own scaled ascent, so a caller never has to reason about
/// baseline metrics here).
///
/// **Solid fill only** -- see `tre_engine::Text`'s own doc comment for
/// why: `RenderingCanvas::draw_text`'s real glyph-quad path takes a flat
/// color, not a gradient/texture fill.
#[pyclass(name = "Text")]
#[derive(Clone)]
pub struct PyText {
    #[pyo3(get, set)]
    pub x: f32,
    #[pyo3(get, set)]
    pub y: f32,
    #[pyo3(get, set)]
    pub text: String,
    #[pyo3(get, set)]
    pub font: Py<PyFont>,
    #[pyo3(get, set)]
    pub px_size: f32,
    #[pyo3(get, set)]
    pub fill_color: u32,
    #[pyo3(get, set)]
    pub opacity: f32,
    #[pyo3(get, set)]
    pub scale_x: f32,
    #[pyo3(get, set)]
    pub scale_y: f32,
    /// Radians, matching `tre_engine::Transform2D::rotation`'s own convention.
    #[pyo3(get, set)]
    pub rotation: f32,
}

#[pymethods]
impl PyText {
    #[new]
    #[pyo3(signature = (x, y, text, font, px_size, fill_color, scale_x = 1.0, scale_y = 1.0, rotation = 0.0))]
    #[allow(
        clippy::too_many_arguments,
        reason = "every trailing parameter has a real default; a \
             real caller almost always writes tre.Text(x, y, text, font, size, color)"
    )]
    fn new(
        x: f32,
        y: f32,
        text: String,
        font: Py<PyFont>,
        px_size: f32,
        fill_color: u32,
        scale_x: f32,
        scale_y: f32,
        rotation: f32,
    ) -> Self {
        Self {
            x,
            y,
            text,
            font,
            px_size,
            fill_color,
            opacity: 1.0,
            scale_x,
            scale_y,
            rotation,
        }
    }
}

/// A quad rendered through a caller-registered custom shader pipeline
/// (Phase 13 Step 13.8, Q13). Mirrors `tre_engine::CustomShaded`. Get a
/// real `CustomShaderId` via `renderer.create_custom_shader(fragment_
/// source)` first -- this class carries no shader source itself, only
/// a reference to an already-compiled, already-registered pipeline.
///
/// `fill_color` here is a plain flat color multiplier (the vertex
/// color a custom fragment shader receives as `frag_color`), not the
/// polymorphic `int | GradientId | Texture` union every other shape's
/// `fill_color` accepts -- a real, disclosed simplification matching
/// `Svg`'s own "solid fill only" precedent, since what a custom
/// fragment shader actually does with `frag_color` is entirely up to
/// its own GLSL source.
#[pyclass(name = "CustomShaded")]
#[derive(Clone)]
pub struct PyCustomShaded {
    #[pyo3(get, set)]
    pub x: f32,
    #[pyo3(get, set)]
    pub y: f32,
    #[pyo3(get, set)]
    pub width: f32,
    #[pyo3(get, set)]
    pub height: f32,
    #[pyo3(get, set)]
    pub pipeline_id: PyCustomShaderId,
    #[pyo3(get, set)]
    pub fill_color: u32,
    #[pyo3(get, set)]
    pub opacity: f32,
    #[pyo3(get, set)]
    pub scale_x: f32,
    #[pyo3(get, set)]
    pub scale_y: f32,
    #[pyo3(get, set)]
    pub rotation: f32,
}

#[pymethods]
impl PyCustomShaded {
    #[new]
    #[pyo3(signature = (x, y, width, height, pipeline_id, fill_color, scale_x = 1.0, scale_y = 1.0, rotation = 0.0))]
    #[allow(
        clippy::too_many_arguments,
        reason = "every trailing parameter has a real default; a \
             real caller almost always writes tre.CustomShaded(x, y, w, h, pipeline_id, color)"
    )]
    fn new(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        pipeline_id: PyCustomShaderId,
        fill_color: u32,
        scale_x: f32,
        scale_y: f32,
        rotation: f32,
    ) -> Self {
        Self {
            x,
            y,
            width,
            height,
            pipeline_id,
            fill_color,
            opacity: 1.0,
            scale_x,
            scale_y,
            rotation,
        }
    }
}

impl PyCustomShaded {
    fn to_engine(&self) -> CustomShaded {
        let mut shape = CustomShaded::new(
            [self.width, self.height],
            self.pipeline_id.0,
            self.fill_color as Color,
        );
        shape.common = common(
            self.x,
            self.y,
            self.opacity,
            self.scale_x,
            self.scale_y,
            self.rotation,
        );
        shape
    }
}

/// The generational shape store (`tre_engine::ShapeRegistry`). Insert
/// shapes, get back a stable [`PyShapeId`], then render via
/// [`crate::renderer::PyHeadlessRenderer`].
#[pyclass(name = "ShapeRegistry")]
pub struct PyShapeRegistry {
    pub(crate) inner: ShapeRegistry,
    /// This registry's own real font table (Phase 12 Step 12.3) --
    /// owned here, not by the renderer, so a caller never has to reason
    /// about which registry a `Font` is "scoped to": `insert_text`
    /// resolves any `Font` into a real `FontId` in this table lazily,
    /// the first time it's actually used.
    fonts: FontRegistry,
    /// Caches each distinct `Font` (by its own `uid`, not its bytes) to
    /// the `FontId` this registry already resolved it to, so inserting
    /// many `Text` shapes that share one `Font` registers that font's
    /// bytes with `tre_engine::FontRegistry` exactly once, not once per
    /// `insert_text` call.
    font_ids: HashMap<u64, FontId>,
    /// Keeps every `Texture` ever used as a shape's `fill_color` alive
    /// for as long as this registry exists (Phase 12 Step 12.4) -- once
    /// `insert_rectangle`/etc. resolves a `Texture` into a raw bindless
    /// `u32` for `FillStyle::Texture`, nothing else keeps that texture's
    /// own `Box<dyn RhiTexture>` alive; without this, it could be dropped
    /// (freeing its real GPU resources) the moment the original Python
    /// `Texture` object is garbage-collected, while a shape here still
    /// references its now-dangling bindless index.
    textures_kept_alive: Vec<Arc<Box<dyn RhiTexture>>>,
}

#[pymethods]
impl PyShapeRegistry {
    #[new]
    fn new() -> Self {
        Self {
            inner: ShapeRegistry::new(),
            fonts: FontRegistry::new(),
            font_ids: HashMap::new(),
            textures_kept_alive: Vec::new(),
        }
    }

    /// Registers `gradient` on this registry, returning a real
    /// `GradientId` usable as any shape's `fill_color` -- see
    /// [`PyGradientId`]'s own doc comment for why the result is scoped to
    /// *this* registry specifically.
    ///
    /// # Errors
    /// Raises `ValueError` if `gradient`'s own stops are invalid (too
    /// many, non-finite, out of `0.0..=1.0`, or out of order) or its
    /// radial radius isn't positive -- `tre_engine::ShapeRegistry::
    /// create_gradient`'s own real, existing validation.
    fn create_gradient(&mut self, gradient: &PyGradient) -> PyResult<PyGradientId> {
        self.inner
            .create_gradient(gradient.def.clone())
            .map(PyGradientId)
            .map_err(gradient_err)
    }

    /// # Errors
    /// Raises `ValueError` if any of `rect`'s numeric fields is
    /// non-finite, if `width`/`height` is negative (REVIEW.md #202), or
    /// if `rect.fill_color` is neither an `int`, a `GradientId`, nor a
    /// `Texture`.
    fn insert_rectangle(&mut self, rect: &PyRectangle, py: Python<'_>) -> PyResult<PyShapeId> {
        validate_finite("x", rect.x)?;
        validate_finite("y", rect.y)?;
        validate_non_negative("width", rect.width)?;
        validate_non_negative("height", rect.height)?;
        validate_non_negative("border_thickness", rect.border_thickness)?;
        validate_non_negative("corner_radius", rect.corner_radius)?;
        validate_finite("opacity", rect.opacity)?;
        validate_finite("scale_x", rect.scale_x)?;
        validate_finite("scale_y", rect.scale_y)?;
        validate_finite("rotation", rect.rotation)?;
        let fill = self.resolve_fill(py, &rect.fill_color)?;
        let mut rect = rect.to_engine(fill);
        // Documented 0.0..=1.0 contract (gpu_style.rs), never enforced
        // before this fix -- clamped, not rejected, since it's a
        // cosmetic parameter an animation can briefly overshoot.
        rect.corner_smoothing = rect.corner_smoothing.clamp(0.0, 1.0);
        Ok(PyShapeId(
            self.inner.insert(ShapePrimitive::Rectangle(rect)),
        ))
    }

    /// # Errors
    /// Raises `ValueError` if any of `circle`'s numeric fields is
    /// non-finite, if `radius_x`/`radius_y` is negative (REVIEW.md #202),
    /// or if `circle.fill_color` is neither an `int`, a `GradientId`, nor
    /// a `Texture`.
    fn insert_circle(&mut self, circle: &PyCircle, py: Python<'_>) -> PyResult<PyShapeId> {
        validate_finite("x", circle.x)?;
        validate_finite("y", circle.y)?;
        validate_non_negative("radius_x", circle.radius_x)?;
        validate_non_negative("radius_y", circle.radius_y)?;
        validate_non_negative("border_thickness", circle.border_thickness)?;
        validate_finite("arc_length", circle.arc_length)?;
        validate_finite("opacity", circle.opacity)?;
        validate_finite("scale_x", circle.scale_x)?;
        validate_finite("scale_y", circle.scale_y)?;
        validate_finite("rotation", circle.rotation)?;
        let fill = self.resolve_fill(py, &circle.fill_color)?;
        Ok(PyShapeId(
            self.inner
                .insert(ShapePrimitive::Circle(circle.to_engine(fill))),
        ))
    }

    /// # Errors
    /// Raises `ValueError` if `polygon.sides` or `polygon.star_points`
    /// exceeds [`MAX_POLYGON_SIDES`], if any numeric field is
    /// non-finite, if `radius`/`vertex_radius` is negative (REVIEW.md
    /// #202), or if `polygon.fill_color` is neither an `int`, a
    /// `GradientId`, nor a `Texture`.
    fn insert_polygon(&mut self, polygon: &PyPolygon, py: Python<'_>) -> PyResult<PyShapeId> {
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
        validate_finite("scale_x", polygon.scale_x)?;
        validate_finite("scale_y", polygon.scale_y)?;
        validate_finite("rotation", polygon.rotation)?;
        let fill = self.resolve_fill(py, &polygon.fill_color)?;
        Ok(PyShapeId(
            self.inner
                .insert(ShapePrimitive::Polygon(polygon.to_engine(fill))),
        ))
    }

    /// # Errors
    /// Raises `ValueError` if `path.border_thickness`/`path.opacity` is
    /// non-finite, `border_thickness` is negative (REVIEW.md #202), or
    /// `path.fill_color` is neither an `int`, a `GradientId`, nor a
    /// `Texture`. Path command coordinates are already validated at
    /// `move_to`/`line_to`/`quad_to`/`cubic_to` call time.
    fn insert_path(&mut self, path: &PyPath, py: Python<'_>) -> PyResult<PyShapeId> {
        validate_non_negative("border_thickness", path.border_thickness)?;
        validate_finite("opacity", path.opacity)?;
        validate_finite("scale_x", path.scale_x)?;
        validate_finite("scale_y", path.scale_y)?;
        validate_finite("rotation", path.rotation)?;
        let fill = self.resolve_fill(py, &path.fill_color)?;
        Ok(PyShapeId(
            self.inner
                .insert(ShapePrimitive::Path(path.to_engine(fill))),
        ))
    }

    /// # Errors
    /// Raises `ValueError` if `svg.x`/`svg.y`/`svg.opacity` is
    /// non-finite. `svg.fill_color` is always a plain `int` (see
    /// [`crate::svg::PySvg`]'s own doc comment for why it doesn't accept
    /// a `GradientId`/`Texture` the way every other shape's `fill_color`
    /// does).
    fn insert_svg(&mut self, svg: &crate::svg::PySvg) -> PyResult<PyShapeId> {
        validate_finite("x", svg.x)?;
        validate_finite("y", svg.y)?;
        validate_finite("opacity", svg.opacity)?;
        validate_finite("scale_x", svg.scale_x)?;
        validate_finite("scale_y", svg.scale_y)?;
        validate_finite("rotation", svg.rotation)?;
        Ok(PyShapeId(
            self.inner.insert(ShapePrimitive::Svg(svg.to_engine())),
        ))
    }

    /// # Errors
    /// Raises `ValueError` if any of `custom`'s numeric fields is
    /// non-finite, or `width`/`height` is negative (REVIEW.md #202).
    /// Does NOT validate that `custom.pipeline_id` is actually
    /// registered against whatever renderer eventually renders this
    /// shape -- that's a real runtime failure surfaced at render time
    /// instead (matching `execute_frame`'s own existing "unregistered
    /// pipeline id" contract for every other shape kind).
    fn insert_custom_shaded(&mut self, custom: &PyCustomShaded) -> PyResult<PyShapeId> {
        validate_finite("x", custom.x)?;
        validate_finite("y", custom.y)?;
        validate_non_negative("width", custom.width)?;
        validate_non_negative("height", custom.height)?;
        validate_finite("opacity", custom.opacity)?;
        validate_finite("scale_x", custom.scale_x)?;
        validate_finite("scale_y", custom.scale_y)?;
        validate_finite("rotation", custom.rotation)?;
        Ok(PyShapeId(
            self.inner
                .insert(ShapePrimitive::CustomShaded(custom.to_engine())),
        ))
    }

    /// # Errors
    /// Raises `ValueError` if `text.x`/`text.y`/`text.opacity` is
    /// non-finite, or `text.px_size` is non-finite or not `> 0`.
    fn insert_text(&mut self, text: &PyText, py: Python<'_>) -> PyResult<PyShapeId> {
        validate_finite("x", text.x)?;
        validate_finite("y", text.y)?;
        validate_finite("px_size", text.px_size)?;
        if text.px_size <= 0.0 {
            return Err(PyValueError::new_err(format!(
                "px_size must be > 0, got {}",
                text.px_size
            )));
        }
        validate_finite("opacity", text.opacity)?;
        validate_finite("scale_x", text.scale_x)?;
        validate_finite("scale_y", text.scale_y)?;
        validate_finite("rotation", text.rotation)?;

        let font_id = {
            let font = text.font.borrow(py);
            self.resolve_font(&font)
        };

        let mut shape = EngineText::new(
            text.text.clone(),
            font_id,
            text.px_size,
            text.fill_color as Color,
        );
        shape.common = common(
            text.x,
            text.y,
            text.opacity,
            text.scale_x,
            text.scale_y,
            text.rotation,
        );
        Ok(PyShapeId(self.inner.insert(ShapePrimitive::Text(shape))))
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

impl PyShapeRegistry {
    /// Resolves a shape's own `fill_color` field (an `int`, a
    /// [`PyGradientId`], or a [`PyTexture`] -- the real `int |
    /// GradientId | Texture` union every `insert_*` method now accepts,
    /// Phase 12 Step 12.4) into the matching real `FillStyle`. A
    /// resolved `Texture`'s own underlying GPU resource is kept alive in
    /// `textures_kept_alive` for as long as this registry exists.
    fn resolve_fill(&mut self, py: Python<'_>, value: &Py<PyAny>) -> PyResult<FillStyle> {
        let bound = value.bind(py);
        if let Ok(color) = bound.extract::<u32>() {
            return Ok(FillStyle::Solid(color));
        }
        if let Ok(gradient_id) = bound.extract::<PyGradientId>() {
            return Ok(FillStyle::Gradient(gradient_id.0));
        }
        if let Ok(texture) = bound.extract::<PyTexture>() {
            self.textures_kept_alive.push(texture.texture.clone());
            return Ok(FillStyle::Texture(texture.bindless_index));
        }
        Err(PyValueError::new_err(
            "fill_color must be an int (solid RGBA8, e.g. via tre.rgba8), a Gradient, or a \
             Texture",
        ))
    }

    /// Resolves `font` (a Python-visible [`PyFont`]) into a real
    /// `FontId` scoped to this registry's own `FontRegistry`, loading
    /// its bytes into that table on first use and reusing the cached
    /// `FontId` on every later call with the same `Font` object.
    fn resolve_font(&mut self, font: &PyFont) -> FontId {
        if let Some(&id) = self.font_ids.get(&font.uid) {
            return id;
        }
        let id = self
            .fonts
            .load_bytes((*font.bytes).clone())
            .expect("PyFont::from_bytes already validated these exact bytes load successfully");
        self.font_ids.insert(font.uid, id);
        id
    }

    /// Splits this registry into simultaneous `&mut` access to the real
    /// `tre_engine::ShapeRegistry` (for `flatten_into`) and `&` access to
    /// this registry's own font table (for building a `TextFlattenContext`
    /// to pass to it) -- a renderer's `render()` needs both at once, and
    /// a plain `&self` accessor method for the font table alone would
    /// borrow all of `self` for its return value's lifetime, blocking the
    /// separate `&mut` borrow `flatten_into` itself needs (direct field
    /// projection, as done here, is what lets the borrow checker see
    /// these two borrows as disjoint).
    pub(crate) fn inner_and_fonts(&mut self) -> (&mut ShapeRegistry, &FontRegistry) {
        (&mut self.inner, &self.fonts)
    }
}
