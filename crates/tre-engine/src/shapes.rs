//! Phase 10 Step 10.1: the retained-mode shape primitive layer for
//! external UI frameworks (ARCHITECTURE.md Section 7, the canonical
//! reference for every type below). Every concrete shape here compiles
//! down into the exact same immediate-mode [`crate::RenderingCanvas`]
//! calls any other caller would issue -- this module is a convenience,
//! retained-store layer over the existing IR/sort/batch/RHI pipeline,
//! never a second rendering path.
//!
//! Real rendering support today is intentionally narrow. Only a
//! `Rectangle` with a uniform `CornerRadii` (all four corners equal), a
//! zero `corner_smoothing`, a `FillStyle::Solid` fill, and a zero
//! `border_thickness` can actually be flattened right now -- that is
//! exactly, and only, what today's real `draw_rounded_rect` shader
//! supports (Phase 3 Step 3.2). Every other shape/field combination
//! this module's own data model allows for (`Circle`/`Polygon`/`Path`,
//! non-uniform corners, a nonzero `corner_smoothing`, borders,
//! gradients) has no rendering support anywhere in this engine yet --
//! [`ShapeRegistry::flatten_into`] panics loudly on any of them
//! (`unimplemented!`), rather than silently skipping or rendering
//! something wrong, matching this project's own established "fail loud
//! on a real, not-yet-built capability" discipline. See
//! ARCHITECTURE.md Section 7.5's own "Implementation status" note and
//! IMPLEMENTATION.md Step 10.1's "Explicitly out of scope" list for the
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
    /// # Panics
    /// Panics (`unimplemented!`) on any shape/field combination that has
    /// no real rendering support yet -- see this module's own top-level
    /// doc comment for the complete, itemized list. This is a
    /// deliberate, loud failure for a genuinely-not-yet-built rendering
    /// capability, not a recoverable `EngineError` condition.
    pub fn flatten_into(&mut self, canvas: &mut RenderingCanvas) {
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
                ShapePrimitive::Rectangle(rect) => flatten_rectangle(canvas, rect),
                ShapePrimitive::Circle(_) => {
                    unimplemented!(
                        "Circle/Ellipse rendering has no shader support yet -- see \
                         ARCHITECTURE.md Section 7.5's own Implementation status note"
                    )
                }
                ShapePrimitive::Polygon(_) => {
                    unimplemented!(
                        "Polygon/Star rendering has no geometry-generation support yet -- see \
                         ARCHITECTURE.md Section 7.5's own Implementation status note"
                    )
                }
                ShapePrimitive::Path(_) => {
                    unimplemented!(
                        "Path rendering (fill or stroke) is not yet wired into this registry's \
                         own flattening pass -- see ARCHITECTURE.md Section 7.5's own \
                         Implementation status note"
                    )
                }
            }

            if slot.clip_bounds.is_some() {
                canvas.pop_clip();
            }
            canvas.restore();
        }
    }
}

/// `ShapeRegistry::flatten_into`'s own `Rectangle` case -- the one real,
/// rendering-supported shape today. Panics if `rect` uses any field
/// combination today's shader cannot express; see this module's own
/// top-level doc comment.
fn flatten_rectangle(canvas: &mut RenderingCanvas, rect: &Rectangle) {
    assert!(
        rect.corner_radius.is_uniform(),
        "non-uniform corner_radius has no shader support yet -- see ARCHITECTURE.md Section \
         7.5's own Implementation status note"
    );
    assert!(
        rect.corner_smoothing == 0.0,
        "corner_smoothing > 0.0 (squircle rounding) has no shader support yet -- see \
         ARCHITECTURE.md Section 7.5's own Implementation status note"
    );
    assert!(
        rect.border_thickness == 0.0,
        "border rendering has no shader support yet -- see ARCHITECTURE.md Section 7.5's own \
         Implementation status note"
    );
    let FillStyle::Solid(color) = rect.fill else {
        unimplemented!(
            "FillStyle::Gradient/Texture have no rendering support wired into this registry's \
             own flattening pass yet -- see ARCHITECTURE.md Section 7.5's own Implementation \
             status note"
        );
    };
    canvas.draw_rounded_rect(
        0.0,
        0.0,
        rect.size[0],
        rect.size[1],
        rect.corner_radius.top_left,
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut registry = ShapeRegistry::new();
        let id = registry.insert(ShapePrimitive::Rectangle(Rectangle::new(
            [10.0, 10.0],
            0xFFFF_FFFF,
        )));
        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas);
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
        let mut registry = ShapeRegistry::new();
        registry.insert(ShapePrimitive::Rectangle(Rectangle::new(
            [10.0, 10.0],
            0xFFFF_FFFF,
        )));
        let mut warm_up = RenderingCanvas::new();
        registry.flatten_into(&mut warm_up); // clears layout_dirty

        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas);
        let frame = canvas.flatten();
        assert!(
            frame.vertices.is_empty(),
            "a shape that is neither dirty nor animating must not be re-recorded"
        );
    }

    #[test]
    fn flatten_into_skips_hidden_and_collapsed_shapes_without_panicking() {
        let mut registry = ShapeRegistry::new();
        let mut hidden = Rectangle::new([10.0, 10.0], 0xFFFF_FFFF);
        hidden.common.visibility = Visibility::Hidden;
        registry.insert(ShapePrimitive::Rectangle(hidden));
        let mut collapsed = Rectangle::new([10.0, 10.0], 0xFFFF_FFFF);
        collapsed.common.visibility = Visibility::Collapsed;
        registry.insert(ShapePrimitive::Rectangle(collapsed));

        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas);
        let frame = canvas.flatten();
        assert!(
            frame.vertices.is_empty(),
            "Hidden/Collapsed shapes must not be recorded into the IR"
        );
    }

    #[test]
    #[should_panic(expected = "non-uniform corner_radius")]
    fn flatten_into_panics_on_a_non_uniform_corner_radius() {
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
        registry.flatten_into(&mut canvas);
    }

    #[test]
    #[should_panic(expected = "Circle/Ellipse rendering has no shader support")]
    fn flatten_into_panics_on_a_circle() {
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
        registry.flatten_into(&mut canvas);
    }

    #[test]
    fn an_active_animation_keeps_a_shape_flattened_every_frame_even_without_layout_dirty() {
        let mut registry = ShapeRegistry::new();
        let id = registry.insert(ShapePrimitive::Rectangle(Rectangle::new(
            [10.0, 10.0],
            0xFFFF_FFFF,
        )));
        let mut warm_up = RenderingCanvas::new();
        registry.flatten_into(&mut warm_up); // clears layout_dirty

        registry
            .get_mut(id)
            .expect("shape exists")
            .active_animations
            .push(AnimationId(1));

        let mut canvas = RenderingCanvas::new();
        registry.flatten_into(&mut canvas);
        let frame = canvas.flatten();
        assert_eq!(
            frame.vertices.len(),
            4,
            "a non-empty active_animations must still trigger re-flattening"
        );
    }
}
