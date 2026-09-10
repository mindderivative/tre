//! Phase 10 Step 10.1: the retained-mode shape primitive layer for
//! external UI frameworks (ARCHITECTURE.md Section 7, the canonical
//! reference for every type below). Every concrete shape here compiles
//! down into the exact same immediate-mode [`crate::RenderingCanvas`]
//! calls any other caller would issue -- this module is a convenience,
//! retained-store layer over the existing IR/sort/batch/RHI pipeline,
//! never a second rendering path.
//!
//! Real rendering support (Phase 10 Step 10.2): `Rectangle` (any corner
//! radii, uniform or not, plus real borders and corner smoothing) and
//! `Circle`/`Ellipse` (including borders and a partial-arc sweep) both
//! flatten for real now, via `RenderingCanvas::draw_styled_rectangle`/
//! `draw_ellipse` and the two new `sdf_rect_styled`/`sdf_ellipse` GPU
//! pipelines -- `crates/tre-engine/src/gpu_style.rs`'s own doc comment
//! covers the GPU-side style-buffer mechanism both rely on. `Polygon`/
//! `Path` still have no geometry-generation/tessellation wiring, and
//! `FillStyle::Gradient`/`Texture` still have no evaluator anywhere for
//! ANY shape kind -- [`ShapeRegistry::flatten_into`] panics loudly on
//! any of those (`unimplemented!`), rather than silently skipping or
//! rendering something wrong, matching this project's own established
//! "fail loud on a real, not-yet-built capability" discipline. See
//! ARCHITECTURE.md Section 7.5's own "Implementation status" note and
//! IMPLEMENTATION.md Step 10.2's "Explicitly out of scope" list for the
//! full, itemized disposition.

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
/// field holds. **No rendering support exists for any non-`Normal`
/// variant** -- see this module's own top-level doc comment.
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
/// 7.2). `Texture` is real today (the existing bindless texture-handle
/// system); `Gradient` has no evaluator anywhere in this codebase yet
/// -- see this module's own top-level doc comment.
#[derive(Debug, Clone, Copy)]
pub enum FillStyle {
    Solid(Color),
    Gradient(GradientId),
    Texture(u32),
}

/// Opaque handle into the not-yet-built gradient evaluator's own future
/// table (ARCHITECTURE.md Section 7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradientId(pub u32);

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
    /// A corner-radius-free rectangle of `size`, filled `color`, at the
    /// identity transform -- the common case, and the one real-
    /// rendering-supported shape this module currently produces.
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
/// otherwise an ellipse (ARCHITECTURE.md Section 7.3). No rendering
/// support exists for this shape at all yet -- see this module's own
/// top-level doc comment.
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

impl Primitive for Circle {
    fn common(&self) -> &PrimitiveCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut PrimitiveCommon {
        &mut self.common
    }
}

/// Covers triangle/hexagon/N-gon and star shapes with one struct
/// (ARCHITECTURE.md Section 7.3). No rendering support exists for this
/// shape at all yet -- see this module's own top-level doc comment.
#[derive(Debug, Clone, Copy)]
pub struct Polygon {
    pub common: PrimitiveCommon,
    pub sides: u32,
    pub radius: f32,
    pub vertex_radius: f32,
    pub star_points: Option<u32>,
    pub fill: FillStyle,
    pub border_color: Color,
    pub border_thickness: f32,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// The one shape whose own data model already names a real, deliberate
/// heap allocation (`commands: Vec<PathCommand>`) -- consistent with
/// DESIGN.md Section 2.1's own zero-allocation boundary, which exempts
/// complex external subsystems from the per-frame steady-state rule
/// (ARCHITECTURE.md Section 7.3). No rendering support (stroking, or
/// wiring into the existing `tre-svg` fill tessellator) exists for this
/// shape yet -- see this module's own top-level doc comment.
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
}

impl ShapePrimitive {
    #[must_use]
    pub fn common(&self) -> &PrimitiveCommon {
        match self {
            Self::Rectangle(shape) => shape.common(),
            Self::Circle(shape) => shape.common(),
            Self::Polygon(shape) => shape.common(),
            Self::Path(shape) => shape.common(),
        }
    }

    pub fn common_mut(&mut self) -> &mut PrimitiveCommon {
        match self {
            Self::Rectangle(shape) => shape.common_mut(),
            Self::Circle(shape) => shape.common_mut(),
            Self::Polygon(shape) => shape.common_mut(),
            Self::Path(shape) => shape.common_mut(),
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
}

impl ShapeRegistry {
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

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live_count == 0
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
    /// `device` (Phase 10 Step 10.2, new this step) is needed because
    /// `Rectangle`/`Circle`'s real rendering paths beyond the trivial
    /// case write a style record into the device's live, per-frame-
    /// segmented shape-style buffer at flatten time -- see
    /// `RenderingCanvas::draw_styled_rectangle`'s own doc comment for why
    /// that can't be deferred to upload time the way plain vertex/index
    /// data is.
    ///
    /// # Panics
    /// Panics (`unimplemented!`) on any shape/field combination that has
    /// no real rendering support yet -- see this module's own top-level
    /// doc comment for the complete, itemized list. This is a
    /// deliberate, loud failure for a genuinely-not-yet-built rendering
    /// capability, not a recoverable `EngineError` condition.
    pub fn flatten_into(&mut self, canvas: &mut RenderingCanvas, device: &dyn crate::RhiDevice) {
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
                ShapePrimitive::Rectangle(rect) => flatten_rectangle(canvas, device, rect),
                ShapePrimitive::Circle(circle) => flatten_circle(canvas, device, circle),
                ShapePrimitive::Polygon(polygon) => flatten_polygon(canvas, polygon),
                ShapePrimitive::Path(path) => flatten_path_shape(canvas, path),
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

/// `ShapeRegistry::flatten_into`'s own `Rectangle` case (Phase 10 Step
/// 10.2: now real for non-uniform corners, borders, and corner smoothing
/// too, via `RenderingCanvas::draw_styled_rectangle`). The plain, older
/// `draw_rounded_rect` path is still used for the common trivial case
/// (uniform radius, no border, no smoothing) -- cheaper (no style-buffer
/// write) and byte-for-byte what it always rendered. `FillStyle::
/// Gradient`/`Texture` remain genuinely unimplemented; see this module's
/// own top-level doc comment.
fn flatten_rectangle(
    canvas: &mut RenderingCanvas,
    device: &dyn crate::RhiDevice,
    rect: &Rectangle,
) {
    let FillStyle::Solid(color) = rect.fill else {
        unimplemented!(
            "FillStyle::Gradient/Texture have no rendering support wired into this registry's \
             own flattening pass yet -- see ARCHITECTURE.md Section 7.5's own Implementation \
             status note"
        );
    };

    let needs_styled_path = !rect.corner_radius.is_uniform()
        || rect.corner_smoothing != 0.0
        || rect.border_thickness > 0.0;

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
fn flatten_circle(canvas: &mut RenderingCanvas, device: &dyn crate::RhiDevice, circle: &Circle) {
    const TWELVE_OCLOCK: f32 = -std::f32::consts::FRAC_PI_2;

    let FillStyle::Solid(color) = circle.fill else {
        unimplemented!(
            "FillStyle::Gradient/Texture have no rendering support wired into this registry's \
             own flattening pass yet -- see ARCHITECTURE.md Section 7.5's own Implementation \
             status note"
        );
    };

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
/// is exactly what makes [`fan_from_center`]'s triangulation valid for
/// it, unlike an arbitrary (possibly non-star-shaped) polygon.
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
fn fan_from_center(vertex_count: u32) -> Vec<[u32; 3]> {
    if vertex_count < 3 {
        return Vec::new();
    }
    (1..vertex_count)
        .map(|i| [0, i, i + 1])
        .chain(std::iter::once([0, vertex_count, 1]))
        .collect()
}

/// `ShapeRegistry::flatten_into`'s own `Polygon` case -- fill via
/// [`fan_from_center`] (unaffected by the lyon migration below: a
/// regular/star polygon is star-shaped with respect to its own center by
/// construction, so the simple fan is already exactly correct and does
/// not need a general tessellator). Border/stroke is real now too
/// (Phase 10 Step 10.2 follow-up), via [`tessellate_stroke`].
fn flatten_polygon(canvas: &mut RenderingCanvas, polygon: &Polygon) {
    let FillStyle::Solid(color) = polygon.fill else {
        unimplemented!(
            "FillStyle::Gradient/Texture have no rendering support wired into this registry's \
             own flattening pass yet -- see ARCHITECTURE.md Section 7.5's own Implementation \
             status note"
        );
    };

    let boundary = generate_polygon_points(polygon);
    let vertex_count = u32::try_from(boundary.len()).unwrap_or(0);
    let mut points = Vec::with_capacity(boundary.len() + 1);
    points.push([0.0, 0.0]); // the fan's own pivot -- the polygon's local center.
    points.extend(&boundary);
    let triangles = fan_from_center(vertex_count);
    canvas.draw_flat_polygon(&points, &triangles, color);

    if polygon.border_thickness > 0.0 {
        // A polygon boundary is always closed.
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
fn flatten_path_shape(canvas: &mut RenderingCanvas, path: &Path) {
    let FillStyle::Solid(color) = path.fill else {
        unimplemented!(
            "FillStyle::Gradient/Texture have no rendering support wired into this registry's \
             own flattening pass yet -- see ARCHITECTURE.md Section 7.5's own Implementation \
             status note"
        );
    };

    let subpaths_with_closed = flatten_path_with_closed(&path.commands);
    let subpaths: Vec<Vec<Vec2>> = subpaths_with_closed
        .iter()
        .map(|(points, _)| points.clone())
        .collect();

    let (fill_positions, fill_triangles) = tessellate_fill(&subpaths);
    canvas.draw_flat_polygon(&fill_positions, &fill_triangles, color);

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
    use crate::{RhiBuffer, RhiDynamicRingBuffer};
    use std::cell::RefCell;

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
    }

    #[derive(Default)]
    struct FakeStyleBuffer {
        bytes: RefCell<Vec<u8>>,
    }

    impl RhiBuffer for FakeStyleBuffer {
        fn raw_handle(&self) -> u64 {
            0
        }
    }

    impl RhiDynamicRingBuffer for FakeStyleBuffer {
        fn write(&self, bytes: &[u8]) -> Option<u32> {
            let mut buf = self.bytes.borrow_mut();
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
        registry.flatten_into(&mut canvas, &device);
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
        registry.flatten_into(&mut warm_up, &device); // clears layout_dirty

        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device);
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
        registry.flatten_into(&mut canvas, &device);
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
        registry.flatten_into(&mut canvas, &device);
        let frame = canvas.flatten();

        assert_eq!(frame.vertices.len(), 4);
        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::SdfRectStyled as u16,
            "a non-uniform corner radius must route through the styled pipeline, not \
             draw_rounded_rect's uniform-only one"
        );
        assert_eq!(
            device.style_buffer.bytes.borrow().len(),
            28,
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
        registry.flatten_into(&mut canvas, &device);
        let frame = canvas.flatten();

        assert_eq!(frame.vertices.len(), 4);
        assert_eq!(
            frame.commands[0].pipeline_state_id,
            crate::PipelineKind::SdfEllipse as u16
        );
        assert_eq!(
            device.style_buffer.bytes.borrow().len(),
            16,
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
        registry.flatten_into(&mut warm_up, &device); // clears layout_dirty

        registry
            .get_mut(id)
            .expect("shape exists")
            .active_animations
            .push(AnimationId(1));

        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas, &device);
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
    fn fan_from_center_of_a_square_covers_all_four_edges_including_the_wraparound() {
        let triangles = fan_from_center(4);
        assert_eq!(triangles, vec![[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 1]]);
    }

    #[test]
    fn fan_from_center_of_fewer_than_three_vertices_is_empty() {
        assert_eq!(fan_from_center(2), Vec::<[u32; 3]>::new());
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
        registry.flatten_into(&mut canvas, &device);
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
        registry.flatten_into(&mut canvas, &device);
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
            registry.flatten_into(&mut canvas, &device);
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
        registry.flatten_into(&mut canvas, &device);
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
            registry.flatten_into(&mut canvas, &device);
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
        registry.flatten_into(&mut canvas, &device);
        let frame = canvas.flatten();

        assert!(
            frame.vertices.len() > fill_only_vertex_count,
            "a real border must contribute real extra vertices on top of the fill-only case \
             ({} fill-only vs {} with border)",
            fill_only_vertex_count,
            frame.vertices.len()
        );
    }
}
