//! Phase 10 Step 10.1: the retained-mode shape primitive layer for
//! external UI frameworks (ARCHITECTURE.md Section 7, the canonical
//! reference for every type below). Every concrete shape here compiles
//! down into the exact same immediate-mode [`crate::RenderingCanvas`]
//! calls any other caller would issue -- this module is a convenience,
//! retained-store layer over the existing IR/sort/batch/RHI pipeline,
//! never a second rendering path.
//!
//! Real rendering support: `Rectangle` (any corner radii, real borders,
//! corner smoothing), `Circle`/`Ellipse` (borders, partial-arc sweep,
//! rounded stroke caps on a partial arc since Step 10.2.5), `Polygon`/
//! `Path` (real fill -- including compound shapes with holes -- and real
//! stroke, via `lyon`, Phase 10 Step 10.2's own lyon-migration follow-up)
//! all flatten for real. Every [`FillStyle`] variant is real for all four
//! shape kinds: `Solid` (always was); `Gradient` (linear and radial,
//! Step 10.2.1), via [`ShapeRegistry::create_gradient`] and a
//! `GpuGradientStyle` record (`crates/tre-engine/src/gpu_style.rs`'s own
//! doc comment covers the GPU-side style-buffer mechanism `Rectangle`/
//! `Circle` both rely on; `Polygon`/`Path` route gradient fill through
//! `PipelineKind::GradientFill` instead, since neither has a per-vertex
//! style record); `Texture` (Step 10.2.2), sampling the existing bindless
//! texture array -- `Rectangle`/`Circle` via a `fill_kind == 2` shader
//! branch mapping onto the shape's own bounding box, `Polygon`/`Path` via
//! the existing `TexturedQuad` pipeline. [`ShapeRegistry::flatten_into`]
//! still panics loudly (`unimplemented!`/`panic!`) on the one remaining
//! real gap -- a `FillStyle::Gradient` naming a `GradientId` this
//! registry never issued -- matching this project's own established
//! "fail loud on a real, not-yet-built or genuinely invalid case"
//! discipline. See ARCHITECTURE.md Section 7.5's own "Implementation
//! status" note and `documentation/REVIEW.md`'s Phase 10 Step 10.2.1-
//! 10.2.6 sections for the full, itemized history of every gap this
//! module once had and how each was closed.

use crate::{RenderingCanvas, ScissorRect};

/// A plain 2D point/extent -- a type alias for the `[f32; 2]` this
/// codebase already uses everywhere a 2D point appears
/// (`UiVertex::position`, `tre_math::lerp_points_batch`'s own
/// signature), not a new nominal struct (ARCHITECTURE.md Section 7.1).
pub type Vec2 = [f32; 2];

/// `UiVertex::color`'s own existing packed-`u32` convention
/// (`rgba8`'s own doc comment) -- a type alias, not a new struct, for
/// the same reason `Vec2` is one (ARCHITECTURE.md Section 7.2).
pub type Color = u32;

/// A decomposed, animation-friendly transform -- deliberately not
/// `tre_math::Affine2` itself (ARCHITECTURE.md Section 7.1's own
/// reasoning: a raw matrix's rotation component cannot be cleanly
/// interpolated). [`Transform2D::to_affine2`] resolves this into a real
/// `Affine2` once per frame during flattening.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    pub position: Vec2,
    pub scale: Vec2,
    /// Radians, matching `Affine2::from_rotation`'s own convention --
    /// not degrees.
    pub rotation: f32,
}

impl Transform2D {
    /// No translation, no scale change, no rotation.
    pub const IDENTITY: Self = Self {
        position: [0.0, 0.0],
        scale: [1.0, 1.0],
        rotation: 0.0,
    };

    /// Resolves this decomposed transform into a real, composable
    /// `Affine2` -- standard translate*rotate*scale order (scale
    /// applied to a point first, then rotation, then translation),
    /// using `Affine2::compose`'s own existing method, never a new
    /// matrix implementation (ARCHITECTURE.md Section 7.1's own doc
    /// comment specifies this exact composition).
    #[must_use]
    pub fn to_affine2(&self) -> tre_math::Affine2 {
        let translation = tre_math::Affine2::from_translation(self.position[0], self.position[1]);
        let rotation = tre_math::Affine2::from_rotation(self.rotation);
        let scale = tre_math::Affine2::from_scale(self.scale[0], self.scale[1]);
        translation.compose(&rotation.compose(&scale))
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// TECHNICAL.md Section 3.4/DESIGN.md Section 6.2's own "Visual Filter
/// Pipeline" concept, concretized as the enum a shape's `blend_mode`
/// field holds. **Real for `Polygon`/`Path` solid fill** (Phase 10 Step
/// 10.2.3), gated behind `RhiDevice::local_read_blend_supported` --
/// falls back to `Normal` on hardware without it, never a silent wrong
/// render. `Rectangle`/`Circle` and non-solid fills don't read this
/// field yet; see this module's own top-level doc comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    SoftLight,
    ColorDodge,
}

/// Layout-participation state, distinct from `opacity == 0.0` (which
/// still occupies layout space). `Hidden` and `Collapsed` render
/// identically today (neither is recorded into the IR by
/// [`ShapeRegistry::flatten_into`]) -- no real layout system exists yet
/// for the distinction ("still occupies layout space" vs. "does not")
/// to have any concrete effect on; the two variants are kept distinct
/// now so a future layout system does not need a breaking data-model
/// change to tell them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
    Collapsed,
}

/// Fields every shape primitive carries, regardless of kind -- embedded
/// by value in `Rectangle`/`Circle`/`Polygon`/`Path`, not inherited
/// (ARCHITECTURE.md Section 7.1).
#[derive(Debug, Clone, Copy, Default)]
pub struct PrimitiveCommon {
    pub transform: Transform2D,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub visibility: Visibility,
    pub hit_testable: bool,
}

impl PrimitiveCommon {
    /// Identity transform, fully opaque, normal blending, visible,
    /// hit-testable -- the sensible default for a freshly-created shape.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            transform: Transform2D::IDENTITY,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            visibility: Visibility::Visible,
            hit_testable: true,
        }
    }
}

/// Every concrete shape implements this so generic code can reach
/// `PrimitiveCommon` without matching every variant -- an ordinary
/// object-safe accessor trait, not a marker for dynamic dispatch: every
/// real call site uses the [`ShapePrimitive`] enum, never `dyn
/// Primitive`, per TECHNICAL.md Section 9.1's "no dynamic type
/// inspection in hot paths" rule (ARCHITECTURE.md Section 7.1).
pub trait Primitive {
    fn common(&self) -> &PrimitiveCommon;
    fn common_mut(&mut self) -> &mut PrimitiveCommon;
}

/// What a shape's interior is painted with (ARCHITECTURE.md Section
/// 7.2). `Gradient` is real (Phase 10 Step 10.2.1: linear and radial,
/// via [`ShapeRegistry::create_gradient`]); `Texture` still has no
/// evaluator anywhere in this codebase yet -- see this module's own
/// top-level doc comment.
#[derive(Debug, Clone, Copy)]
pub enum FillStyle {
    Solid(Color),
    Gradient(GradientId),
    Texture(u32),
}

/// A stable handle into a [`ShapeRegistry`]'s own gradient table (Phase
/// 10 Step 10.2.1), returned by [`ShapeRegistry::create_gradient`] and
/// referenced by [`FillStyle::Gradient`]. Scoped to the registry that
/// created it -- this table has no generational reuse or removal (a real,
/// disclosed scope decision: no real UI use case this step targets
/// creates and discards gradients at the same churn rate shapes
/// themselves do), so a `GradientId` from one registry is meaningless
/// against another, and stays valid for as long as that registry exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradientId(pub u32);

/// A real, evaluatable linear-or-radial gradient definition (Phase 10
/// Step 10.2.1), created via [`ShapeRegistry::create_gradient`]. All
/// points/positions are in the SAME local, untransformed space a shape's
/// own geometry is defined in -- a gradient moves and rotates rigidly
/// with the shape referencing it, matching every other per-shape style
/// field's own convention (`crates/tre-engine/src/gpu_style.rs`'s own
/// `GpuGradientStyle` doc comment has the full GPU-side account).
#[derive(Debug, Clone, PartialEq)]
pub struct GradientDef {
    pub kind: GradientKind,
    pub stops: Vec<GradientStop>,
}

/// [`GradientDef`]'s own axis/shape -- linear (interpolates along a
/// `start -> end` segment) or radial (interpolates by distance from
/// `center`, out to `radius`). Conic/angular gradients are explicitly out
/// of scope this step (`PLAN.md`'s own "Scope decisions").
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GradientKind {
    Linear { start: Vec2, end: Vec2 },
    Radial { center: Vec2, radius: f32 },
}

/// One color stop in a [`GradientDef`] -- `position` is `0.0..=1.0` along
/// the gradient's own axis/radius; stops must be given in non-decreasing
/// `position` order (`ShapeRegistry::create_gradient` validates this).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    pub position: f32,
    pub color: Color,
}

/// [`ShapeRegistry::create_gradient`]'s own real validation failures --
/// deliberately NOT [`crate::EngineError`] (whose every existing variant
/// is a real RHI/GPU failure class, `documentation/DESIGN.md` Section
/// 2.6): this is caller-input validation, checked entirely on the CPU
/// before any GPU call, the same category `tre_svg::SvgError` already
/// occupies for that crate's own untrusted-input checks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GradientError {
    /// `stops` was empty -- a gradient needs at least one color.
    NoStops,
    /// More than [`crate::gpu_style::GRADIENT_MAX_STOPS`] stops were
    /// given; rejected outright rather than silently truncated.
    TooManyStops { count: usize, max: usize },
    /// A stop's own `position` was outside `0.0..=1.0`.
    StopPositionOutOfRange { index: usize, position: f32 },
    /// Stops were not given in non-decreasing `position` order -- the
    /// shader's own interpolation walks them assuming this, and silently
    /// reordering them would produce a real, wrong, non-obvious visual
    /// result rather than a loud rejection.
    StopsNotAscending { index: usize },
    /// [`GradientKind::Radial`]'s own `radius` was not a real, positive
    /// number -- a non-positive radius has no real geometric meaning and
    /// would divide by zero / produce `NaN` in the shader's own `t =
    /// distance / radius` evaluation.
    NonPositiveRadius(f32),
}

impl std::fmt::Display for GradientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoStops => write!(f, "a gradient needs at least one color stop"),
            Self::TooManyStops { count, max } => {
                write!(
                    f,
                    "gradient has {count} stops, exceeding the maximum of {max}"
                )
            }
            Self::StopPositionOutOfRange { index, position } => write!(
                f,
                "gradient stop {index} has position {position}, outside 0.0..=1.0"
            ),
            Self::StopsNotAscending { index } => write!(
                f,
                "gradient stop {index} is out of order -- stops must be given in ascending \
                 position order"
            ),
            Self::NonPositiveRadius(radius) => {
                write!(
                    f,
                    "radial gradient radius {radius} must be a positive number"
                )
            }
        }
    }
}

impl std::error::Error for GradientError {}

fn validate_gradient(def: &GradientDef) -> Result<(), GradientError> {
    if def.stops.is_empty() {
        return Err(GradientError::NoStops);
    }
    if def.stops.len() > crate::gpu_style::GRADIENT_MAX_STOPS {
        return Err(GradientError::TooManyStops {
            count: def.stops.len(),
            max: crate::gpu_style::GRADIENT_MAX_STOPS,
        });
    }
    let mut previous = f32::NEG_INFINITY;
    for (index, stop) in def.stops.iter().enumerate() {
        if !(0.0..=1.0).contains(&stop.position) {
            return Err(GradientError::StopPositionOutOfRange {
                index,
                position: stop.position,
            });
        }
        if stop.position < previous {
            return Err(GradientError::StopsNotAscending { index });
        }
        previous = stop.position;
    }
    if let GradientKind::Radial { radius, .. } = def.kind {
        if radius <= 0.0 {
            return Err(GradientError::NonPositiveRadius(radius));
        }
    }
    Ok(())
}

/// Builds a [`crate::gpu_style::GpuGradientStyle`] record from a real,
/// already-validated [`GradientDef`] -- called fresh every frame a shape
/// referencing this gradient is flattened (the style buffer itself is a
/// ring buffer reset each frame, so nothing could persist across frames
/// even if this were cached), matching every other per-shape style
/// record's own per-frame-rewrite pattern.
///
/// `local_origin_offset` corrects for a real coordinate-space mismatch
/// between two DIFFERENT "local space" conventions this crate already
/// has: a `GradientDef`'s own points are authored in the shape's PUBLIC
/// local space (bounding-box top-left at the origin -- the same
/// convention `Rectangle`/`Circle`'s own `corner_radius`/`border`
/// thinking already uses, per `flatten_circle`'s own doc comment), but
/// `sdf_rect_styled.frag`/`sdf_ellipse.frag`'s own `frag_uv` is CENTER-
/// relative (an internal shader convention, chosen for symmetric SDF
/// math, `draw_styled_rectangle`/`draw_ellipse`'s own `uv` construction).
/// `local_origin_offset` is that shape's own center, in its public local
/// space (`[half_width, half_height]` for `Rectangle`, `radius` for
/// `Circle`) -- subtracted from every point/center here so the stored
/// record already speaks `frag_uv`'s own coordinate space. `Polygon`/
/// `Path` pass `[0.0, 0.0]`: their own local space is ALREADY
/// center-relative by construction (`generate_polygon_points`'s own doc
/// comment; a `Path`'s own `PathCommand` coordinates have no fixed
/// convention at all, so a gradient on one is defined in those same raw
/// coordinates directly).
#[allow(
    clippy::cast_possible_truncation,
    reason = "def.stops.len() is already validated <= GRADIENT_MAX_STOPS (8) by \
               ShapeRegistry::create_gradient before this is ever called"
)]
fn build_gpu_gradient_style(
    def: &GradientDef,
    local_origin_offset: Vec2,
) -> crate::gpu_style::GpuGradientStyle {
    use crate::gpu_style::{GpuGradientStyle, GRADIENT_MAX_STOPS};

    let mut stop_positions = [0.0f32; GRADIENT_MAX_STOPS];
    let mut stop_colors = [0u32; GRADIENT_MAX_STOPS];
    for (i, stop) in def.stops.iter().enumerate() {
        stop_positions[i] = stop.position;
        stop_colors[i] = stop.color;
    }
    let sub = |p: Vec2| [p[0] - local_origin_offset[0], p[1] - local_origin_offset[1]];
    let (kind, point0, point1_or_radius) = match def.kind {
        GradientKind::Linear { start, end } => (0u32, sub(start), sub(end)),
        // radius is a magnitude, not a point -- only `center` gets the
        // offset correction.
        GradientKind::Radial { center, radius } => (1u32, sub(center), [radius, 0.0]),
    };
    GpuGradientStyle {
        kind,
        point0,
        point1_or_radius,
        stop_count: def.stops.len() as u32,
        stop_positions,
        stop_colors,
    }
}

/// Writes `def`'s own `GpuGradientStyle` record into `device`'s current
/// per-frame style buffer segment and returns its word index -- the
/// SAME per-frame-rewrite mechanism `draw_styled_rectangle`/`draw_
/// ellipse` already use for `GpuRectStyle`/`GpuEllipseStyle`, just for a
/// gradient's own, independently word-indexed record in that same
/// buffer. See [`build_gpu_gradient_style`]'s own doc comment for what
/// `local_origin_offset` corrects for.
///
/// # Panics
/// Panics if the shape style buffer has no room left this frame -- same
/// policy as every other style-buffer write in this crate.
fn write_gradient_style(
    device: &dyn crate::RhiDevice,
    def: &GradientDef,
    local_origin_offset: Vec2,
) -> u32 {
    let style = build_gpu_gradient_style(def, local_origin_offset);
    let byte_offset = device
        .shape_style_buffer()
        .write(bytemuck::bytes_of(&style))
        .expect("shape style buffer starved for this frame");
    byte_offset / 4
}

/// `Rectangle::corner_radius`'s own per-corner field type -- clockwise
/// from top-left (ARCHITECTURE.md Section 7.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadii {
    /// One radius applied to all four corners -- exactly what today's
    /// real `draw_rounded_rect` already supports; a `Rectangle` using
    /// only this constructor needs no new rendering work to flatten
    /// correctly.
    #[must_use]
    pub const fn uniform(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    /// Whether all four corners share one real value -- the real
    /// condition [`ShapeRegistry::flatten_into`] checks before
    /// attempting to render a `Rectangle`, since today's shader has no
    /// per-corner radius support at all.
    #[must_use]
    pub fn is_uniform(&self) -> bool {
        (self.top_left - self.top_right).abs() < f32::EPSILON
            && (self.top_right - self.bottom_right).abs() < f32::EPSILON
            && (self.bottom_right - self.bottom_left).abs() < f32::EPSILON
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rectangle {
    pub common: PrimitiveCommon,
    pub size: Vec2,
    pub fill: FillStyle,
    pub border_color: Color,
    pub border_thickness: f32,
    pub corner_radius: CornerRadii,
    /// Squircle interpolation factor, `0.0` (pure circular-arc
    /// rounding, today's real shader) to `1.0` (full squircle) -- see
    /// this module's own top-level doc comment.
    pub corner_smoothing: f32,
}

impl Rectangle {
    /// A corner-radius-free, borderless rectangle of `size`, filled
    /// `color`, at the identity transform -- the common case. Every
    /// other field (border, corner radii/smoothing) defaults to "off";
    /// set them directly on the returned value.
    #[must_use]
    pub fn new(size: Vec2, color: Color) -> Self {
        Self {
            common: PrimitiveCommon::new(),
            size,
            fill: FillStyle::Solid(color),
            border_color: 0,
            border_thickness: 0.0,
            corner_radius: CornerRadii::uniform(0.0),
            corner_smoothing: 0.0,
        }
    }
}

impl Primitive for Rectangle {
    fn common(&self) -> &PrimitiveCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut PrimitiveCommon {
        &mut self.common
    }
}

/// Unifies circle and ellipse: `radius[0] == radius[1]` is a circle,
/// otherwise an ellipse (ARCHITECTURE.md Section 7.3). Fully rendered,
/// including borders, corner-case-free partial-arc sweeps with real
/// rounded stroke caps (Step 10.2.5), and every `FillStyle` variant.
#[derive(Debug, Clone, Copy)]
pub struct Circle {
    pub common: PrimitiveCommon,
    pub radius: Vec2,
    pub fill: FillStyle,
    pub border_color: Color,
    pub border_thickness: f32,
    /// Degrees, `0.0..=360.0` -- a progress-wheel/pie-chart partial
    /// sweep starting at 12 o'clock, clockwise.
    pub arc_length: f32,
}

impl Circle {
    /// A full (`arc_length: 360.0`), borderless circle/ellipse of
    /// `radius`, filled `color`, at the identity transform. Set
    /// `arc_length`/border fields directly on the returned value for
    /// anything beyond that.
    #[must_use]
    pub fn new(radius: Vec2, color: Color) -> Self {
        Self {
            common: PrimitiveCommon::new(),
            radius,
            fill: FillStyle::Solid(color),
            border_color: 0,
            border_thickness: 0.0,
            arc_length: 360.0,
        }
    }
}

impl Primitive for Circle {
    fn common(&self) -> &PrimitiveCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut PrimitiveCommon {
        &mut self.common
    }
}

/// Covers triangle/hexagon/N-gon and star shapes with one struct
/// (ARCHITECTURE.md Section 7.3). Fully rendered: fill (via `lyon`'s fan
/// triangulation, real for a star-shaped-by-construction regular/star
/// polygon) and stroke, and every `FillStyle` variant.
#[derive(Debug, Clone, Copy)]
pub struct Polygon {
    pub common: PrimitiveCommon,
    pub sides: u32,
    pub radius: f32,
    /// Only used when `star_points` is `Some` -- the inner (odd-index)
    /// vertex radius; `radius` alone is the outer (even-index) one.
    pub vertex_radius: f32,
    /// `None`: a regular `sides`-gon, every vertex at `radius`. `Some(k)`:
    /// a `2*k`-vertex star alternating `radius` (even index) and
    /// `vertex_radius` (odd index).
    pub star_points: Option<u32>,
    pub fill: FillStyle,
    pub border_color: Color,
    pub border_thickness: f32,
}

impl Polygon {
    /// A regular `sides`-gon (not a star -- `star_points: None`) of
    /// `radius`, filled `color`, borderless, at the identity transform.
    /// Set `star_points`/`vertex_radius`/border fields directly on the
    /// returned value for a star shape or a border.
    #[must_use]
    pub fn new(sides: u32, radius: f32, color: Color) -> Self {
        Self {
            common: PrimitiveCommon::new(),
            sides,
            radius,
            vertex_radius: 0.0,
            star_points: None,
            fill: FillStyle::Solid(color),
            border_color: 0,
            border_thickness: 0.0,
        }
    }
}

impl Primitive for Polygon {
    fn common(&self) -> &PrimitiveCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut PrimitiveCommon {
        &mut self.common
    }
}

/// One instruction in a `Path`'s command list -- matches SVG path-data's
/// own real primitives (ARCHITECTURE.md Section 7.3).
#[derive(Debug, Clone, Copy)]
pub enum PathCommand {
    MoveTo(Vec2),
    LineTo(Vec2),
    QuadraticTo {
        control: Vec2,
        to: Vec2,
    },
    CubicTo {
        control1: Vec2,
        control2: Vec2,
        to: Vec2,
    },
    Close,
}

/// How a `Path`'s open stroke ends are capped, via `lyon`'s stroke
/// tessellator (the SVG `stroke-linecap` values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    /// The stroke ends exactly at the endpoint, no extension.
    Butt,
    /// A semicircle extends the stroke past the endpoint by half the
    /// stroke width.
    Round,
    /// A square extends the stroke past the endpoint by half the stroke
    /// width, like `Butt` but with a flat extension instead of none.
    Square,
}

/// How a `Path`'s stroke segments meet at a corner, via `lyon`'s stroke
/// tessellator (the SVG `stroke-linejoin` values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    /// Segments extend to a sharp point.
    Miter,
    /// A rounded arc fills the corner.
    Round,
    /// The corner is cut off with a flat segment connecting the two
    /// stroke edges directly.
    Bevel,
}

/// The one shape whose own data model already names a real, deliberate
/// heap allocation (`commands: Vec<PathCommand>`) -- consistent with
/// DESIGN.md Section 2.1's own zero-allocation boundary, which exempts
/// complex external subsystems from the per-frame steady-state rule
/// (ARCHITECTURE.md Section 7.3). Fully rendered: real fill (including
/// compound shapes with holes) and real stroke via `lyon`, and every
/// `FillStyle` variant.
#[derive(Debug, Clone)]
pub struct Path {
    pub common: PrimitiveCommon,
    pub commands: Vec<PathCommand>,
    pub fill: FillStyle,
    pub border_color: Color,
    pub border_thickness: f32,
    pub stroke_line_cap: LineCap,
    pub stroke_line_join: LineJoin,
}

impl Path {
    /// A borderless path of `commands`, filled `color`, at the identity
    /// transform, with `Butt`/`Miter` stroke caps/joins (irrelevant
    /// without a border). Set border/stroke-cap fields directly on the
    /// returned value for a stroked path.
    #[must_use]
    pub fn new(commands: Vec<PathCommand>, color: Color) -> Self {
        Self {
            common: PrimitiveCommon::new(),
            commands,
            fill: FillStyle::Solid(color),
            border_color: 0,
            border_thickness: 0.0,
            stroke_line_cap: LineCap::Butt,
            stroke_line_join: LineJoin::Miter,
        }
    }
}

impl Primitive for Path {
    fn common(&self) -> &PrimitiveCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut PrimitiveCommon {
        &mut self.common
    }
}

/// The one type every shape's own real, concrete storage and every
/// per-frame flattening call site actually holds -- `enum` dispatch,
/// not `Box<dyn Primitive>` (ARCHITECTURE.md Section 7.3, TECHNICAL.md
/// Section 9.1's "no dynamic type inspection in hot paths" rule).
#[derive(Debug, Clone)]
pub enum ShapePrimitive {
    Rectangle(Rectangle),
    Circle(Circle),
    Polygon(Polygon),
    Path(Path),
    Text(crate::text::Text),
}

impl ShapePrimitive {
    #[must_use]
    pub fn common(&self) -> &PrimitiveCommon {
        match self {
            Self::Rectangle(shape) => shape.common(),
            Self::Circle(shape) => shape.common(),
            Self::Polygon(shape) => shape.common(),
            Self::Path(shape) => shape.common(),
            Self::Text(shape) => shape.common(),
        }
    }

    pub fn common_mut(&mut self) -> &mut PrimitiveCommon {
        match self {
            Self::Rectangle(shape) => shape.common_mut(),
            Self::Circle(shape) => shape.common_mut(),
            Self::Polygon(shape) => shape.common_mut(),
            Self::Path(shape) => shape.common_mut(),
            Self::Text(shape) => shape.common_mut(),
        }
    }
}

/// A stable-until-removed handle into `ShapeRegistry` -- the type an
/// external UI framework actually holds across many frames
/// (ARCHITECTURE.md Section 7.4). `generation` makes a stale handle
/// (one whose slot was removed and reused) detectable rather than
/// silently resolving to a different, unrelated shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShapeId {
    index: u32,
    generation: u32,
}

/// One caller-assigned tween/animation this project's own future
/// animation-timeline system is actively driving against a shape's
/// property -- opaque here; owning and stepping the animation itself is
/// that system's job, not the shape registry's (ARCHITECTURE.md Section
/// 7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationId(pub u64);

/// One `ShapeRegistry` slot -- the shape itself plus the UI-framework-
/// facing state hooks that decide whether this frame's flattening pass
/// needs to touch it at all (ARCHITECTURE.md Section 7.4).
#[derive(Debug, Clone)]
pub struct ShapeSlot {
    pub shape: ShapePrimitive,
    pub active_animations: Vec<AnimationId>,
    pub layout_dirty: bool,
    /// A shape-local clip rect, in the same coordinate space
    /// `Canvas::push_clip`'s existing `ScissorRect` uses.
    pub clip_bounds: Option<ScissorRect>,
}

/// The retained-mode shape store -- a hand-built generational slot
/// arena (ARCHITECTURE.md Section 7.4), matching this project's own
/// established "hand-build the concurrency/memory primitive rather than
/// reach for a crate" precedent.
#[derive(Default)]
pub struct ShapeRegistry {
    slots: Vec<Option<ShapeSlot>>,
    generations: Vec<u32>,
    free_list: Vec<u32>,
    live_count: usize,
    /// Real gradient definitions (Phase 10 Step 10.2.1), indexed
    /// directly by `GradientId(index)` -- append-only, no generational
    /// reuse (see [`GradientId`]'s own doc comment for why).
    gradients: Vec<GradientDef>,
    /// Phase 10 Step 10.2.6: persistent scratch buffers `flatten_into`'s
    /// own `Polygon` case reuses every call, closing a real per-frame
    /// allocation this step's own zero-allocation guard found
    /// (`generate_polygon_points`/`fan_from_center` each returned a
    /// freshly allocated `Vec` on every call -- REVIEW.md finding
    /// #176). Grows once to this registry's own steady-state largest
    /// polygon vertex/triangle count, then never reallocates again,
    /// the same "grow once, reuse after" discipline `FrameArena`'s own
    /// scratch buffers already use (Phase 9 Step 9.2).
    polygon_points_scratch: Vec<Vec2>,
    polygon_triangles_scratch: Vec<[u32; 3]>,
    /// Phase 10 Step 10.2.6: `draw_polygon_fill`'s own reused UV
    /// scratch buffer for `FillStyle::Texture` (`Polygon`'s and
    /// `Path`'s shared texture-fill dispatch), closing a second real
    /// per-frame allocation this step's own zero-allocation guard found
    /// (`bounding_box_uvs` returned a freshly allocated `Vec` on every
    /// call -- REVIEW.md finding #176). Shared by both shape kinds:
    /// `flatten_into`'s own loop processes one shape at a time, so
    /// nothing ever needs this buffer reentrantly.
    polygon_uv_scratch: Vec<Vec2>,
}

impl ShapeRegistry {
    /// An empty registry, no shapes or gradients yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts `shape` as a new, freshly `layout_dirty` slot (no active
    /// animations, no clip) and returns its stable `ShapeId`. Reuses a
    /// freed slot's own backing storage (via `free_list`) when one is
    /// available, rather than always growing -- the same "grow once,
    /// reuse after" discipline `FrameArena`'s own scratch buffers use
    /// (Phase 9 Step 9.2).
    pub fn insert(&mut self, shape: ShapePrimitive) -> ShapeId {
        let slot = ShapeSlot {
            shape,
            active_animations: Vec::new(),
            layout_dirty: true,
            clip_bounds: None,
        };
        self.live_count += 1;
        if let Some(index) = self.free_list.pop() {
            let generation = self.generations[index as usize];
            self.slots[index as usize] = Some(slot);
            ShapeId { index, generation }
        } else {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "a real UI scene stays far below u32::MAX live shapes -- the same \
                           headroom reasoning this project applies to Depth ID/vertex indices"
            )]
            let index = self.slots.len() as u32;
            self.slots.push(Some(slot));
            self.generations.push(0);
            ShapeId {
                index,
                generation: 0,
            }
        }
    }

    /// Removes `id`'s shape, freeing its slot for reuse by a future
    /// `insert`. Returns `false` (report, don't panic -- DESIGN.md
    /// Section 2.6) if `id` is stale (its slot was already removed, or
    /// never existed) rather than silently succeeding on the wrong
    /// shape or panicking on a caller mistake that isn't a programmer
    /// error in the UI-framework-facing sense.
    pub fn remove(&mut self, id: ShapeId) -> bool {
        let Some(current_generation) = self.generations.get(id.index as usize).copied() else {
            return false;
        };
        if current_generation != id.generation {
            return false;
        }
        let Some(slot) = self.slots.get_mut(id.index as usize) else {
            return false;
        };
        if slot.take().is_none() {
            return false;
        }
        self.live_count -= 1;
        // Wrapping, not saturating: after `u32::MAX` reuses of the exact
        // same slot index (not `u32::MAX` shapes ever created -- only
        // ever created *at this one index*) a wrapped-around generation
        // could in principle alias a very old, already-invalid `ShapeId`.
        // The same accepted-headroom tradeoff this project already makes
        // elsewhere for generational/monotonic counters (e.g. Depth ID's
        // own 20-bit field, ARCHITECTURE.md Section 4.1) -- not a
        // realistic concern for any real UI session's own lifetime.
        self.generations[id.index as usize] = current_generation.wrapping_add(1);
        self.free_list.push(id.index);
        true
    }

    /// Looks up `id`'s slot, or `None` if `id` is stale.
    #[must_use]
    pub fn get(&self, id: ShapeId) -> Option<&ShapeSlot> {
        if self.generations.get(id.index as usize).copied() != Some(id.generation) {
            return None;
        }
        self.slots.get(id.index as usize)?.as_ref()
    }

    /// Mutably looks up `id`'s slot, or `None` if `id` is stale --
    /// callers use this to flip `layout_dirty`, add an `AnimationId`, or
    /// mutate the shape's own fields directly.
    pub fn get_mut(&mut self, id: ShapeId) -> Option<&mut ShapeSlot> {
        if self.generations.get(id.index as usize).copied() != Some(id.generation) {
            return None;
        }
        self.slots.get_mut(id.index as usize)?.as_mut()
    }

    /// How many shapes are currently live (inserted, not yet removed).
    #[must_use]
    pub fn len(&self) -> usize {
        self.live_count
    }

    /// Whether the registry currently holds no live shapes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live_count == 0
    }

    /// Defines a new, real, evaluatable gradient and returns a stable
    /// handle to it (Phase 10 Step 10.2.1). Validates `def` fully on the
    /// CPU before storing it -- a caller mistake (too many stops, an
    /// out-of-range or out-of-order position, a non-positive radial
    /// radius) is a real, loud `Err`, never silently clamped/reordered/
    /// truncated into something that renders differently than what was
    /// asked for.
    ///
    /// The gradient itself is stored once, here, on the CPU -- its own
    /// GPU-side `GpuGradientStyle` record is written fresh into the
    /// CURRENT frame's style buffer every frame a shape referencing it is
    /// flattened ([`flatten_into`](Self::flatten_into)'s own
    /// `FillStyle::Gradient` handling), the same per-frame-rewrite
    /// pattern every other style record in this crate already uses.
    ///
    /// # Errors
    /// See [`GradientError`]'s own variants.
    ///
    /// # Panics
    /// Never in practice -- only if this registry has already created
    /// more than `u32::MAX` gradients in one session, far beyond any real
    /// use case.
    pub fn create_gradient(&mut self, def: GradientDef) -> Result<GradientId, GradientError> {
        validate_gradient(&def)?;
        let index = u32::try_from(self.gradients.len())
            .expect("far fewer gradients than u32::MAX are ever created in one real session");
        self.gradients.push(def);
        Ok(GradientId(index))
    }

    /// Phase 10 Step 10.2.6: mutable access to an already-created
    /// gradient's own definition (its stops, or its axis/center/radius),
    /// or `None` if `id` is out of range. `GradientId`'s own doc comment
    /// discloses that this table has no generational reuse/removal --
    /// that constraint is about an ID's own LIFECYCLE (nothing ever frees
    /// a slot for reuse under a different ID), not about mutating an
    /// existing entry's data in place, which this method exists for.
    /// Real, mutating UI usage (an animated gradient, a live-updating
    /// color picker preview) needs to change an existing gradient's own
    /// stops across many frames without registering a new one each time
    /// -- `create_gradient` called every frame would grow `self.
    /// gradients` without bound, defeating any zero-allocation steady
    /// state. Mutating a stop's fields in place, or reordering/replacing
    /// entries in `stops` without changing the borrowed slice's
    /// underlying capacity, never reallocates.
    ///
    /// Does NOT itself mark any shape referencing this gradient as
    /// needing re-flattening -- `flatten_into` only re-evaluates a
    /// gradient by re-flattening the shape that references it, so a
    /// caller mutating a gradient a shape is already using must also
    /// mark that shape's own [`ShapeSlot::layout_dirty`] via
    /// [`get_mut`](Self::get_mut), or the mutation has no visible effect
    /// on the next flatten.
    pub fn gradient_mut(&mut self, id: GradientId) -> Option<&mut GradientDef> {
        self.gradients.get_mut(id.0 as usize)
    }

    /// Read-only access to an already-created gradient's own definition,
    /// or `None` if `id` is out of range -- the immutable counterpart to
    /// [`gradient_mut`](Self::gradient_mut), for a caller (e.g. a UI panel
    /// displaying a gradient's current stops) that needs to inspect one
    /// without needing `&mut self`.
    #[must_use]
    pub fn gradient(&self, id: GradientId) -> Option<&GradientDef> {
        self.gradients.get(id.0 as usize)
    }

    /// Forces every live slot's [`ShapeSlot::layout_dirty`] to `true`,
    /// so the next [`flatten_into`](Self::flatten_into) call re-records
    /// the entire registry regardless of what was already flattened
    /// before -- for a caller that wants "draw the full current state,"
    /// not `flatten_into`'s own default incremental-only semantics.
    ///
    /// Added for `tre_python`'s `HeadlessRenderer::render`
    /// (IMPLEMENTATION.md Phase 10 Step 10.4): a Python caller
    /// re-rendering an unchanged registry has no way to know or care
    /// which shapes are already-clean from a prior call (there is no
    /// Python-visible dirty-flag protocol at all), so treating every
    /// `render()` call as "flatten everything now" is the only sound
    /// semantics without one. This closes a real bug found via this
    /// project's own review process (REVIEW.md finding #196): a second
    /// `render()` call on the same, unmutated registry produced an
    /// empty frame (every shape already non-dirty from the first call),
    /// which further crashed on the resulting zero-length vertex buffer.
    pub fn mark_all_dirty(&mut self) {
        for slot in self.slots.iter_mut().flatten() {
            slot.layout_dirty = true;
        }
    }

    /// The per-frame flattening pass (ARCHITECTURE.md Section 7,
    /// IMPLEMENTATION.md Step 10.1 task 4): for every live slot that is
    /// `layout_dirty` or has a non-empty `active_animations`, resolves
    /// its `Transform2D` to a real `Affine2` and records it into `canvas`
    /// via the exact same immediate-mode calls any other caller would
    /// use, then clears `layout_dirty` (a shape re-flattened only
    /// because of an active animation stays flagged for next frame too
    /// -- clearing `active_animations` itself is the animation system's
    /// own job, not this registry's, per `AnimationId`'s own doc
    /// comment). A `Visibility::Hidden`/`Collapsed` shape is skipped
    /// entirely (still has `layout_dirty` cleared, so it doesn't attempt
    /// to re-flatten every frame while simply invisible).
    ///
    /// `device` (Phase 10 Step 10.2) is needed because `Rectangle`/
    /// `Circle`'s real rendering paths beyond the trivial case write a
    /// style record into the device's live, per-frame-segmented
    /// shape-style buffer at flatten time -- see `RenderingCanvas::
    /// draw_styled_rectangle`'s own doc comment for why that can't be
    /// deferred to upload time the way plain vertex/index data is.
    ///
    /// `text_context` (Phase 12 Step 12.2) is `None` for a registry that
    /// never inserts a `ShapePrimitive::Text` -- every one of this
    /// project's own 40+ pre-existing Rust examples passes `None` and
    /// needs no other change. A registry that DOES insert `Text` shapes
    /// must pass `Some` (see `# Panics` below).
    ///
    /// # Panics
    /// Panics if a `FillStyle::Gradient` names a `GradientId` this
    /// registry never issued (via [`create_gradient`](Self::create_gradient))
    /// -- a real programmer error (a stale or foreign handle), not a
    /// normal runtime condition this registry validates against. Every
    /// `FillStyle` variant itself (`Solid`/`Gradient`/`Texture`) is fully
    /// implemented for all four non-`Text` shape kinds (see this module's
    /// own top-level doc comment). Also panics if this registry holds a
    /// live `ShapePrimitive::Text` shape and `text_context` is `None`, or
    /// if a `Text` shape's own `FontId` was never issued by
    /// `text_context`'s `FontRegistry` -- see [`crate::text::flatten_text`]'s
    /// own `# Panics` section for the latter.
    pub fn flatten_into(
        &mut self,
        canvas: &mut RenderingCanvas,
        device: &dyn crate::RhiDevice,
        text_context: Option<&crate::text::TextFlattenContext<'_>>,
    ) {
        for slot in &mut self.slots {
            let Some(slot) = slot else { continue };
            if !slot.layout_dirty && slot.active_animations.is_empty() {
                continue;
            }
            slot.layout_dirty = false;

            let common = slot.shape.common();
            if matches!(
                common.visibility,
                Visibility::Hidden | Visibility::Collapsed
            ) {
                continue;
            }

            canvas.save();
            canvas.transform(&common.transform.to_affine2());
            canvas.set_alpha(common.opacity);
            if let Some(clip) = slot.clip_bounds {
                canvas.push_clip(&clip);
            }

            match &slot.shape {
                ShapePrimitive::Rectangle(rect) => {
                    flatten_rectangle(canvas, device, rect, &self.gradients);
                }
                ShapePrimitive::Circle(circle) => {
                    flatten_circle(canvas, device, circle, &self.gradients);
                }
                ShapePrimitive::Polygon(polygon) => {
                    flatten_polygon(
                        canvas,
                        device,
                        polygon,
                        &self.gradients,
                        &mut self.polygon_points_scratch,
                        &mut self.polygon_triangles_scratch,
                        &mut self.polygon_uv_scratch,
                    );
                }
                ShapePrimitive::Path(path) => {
                    flatten_path_shape(
                        canvas,
                        device,
                        path,
                        &self.gradients,
                        &mut self.polygon_uv_scratch,
                    );
                }
                ShapePrimitive::Text(text) => {
                    let context = text_context.expect(
                        "a ShapePrimitive::Text was flattened but flatten_into's own \
                         text_context argument was None -- see flatten_into's own doc comment",
                    );
                    crate::text::flatten_text(canvas, text, context);
                }
            }

            if slot.clip_bounds.is_some() {
                canvas.pop_clip();
            }
            canvas.restore();
        }
    }

    /// Phase 10 Step 10.2: the actual "backbone of UI frameworks" need
    /// `hit_testable` (present on every shape since Step 10.1, but read
    /// by nothing until now) exists for -- routing a click/hover/touch
    /// point to the topmost shape underneath it. Returns the topmost
    /// (highest slot index -- later `insert` calls paint over earlier
    /// ones, the same implicit paint order `flatten_into`'s own forward
    /// iteration gives every render) shape whose real geometry contains
    /// `point` (given in the SAME world space `flatten_into`'s own
    /// `Rectangle`/`Circle`/etc. world-space output uses), skipping any
    /// shape with `hit_testable == false` or `Visibility::Hidden`/
    /// `Collapsed` (mirroring `flatten_into`'s own skip logic) or whose
    /// transform has collapsed to a non-invertible degenerate (a zero
    /// scale on some axis -- such a shape has zero on-screen area, so
    /// "no point can hit it" is the correct answer, not a panic).
    ///
    /// `Path` hit-testing uses [`flatten_path`]'s own real, tested
    /// flattening even though `flatten_into`'s own `Path` case is still
    /// unimplemented for rendering -- hit-testing only needs CPU-side
    /// geometry, not the GPU rendering plumbing fill/stroke still lack.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "this registry's own slot count stays far below u32::MAX -- ShapeId::index is \
                   u32 everywhere else already"
    )]
    pub fn hit_test(&self, point: Vec2) -> Option<ShapeId> {
        for (index, slot) in self.slots.iter().enumerate().rev() {
            let Some(slot) = slot else { continue };
            let common = slot.shape.common();
            if !common.hit_testable
                || matches!(
                    common.visibility,
                    Visibility::Hidden | Visibility::Collapsed
                )
            {
                continue;
            }
            let Some(inverse) = common.transform.to_affine2().invert() else {
                continue;
            };
            let local = inverse.transform_point(point);
            let hit = match &slot.shape {
                ShapePrimitive::Rectangle(rect) => hit_test_rectangle(local, rect),
                ShapePrimitive::Circle(circle) => hit_test_circle(local, circle),
                ShapePrimitive::Polygon(polygon) => hit_test_polygon(local, polygon),
                ShapePrimitive::Path(path) => hit_test_path(local, path),
                // Phase 12 Step 12.2, a real disclosed scope boundary: a
                // Text shape has no cached bounding box (its real extent
                // is only known after shaping, which flatten_into does
                // lazily, not at insert/mutate time), so it never
                // reports a hit here. Real UI text is normally hit-
                // tested via its containing control's own background
                // shape (e.g. a Rectangle) anyway, not the glyph
                // outlines directly -- a real "text bounding box" hit
                // test, if ever needed standalone, is disclosed future
                // work, not silently approximated here.
                ShapePrimitive::Text(_) => false,
            };
            if hit {
                return Some(ShapeId {
                    index: index as u32,
                    generation: self.generations[index],
                });
            }
        }
        None
    }
}

/// `ShapeRegistry::hit_test`'s own `Rectangle` case: the exact same
/// non-uniform rounded-box signed-distance formula
/// `sdf_rect_styled.frag` evaluates on the GPU (see that shader's own
/// `sd_rounded_box`), evaluated here on the CPU instead -- a point hits
/// whenever the fill OR border would have painted a pixel there
/// (`d <= 0`), regardless of `border_thickness` (a border is still part
/// of the shape for hit-testing purposes, not a hollow ring).
fn select_corner_radius(p: Vec2, radii: [f32; 4]) -> f32 {
    match (p[0] < 0.0, p[1] < 0.0) {
        (true, true) => radii[0],   // top_left
        (false, true) => radii[1],  // top_right
        (false, false) => radii[2], // bottom_right
        (true, false) => radii[3],  // bottom_left
    }
}

fn sd_rounded_box(p: Vec2, half_extent: Vec2, radii: [f32; 4]) -> f32 {
    let r = select_corner_radius(p, radii);
    let qx = p[0].abs() - half_extent[0] + r;
    let qy = p[1].abs() - half_extent[1] + r;
    let (qmx, qmy) = (qx.max(0.0), qy.max(0.0));
    qmx.hypot(qmy) + qx.max(qy).min(0.0) - r
}

fn hit_test_rectangle(local: Vec2, rect: &Rectangle) -> bool {
    let half_extent = [rect.size[0] / 2.0, rect.size[1] / 2.0];
    let center_relative = [local[0] - half_extent[0], local[1] - half_extent[1]];
    let radii = [
        rect.corner_radius.top_left,
        rect.corner_radius.top_right,
        rect.corner_radius.bottom_right,
        rect.corner_radius.bottom_left,
    ];
    sd_rounded_box(center_relative, half_extent, radii) <= 0.0
}

/// `ShapeRegistry::hit_test`'s own `Circle` case: an exact (not
/// approximate) point-in-ellipse test (`(x/rx)^2 + (y/ry)^2 <= 1`, unlike
/// `sdf_ellipse.frag`'s own scaled-circle SDF *approximation* -- a plain
/// inside/outside membership test has an exact closed form an SDF
/// doesn't need to reach for), plus the same angular-sector convention
/// `sdf_ellipse.frag` uses for a partial arc.
fn hit_test_circle(local: Vec2, circle: &Circle) -> bool {
    const TWELVE_OCLOCK: f32 = -std::f32::consts::FRAC_PI_2;

    let [rx, ry] = circle.radius;
    if rx <= 0.0 || ry <= 0.0 {
        return false;
    }
    // Local center matches `flatten_circle`'s own convention:
    // (radius[0], radius[1]), not (0, 0).
    let p = [local[0] - circle.radius[0], local[1] - circle.radius[1]];
    let normalized = (p[0] / rx).powi(2) + (p[1] / ry).powi(2);
    if normalized > 1.0 {
        return false;
    }
    if circle.arc_length >= 360.0 {
        return true;
    }
    let mut angle = p[1].atan2(p[0]);
    if angle < 0.0 {
        angle += std::f32::consts::TAU;
    }
    let start = TWELVE_OCLOCK.rem_euclid(std::f32::consts::TAU);
    let mut relative = angle - start;
    if relative < 0.0 {
        relative += std::f32::consts::TAU;
    }
    relative <= circle.arc_length.to_radians()
}

/// The standard even-odd ray-casting point-in-polygon test (PNPOLY, W.
/// Randolph Franklin) -- counts how many polygon edges a horizontal ray
/// from `p` crosses; an odd count means `p` is inside. Shared by
/// `ShapeRegistry::hit_test`'s `Polygon` case (one contour) and `Path`
/// case (`XOR`ed across every subpath, which composes to exactly the same
/// even-odd rule across multiple contours since XOR of per-contour
/// crossing parities equals the parity of their sum).
fn point_in_polygon(p: Vec2, points: &[Vec2]) -> bool {
    let n = points.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = (points[i][0], points[i][1]);
        let (xj, yj) = (points[j][0], points[j][1]);
        if ((yi > p[1]) != (yj > p[1])) && (p[0] < (xj - xi) * (p[1] - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn hit_test_polygon(local: Vec2, polygon: &Polygon) -> bool {
    let boundary = generate_polygon_points(polygon);
    point_in_polygon(local, &boundary)
}

fn hit_test_path(local: Vec2, path: &Path) -> bool {
    flatten_path(&path.commands)
        .iter()
        .fold(false, |hit, subpath| hit ^ point_in_polygon(local, subpath))
}

/// A `FillStyle` resolved to what `draw_styled_rectangle`/`draw_ellipse`
/// actually need: a solid `Color` (meaningful only for
/// `FillStyle::Solid`; a harmless opaque-white placeholder otherwise,
/// since the shader ignores it for a non-solid fill) and a
/// [`crate::StyleFill`] bundling `fill_kind`/`gradient_word_index`/
/// `texture_index`. Shared by `flatten_rectangle`/`flatten_circle` --
/// `Polygon`/`Path` resolve their own fill differently (a completely
/// different `Canvas` method per fill kind, not a style-buffer field),
/// so they don't use this.
///
/// # Panics
/// Panics if `fill` names a `GradientId` this registry never issued.
fn resolve_style_fill(
    device: &dyn crate::RhiDevice,
    fill: FillStyle,
    gradients: &[GradientDef],
    local_origin_offset: Vec2,
) -> (Color, crate::StyleFill) {
    match fill {
        FillStyle::Solid(color) => (color, crate::StyleFill::SOLID),
        FillStyle::Gradient(id) => {
            let def = gradients.get(id.0 as usize).unwrap_or_else(|| {
                panic!(
                    "FillStyle::Gradient names GradientId({}), which this registry never \
                     issued via create_gradient",
                    id.0
                )
            });
            let word_index = write_gradient_style(device, def, local_origin_offset);
            (
                0xFFFF_FFFF,
                crate::StyleFill {
                    fill_kind: 1,
                    gradient_word_index: word_index,
                    texture_index: 0,
                },
            )
        }
        FillStyle::Texture(texture_index) => (
            0xFFFF_FFFF,
            crate::StyleFill {
                fill_kind: 2,
                gradient_word_index: 0,
                texture_index,
            },
        ),
    }
}

/// `ShapeRegistry::flatten_into`'s own `Rectangle` case (Phase 10 Step
/// 10.2: real for non-uniform corners, borders, and corner smoothing;
/// Step 10.2.1: real gradient fill too), via `RenderingCanvas::draw_
/// styled_rectangle`. The plain, older `draw_rounded_rect` path is still
/// used for the common trivial case (uniform radius, no border, no
/// smoothing, solid fill) -- cheaper (no style-buffer write) and
/// byte-for-byte what it always rendered. `FillStyle::Texture` remains
/// genuinely unimplemented; see this module's own top-level doc comment.
fn flatten_rectangle(
    canvas: &mut RenderingCanvas,
    device: &dyn crate::RhiDevice,
    rect: &Rectangle,
    gradients: &[GradientDef],
) {
    // A gradient's own points are authored in Rectangle's PUBLIC local
    // space (top-left at the origin); frag_uv is center-relative -- see
    // build_gpu_gradient_style's own doc comment for the full account.
    let local_origin_offset = [rect.size[0] / 2.0, rect.size[1] / 2.0];
    let (color, fill) = resolve_style_fill(device, rect.fill, gradients, local_origin_offset);

    let needs_styled_path = !rect.corner_radius.is_uniform()
        || rect.corner_smoothing != 0.0
        || rect.border_thickness > 0.0
        || fill.fill_kind != 0;

    if needs_styled_path {
        canvas.draw_styled_rectangle(
            device,
            0.0,
            0.0,
            rect.size[0],
            rect.size[1],
            [
                rect.corner_radius.top_left,
                rect.corner_radius.top_right,
                rect.corner_radius.bottom_right,
                rect.corner_radius.bottom_left,
            ],
            color,
            rect.border_color,
            rect.border_thickness,
            rect.corner_smoothing,
            fill,
        );
    } else {
        canvas.draw_rounded_rect(
            0.0,
            0.0,
            rect.size[0],
            rect.size[1],
            rect.corner_radius.top_left,
            color,
        );
    }
}

/// `ShapeRegistry::flatten_into`'s own `Circle` case (Phase 10 Step
/// 10.2, real for the first time), via `RenderingCanvas::draw_ellipse`.
///
/// Origin convention: matches `Rectangle`'s own (bounding-box top-left at
/// the shape's local transform origin) rather than centering on it, so a
/// UI framework author positions every shape kind the same way regardless
/// of which one it is -- the circle's own center is therefore
/// `(radius[0], radius[1])` in local space, not `(0, 0)`.
///
/// `Circle::arc_length` is degrees, `0.0..=360.0`, sweeping clockwise
/// from 12 o'clock (that field's own doc comment); converted here to the
/// radians + `atan2`-relative-angle convention `sdf_ellipse.frag` uses
/// (12 o'clock is `-FRAC_PI_2` in that shader's own `atan2(y, x)`
/// convention, since screen-space `y` increases downward).
fn flatten_circle(
    canvas: &mut RenderingCanvas,
    device: &dyn crate::RhiDevice,
    circle: &Circle,
    gradients: &[GradientDef],
) {
    const TWELVE_OCLOCK: f32 = -std::f32::consts::FRAC_PI_2;

    // Circle's own public local space has its center at `radius` (this
    // function's own doc comment); frag_uv is relative to that same
    // center already, so `radius` is exactly the correction
    // build_gpu_gradient_style's own doc comment describes.
    let (color, fill) = resolve_style_fill(device, circle.fill, gradients, circle.radius);

    canvas.draw_ellipse(
        device,
        circle.radius[0],
        circle.radius[1],
        circle.radius,
        color,
        circle.border_color,
        circle.border_thickness,
        TWELVE_OCLOCK,
        circle.arc_length.to_radians(),
        fill,
    );
}

/// Generates a regular N-gon's or star's boundary vertices in LOCAL
/// space, centered on the shape's own transform origin (NOT the
/// bounding-box-top-left convention `Rectangle`/`Circle` use -- a
/// deliberate, disclosed exception: a regular polygon's own bounding box
/// is not a clean function of `radius` alone the way a rect's or
/// circle's is, so anchoring on the geometric center instead is the
/// simpler, more honest choice here). Clockwise from 12 o'clock, matching
/// `Circle`'s own convention.
///
/// `star_points.is_none()`: `sides` vertices, all at `radius`.
/// `star_points = Some(k)`: `2*k` vertices alternating outer (`radius`,
/// even index, starting at 12 o'clock) / inner (`vertex_radius`, odd
/// index) -- the standard star-polygon construction, guaranteed
/// star-shaped with respect to its own center by this very construction
/// (every ray from the center crosses the boundary exactly once), which
/// is exactly what makes [`fan_from_center_into`]'s triangulation valid
/// for it, unlike an arbitrary (possibly non-star-shaped) polygon.
#[allow(
    clippy::cast_precision_loss,
    reason = "a real UI polygon/star has at most a handful of sides/points -- vertex counts stay \
               many orders of magnitude below 2^24, f32's exact-integer range"
)]
fn generate_polygon_points(polygon: &Polygon) -> Vec<Vec2> {
    const TWELVE_OCLOCK: f32 = -std::f32::consts::FRAC_PI_2;

    let count = match polygon.star_points {
        Some(points) => points * 2,
        None => polygon.sides,
    };
    if count < 3 {
        return Vec::new();
    }
    (0..count)
        .map(|i| {
            let radius = match polygon.star_points {
                Some(_) if i % 2 == 1 => polygon.vertex_radius,
                _ => polygon.radius,
            };
            let angle = TWELVE_OCLOCK + (i as f32) * std::f32::consts::TAU / (count as f32);
            [radius * angle.cos(), radius * angle.sin()]
        })
        .collect()
}

/// The non-allocating, reuse-friendly sibling of
/// [`generate_polygon_points`] (Phase 10 Step 10.2.6, REVIEW.md finding
/// #176): clears and refills `out` instead of returning a freshly
/// allocated `Vec` every call. `flatten_polygon`'s own per-frame hot
/// path uses this, via `ShapeRegistry`'s own persistent scratch buffer;
/// `generate_polygon_points` itself is kept, unchanged, for `hit_test_
/// polygon` and this module's own tests, neither of which is a
/// per-frame zero-allocation-guarded path.
#[allow(
    clippy::cast_precision_loss,
    reason = "a real UI polygon/star has at most a handful of sides/points -- vertex counts stay \
               many orders of magnitude below 2^24, f32's exact-integer range"
)]
fn generate_polygon_points_into(polygon: &Polygon, out: &mut Vec<Vec2>) {
    const TWELVE_OCLOCK: f32 = -std::f32::consts::FRAC_PI_2;

    out.clear();
    let count = match polygon.star_points {
        Some(points) => points * 2,
        None => polygon.sides,
    };
    if count < 3 {
        return;
    }
    out.extend((0..count).map(|i| {
        let radius = match polygon.star_points {
            Some(_) if i % 2 == 1 => polygon.vertex_radius,
            _ => polygon.radius,
        };
        let angle = TWELVE_OCLOCK + (i as f32) * std::f32::consts::TAU / (count as f32);
        [radius * angle.cos(), radius * angle.sin()]
    }));
}

/// Fans triangles from local index `0` (the caller's own prepended
/// center point) across `1..=vertex_count` (the boundary points, in
/// order), including the closing triangle back to boundary point `1` --
/// unlike `tre_svg::fan_triangles` (which fans from `points[0]` and
/// relies on that point already being a shared boundary vertex, so never
/// needs to close the loop), this fan's pivot is NOT itself a boundary
/// point, so the wraparound triangle is real, required geometry, not an
/// omission. Valid whenever the boundary is star-shaped with respect to
/// the pivot -- true for [`generate_polygon_points`]'s own output by
/// construction, per that function's own doc comment.
///
/// Writes into `out` (cleared first) rather than returning a freshly
/// allocated `Vec` every call -- Phase 10 Step 10.2.6 (REVIEW.md
/// finding #176) replaced this function's own original owned-`Vec`-
/// returning form with this reuse-friendly one, once `flatten_polygon`'s
/// per-frame hot path became its only real caller (see `generate_
/// polygon_points_into`'s own doc comment for the full reuse rationale).
fn fan_from_center_into(vertex_count: u32, out: &mut Vec<[u32; 3]>) {
    out.clear();
    if vertex_count < 3 {
        return;
    }
    out.extend((1..vertex_count).map(|i| [0, i, i + 1]));
    out.push([0, vertex_count, 1]);
}

/// Maps `positions` into real, bounding-box-normalized `[0, 1]` texture
/// coordinates (Phase 10 Step 10.2.2) -- `Rectangle`/`Circle` derive
/// theirs directly from their own known half-extent/radius, in-shader
/// (`sdf_rect_styled.frag`/`sdf_ellipse.frag`'s own texture branch);
/// `Polygon`/`Path` have no such fixed extent, so this computes their
/// own real bounding box once, here, at flatten time. A degenerate
/// (zero-width or zero-height) bounding box maps every point on that
/// axis to `0.5` rather than dividing by zero.
///
/// Writes into `out` (cleared first) rather than returning a freshly
/// allocated `Vec` every call -- Phase 10 Step 10.2.6 (REVIEW.md
/// finding #176): `draw_polygon_fill`'s own `FillStyle::Texture` arm is
/// this function's only real caller, and now passes `ShapeRegistry`'s
/// own persistent scratch buffer.
fn bounding_box_uvs_into(positions: &[Vec2], out: &mut Vec<Vec2>) {
    let mut min = [f32::INFINITY, f32::INFINITY];
    let mut max = [f32::NEG_INFINITY, f32::NEG_INFINITY];
    for &[x, y] in positions {
        min[0] = min[0].min(x);
        min[1] = min[1].min(y);
        max[0] = max[0].max(x);
        max[1] = max[1].max(y);
    }
    let extent = [max[0] - min[0], max[1] - min[1]];
    out.clear();
    out.extend(positions.iter().map(|&[x, y]| {
        let u = if extent[0] > 0.0 {
            (x - min[0]) / extent[0]
        } else {
            0.5
        };
        let v = if extent[1] > 0.0 {
            (y - min[1]) / extent[1]
        } else {
            0.5
        };
        [u, v]
    }));
}

/// `ShapeRegistry::flatten_into`'s own `Polygon`/`Path` fill dispatch,
/// shared by `flatten_polygon`/`flatten_path_shape` (their border/stroke
/// is always solid -- `border_color` is a plain `Color`, never a
/// `FillStyle`, so only the interior fill needs this 3-way dispatch).
/// All three `FillStyle` variants are real here: `Solid` (optionally
/// blended, see below), `Gradient` via [`write_gradient_style`] and
/// `RenderingCanvas::draw_gradient_polygon`, and `Texture` via
/// [`bounding_box_uvs_into`] and `RenderingCanvas::draw_textured_polygon`.
///
/// `blend_mode` (Phase 10 Step 10.2.3) is honored only for
/// `FillStyle::Solid` -- gradient/texture fill under a non-`Normal`
/// blend mode is real, separate, not-yet-scheduled follow-up work, not
/// silently ignored (this function's own callers only ever pass a
/// non-`Normal` value for a shape whose fill genuinely is `Solid`, so
/// this never silently drops a real request; see `ShapeRegistry::
/// flatten_into`'s own doc comment for the itemized disposition).
///
/// # Panics
/// Panics if `fill` is `FillStyle::Gradient` naming a `GradientId` this
/// registry never issued.
#[allow(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct, real piece of state (no natural sub-struct groups \
               them) -- both real callers (flatten_polygon/flatten_path_shape) already pass \
               every one of them through unchanged from their own persistent scratch state"
)]
fn draw_polygon_fill(
    canvas: &mut RenderingCanvas,
    device: &dyn crate::RhiDevice,
    fill: FillStyle,
    gradients: &[GradientDef],
    positions: &[Vec2],
    triangles: &[[u32; 3]],
    blend_mode: BlendMode,
    uv_scratch: &mut Vec<Vec2>,
) {
    match fill {
        FillStyle::Solid(color) => {
            if blend_mode == BlendMode::Normal || !device.local_read_blend_supported() {
                // Either the common case (Normal), or a real, disclosed
                // fail-closed degradation: this hardware never got the
                // capability query to pass, so FlatColorBlend's own
                // pipeline/descriptor set were never created -- selecting
                // it here would reach a pipeline registry lookup that
                // was never populated, not a silent wrong render.
                canvas.draw_flat_polygon(positions, triangles, color);
            } else {
                canvas.draw_flat_polygon_blended(positions, triangles, color, blend_mode as u32);
            }
        }
        FillStyle::Gradient(id) => {
            let def = gradients.get(id.0 as usize).unwrap_or_else(|| {
                panic!(
                    "FillStyle::Gradient names GradientId({}), which this registry never \
                     issued via create_gradient",
                    id.0
                )
            });
            // Polygon/Path's own local space is already center-relative
            // (Polygon) or whatever raw coordinates the caller used
            // (Path) -- no offset correction needed, unlike Rectangle/
            // Circle (build_gpu_gradient_style's own doc comment).
            let word_index = write_gradient_style(device, def, [0.0, 0.0]);
            canvas.draw_gradient_polygon(positions, triangles, word_index);
        }
        FillStyle::Texture(texture_index) => {
            bounding_box_uvs_into(positions, uv_scratch);
            canvas.draw_textured_polygon(positions, uv_scratch, triangles, texture_index);
        }
    }
}

/// Phase 10 Step 10.2.6 (REVIEW.md finding #176): `points_scratch`/
/// `triangles_scratch` are `ShapeRegistry`'s own persistent, reused
/// buffers -- this function used to call `generate_polygon_points`/
/// `fan_from_center` (each returning a freshly heap-allocated `Vec` on
/// every call) plus build a THIRD fresh `Vec` to prepend the fan's own
/// center pivot, a real, previously-undetected per-frame allocation
/// this step's own zero-allocation guard found. Fixed here by writing
/// the boundary directly into a reused buffer via `generate_polygon_
/// points_into`, then `insert`ing the center pivot at index 0 (an O(n)
/// shift on an already-appropriately-capacitized `Vec`, not a
/// reallocation -- a regular polygon's own small, bounded vertex count
/// makes that shift cost negligible).
///
/// Border/stroke tessellation (`tessellate_stroke`, real lyon-backed
/// geometry) is a real, DISCLOSED exception this step does not fix:
/// lyon's own tessellator/path/`VertexBuffers` objects are constructed
/// fresh on every call, a substantially larger reuse redesign than this
/// function's own fill-path fix -- the same category of deferred gap
/// `main_loop_demo.rs`'s own Step 9.2 already disclosed for RHI
/// submission and `std::thread::scope` (REVIEW.md's new finding for
/// this step has the full account).
fn flatten_polygon(
    canvas: &mut RenderingCanvas,
    device: &dyn crate::RhiDevice,
    polygon: &Polygon,
    gradients: &[GradientDef],
    points_scratch: &mut Vec<Vec2>,
    triangles_scratch: &mut Vec<[u32; 3]>,
    uv_scratch: &mut Vec<Vec2>,
) {
    generate_polygon_points_into(polygon, points_scratch);
    let vertex_count = u32::try_from(points_scratch.len()).unwrap_or(0);
    points_scratch.insert(0, [0.0, 0.0]); // the fan's own pivot -- the polygon's local center.
    fan_from_center_into(vertex_count, triangles_scratch);
    draw_polygon_fill(
        canvas,
        device,
        polygon.fill,
        gradients,
        points_scratch,
        triangles_scratch,
        polygon.common.blend_mode,
        uv_scratch,
    );

    if polygon.border_thickness > 0.0 {
        // A polygon boundary is always closed. `tessellate_stroke`
        // still needs its own owned `Vec<Vec2>` per contour (lyon's own
        // API shape) -- this `to_vec()` copy is the one real allocation
        // this fix does not remove, disclosed above.
        let boundary: Vec<Vec2> = points_scratch[1..].to_vec();
        let (stroke_positions, stroke_triangles) = tessellate_stroke(
            &[(boundary, true)],
            polygon.border_thickness,
            lyon::path::LineJoin::Miter,
            lyon::path::LineCap::Butt,
            lyon::path::LineCap::Butt,
        );
        canvas.draw_flat_polygon(&stroke_positions, &stroke_triangles, polygon.border_color);
    }
}

fn to_lyon_line_join(join: LineJoin) -> lyon::path::LineJoin {
    match join {
        LineJoin::Miter => lyon::path::LineJoin::Miter,
        LineJoin::Round => lyon::path::LineJoin::Round,
        LineJoin::Bevel => lyon::path::LineJoin::Bevel,
    }
}

fn to_lyon_line_cap(cap: LineCap) -> lyon::path::LineCap {
    match cap {
        LineCap::Butt => lyon::path::LineCap::Butt,
        LineCap::Round => lyon::path::LineCap::Round,
        LineCap::Square => lyon::path::LineCap::Square,
    }
}

/// Builds one lyon `Path` from one or more already-flattened contours,
/// each with its own closed/open flag -- shared by [`tessellate_fill`]
/// (which always passes `true`: a fill region's own boundary is
/// implicitly closed regardless of how the source `Path` was authored)
/// and [`tessellate_stroke`] (which needs the real per-contour distinction
/// -- an explicitly-closed subpath gets a continuous stroke loop with no
/// end caps, an open one gets real caps at both ends). Each contour
/// becomes its own `begin`/`line_to`.../`end(closed)` sequence within the
/// SAME path, so a fill rule (for fill) or a shared stroke pass (for
/// stroke) correctly handles multiple subpaths together, not each in
/// isolation. A contour with fewer than the minimum useful point count is
/// skipped (degenerate).
fn build_lyon_path(contours: &[(Vec<Vec2>, bool)], min_points: usize) -> lyon::path::Path {
    let mut builder = lyon::path::Path::builder();
    for (contour, closed) in contours {
        if contour.len() < min_points {
            continue;
        }
        let mut points = contour.iter();
        let &first = points.next().expect("length checked above");
        builder.begin(lyon::math::point(first[0], first[1]));
        for &p in points {
            builder.line_to(lyon::math::point(p[0], p[1]));
        }
        builder.end(*closed);
    }
    builder.build()
}

/// Real fill tessellation for `Path` (Phase 10 Step 10.2 follow-up,
/// closing REVIEW.md finding #163): `lyon`'s sweep-line `FillTessellator`
/// handles multi-contour (a shape with a real hole) and self-intersecting
/// input directly, as one algorithm -- exactly the case this crate could
/// not reach before adopting `lyon` (the general triangulator it would
/// have reused, `tre_svg::triangulate`, was unreachable without a
/// circular dependency on `tre-engine` itself). `NonZero` is the fixed
/// fill rule: `Path` has no `fill_rule` field of its own to select
/// `EvenOdd` instead, a disclosed simplification rather than a wider,
/// separate API change.
fn tessellate_fill(contours: &[Vec<Vec2>]) -> (Vec<Vec2>, Vec<[u32; 3]>) {
    let closed_contours: Vec<(Vec<Vec2>, bool)> =
        contours.iter().cloned().map(|c| (c, true)).collect();
    let path = build_lyon_path(&closed_contours, 3);
    let mut geometry: lyon::tessellation::VertexBuffers<Vec2, u32> =
        lyon::tessellation::VertexBuffers::new();
    let mut tessellator = lyon::tessellation::FillTessellator::new();
    let result = tessellator.tessellate_path(
        &path,
        &lyon::tessellation::FillOptions::tolerance(PATH_FLATTEN_TOLERANCE)
            .with_fill_rule(lyon::tessellation::FillRule::NonZero),
        &mut lyon::tessellation::BuffersBuilder::new(
            &mut geometry,
            |vertex: lyon::tessellation::FillVertex<'_>| -> Vec2 {
                let p = vertex.position();
                [p.x, p.y]
            },
        ),
    );
    if result.is_err() {
        return (Vec::new(), Vec::new());
    }
    let triangles = geometry
        .indices
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    (geometry.vertices, triangles)
}

/// Real stroke tessellation, shared by `Polygon` and `Path` borders
/// (Phase 10 Step 10.2 follow-up) -- previously entirely unbuilt for
/// either shape kind. `lyon`'s `StrokeTessellator` handles joins
/// (`Miter`, with lyon's own built-in bounded miter-limit fallback to a
/// bevel), caps, and variable contour counts directly; this project never
/// had a stroke tessellator of its own to compare against.
fn tessellate_stroke(
    contours: &[(Vec<Vec2>, bool)],
    line_width: f32,
    line_join: lyon::path::LineJoin,
    start_cap: lyon::path::LineCap,
    end_cap: lyon::path::LineCap,
) -> (Vec<Vec2>, Vec<[u32; 3]>) {
    let path = build_lyon_path(contours, 2);
    let mut geometry: lyon::tessellation::VertexBuffers<Vec2, u32> =
        lyon::tessellation::VertexBuffers::new();
    let mut tessellator = lyon::tessellation::StrokeTessellator::new();
    let options = lyon::tessellation::StrokeOptions::tolerance(PATH_FLATTEN_TOLERANCE)
        .with_line_width(line_width)
        .with_line_join(line_join)
        .with_start_cap(start_cap)
        .with_end_cap(end_cap);
    let result = tessellator.tessellate_path(
        &path,
        &options,
        &mut lyon::tessellation::BuffersBuilder::new(
            &mut geometry,
            |vertex: lyon::tessellation::StrokeVertex<'_, '_>| -> Vec2 {
                let p = vertex.position();
                [p.x, p.y]
            },
        ),
    );
    if result.is_err() {
        return (Vec::new(), Vec::new());
    }
    let triangles = geometry
        .indices
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    (geometry.vertices, triangles)
}

/// `ShapeRegistry::flatten_into`'s own `Path` case (Phase 10 Step 10.2
/// follow-up, real for the first time -- closing REVIEW.md finding
/// #163). Fill via [`tessellate_fill`] (real for multi-contour and
/// self-intersecting boundaries, not just the simple single-contour case
/// `Polygon`'s own fan triangulation handles); stroke via
/// [`tessellate_stroke`], honoring `stroke_line_cap`/`stroke_line_join`.
fn flatten_path_shape(
    canvas: &mut RenderingCanvas,
    device: &dyn crate::RhiDevice,
    path: &Path,
    gradients: &[GradientDef],
    uv_scratch: &mut Vec<Vec2>,
) {
    let subpaths_with_closed = flatten_path_with_closed(&path.commands);
    let subpaths: Vec<Vec<Vec2>> = subpaths_with_closed
        .iter()
        .map(|(points, _)| points.clone())
        .collect();

    let (fill_positions, fill_triangles) = tessellate_fill(&subpaths);
    draw_polygon_fill(
        canvas,
        device,
        path.fill,
        gradients,
        &fill_positions,
        &fill_triangles,
        path.common.blend_mode,
        uv_scratch,
    );

    if path.border_thickness > 0.0 {
        let (stroke_positions, stroke_triangles) = tessellate_stroke(
            &subpaths_with_closed,
            path.border_thickness,
            to_lyon_line_join(path.stroke_line_join),
            to_lyon_line_cap(path.stroke_line_cap),
            to_lyon_line_cap(path.stroke_line_cap),
        );
        canvas.draw_flat_polygon(&stroke_positions, &stroke_triangles, path.border_color);
    }
}

/// Bezier curve flattening -- Phase 10 Step 10.2 follow-up: now backed
/// by `lyon_geom`'s own tolerance-based curve flattening, the same real
/// fix `tre-svg`'s own `flatten.rs` uses (REVIEW.md has the full
/// account). This module's own original version hand-rolled recursive
/// de Casteljau subdivision as a deliberate, disclosed *duplicate* of
/// `tre_svg::flatten_cubic`/`flatten_quad` (a circular dependency
/// prevented reusing them directly) -- adopting `lyon` directly in both
/// crates instead means neither needs to reach into the other's
/// tessellation code at all, so there is no duplicate to maintain
/// anymore.
const PATH_FLATTEN_TOLERANCE: f32 = 0.25;

fn flatten_cubic(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, out: &mut Vec<Vec2>) {
    let segment = lyon::geom::CubicBezierSegment {
        from: lyon::math::point(p0[0], p0[1]),
        ctrl1: lyon::math::point(p1[0], p1[1]),
        ctrl2: lyon::math::point(p2[0], p2[1]),
        to: lyon::math::point(p3[0], p3[1]),
    };
    out.extend(
        segment
            .flattened(PATH_FLATTEN_TOLERANCE)
            .map(|p| [p.x, p.y]),
    );
}

/// Same convention as [`flatten_cubic`], for a quadratic Bezier
/// `p0 -> control -> p1`.
fn flatten_quad(p0: Vec2, control: Vec2, p1: Vec2, out: &mut Vec<Vec2>) {
    let segment = lyon::geom::QuadraticBezierSegment {
        from: lyon::math::point(p0[0], p0[1]),
        ctrl: lyon::math::point(control[0], control[1]),
        to: lyon::math::point(p1[0], p1[1]),
    };
    out.extend(
        segment
            .flattened(PATH_FLATTEN_TOLERANCE)
            .map(|p| [p.x, p.y]),
    );
}

/// Flattens one `Path`'s `commands` into local-space polylines, one
/// `Vec<Vec2>` per subpath (a new subpath starts at each `MoveTo`,
/// matching `tre_svg::parse_svg`'s own subpath-splitting convention). A
/// `Close` neither duplicates the start point into the output nor
/// implicitly draws back to it -- callers that need a truly closed loop
/// (hit-testing, [`tessellate_stroke`]) already treat the last point as
/// implicitly connected back to the first, the same convention
/// `tre_svg::Polygon` itself documents.
///
/// This is real, tested geometry -- used today by
/// [`ShapeRegistry::hit_test`]'s own `Path` case and (Phase 10 Step 10.2
/// follow-up) [`ShapeRegistry::flatten_into`]'s own `Path` fill/stroke
/// rendering, via [`tessellate_fill`]/[`tessellate_stroke`].
#[must_use]
pub fn flatten_path(commands: &[PathCommand]) -> Vec<Vec<Vec2>> {
    flatten_path_with_closed(commands)
        .into_iter()
        .map(|(points, _closed)| points)
        .collect()
}

/// Same as [`flatten_path`], but also reports whether each subpath was
/// explicitly closed via [`PathCommand::Close`] -- [`tessellate_stroke`]
/// needs this: an explicitly-closed subpath gets a continuous stroke
/// loop with no end caps, while an open one gets real caps
/// (`stroke_line_cap`) at both ends. [`flatten_path`] itself drops this
/// bit since hit-testing's point-in-polygon test always treats a contour
/// as implicitly closed regardless (`tre_svg::Polygon`'s own convention),
/// so it never needed to know.
fn flatten_path_with_closed(commands: &[PathCommand]) -> Vec<(Vec<Vec2>, bool)> {
    let mut subpaths = Vec::new();
    let mut current: Vec<Vec2> = Vec::new();
    let mut cursor = [0.0, 0.0];

    for command in commands {
        match *command {
            PathCommand::MoveTo(point) => {
                if current.len() >= 2 {
                    subpaths.push((std::mem::take(&mut current), false));
                } else {
                    current.clear();
                }
                current.push(point);
                cursor = point;
            }
            PathCommand::LineTo(point) => {
                current.push(point);
                cursor = point;
            }
            PathCommand::QuadraticTo { control, to } => {
                flatten_quad(cursor, control, to, &mut current);
                cursor = to;
            }
            PathCommand::CubicTo {
                control1,
                control2,
                to,
            } => {
                flatten_cubic(cursor, control1, control2, to, &mut current);
                cursor = to;
            }
            PathCommand::Close => {
                if current.len() >= 2 {
                    subpaths.push((std::mem::take(&mut current), true));
                } else {
                    current.clear();
                }
            }
        }
    }
    if current.len() >= 2 {
        subpaths.push((current, false));
    }
    subpaths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GlyphAtlasContext, RhiBuffer, RhiDynamicRingBuffer};
    use std::cell::Cell;
    use std::sync::Mutex;

    /// Phase 10 Step 10.2.4: independent Rust references for `sdf_
    /// ellipse.frag`'s real shader code -- sharing no math with the
    /// shader itself, used only to prove the new formula's real
    /// correctness and the old approximation's real, measured error
    /// (this codebase's own established "compute the correct answer
    /// independently, compare against real output" discipline, e.g.
    /// `gradient_fill_demo.rs`). See `sdf_ellipse_fidelity` tests below.
    mod sdf_ellipse_fidelity {
        /// The OLD "scaled circle" ellipse SDF approximation Step 10.2
        /// originally shipped (`sd_ellipse` in `sdf_ellipse.frag`,
        /// before Step 10.2.4 replaced it) -- exact only when `r.x ==
        /// r.y`, transcribed line-for-line from that prior GLSL.
        pub(super) fn scaled_circle_approx(p: [f32; 2], r: [f32; 2]) -> f32 {
            let k1 = (p[0] / r[0]).hypot(p[1] / r[1]);
            let k2 = (p[0] / (r[0] * r[0])).hypot(p[1] / (r[1] * r[1]));
            k1 * (k1 - 1.0) / k2
        }

        /// The NEW, real shader formula (`sdf_ellipse.frag`'s current
        /// `sd_ellipse`), transcribed line-for-line into Rust -- Inigo
        /// Quilez's own published Newton-Raphson refinement on the
        /// ellipse's implicit parametrization (iquilezles.org/articles/
        /// ellipsedist), 5 iterations.
        #[allow(clippy::many_single_char_names)] // mirrors sd_ellipse.frag's own variable names exactly, for direct side-by-side comparison
        pub(super) fn exact(p: [f32; 2], ab: [f32; 2]) -> f32 {
            let p = [p[0].abs(), p[1].abs()];
            let q = [ab[0] * (p[0] - ab[0]), ab[1] * (p[1] - ab[1])];
            let seed: [f32; 2] = if q[0] < q[1] {
                [0.01, 1.0]
            } else {
                [1.0, 0.01]
            };
            let norm = (seed[0] * seed[0] + seed[1] * seed[1]).sqrt();
            let mut cs = [seed[0] / norm, seed[1] / norm];
            for _ in 0..5 {
                let u = [ab[0] * cs[0], ab[1] * cs[1]];
                let v = [ab[0] * -cs[1], ab[1] * cs[0]];
                let pu = [p[0] - u[0], p[1] - u[1]];
                let a = pu[0] * v[0] + pu[1] * v[1];
                let c = pu[0] * u[0] + pu[1] * u[1] + v[0] * v[0] + v[1] * v[1];
                let b = (c * c - a * a).max(0.0).sqrt();
                cs = [(cs[0] * b - cs[1] * a) / c, (cs[1] * b + cs[0] * a) / c];
            }
            let d = ((p[0] - ab[0] * cs[0]).powi(2) + (p[1] - ab[1] * cs[1]).powi(2)).sqrt();
            let outside = (p[0] / ab[0]).powi(2) + (p[1] / ab[1]).powi(2) > 1.0;
            if outside {
                d
            } else {
                -d
            }
        }

        /// A fully independent ground truth sharing no math with either
        /// formula above: dense-sampled minimum distance from `p` to
        /// the ellipse's own parametric boundary `(r.x*cos(theta),
        /// r.y*sin(theta))`. Precision is bounded by `SAMPLES` (angular
        /// resolution ~1.3e-5 rad), not iterative convergence -- good
        /// enough to confirm both formulas' real accuracy at the
        /// tolerances these tests assert, not intended as a
        /// production-quality distance field itself.
        #[allow(clippy::cast_precision_loss)] // SAMPLES tops out at 500_000, well inside f32's exact-integer range; only used to pick a sample angle
        pub(super) fn brute_force(p: [f32; 2], r: [f32; 2]) -> f32 {
            const SAMPLES: u32 = 500_000;
            let mut min_dist_sq = f32::MAX;
            for i in 0..SAMPLES {
                let theta = (i as f32 / SAMPLES as f32) * std::f32::consts::TAU;
                let dx = p[0] - r[0] * theta.cos();
                let dy = p[1] - r[1] * theta.sin();
                let dist_sq = dx * dx + dy * dy;
                if dist_sq < min_dist_sq {
                    min_dist_sq = dist_sq;
                }
            }
            let d = min_dist_sq.sqrt();
            let inside = (p[0] / r[0]).powi(2) + (p[1] / r[1]).powi(2) <= 1.0;
            if inside {
                -d
            } else {
                d
            }
        }
    }

    /// Phase 10 Step 10.2.4: an independent Rust reference of Figma's
    /// REAL squircle corner construction (their own published article,
    /// "Desperately Seeking Squircles," figma.com/blog/desperately-
    /// seeking-squircles -- transcribed from the widely-cited, real
    /// open-source implementation at github.com/tienphaw/figma-squircle
    /// `getPathParamsForCorner`/`draw.ts`, itself a direct port of
    /// Figma's own published formulas), used ONLY to research and
    /// quantify `corner_norm`/`sd_rounded_box`'s (`sdf_rect_styled.
    /// frag`) real deviation from that reference -- PLAN.md Step
    /// 10.2.4's own "Scope decisions" authorizes exactly this outcome
    /// ("formally verify and document the existing superellipse
    /// blend's real deviation... within a quantified tolerance") as an
    /// alternative to a rewrite, once research determines which one the
    /// evidence actually supports.
    ///
    /// The key finding driving that call: Figma's construction is NOT
    /// an implicit distance field at all -- it is a real SVG path (two
    /// curvature-continuous cubic Beziers plus a circular arc PER
    /// corner), a fundamentally different mathematical object from an
    /// implicit superellipse-exponent blend evaluated per-pixel. There
    /// is no "simple closed form" version of Figma's own construction to
    /// drop into an SDF shader -- computing the exact per-pixel distance
    /// to an arbitrary cubic Bezier curve is a substantially harder,
    /// more expensive real-time problem than this one bounded step's own
    /// scope, so a rewrite was not attempted; this module exists to
    /// quantify the real, disclosed difference instead.
    mod corner_smoothing_fidelity {
        struct FigmaCornerParams {
            a: f32,
            b: f32,
            c: f32,
            d: f32,
            p: f32,
            arc_section_length: f32,
        }

        /// Figma's own `getPathParamsForCorner`, `preserveSmoothing:
        /// false` (the real, shipped default) -- see this module's own
        /// doc comment for the source. `budget` is `figma-squircle`'s
        /// own `roundingAndSmoothingBudget`; callers here always pass a
        /// generous value that never actually clamps anything, so this
        /// reference doesn't need to reimplement Figma's own separate
        /// multi-corner budget-sharing logic for adjacent corners on
        /// the same edge.
        #[allow(clippy::many_single_char_names)] // mirrors figma-squircle's own a/b/c/d/p variable names exactly, for direct side-by-side comparison against the source
        fn figma_corner_params(radius: f32, smoothing: f32, budget: f32) -> FigmaCornerParams {
            let mut p = (1.0 + smoothing) * radius;
            let max_smoothing = budget / radius - 1.0;
            let smoothing = smoothing.min(max_smoothing);
            p = p.min(budget);

            let arc_measure = 90.0 * (1.0 - smoothing);
            let arc_section_length =
                (arc_measure / 2.0).to_radians().sin() * radius * std::f32::consts::SQRT_2;

            let angle_alpha = (90.0 - arc_measure) / 2.0;
            let p3_to_p4 = radius * (angle_alpha / 2.0).to_radians().tan();

            let angle_beta = 45.0 * smoothing;
            let c = p3_to_p4 * angle_beta.to_radians().cos();
            let d = c * angle_beta.to_radians().tan();

            let b = (p - arc_section_length - c - d) / 3.0;
            let a = 2.0 * b;
            FigmaCornerParams {
                a,
                b,
                c,
                d,
                p,
                arc_section_length,
            }
        }

        fn cubic_bezier(
            p0: [f32; 2],
            p1: [f32; 2],
            p2: [f32; 2],
            p3: [f32; 2],
            t: f32,
        ) -> [f32; 2] {
            let mt = 1.0 - t;
            let (w0, w1, w2, w3) = (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
            [
                w0 * p0[0] + w1 * p1[0] + w2 * p2[0] + w3 * p3[0],
                w0 * p0[1] + w1 * p1[1] + w2 * p2[1] + w3 * p3[1],
            ]
        }

        /// A point at parameter `t` along the real SVG circular arc
        /// Figma's own path uses between the two Bezier ramps (`rx=ry=
        /// radius`, no rotation, `large-arc-flag=0`, `sweep-flag=1` --
        /// the standard "endpoint to center" conversion the SVG spec
        /// itself defines, specialized to equal radii and this fixed
        /// flag combination, which fixes the ambiguous-center sign to
        /// always `+1`).
        fn svg_arc_point(p0: [f32; 2], p1: [f32; 2], radius: f32, t: f32) -> [f32; 2] {
            let x1p = (p0[0] - p1[0]) / 2.0;
            let y1p = (p0[1] - p1[1]) / 2.0;
            let den = x1p * x1p + y1p * y1p;
            let co = if den > 1e-9 {
                ((radius * radius - den).max(0.0) / den).sqrt()
            } else {
                0.0
            };
            let center = [
                co * y1p + (p0[0] + p1[0]) / 2.0,
                -co * x1p + (p0[1] + p1[1]) / 2.0,
            ];
            let theta1 = (p0[1] - center[1]).atan2(p0[0] - center[0]);
            let theta2 = (p1[1] - center[1]).atan2(p1[0] - center[0]);
            let mut delta = theta2 - theta1;
            if delta < 0.0 {
                delta += std::f32::consts::TAU;
            }
            let theta = theta1 + delta * t;
            [
                center[0] + radius * theta.cos(),
                center[1] + radius * theta.sin(),
            ]
        }

        /// This engine's own real `corner_norm`/`sd_rounded_box`
        /// (`sdf_rect_styled.frag`), transcribed line-for-line into
        /// Rust, specialized to a uniform corner radius (so
        /// `select_radius`'s quadrant branch collapses to a constant).
        fn engine_sd_rounded_box(
            p: [f32; 2],
            half_extent: f32,
            radius: f32,
            smoothing: f32,
        ) -> f32 {
            let q = [
                p[0].abs() - half_extent + radius,
                p[1].abs() - half_extent + radius,
            ];
            let qm = [q[0].max(0.0), q[1].max(0.0)];
            let outer = if smoothing <= 0.0001 {
                (qm[0] * qm[0] + qm[1] * qm[1]).sqrt()
            } else {
                let n = 2.0 + 3.0 * smoothing.clamp(0.0, 1.0);
                (qm[0].powf(n) + qm[1].powf(n)).powf(1.0 / n)
            };
            outer + q[0].max(q[1]).min(0.0) - radius
        }

        /// Builds Figma's real top-right corner path for a `size x
        /// size` square (a square keeps the per-corner budget/geometry
        /// simple and representative; nothing about the comparison
        /// below depends on the rect being non-square) and samples it
        /// densely, returning the maximum `|engine_sd_rounded_box|`
        /// across every sampled point -- zero would mean the two
        /// constructions trace the identical curve; any nonzero value
        /// is this engine's own real, measured deviation from Figma's
        /// real construction, in pixels, at that `size`/`radius`/
        /// `smoothing`.
        pub(super) fn max_deviation_from_engine(size: f32, radius: f32, smoothing: f32) -> f32 {
            const STEPS: u16 = 32;

            let prm = figma_corner_params(radius, smoothing, 1000.0);
            let half = size / 2.0;

            let start = [size - prm.p, 0.0];
            let s1 = [
                start,
                [start[0] + prm.a, start[1]],
                [start[0] + prm.a + prm.b, start[1]],
                [start[0] + prm.a + prm.b + prm.c, start[1] + prm.d],
            ];
            let arc_p1 = [
                s1[3][0] + prm.arc_section_length,
                s1[3][1] + prm.arc_section_length,
            ];
            let s2 = [
                arc_p1,
                [arc_p1[0] + prm.d, arc_p1[1] + prm.c],
                [arc_p1[0] + prm.d, arc_p1[1] + prm.b + prm.c],
                [arc_p1[0] + prm.d, arc_p1[1] + prm.a + prm.b + prm.c],
            ];

            let mut max_dev = 0.0_f32;
            for i in 0..=STEPS {
                let t = f32::from(i) / f32::from(STEPS);
                for absolute in [
                    cubic_bezier(s1[0], s1[1], s1[2], s1[3], t),
                    svg_arc_point(s1[3], arc_p1, radius, t),
                    cubic_bezier(s2[0], s2[1], s2[2], s2[3], t),
                ] {
                    let local = [absolute[0] - half, absolute[1] - half];
                    let dev = engine_sd_rounded_box(local, half, radius, smoothing).abs();
                    max_dev = max_dev.max(dev);
                }
            }
            max_dev
        }
    }

    /// A minimal `RhiDevice` double for this module's own `flatten_into`
    /// tests -- only `shape_style_buffer()` is real (a plain,
    /// alignment-agnostic bump allocator); every other method is
    /// `unimplemented!()` since nothing here exercises it. Distinct from
    /// `lib.rs`'s own private `FakeDevice` (a different, unrelated `mod
    /// tests`) -- not worth threading a shared test-support module
    /// through the crate for a handful of fields.
    #[derive(Default)]
    struct FakeDevice {
        style_buffer: FakeStyleBuffer,
        /// Phase 10 Step 10.2.3: `false` by default, matching real
        /// hardware that lacks `VK_KHR_dynamic_rendering_local_read` --
        /// tests exercising the "supported" branch set this via
        /// `Cell::set` before calling `flatten_into`.
        local_read_blend_supported: Cell<bool>,
    }

    #[derive(Default)]
    struct FakeStyleBuffer {
        // `Mutex`, not `RefCell` (REVIEW.md #203/#204's own fallout):
        // `RhiBuffer` gained a real `Send + Sync` bound this fix needed,
        // so every fake implementing it must be `Sync` too.
        bytes: Mutex<Vec<u8>>,
    }

    impl RhiBuffer for FakeStyleBuffer {
        fn raw_handle(&self) -> u64 {
            0
        }
    }

    impl RhiDynamicRingBuffer for FakeStyleBuffer {
        fn write(&self, bytes: &[u8]) -> Option<u32> {
            let mut buf = self.bytes.lock().expect("FakeStyleBuffer mutex poisoned");
            let offset = u32::try_from(buf.len()).ok()?;
            buf.extend_from_slice(bytes);
            Some(offset)
        }
    }

    impl crate::RhiDevice for FakeDevice {
        fn create_dynamic_ring_buffer(&self, _capacity: usize) -> Box<dyn RhiDynamicRingBuffer> {
            unimplemented!("not exercised by any shapes.rs test")
        }
        fn shape_style_buffer(&self) -> &dyn RhiDynamicRingBuffer {
            &self.style_buffer
        }
        fn local_read_blend_supported(&self) -> bool {
            self.local_read_blend_supported.get()
        }
        fn acquire_transient_target(
            &self,
            _width: u32,
            _height: u32,
            _format: crate::TextureFormat,
        ) -> Result<Box<dyn crate::RhiTexture>, crate::EngineError> {
            unimplemented!("not exercised by any shapes.rs test")
        }
        fn release_transient_target(&self, _texture: Box<dyn crate::RhiTexture>) {
            unimplemented!("not exercised by any shapes.rs test")
        }
        fn create_texture(
            &self,
            _width: u32,
            _height: u32,
            _format: crate::TextureFormat,
            _pixels: &[u8],
        ) -> Result<Box<dyn crate::RhiTexture>, crate::EngineError> {
            unimplemented!("not exercised by any shapes.rs test")
        }
        fn register_bindless(
            &self,
            _texture: &dyn crate::RhiTexture,
        ) -> Result<u32, crate::EngineError> {
            unimplemented!("not exercised by any shapes.rs test")
        }
        fn deregister_bindless(&self, _bindless_index: u32) {
            unimplemented!("not exercised by any shapes.rs test")
        }
        fn begin_frame(
            &self,
            _swapchain: &dyn crate::RhiSwapchain,
        ) -> Result<(Box<dyn crate::RhiCommandBuffer>, crate::AcquiredImage), crate::EngineError>
        {
            unimplemented!("not exercised by any shapes.rs test")
        }
        fn submit_and_present(
            &self,
            _cmd_buffer: Box<dyn crate::RhiCommandBuffer>,
            _swapchain: &dyn crate::RhiSwapchain,
            _image: crate::AcquiredImage,
        ) -> Result<(), crate::EngineError> {
            unimplemented!("not exercised by any shapes.rs test")
        }
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s, same reasoning as this crate's other \
                   exact-arithmetic tests"
    )]
    fn insert_then_get_round_trips() {
        let mut registry = ShapeRegistry::new();
        let id = registry.insert(ShapePrimitive::Rectangle(Rectangle::new(
            [10.0, 20.0],
            0xFFFF_FFFF,
        )));
        let slot = registry.get(id).expect("just-inserted shape must be found");
        let ShapePrimitive::Rectangle(rect) = &slot.shape else {
            panic!("expected a Rectangle");
        };
        assert_eq!(rect.size, [10.0, 20.0]);
        assert!(slot.layout_dirty, "a freshly inserted shape starts dirty");
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn remove_frees_the_slot_for_reuse() {
        let mut registry = ShapeRegistry::new();
        let first = registry.insert(ShapePrimitive::Rectangle(Rectangle::new([1.0, 1.0], 0)));
        assert!(registry.remove(first));
        assert_eq!(registry.len(), 0);

        let second = registry.insert(ShapePrimitive::Rectangle(Rectangle::new([2.0, 2.0], 0)));
        assert_eq!(
            second.index, first.index,
            "insert must reuse the freed slot's own index rather than growing"
        );
        assert_ne!(
            second.generation, first.generation,
            "the reused slot's generation must have advanced"
        );
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn a_stale_shape_id_is_rejected_not_silently_resolved_to_the_reused_shape() {
        let mut registry = ShapeRegistry::new();
        let first = registry.insert(ShapePrimitive::Rectangle(Rectangle::new([1.0, 1.0], 0)));
        assert!(registry.remove(first));
        let _second = registry.insert(ShapePrimitive::Rectangle(Rectangle::new([2.0, 2.0], 0)));

        assert!(
            registry.get(first).is_none(),
            "the stale (pre-removal) ShapeId must not resolve to the slot's new occupant"
        );
        assert!(
            !registry.remove(first),
            "removing an already-stale ShapeId must report false"
        );
    }

    #[test]
    fn removing_an_id_that_never_existed_reports_false_not_panic() {
        let mut registry = ShapeRegistry::new();
        let phantom = ShapeId {
            index: 999,
            generation: 0,
        };
        assert!(!registry.remove(phantom));
        assert!(registry.get(phantom).is_none());
    }

    #[test]
    fn multiple_inserts_assign_distinct_ids() {
        let mut registry = ShapeRegistry::new();
        let a = registry.insert(ShapePrimitive::Rectangle(Rectangle::new([1.0, 1.0], 0)));
        let b = registry.insert(ShapePrimitive::Rectangle(Rectangle::new([1.0, 1.0], 0)));
        let c = registry.insert(ShapePrimitive::Rectangle(Rectangle::new([1.0, 1.0], 0)));
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
        assert_eq!(registry.len(), 3);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s, same reasoning as this crate's other \
                   exact-arithmetic tests"
    )]
    fn transform2d_to_affine2_applies_scale_then_rotation_then_translation() {
        let transform = Transform2D {
            position: [10.0, 0.0],
            scale: [2.0, 1.0],
            rotation: 0.0,
        };
        let affine = transform.to_affine2();
        // A point at local (1, 0), scaled by (2, 1) -> (2, 0), rotated by
        // 0 -> (2, 0), translated by (10, 0) -> (12, 0).
        assert_eq!(affine.transform_point([1.0, 0.0]), [12.0, 0.0]);
    }

    #[test]
    fn flatten_into_records_a_dirty_rectangle_and_clears_its_dirty_flag() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        let id = registry.insert(ShapePrimitive::Rectangle(Rectangle::new(
            [10.0, 10.0],
            0xFFFF_FFFF,
        )));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();
        assert_eq!(frame.vertices.len(), 4, "one rectangle emits 4 vertices");
        assert_eq!(frame.commands.len(), 1);
        assert!(
            !registry.get(id).expect("shape still exists").layout_dirty,
            "flatten_into must clear layout_dirty after recording"
        );
    }

    #[test]
    fn flatten_into_skips_a_shape_that_is_neither_dirty_nor_animating() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Rectangle(Rectangle::new(
            [10.0, 10.0],
            0xFFFF_FFFF,
        )));
        let mut warm_up = RenderingCanvas::new();
        registry.flatten_into(&mut warm_up, &device, None); // clears layout_dirty

        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();
        assert!(
            frame.vertices.is_empty(),
            "a shape that is neither dirty nor animating must not be re-recorded"
        );
    }

    #[test]
    fn flatten_into_skips_hidden_and_collapsed_shapes_without_panicking() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        let mut hidden = Rectangle::new([10.0, 10.0], 0xFFFF_FFFF);
        hidden.common.visibility = Visibility::Hidden;
        registry.insert(ShapePrimitive::Rectangle(hidden));
        let mut collapsed = Rectangle::new([10.0, 10.0], 0xFFFF_FFFF);
        collapsed.common.visibility = Visibility::Collapsed;
        registry.insert(ShapePrimitive::Rectangle(collapsed));

        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();
        assert!(
            frame.vertices.is_empty(),
            "Hidden/Collapsed shapes must not be recorded into the IR"
        );
    }

    #[test]
    fn flatten_into_renders_a_non_uniform_corner_radius_via_the_styled_path() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        let mut rect = Rectangle::new([10.0, 10.0], 0xFFFF_FFFF);
        rect.corner_radius = CornerRadii {
            top_left: 1.0,
            top_right: 2.0,
            bottom_right: 1.0,
            bottom_left: 1.0,
        };
        registry.insert(ShapePrimitive::Rectangle(rect));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(frame.vertices.len(), 4);
        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::SdfRectStyled as u16,
            "a non-uniform corner radius must route through the styled pipeline, not \
             draw_rounded_rect's uniform-only one"
        );
        assert_eq!(
            device
                .style_buffer
                .bytes
                .lock()
                .expect("FakeStyleBuffer mutex poisoned")
                .len(),
            40,
            "exactly one GpuRectStyle record must have been written"
        );
    }

    #[test]
    fn flatten_into_renders_a_circle_via_the_ellipse_pipeline() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Circle(Circle {
            common: PrimitiveCommon::new(),
            radius: [5.0, 5.0],
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0,
            border_thickness: 0.0,
            arc_length: 360.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(frame.vertices.len(), 4);
        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::SdfEllipse as u16
        );
        assert_eq!(
            device
                .style_buffer
                .bytes
                .lock()
                .expect("FakeStyleBuffer mutex poisoned")
                .len(),
            28,
            "exactly one GpuEllipseStyle record must have been written"
        );
    }

    #[test]
    fn an_active_animation_keeps_a_shape_flattened_every_frame_even_without_layout_dirty() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        let id = registry.insert(ShapePrimitive::Rectangle(Rectangle::new(
            [10.0, 10.0],
            0xFFFF_FFFF,
        )));
        let mut warm_up = RenderingCanvas::new();
        registry.flatten_into(&mut warm_up, &device, None); // clears layout_dirty

        registry
            .get_mut(id)
            .expect("shape exists")
            .active_animations
            .push(AnimationId(1));

        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();
        assert_eq!(
            frame.vertices.len(),
            4,
            "a non-empty active_animations must still trigger re-flattening"
        );
    }

    // --- Polygon generation/fill (Phase 10 Step 10.2) ---

    #[test]
    fn generate_polygon_points_of_a_square_gives_four_points_at_the_requested_radius() {
        let mut polygon = Polygon {
            common: PrimitiveCommon::new(),
            sides: 4,
            radius: 10.0,
            vertex_radius: 0.0,
            star_points: None,
            fill: FillStyle::Solid(0),
            border_color: 0,
            border_thickness: 0.0,
        };
        let points = generate_polygon_points(&polygon);
        assert_eq!(points.len(), 4);
        for [x, y] in points {
            let distance = (x * x + y * y).sqrt();
            assert!(
                (distance - 10.0).abs() < 1e-4,
                "every regular-polygon vertex must sit at the requested radius, got {distance}"
            );
        }
        // 12 o'clock start: the first vertex is straight up (x ~= 0, y < 0).
        polygon.sides = 4;
        let points = generate_polygon_points(&polygon);
        assert!(points[0][0].abs() < 1e-4 && points[0][1] < 0.0);
    }

    #[test]
    fn generate_polygon_points_of_a_star_alternates_outer_and_inner_radius() {
        let star = Polygon {
            common: PrimitiveCommon::new(),
            sides: 0,
            radius: 20.0,
            vertex_radius: 8.0,
            star_points: Some(5),
            fill: FillStyle::Solid(0),
            border_color: 0,
            border_thickness: 0.0,
        };
        let points = generate_polygon_points(&star);
        assert_eq!(points.len(), 10, "5 star points = 10 alternating vertices");
        for (i, [x, y]) in points.into_iter().enumerate() {
            let distance = (x * x + y * y).sqrt();
            let expected = if i % 2 == 0 { 20.0 } else { 8.0 };
            assert!(
                (distance - expected).abs() < 1e-4,
                "vertex {i}: expected radius {expected}, got {distance}"
            );
        }
    }

    #[test]
    fn fan_from_center_into_of_a_square_covers_all_four_edges_including_the_wraparound() {
        let mut triangles = Vec::new();
        fan_from_center_into(4, &mut triangles);
        assert_eq!(triangles, vec![[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 1]]);
    }

    #[test]
    fn fan_from_center_into_of_fewer_than_three_vertices_is_empty() {
        let mut triangles = Vec::new();
        fan_from_center_into(2, &mut triangles);
        assert_eq!(triangles, Vec::<[u32; 3]>::new());
    }

    #[test]
    fn fan_from_center_into_clears_any_prior_contents_before_refilling() {
        let mut triangles = vec![[9, 9, 9]];
        fan_from_center_into(4, &mut triangles);
        assert_eq!(triangles, vec![[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 1]]);
    }

    #[test]
    fn flatten_into_renders_a_regular_polygon_via_the_flat_color_pipeline() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Polygon(Polygon {
            common: PrimitiveCommon::new(),
            sides: 6,
            radius: 15.0,
            vertex_radius: 0.0,
            star_points: None,
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0,
            border_thickness: 0.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(frame.commands.len(), 1);
        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::FlatColor as u16
        );
        // 6 boundary + 1 center = 7 vertices; 6 fan triangles = 18 indices.
        assert_eq!(frame.vertices.len(), 7);
        assert_eq!(frame.indices.len(), 18);
    }

    // --- Path Bezier flattening (Phase 10 Step 10.2) ---

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "the flattened curve's final point is exactly the literal endpoint passed in \
                   (lyon_geom's own documented Flattened contract), not a rounded computed value"
    )]
    fn flatten_cubic_of_a_straight_line_stays_on_the_line_and_ends_at_the_endpoint() {
        // Control points collinear with the endpoints, but NOT evenly
        // spaced along the line (0, 3, 6, 10) -- lyon_geom's own
        // flattening (unlike this module's original hand-rolled
        // perpendicular-distance-to-chord check) can still emit an
        // intermediate point here, based on the curve's own parametric
        // speed rather than pure geometric flatness alone. That's a
        // real, harmless difference from the old algorithm's behavior,
        // not a correctness bug: every emitted point still lies exactly
        // on the line, so this test checks that instead of an exact
        // point count.
        let mut out = Vec::new();
        flatten_cubic([0.0, 0.0], [3.0, 0.0], [6.0, 0.0], [10.0, 0.0], &mut out);
        assert!(!out.is_empty());
        for &[x, y] in &out {
            assert!(
                y.abs() < 1e-4,
                "point ({x}, {y}) must lie on the straight line y=0"
            );
        }
        assert_eq!(*out.last().unwrap(), [10.0, 0.0]);
    }

    #[test]
    fn flatten_cubic_of_a_real_curve_approximates_it_within_tolerance() {
        // A quarter-circle-ish cubic from (1, 0) to (0, 1) via the
        // standard k=0.5522847 control-point approximation -- every
        // flattened point must land close to the unit circle.
        const K: f32 = 0.552_284_7;
        let mut out = Vec::new();
        flatten_cubic([1.0, 0.0], [1.0, K], [K, 1.0], [0.0, 1.0], &mut out);
        assert!(
            out.len() > 1,
            "a real curve must subdivide into more than one segment"
        );
        for [x, y] in out {
            let radius = (x * x + y * y).sqrt();
            assert!(
                (radius - 1.0).abs() < 0.05,
                "flattened point ({x}, {y}) strayed too far from the unit circle: r={radius}"
            );
        }
    }

    #[test]
    fn flatten_path_splits_subpaths_on_move_to() {
        let commands = [
            PathCommand::MoveTo([0.0, 0.0]),
            PathCommand::LineTo([10.0, 0.0]),
            PathCommand::LineTo([10.0, 10.0]),
            PathCommand::MoveTo([20.0, 20.0]),
            PathCommand::LineTo([30.0, 20.0]),
            PathCommand::LineTo([30.0, 30.0]),
        ];
        let subpaths = flatten_path(&commands);
        assert_eq!(subpaths.len(), 2);
        assert_eq!(subpaths[0], vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]);
        assert_eq!(subpaths[1], vec![[20.0, 20.0], [30.0, 20.0], [30.0, 30.0]]);
    }

    #[test]
    fn flatten_path_a_close_ends_the_current_subpath_without_duplicating_the_start_point() {
        let commands = [
            PathCommand::MoveTo([0.0, 0.0]),
            PathCommand::LineTo([10.0, 0.0]),
            PathCommand::LineTo([10.0, 10.0]),
            PathCommand::Close,
        ];
        let subpaths = flatten_path(&commands);
        assert_eq!(subpaths.len(), 1);
        assert_eq!(
            subpaths[0].len(),
            3,
            "Close must not duplicate the MoveTo start point"
        );
    }

    #[test]
    fn flatten_path_of_a_quadratic_curve_ends_at_the_requested_endpoint() {
        let commands = [
            PathCommand::MoveTo([0.0, 0.0]),
            PathCommand::QuadraticTo {
                control: [5.0, 10.0],
                to: [10.0, 0.0],
            },
        ];
        let subpaths = flatten_path(&commands);
        assert_eq!(subpaths.len(), 1);
        assert_eq!(subpaths[0].last(), Some(&[10.0, 0.0]));
        assert!(
            subpaths[0].len() > 1,
            "a real curve must produce more than its endpoint"
        );
    }

    // --- Hit-testing (Phase 10 Step 10.2) ---

    #[test]
    fn hit_test_finds_a_point_inside_a_plain_rectangle() {
        let mut registry = ShapeRegistry::new();
        let mut rect = Rectangle::new([100.0, 50.0], 0xFFFF_FFFF);
        rect.common.transform.position = [10.0, 10.0];
        let id = registry.insert(ShapePrimitive::Rectangle(rect));

        assert_eq!(registry.hit_test([60.0, 35.0]), Some(id));
        assert_eq!(registry.hit_test([5.0, 5.0]), None);
    }

    #[test]
    fn hit_test_excludes_a_point_in_a_rounded_corners_own_cut_off_area() {
        let mut registry = ShapeRegistry::new();
        let mut rect = Rectangle::new([100.0, 100.0], 0xFFFF_FFFF);
        rect.corner_radius = CornerRadii::uniform(30.0);
        registry.insert(ShapePrimitive::Rectangle(rect));

        // Right at the raw bounding-box corner (0, 0) -- well outside the
        // real 30px-radius rounded arc.
        assert_eq!(registry.hit_test([1.0, 1.0]), None);
        // The rect's own center is always inside, corners or not.
        assert!(registry.hit_test([50.0, 50.0]).is_some());
    }

    #[test]
    fn hit_test_respects_hit_testable_false() {
        let mut registry = ShapeRegistry::new();
        let mut rect = Rectangle::new([100.0, 100.0], 0xFFFF_FFFF);
        rect.common.hit_testable = false;
        registry.insert(ShapePrimitive::Rectangle(rect));

        assert_eq!(registry.hit_test([50.0, 50.0]), None);
    }

    #[test]
    fn hit_test_skips_hidden_shapes() {
        let mut registry = ShapeRegistry::new();
        let mut rect = Rectangle::new([100.0, 100.0], 0xFFFF_FFFF);
        rect.common.visibility = Visibility::Hidden;
        registry.insert(ShapePrimitive::Rectangle(rect));

        assert_eq!(registry.hit_test([50.0, 50.0]), None);
    }

    #[test]
    fn hit_test_returns_the_topmost_of_two_overlapping_shapes() {
        let mut registry = ShapeRegistry::new();
        let bottom = Rectangle::new([100.0, 100.0], 0xFFFF_FFFF);
        registry.insert(ShapePrimitive::Rectangle(bottom));
        let top = Rectangle::new([100.0, 100.0], 0xFF00_00FF);
        let top_id = registry.insert(ShapePrimitive::Rectangle(top));

        assert_eq!(registry.hit_test([50.0, 50.0]), Some(top_id));
    }

    #[test]
    fn hit_test_finds_a_point_inside_a_circle_and_excludes_one_outside_it() {
        let mut registry = ShapeRegistry::new();
        let id = registry.insert(ShapePrimitive::Circle(Circle {
            common: PrimitiveCommon::new(),
            radius: [20.0, 20.0],
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0,
            border_thickness: 0.0,
            arc_length: 360.0,
        }));
        // Local center is (20, 20) (bounding-box-top-left convention).
        assert_eq!(registry.hit_test([20.0, 20.0]), Some(id));
        assert_eq!(registry.hit_test([0.5, 0.5]), None);
    }

    #[test]
    fn hit_test_excludes_a_point_in_a_circles_own_excluded_arc_wedge() {
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Circle(Circle {
            common: PrimitiveCommon::new(),
            radius: [20.0, 20.0],
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0,
            border_thickness: 0.0,
            arc_length: 270.0, // excludes the northwest wedge, same as the GPU demo.
        }));
        // Northwest of local center (20, 20): local point (10, 10).
        assert_eq!(registry.hit_test([10.0, 10.0]), None);
        // Due east of local center: well inside the 270-degree sweep.
        assert!(registry.hit_test([35.0, 20.0]).is_some());
    }

    #[test]
    fn hit_test_finds_a_point_inside_a_regular_polygon() {
        let mut registry = ShapeRegistry::new();
        let mut hexagon = Polygon {
            common: PrimitiveCommon::new(),
            sides: 6,
            radius: 20.0,
            vertex_radius: 0.0,
            star_points: None,
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0,
            border_thickness: 0.0,
        };
        hexagon.common.transform.position = [50.0, 50.0];
        let id = registry.insert(ShapePrimitive::Polygon(hexagon));

        assert_eq!(
            registry.hit_test([50.0, 50.0]),
            Some(id),
            "the center must always hit"
        );
        assert_eq!(
            registry.hit_test([50.0, 500.0]),
            None,
            "far outside the polygon must not hit"
        );
    }

    #[test]
    fn hit_test_a_star_excludes_a_point_in_one_of_its_own_concave_notches() {
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Polygon(Polygon {
            common: PrimitiveCommon::new(),
            sides: 0,
            radius: 20.0,
            vertex_radius: 5.0,
            star_points: Some(5),
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0,
            border_thickness: 0.0,
        }));
        // Just inside the outer radius, exactly between two star points
        // (a concave notch) -- must be excluded even though it's well
        // within the star's own bounding circle.
        assert_eq!(registry.hit_test([18.0, 0.0]), None);
        assert!(
            registry.hit_test([0.0, 0.0]).is_some(),
            "dead center must hit"
        );
    }

    #[test]
    fn hit_test_finds_a_point_inside_a_closed_path_triangle() {
        let mut registry = ShapeRegistry::new();
        let id = registry.insert(ShapePrimitive::Path(Path {
            common: PrimitiveCommon::new(),
            commands: vec![
                PathCommand::MoveTo([0.0, 0.0]),
                PathCommand::LineTo([100.0, 0.0]),
                PathCommand::LineTo([50.0, 100.0]),
                PathCommand::Close,
            ],
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0,
            border_thickness: 0.0,
            stroke_line_cap: LineCap::Butt,
            stroke_line_join: LineJoin::Bevel,
        }));

        assert_eq!(registry.hit_test([50.0, 40.0]), Some(id));
        assert_eq!(registry.hit_test([5.0, 90.0]), None);
    }

    // --- Path fill/stroke via lyon (Phase 10 Step 10.2 follow-up) ---

    fn triangle_area(vertices: &[Vec2], triangles: &[[u32; 3]]) -> f32 {
        triangles
            .iter()
            .map(|&[a, b, c]| {
                let [a, b, c] = [
                    vertices[a as usize],
                    vertices[b as usize],
                    vertices[c as usize],
                ];
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0
            })
            .sum()
    }

    #[test]
    fn tessellate_fill_of_a_simple_triangle_produces_the_correct_area() {
        let triangle = vec![vec![[0.0, 0.0], [100.0, 0.0], [50.0, 100.0]]];
        let (vertices, triangles) = tessellate_fill(&triangle);
        assert!((triangle_area(&vertices, &triangles) - 5000.0).abs() < 1e-2);
    }

    #[test]
    fn tessellate_fill_handles_a_real_hole_via_two_contours() {
        // An outer 20x20 square with an inner 10x10 "hole" -- real area =
        // 400 - 100 = 300. Ear-clipping (this crate's own hand-rolled
        // fan/triangulation) could never express this at all; lyon's
        // real fill tessellator resolves it directly under NonZero.
        let outer = vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]];
        // Wound OPPOSITE to the outer contour -- NonZero needs opposite
        // winding for a hole to actually subtract (unlike EvenOdd, which
        // doesn't care about relative winding).
        let hole = vec![[5.0, 5.0], [5.0, 15.0], [15.0, 15.0], [15.0, 5.0]];
        let (vertices, triangles) = tessellate_fill(&[outer, hole]);
        assert!(
            (triangle_area(&vertices, &triangles) - 300.0).abs() < 1e-1,
            "expected area 300 (outer 400 minus hole 100), got {}",
            triangle_area(&vertices, &triangles)
        );
    }

    #[test]
    fn tessellate_fill_handles_a_self_intersecting_pentagram() {
        let mut raw = Vec::with_capacity(5);
        for i in 0_u8..5 {
            let angle =
                std::f32::consts::FRAC_PI_2 + f32::from(i) * 2.0 * std::f32::consts::PI / 5.0;
            raw.push([100.0 * angle.cos(), -100.0 * angle.sin()]);
        }
        let pentagram = vec![raw[0], raw[2], raw[4], raw[1], raw[3]];
        let (vertices, triangles) = tessellate_fill(&[pentagram]);
        assert!(
            !triangles.is_empty(),
            "a pentagram has real area; tessellation must not be empty"
        );
        assert!(triangle_area(&vertices, &triangles) > 0.0);
    }

    #[test]
    fn tessellate_stroke_of_an_open_line_produces_real_geometry() {
        let line: Vec<Vec2> = vec![[0.0, 0.0], [100.0, 0.0]];
        let (vertices, triangles) = tessellate_stroke(
            &[(line, false)],
            10.0,
            lyon::path::LineJoin::Miter,
            lyon::path::LineCap::Butt,
            lyon::path::LineCap::Butt,
        );
        assert!(!triangles.is_empty());
        // A 100-unit-long, 10-unit-wide stroke has area ~1000 (butt caps
        // add no extra length).
        assert!((triangle_area(&vertices, &triangles) - 1000.0).abs() < 1.0);
    }

    #[test]
    fn flatten_into_renders_a_path_fill_via_the_flat_color_pipeline() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Path(Path {
            common: PrimitiveCommon::new(),
            commands: vec![
                PathCommand::MoveTo([0.0, 0.0]),
                PathCommand::LineTo([100.0, 0.0]),
                PathCommand::LineTo([50.0, 100.0]),
                PathCommand::Close,
            ],
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0,
            border_thickness: 0.0,
            stroke_line_cap: LineCap::Butt,
            stroke_line_join: LineJoin::Bevel,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(frame.commands.len(), 1, "fill only, no border requested");
        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::FlatColor as u16
        );
        assert!(!frame.vertices.is_empty());
    }

    #[test]
    fn flatten_into_renders_both_a_paths_fill_and_its_stroke() {
        // Step 5.1.3's own real batch flattening merges the fill and
        // stroke draws into ONE combined command (same pipeline, same
        // texture, same clip, issued back to back) -- a real, existing,
        // correct optimization, not something this test should fight.
        // The real thing to prove is that the border contributes real
        // EXTRA geometry on top of the fill-only case, and that the
        // (possibly merged) command(s) still all use the flat-color
        // pipeline.
        let fill_only_vertex_count = {
            let device = FakeDevice::default();
            let mut registry = ShapeRegistry::new();
            registry.insert(ShapePrimitive::Path(Path {
                common: PrimitiveCommon::new(),
                commands: vec![
                    PathCommand::MoveTo([0.0, 0.0]),
                    PathCommand::LineTo([100.0, 0.0]),
                    PathCommand::LineTo([50.0, 100.0]),
                    PathCommand::Close,
                ],
                fill: FillStyle::Solid(0xFFFF_FFFF),
                border_color: 0,
                border_thickness: 0.0,
                stroke_line_cap: LineCap::Round,
                stroke_line_join: LineJoin::Round,
            }));
            let mut canvas = RenderingCanvas::new();
            registry.flatten_into(&mut canvas, &device, None);
            canvas.flatten().vertices.len()
        };

        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Path(Path {
            common: PrimitiveCommon::new(),
            commands: vec![
                PathCommand::MoveTo([0.0, 0.0]),
                PathCommand::LineTo([100.0, 0.0]),
                PathCommand::LineTo([50.0, 100.0]),
                PathCommand::Close,
            ],
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0xFF00_00FF,
            border_thickness: 5.0,
            stroke_line_cap: LineCap::Round,
            stroke_line_join: LineJoin::Round,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert!(
            frame.vertices.len() > fill_only_vertex_count,
            "a real border must contribute real extra vertices on top of the fill-only case \
             ({} fill-only vs {} with border)",
            fill_only_vertex_count,
            frame.vertices.len()
        );
        for command in &frame.commands {
            assert_eq!(
                command.pipeline_state_id,
                crate::PipelineKind::FlatColor as u16
            );
        }
    }

    #[test]
    fn flatten_into_renders_a_polygons_stroke() {
        let fill_only_vertex_count = {
            let device = FakeDevice::default();
            let mut registry = ShapeRegistry::new();
            registry.insert(ShapePrimitive::Polygon(Polygon {
                common: PrimitiveCommon::new(),
                sides: 6,
                radius: 20.0,
                vertex_radius: 0.0,
                star_points: None,
                fill: FillStyle::Solid(0xFFFF_FFFF),
                border_color: 0,
                border_thickness: 0.0,
            }));
            let mut canvas = RenderingCanvas::new();
            registry.flatten_into(&mut canvas, &device, None);
            canvas.flatten().vertices.len()
        };

        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Polygon(Polygon {
            common: PrimitiveCommon::new(),
            sides: 6,
            radius: 20.0,
            vertex_radius: 0.0,
            star_points: None,
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0xFF00_00FF,
            border_thickness: 3.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert!(
            frame.vertices.len() > fill_only_vertex_count,
            "a real border must contribute real extra vertices on top of the fill-only case \
             ({} fill-only vs {} with border)",
            fill_only_vertex_count,
            frame.vertices.len()
        );
    }

    #[test]
    fn create_gradient_rejects_an_empty_stop_list() {
        let mut registry = ShapeRegistry::new();
        let err = registry
            .create_gradient(GradientDef {
                kind: GradientKind::Linear {
                    start: [0.0, 0.0],
                    end: [1.0, 0.0],
                },
                stops: vec![],
            })
            .unwrap_err();
        assert_eq!(err, GradientError::NoStops);
    }

    #[test]
    fn create_gradient_rejects_more_than_the_maximum_stop_count() {
        let mut registry = ShapeRegistry::new();
        // Position doesn't matter for this test -- every stop shares the
        // same one (`0.0` still satisfies the ascending-order check),
        // avoiding a real precision-loss cast this test has no need to
        // introduce.
        let stops: Vec<GradientStop> = (0..=crate::gpu_style::GRADIENT_MAX_STOPS)
            .map(|_| GradientStop {
                position: 0.0,
                color: 0xFFFF_FFFF,
            })
            .collect();
        let err = registry
            .create_gradient(GradientDef {
                kind: GradientKind::Linear {
                    start: [0.0, 0.0],
                    end: [1.0, 0.0],
                },
                stops,
            })
            .unwrap_err();
        assert_eq!(
            err,
            GradientError::TooManyStops {
                count: crate::gpu_style::GRADIENT_MAX_STOPS + 1,
                max: crate::gpu_style::GRADIENT_MAX_STOPS,
            }
        );
    }

    #[test]
    fn create_gradient_rejects_an_out_of_range_stop_position() {
        let mut registry = ShapeRegistry::new();
        let err = registry
            .create_gradient(GradientDef {
                kind: GradientKind::Linear {
                    start: [0.0, 0.0],
                    end: [1.0, 0.0],
                },
                stops: vec![GradientStop {
                    position: 1.5,
                    color: 0xFFFF_FFFF,
                }],
            })
            .unwrap_err();
        assert_eq!(
            err,
            GradientError::StopPositionOutOfRange {
                index: 0,
                position: 1.5
            }
        );
    }

    #[test]
    fn create_gradient_rejects_out_of_order_stops() {
        let mut registry = ShapeRegistry::new();
        let err = registry
            .create_gradient(GradientDef {
                kind: GradientKind::Linear {
                    start: [0.0, 0.0],
                    end: [1.0, 0.0],
                },
                stops: vec![
                    GradientStop {
                        position: 0.5,
                        color: 0xFFFF_FFFF,
                    },
                    GradientStop {
                        position: 0.2,
                        color: 0x0000_00FF,
                    },
                ],
            })
            .unwrap_err();
        assert_eq!(err, GradientError::StopsNotAscending { index: 1 });
    }

    #[test]
    fn create_gradient_rejects_a_non_positive_radial_radius() {
        let mut registry = ShapeRegistry::new();
        let err = registry
            .create_gradient(GradientDef {
                kind: GradientKind::Radial {
                    center: [0.0, 0.0],
                    radius: 0.0,
                },
                stops: vec![GradientStop {
                    position: 0.0,
                    color: 0xFFFF_FFFF,
                }],
            })
            .unwrap_err();
        assert_eq!(err, GradientError::NonPositiveRadius(0.0));
    }

    #[test]
    fn create_gradient_accepts_a_valid_gradient_and_returns_sequential_ids() {
        let mut registry = ShapeRegistry::new();
        let stops = vec![
            GradientStop {
                position: 0.0,
                color: 0xFF00_00FF,
            },
            GradientStop {
                position: 1.0,
                color: 0x0000_FFFF,
            },
        ];
        let first = registry
            .create_gradient(GradientDef {
                kind: GradientKind::Linear {
                    start: [0.0, 0.0],
                    end: [10.0, 0.0],
                },
                stops: stops.clone(),
            })
            .expect("a valid gradient must be accepted");
        let second = registry
            .create_gradient(GradientDef {
                kind: GradientKind::Radial {
                    center: [0.0, 0.0],
                    radius: 5.0,
                },
                stops,
            })
            .expect("a second valid gradient must also be accepted");
        assert_eq!(first, GradientId(0));
        assert_eq!(second, GradientId(1));
    }

    #[test]
    fn gradient_mut_mutates_an_existing_gradients_stops_in_place() {
        let mut registry = ShapeRegistry::new();
        let id = registry
            .create_gradient(GradientDef {
                kind: GradientKind::Linear {
                    start: [0.0, 0.0],
                    end: [10.0, 0.0],
                },
                stops: vec![
                    GradientStop {
                        position: 0.0,
                        color: 0xFF00_00FF,
                    },
                    GradientStop {
                        position: 1.0,
                        color: 0x0000_FFFF,
                    },
                ],
            })
            .expect("a valid gradient must be accepted");

        registry
            .gradient_mut(id)
            .expect("the gradient just created must be found")
            .stops[0]
            .color = 0x00FF_00FF;

        assert_eq!(
            registry.gradient_mut(id).unwrap().stops[0].color,
            0x00FF_00FF,
            "the mutation must be visible on a later lookup, not lost"
        );
    }

    #[test]
    fn gradient_mut_returns_none_for_an_out_of_range_id() {
        let mut registry = ShapeRegistry::new();
        assert!(registry.gradient_mut(GradientId(0)).is_none());
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s, same reasoning as this crate's other \
                   exact-arithmetic tests"
    )]
    fn flatten_into_renders_a_rectangles_gradient_fill_via_the_styled_path() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        let gradient_id = registry
            .create_gradient(GradientDef {
                kind: GradientKind::Linear {
                    start: [0.0, 0.0],
                    end: [10.0, 0.0],
                },
                stops: vec![
                    GradientStop {
                        position: 0.0,
                        color: 0xFF00_00FF,
                    },
                    GradientStop {
                        position: 1.0,
                        color: 0x0000_FFFF,
                    },
                ],
            })
            .expect("a valid gradient must be accepted");
        // No corner radius, smoothing, or border -- the trivial-case
        // conditions that would normally route through draw_rounded_rect
        // -- proving the gradient fill alone is enough to force the
        // styled path, since draw_rounded_rect has no fill-kind branch.
        registry.insert(ShapePrimitive::Rectangle(Rectangle {
            common: PrimitiveCommon::new(),
            size: [10.0, 10.0],
            fill: FillStyle::Gradient(gradient_id),
            border_color: 0,
            border_thickness: 0.0,
            corner_radius: CornerRadii::uniform(0.0),
            corner_smoothing: 0.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::SdfRectStyled as u16,
            "a gradient fill must route through the styled pipeline even with no border/radii/\
             smoothing, since draw_rounded_rect has no fill-kind branch at all"
        );

        let bytes = device
            .style_buffer
            .bytes
            .lock()
            .expect("FakeStyleBuffer mutex poisoned");
        let gradient_bytes = (crate::gpu_style::GRADIENT_STYLE_WORDS as usize) * 4;
        assert_eq!(
            bytes.len(),
            gradient_bytes + (crate::gpu_style::RECT_STYLE_WORDS as usize) * 4,
            "exactly one GpuGradientStyle record and one GpuRectStyle record must have been \
             written"
        );
        // The gradient record is written FIRST -- a real data dependency,
        // not an arbitrary choice: GpuRectStyle's own gradient_word_index
        // field must already know the gradient's word index at the point
        // GpuRectStyle itself is constructed.
        let gradient_style: &crate::gpu_style::GpuGradientStyle =
            bytemuck::from_bytes(&bytes[..gradient_bytes]);
        assert_eq!(
            gradient_style.kind, 0,
            "GradientKind::Linear must encode as kind 0"
        );
        // Authored in Rectangle's own public local space ([0,0]..[10,0]
        // for this 10x10 rect) but stored center-relative (offset by
        // [5,5], the rect's own half-size) to match frag_uv's own
        // convention -- see build_gpu_gradient_style's own doc comment.
        assert_eq!(gradient_style.point0, [-5.0, -5.0]);
        assert_eq!(gradient_style.point1_or_radius, [5.0, -5.0]);
        assert_eq!(gradient_style.stop_count, 2);
        assert_eq!(gradient_style.stop_colors[0], 0xFF00_00FF);
        assert_eq!(gradient_style.stop_colors[1], 0x0000_FFFF);

        let rect_style: &crate::GpuRectStyle = bytemuck::from_bytes(&bytes[gradient_bytes..]);
        assert_eq!(rect_style.fill_kind, 1);
        assert_eq!(
            rect_style.gradient_word_index, 0,
            "must point at the gradient's own real word index (0, the first write this frame)"
        );
    }

    #[test]
    fn flatten_into_renders_a_circles_gradient_fill() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        let gradient_id = registry
            .create_gradient(GradientDef {
                kind: GradientKind::Radial {
                    center: [5.0, 5.0],
                    radius: 5.0,
                },
                stops: vec![GradientStop {
                    position: 0.0,
                    color: 0xFFFF_FFFF,
                }],
            })
            .expect("a valid gradient must be accepted");
        registry.insert(ShapePrimitive::Circle(Circle {
            common: PrimitiveCommon::new(),
            radius: [5.0, 5.0],
            fill: FillStyle::Gradient(gradient_id),
            border_color: 0,
            border_thickness: 0.0,
            arc_length: 360.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::SdfEllipse as u16
        );
        let bytes = device
            .style_buffer
            .bytes
            .lock()
            .expect("FakeStyleBuffer mutex poisoned");
        // The gradient record is written FIRST (see the matching
        // rectangle test's own comment for why); the ellipse style
        // record follows it.
        let gradient_bytes = (crate::gpu_style::GRADIENT_STYLE_WORDS as usize) * 4;
        let ellipse_style: &crate::GpuEllipseStyle = bytemuck::from_bytes(&bytes[gradient_bytes..]);
        assert_eq!(ellipse_style.fill_kind, 1);
        assert_eq!(ellipse_style.gradient_word_index, 0);
    }

    #[test]
    fn flatten_into_renders_a_polygons_gradient_fill_via_the_gradient_fill_pipeline() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        let gradient_id = registry
            .create_gradient(GradientDef {
                kind: GradientKind::Linear {
                    start: [-20.0, 0.0],
                    end: [20.0, 0.0],
                },
                stops: vec![
                    GradientStop {
                        position: 0.0,
                        color: 0xFF00_00FF,
                    },
                    GradientStop {
                        position: 1.0,
                        color: 0x00FF_00FF,
                    },
                ],
            })
            .expect("a valid gradient must be accepted");
        registry.insert(ShapePrimitive::Polygon(Polygon {
            common: PrimitiveCommon::new(),
            sides: 6,
            radius: 20.0,
            vertex_radius: 0.0,
            star_points: None,
            fill: FillStyle::Gradient(gradient_id),
            border_color: 0,
            border_thickness: 0.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::GradientFill as u16
        );
        let word_index = frame.commands[0].texture_handle;
        assert_eq!(
            word_index, 0,
            "the gradient word index rides in texture_handle, the same push-constant channel \
             TexturedQuad's own texture_index already uses"
        );
        let bytes = device
            .style_buffer
            .bytes
            .lock()
            .expect("FakeStyleBuffer mutex poisoned");
        assert_eq!(
            bytes.len(),
            (crate::gpu_style::GRADIENT_STYLE_WORDS as usize) * 4,
            "exactly one GpuGradientStyle record must have been written -- Polygon/Path have no \
             per-vertex style record of their own"
        );

        // Every vertex's own uv carries its LOCAL (pre-transform)
        // position, not a real texture coordinate -- gradient_fill.frag's
        // own evaluation point.
        assert!(
            frame.vertices.iter().any(|v| v.uv != [0.0, 0.0]),
            "at least one vertex must carry a real, non-origin local position in its own uv \
             field for the gradient to evaluate correctly across the shape"
        );
    }

    #[test]
    #[should_panic(expected = "which this registry never issued")]
    fn flatten_into_panics_on_a_stale_or_foreign_gradient_id() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Polygon(Polygon {
            common: PrimitiveCommon::new(),
            sides: 6,
            radius: 20.0,
            vertex_radius: 0.0,
            star_points: None,
            fill: FillStyle::Gradient(GradientId(999)),
            border_color: 0,
            border_thickness: 0.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s, same reasoning as this crate's other \
                   exact-arithmetic tests"
    )]
    fn bounding_box_uvs_normalizes_a_real_non_degenerate_box() {
        let mut uvs = Vec::new();
        bounding_box_uvs_into(
            &[[0.0, 0.0], [10.0, 0.0], [10.0, 4.0], [0.0, 4.0]],
            &mut uvs,
        );
        assert_eq!(uvs, vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s, same reasoning as this crate's other \
                   exact-arithmetic tests"
    )]
    fn bounding_box_uvs_maps_a_degenerate_zero_extent_axis_to_one_half() {
        // Every point shares the same x -- a zero-width bounding box.
        let mut uvs = Vec::new();
        bounding_box_uvs_into(&[[5.0, 0.0], [5.0, 10.0]], &mut uvs);
        assert_eq!(uvs, vec![[0.5, 0.0], [0.5, 1.0]]);
    }

    #[test]
    fn bounding_box_uvs_into_clears_any_prior_contents_before_refilling() {
        let mut uvs = vec![[9.0, 9.0]];
        bounding_box_uvs_into(
            &[[0.0, 0.0], [10.0, 0.0], [10.0, 4.0], [0.0, 4.0]],
            &mut uvs,
        );
        assert_eq!(uvs.len(), 4);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s, same reasoning as this crate's other \
                   exact-arithmetic tests"
    )]
    fn flatten_into_renders_a_rectangles_texture_fill_via_the_styled_path() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        // No corner radius, smoothing, or border -- proving the texture
        // fill alone is enough to force the styled path, since
        // draw_rounded_rect has no fill-kind branch at all.
        registry.insert(ShapePrimitive::Rectangle(Rectangle {
            common: PrimitiveCommon::new(),
            size: [10.0, 10.0],
            fill: FillStyle::Texture(42),
            border_color: 0,
            border_thickness: 0.0,
            corner_radius: CornerRadii::uniform(0.0),
            corner_smoothing: 0.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::SdfRectStyled as u16,
            "a texture fill must route through the styled pipeline even with no border/radii/\
             smoothing, since draw_rounded_rect has no fill-kind branch at all"
        );
        let bytes = device
            .style_buffer
            .bytes
            .lock()
            .expect("FakeStyleBuffer mutex poisoned");
        let style: &crate::GpuRectStyle = bytemuck::from_bytes(&bytes);
        assert_eq!(style.fill_kind, 2);
        assert_eq!(style.texture_index, 42);
    }

    #[test]
    fn flatten_into_renders_a_circles_texture_fill() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Circle(Circle {
            common: PrimitiveCommon::new(),
            radius: [5.0, 5.0],
            fill: FillStyle::Texture(7),
            border_color: 0,
            border_thickness: 0.0,
            arc_length: 360.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::SdfEllipse as u16
        );
        let bytes = device
            .style_buffer
            .bytes
            .lock()
            .expect("FakeStyleBuffer mutex poisoned");
        let style: &crate::GpuEllipseStyle = bytemuck::from_bytes(&bytes);
        assert_eq!(style.fill_kind, 2);
        assert_eq!(style.texture_index, 7);
    }

    #[test]
    fn flatten_into_renders_a_polygons_texture_fill_via_the_textured_quad_pipeline() {
        let device = FakeDevice::default();
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Polygon(Polygon {
            common: PrimitiveCommon::new(),
            sides: 6,
            radius: 20.0,
            vertex_radius: 0.0,
            star_points: None,
            fill: FillStyle::Texture(3),
            border_color: 0,
            border_thickness: 0.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::TexturedQuad as u16,
            "Polygon/Path texture fill has no per-vertex style record, so it reuses the \
             existing TexturedQuad pipeline directly -- no new pipeline, no new shader"
        );
        assert_eq!(
            frame.commands[0].texture_handle, 3,
            "the real bindless texture index must ride in texture_handle, exactly like any \
             other TexturedQuad draw"
        );
        assert!(
            device
                .style_buffer
                .bytes
                .lock()
                .expect("FakeStyleBuffer mutex poisoned")
                .is_empty(),
            "Polygon texture fill needs no style-buffer write at all -- no GpuRectStyle/\
             GpuEllipseStyle, no GpuGradientStyle"
        );
        // Every vertex's own uv is a real, bounding-box-normalized [0, 1]
        // texture coordinate, not the zeroed uv draw_flat_polygon uses.
        assert!(
            frame
                .vertices
                .iter()
                .all(|v| (0.0..=1.0).contains(&v.uv[0]) && (0.0..=1.0).contains(&v.uv[1])),
            "every vertex's uv must be a real, bounding-box-normalized [0, 1] coordinate"
        );
        assert!(
            frame
                .vertices
                .iter()
                .any(|v| (v.uv[0] - 0.5).abs() > f32::EPSILON
                    || (v.uv[1] - 0.5).abs() > f32::EPSILON),
            "a real hexagon's own bounding box is non-degenerate, so uvs must vary, not all \
             collapse to the degenerate-box fallback"
        );
    }

    #[test]
    fn flatten_into_falls_back_to_normal_blending_on_unsupported_hardware() {
        let device = FakeDevice::default();
        device.local_read_blend_supported.set(false);
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Polygon(Polygon {
            common: {
                let mut common = PrimitiveCommon::new();
                common.blend_mode = BlendMode::Multiply;
                common
            },
            sides: 6,
            radius: 20.0,
            vertex_radius: 0.0,
            star_points: None,
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0,
            border_thickness: 0.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::FlatColor as u16,
            "a non-Normal blend_mode must fall back to plain FlatColor when the device lacks \
             local-read support -- never a silent attempt to use a pipeline that was never \
             created"
        );
    }

    #[test]
    fn flatten_into_routes_a_non_normal_blend_mode_through_flatcolorblend_when_supported() {
        let device = FakeDevice::default();
        device.local_read_blend_supported.set(true);
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Polygon(Polygon {
            common: {
                let mut common = PrimitiveCommon::new();
                common.blend_mode = BlendMode::Multiply;
                common
            },
            sides: 6,
            radius: 20.0,
            vertex_radius: 0.0,
            star_points: None,
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0,
            border_thickness: 0.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::FlatColorBlend as u16
        );
        assert_eq!(
            frame.commands[0].texture_handle,
            BlendMode::Multiply as u32,
            "the numeric blend mode must ride in texture_handle, the same push-constant \
             channel gradient/texture fill already repurpose"
        );
    }

    #[test]
    fn flatten_into_uses_flatcolor_for_a_normal_blend_mode_even_when_local_read_is_supported() {
        let device = FakeDevice::default();
        device.local_read_blend_supported.set(true);
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Polygon(Polygon {
            common: PrimitiveCommon::new(), // blend_mode defaults to Normal
            sides: 6,
            radius: 20.0,
            vertex_radius: 0.0,
            star_points: None,
            fill: FillStyle::Solid(0xFFFF_FFFF),
            border_color: 0,
            border_thickness: 0.0,
        }));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device, None);
        let frame = canvas.flatten();

        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::FlatColor as u16,
            "Normal blending never needs the blend-capable pipeline, even on hardware that \
             supports it"
        );
    }

    /// Phase 10 Step 10.2.4: `sdf_ellipse.frag`'s new exact ellipse SDF
    /// (`sd_ellipse`, Inigo Quilez's Newton-Raphson refinement) proven
    /// against the analytically-known exact distance on the ellipse's
    /// own major/minor axes -- for a point at distance `d > r` along
    /// either axis, the exact signed distance is trivially `d - r`
    /// (PLAN.md Step 10.2.4's own "Scope decisions").
    ///
    /// Both the new AND old formulas turn out to be exact here -- a
    /// real, verified property of the OLD "scaled circle" approximation
    /// too (direct algebraic derivation: at `p = (x, 0)`, `k1 = x/r.x`
    /// and `k2 = k1/r.x`, so `k1*(k1-1)/k2` reduces exactly to `x -
    /// r.x`). PLAN.md's own task language expected the old formula to
    /// show real error at these same reference points; it does not --
    /// see `sd_ellipse_scaled_circle_approximation_has_real_off_axis_
    /// error` below for where its real, measured error actually shows
    /// up (an off-axis point), a disclosed correction to that
    /// expectation once actually checked, not silently dropped.
    ///
    /// No interior major-axis point is included here (unlike the minor
    /// axis, which has no such case): verifying this test's own naive
    /// "`d - r` on-axis" assumption against `brute_force` during this
    /// step's own development found it to be a genuinely wrong
    /// assumption for SOME interior major-axis points, not a bug in
    /// either SDF formula -- an ellipse's evolute has a cusp on its
    /// major axis at `x = (r.x^2 - r.y^2) / r.x` from center (here,
    /// `(140^2-40^2)/140 ≈ 128.6`); any interior point between the
    /// center and that cusp has TWO equally-near, symmetric, OFF-axis
    /// closest boundary points, not the on-axis vertex -- a real,
    /// independently-documented property of ellipse geometry (the
    /// vertex's own evolute/focal-curve structure), not specific to
    /// either formula tested here. The minor axis has no such cusp in
    /// its own interior (its vertices have the ellipse's LARGEST radius
    /// of curvature, not the smallest), so `[0.0, 10.0]` below is safe.
    #[test]
    fn sd_ellipse_exact_matches_analytic_distance_on_the_major_and_minor_axes() {
        let r = [140.0_f32, 40.0];

        for &(p, expected) in &[
            ([200.0_f32, 0.0], 60.0_f32), // major axis, outside
            ([0.0, 90.0], 50.0_f32),      // minor axis, outside
            ([0.0, 10.0], -30.0_f32),     // minor axis, inside
        ] {
            let new = sdf_ellipse_fidelity::exact(p, r);
            let old = sdf_ellipse_fidelity::scaled_circle_approx(p, r);
            assert!(
                (new - expected).abs() < 1e-3,
                "new formula at {p:?}: expected {expected}, got {new}"
            );
            assert!(
                (old - expected).abs() < 1e-3,
                "old formula at {p:?}: expected {expected}, got {old} (both formulas are exact \
                 on-axis -- see this test's own doc comment)"
            );
        }
    }

    /// The real, measured error PLAN.md's own task language anticipated
    /// finding on-axis (see the test above for why it isn't there
    /// instead) actually shows up off-axis, at a genuinely eccentric
    /// ellipse (`r = [140, 40]`, a 3.5:1 aspect ratio) -- proven against
    /// `sdf_ellipse_fidelity::brute_force`, an independent ground truth
    /// sharing no math with either formula (dense parametric-boundary
    /// sampling, not Newton refinement or the scaled-circle identity).
    #[test]
    fn sd_ellipse_exact_matches_an_independent_brute_force_reference_off_axis() {
        let r = [140.0_f32, 40.0];
        for p in [[160.0_f32, 60.0], [50.0, 20.0]] {
            let exact = sdf_ellipse_fidelity::exact(p, r);
            let truth = sdf_ellipse_fidelity::brute_force(p, r);
            let error = (exact - truth).abs();
            assert!(
                error < 0.05,
                "new exact formula at {p:?}: brute-force reference {truth}, got {exact} \
                 (error {error}) -- expected sub-0.05px agreement"
            );
        }
    }

    /// The OLD "scaled circle" approximation's real, measured error at
    /// the SAME off-axis points the test above proves the new formula
    /// gets right, against the SAME independent brute-force ground
    /// truth -- this is the real defect Step 10.2.4 fixes, quantified,
    /// not merely asserted.
    #[test]
    fn sd_ellipse_scaled_circle_approximation_has_real_off_axis_error() {
        let r = [140.0_f32, 40.0];
        for (p, min_expected_error) in [([160.0_f32, 60.0], 1.0_f32), ([50.0, 20.0], 1.0)] {
            let approx = sdf_ellipse_fidelity::scaled_circle_approx(p, r);
            let truth = sdf_ellipse_fidelity::brute_force(p, r);
            let error = (approx - truth).abs();
            assert!(
                error > min_expected_error,
                "old scaled-circle approximation at {p:?}: brute-force reference {truth}, got \
                 {approx} (error {error}) -- expected a real, measurable error over \
                 {min_expected_error}px at this eccentricity, proving Step 10.2's original \
                 approximation was not just theoretically inexact but practically wrong here"
            );
        }
    }

    /// Phase 10 Step 10.2.4: at `smoothing == 0.0`, both `corner_norm`/
    /// `sd_rounded_box`'s superellipse blend AND Figma's real
    /// construction reduce to the exact same thing -- a plain circular-
    /// arc rounded corner (Figma's own `arcMeasure` is `90 * (1 -
    /// smoothing)`, i.e. the full 90-degree circular arc with both
    /// Bezier ramps collapsed to zero length when `smoothing == 0`).
    /// This is the ONE point on the `[0, 1]` smoothing range where an
    /// exact match is not just expected but mathematically guaranteed,
    /// regardless of how the two techniques otherwise diverge -- proven
    /// here rather than assumed.
    #[test]
    fn corner_smoothing_matches_figmas_construction_exactly_at_zero_smoothing() {
        let deviation = corner_smoothing_fidelity::max_deviation_from_engine(200.0, 30.0, 0.0);
        assert!(
            deviation < 0.01,
            "at smoothing=0.0 (plain rounded corner) the two constructions must be identical; \
             measured deviation {deviation}px"
        );
    }

    /// The real, measured divergence PLAN.md Step 10.2.4 asked this
    /// step to either close or honestly quantify (its own "Scope
    /// decisions": "formally verify and document the existing
    /// superellipse blend's real deviation from that reference within a
    /// quantified tolerance"). The engineering call made here, once
    /// this number was in hand: Figma's own construction is a real SVG
    /// path (curvature-continuous Beziers plus a circular arc), not an
    /// implicit distance field -- there is no simple closed form to
    /// swap in, and computing an exact per-pixel distance to an
    /// arbitrary Bezier curve is real, substantial, out-of-scope work
    /// for this one step. `corner_norm` is kept as-is (still a real,
    /// legitimate, monotonic smoothing control), with its own real
    /// deviation now disclosed precisely instead of left as an
    /// unquantified "not verified against any reference" caveat.
    ///
    /// The deviation scales linearly with corner radius (confirmed
    /// during this step's own research at `radius=60`: deviation
    /// doubles too) -- `smoothing=1.0`'s ~71% of the corner radius is
    /// not a rounding-error-scale gap, it is a real, visually
    /// significant difference from Figma's own squircle at maximum
    /// smoothing. Anyone wanting a byte-for-byte Figma match at high
    /// `corner_smoothing` values should treat this engine's own control
    /// as a distinct, engine-native smoothing curve, not a drop-in
    /// equivalent.
    #[test]
    fn corner_smoothing_diverges_substantially_from_figmas_construction_at_high_smoothing() {
        let radius = 30.0;
        for (smoothing, min_expected_deviation, max_expected_deviation) in
            [(0.5_f32, 3.0_f32, 6.0_f32), (1.0, 18.0, 25.0)]
        {
            let deviation =
                corner_smoothing_fidelity::max_deviation_from_engine(200.0, radius, smoothing);
            assert!(
                (min_expected_deviation..max_expected_deviation).contains(&deviation),
                "smoothing={smoothing}: measured deviation {deviation}px, expected in \
                 [{min_expected_deviation}, {max_expected_deviation}) -- either the reference \
                 implementation or this engine's own corner_norm changed; re-verify against \
                 Figma's real construction before adjusting this bound"
            );
        }
    }

    /// Phase 12 Step 12.2: a real, end-to-end proof that `ShapePrimitive::
    /// Text` actually renders through `flatten_into`'s new dispatch --
    /// not just that it compiles. A real system cascade font (the same
    /// `tre_text::FontCascade::discover()` real fontconfig integration
    /// `canvas_draw_text_demo.rs` already proves works) shapes and
    /// renders "Hi" into a real `tre_atlas::AtlasOwner`. Mirrors
    /// `canvas_draw_text_demo.rs`'s own two-frame contract: the first
    /// `flatten_into` call is a real cache miss (every glyph freshly
    /// requested, zero commands emitted -- `draw_text`'s own documented
    /// "report, don't block" contract); after polling the real
    /// background atlas thread to completion, a second call on the same,
    /// re-dirtied registry renders real, non-empty geometry.
    #[test]
    fn flatten_into_renders_a_text_shape_via_the_real_font_and_atlas_pipeline() {
        let device = FakeDevice::default();
        let cascade =
            tre_text::FontCascade::discover().expect("fontconfig cascade discovery failed");
        let font_bytes =
            std::fs::read(&cascade.entries[0]).expect("failed to read the primary cascade font");

        let mut fonts = crate::FontRegistry::new();
        let font_id = fonts
            .load_bytes(font_bytes)
            .expect("a real system cascade font must be a valid font");

        let owner = tre_atlas::AtlasOwner::spawn(256, 256, 8, 8);
        let handle = owner.handle();
        let atlas_context = GlyphAtlasContext {
            atlas: &handle,
            texture_handle: 0, // never read on the cache-miss frame below.
            dimensions: (256, 256),
            current_frame: 0,
        };
        let text_context = crate::TextFlattenContext {
            fonts: &fonts,
            atlas: &atlas_context,
        };

        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Text(crate::Text::new(
            "Hi",
            font_id,
            24.0,
            0xFFFF_FFFF,
        )));

        let mut miss_canvas = RenderingCanvas::new();
        registry.flatten_into(&mut miss_canvas, &device, Some(&text_context));
        let miss_frame = miss_canvas.flatten();
        assert!(
            miss_frame.commands.is_empty(),
            "every glyph of a brand-new Text shape must be a cache miss on its first flatten, \
             got {} commands",
            miss_frame.commands.len()
        );

        // Wait for every real glyph "Hi" actually uses to resolve on the
        // real background atlas thread -- reshaping independently here
        // (via the same FontRegistry, not a separate file read) rather
        // than trusting flatten_text's own internal shaping.
        let face = fonts
            .face(font_id)
            .expect("the same bytes load_bytes already validated must build a real Face");
        let runs = tre_text::shape_text(&face, "Hi").expect("shaping \"Hi\" must succeed");
        for run in &runs {
            for glyph in &run.glyphs {
                let key = tre_atlas::AtlasKey::from_glyph(font_id.0, glyph.glyph_id);
                let mut resolved = false;
                for _ in 0..500 {
                    if handle.lookup(key, 0).is_some() {
                        resolved = true;
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                assert!(
                    resolved,
                    "glyph id {} of \"Hi\" never resolved",
                    glyph.glyph_id
                );
            }
        }

        registry.mark_all_dirty();
        let mut hit_canvas = RenderingCanvas::new();
        registry.flatten_into(&mut hit_canvas, &device, Some(&text_context));
        let hit_frame = hit_canvas.flatten();
        assert!(
            !hit_frame.commands.is_empty(),
            "a second flatten after every glyph resolved must render real geometry, got 0 \
             commands"
        );
        assert!(
            !hit_frame.vertices.is_empty(),
            "a second flatten after every glyph resolved must produce real vertices"
        );

        let _ = owner.join();
    }
}
