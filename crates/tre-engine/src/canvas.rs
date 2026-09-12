//! `RenderingCanvas`/`SubCanvas` (the Drawing Context IR-recording
//! engine, DESIGN.md Section 6) and `FrameArena`/the sort-batch
//! pipeline that back its own `flatten()` -- split out of `lib.rs`
//! (REVIEW.md #205) as one cohesive module rather than two separate
//! ones, since `flatten()` calls directly into the sort/batch helpers
//! defined here (`segment_and_flatten`, `premultiply_alpha`, ...) and
//! `FrameArena` exists specifically to serve multi-threaded canvas
//! recording -- there is no other real consumer of either half
//! independent of the other.

use crate::{
    rgba8, style_index_param, texture_format_to_u16, AccessibilityNode, AccessibilityNodeId,
    AccessibilityRole, CommandType, FlattenedFrame, FocusableNode, GpuEllipseStyle, GpuRectStyle,
    LayerDesc, PipelineKind, RhiDevice, ScissorRect, StyleFill, UiDrawCommand, UiVertex,
    FULL_WINDOW_CLIP, NO_TEXTURE, PIPELINE_MSDF_TEXT,
};

/// DESIGN.md Section 7.2's `Canvas::begin_overlay(OverlayLayerPriority)`
/// -- referenced but never defined there; defined here concretely (Step
/// 5.1.3), the same "adapt the doc sketch to what's actually buildable"
/// precedent Step 5.1.2 already established for `DynamicTextLayout`/
/// `Paint`. An offset added to `OVERLAY_LAYER_BASE` (ARCHITECTURE.md
/// Section 4.1's documented `10000` overlay base) to produce the real
/// Layer ID -- a caller stacking multiple overlay planes (e.g. a
/// tooltip that must always paint above an already-open modal) picks a
/// higher priority for the one that should sort later/on top.
/// Absolute: not composed with any enclosing `begin_overlay` call's own
/// priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayLayerPriority(pub u16);

/// ARCHITECTURE.md Section 4.1: "Standard content uses $0-9999$.
/// Overlays, modal backdrops, and popups use $10000+$."
const OVERLAY_LAYER_BASE: u16 = 10_000;

/// One level of the Drawing Context's hierarchical state stack
/// (DESIGN.md Section 6.1: "dynamic coordinate space transformations
/// [and] global alpha multipliers... via `Canvas::save()` and
/// `Canvas::restore()`"). Deliberately holds only transform and alpha --
/// DESIGN.md's own Section 6 architecture diagram lists a *separate*
/// "Dynamic Scissor / Mask Clip Stack (`PushClip`, `PopClip`)" as a distinct
/// mechanism from this one, and blend mode stays deferred alongside
/// `LayerDesc`'s own visual-filter fields until a later phase implements
/// them (IMPLEMENTATION.md Step 5.1.1).
#[derive(Debug, Clone, Copy)]
struct CanvasState {
    /// World transform accumulated by nested `Canvas::transform()` calls
    /// since the last `save()` -- composed via `Affine2::compose`
    /// (`tre-math`, Phase 3 Step 3.1), not reimplemented here.
    transform: tre_math::Affine2,
    /// Effective (already-multiplied-down) alpha for this stack level --
    /// `Canvas::set_alpha()` multiplies onto whatever `save()` copied
    /// forward, so nested group opacity compounds correctly (a child at
    /// local alpha 0.5 inside a parent already at effective 0.5 renders
    /// at effective 0.25).
    alpha: f32,
}

/// Transforms the four corners of the local rect `(x, y, width, height)`
/// by `state.transform` and returns the real axis-aligned bounding box
/// of those transformed corners, as `(x, y, width, height)` -- shared by
/// `tag_accessibility_node` and `tag_focusable` (Phase 19 Step 19.2:
/// factored out of `tag_accessibility_node`'s own original inline logic
/// rather than duplicated). All four corners are transformed, not just
/// the top-left, because the active transform can rotate -- a naive
/// reuse of the local width/height at a transformed origin would be
/// wrong the moment rotation is involved.
fn transform_bounds(
    state: &CanvasState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> (f32, f32, f32, f32) {
    let corners = [
        [x, y],
        [x + width, y],
        [x + width, y + height],
        [x, y + height],
    ]
    .map(|corner| state.transform.transform_point(corner));

    let mut min = corners[0];
    let mut max = corners[0];
    for corner in &corners[1..] {
        min[0] = min[0].min(corner[0]);
        min[1] = min[1].min(corner[1]);
        max[0] = max[0].max(corner[0]);
        max[1] = max[1].max(corner[1]);
    }

    (min[0], min[1], max[0] - min[0], max[1] - min[1])
}

/// Phase 0 stub: records `Canvas::draw_rounded_rect` calls into a plain
/// `Vec` (IMPLEMENTATION.md Phase 0, task 2 -- "no ring buffer, no arena,
/// no multi-threading yet"). Phase 2 Step 1 builds the real RHI-side ring
/// buffer/transient pool (`RhiDevice::create_dynamic_ring_buffer`/
/// `acquire_transient_target`) as standalone, independently-provable
/// primitives, but does not yet rewire `RenderingCanvas`'s own IR
/// accumulation to write through them -- nothing downstream of `Canvas`
/// consumes a ring-buffer offset yet (the sort/batch/execute pipeline,
/// IMPLEMENTATION.md Phase 6, is what would), so wiring it in now would
/// be plumbing with no real consumer to verify it against. Deferred.
///
/// IMPLEMENTATION.md Step 5.1.1 added the real Drawing Context state:
/// `state_stack` (transform + alpha, `save`/`restore`) and `clip_stack`
/// (scissor rects, `push_clip`/`pop_clip`) are two genuinely separate
/// stacks, per `CanvasState`'s own doc comment -- not one bundled state
/// object.
#[derive(Default)]
pub struct RenderingCanvas {
    // `pub(crate)` on `vertices`/`commands` (REVIEW.md #205's own
    // fallout): the surviving test module in `lib.rs` is now this
    // struct's sibling, not a descendant, and directly asserts against
    // these two fields in several existing tests.
    pub(crate) vertices: Vec<UiVertex>,
    indices: Vec<u32>,
    pub(crate) commands: Vec<UiDrawCommand>,
    /// `push_layer`/`pop_layer`'s stack of in-flight `LayerDesc`s
    /// (IMPLEMENTATION.md Step 2.2 task 5, extended Step 6.4.2): pushed/
    /// popped on each call, asserted empty at `flatten()`. Was a bare
    /// balance counter (`layer_depth: u32`) until Step 6.4.2, when
    /// `pop_layer` needed the *original* pushed `LayerDesc` back (its
    /// position/size/format) to bake real composite-quad geometry --
    /// `.len()` still serves the same balance-counter role a plain `u32`
    /// did.
    layer_stack: Vec<LayerDesc>,
    /// The Drawing Context's transform/alpha stack (Step 5.1.1). Always
    /// has at least one entry -- the base level `save`/`restore` can
    /// never pop past -- so every read of `.last()` is infallible by
    /// construction, not just by convention.
    state_stack: Vec<CanvasState>,
    /// The independent scissor-clip stack (Step 5.1.1). Empty means "no
    /// clip, full window" -- the same sentinel `draw_rounded_rect`
    /// already used unconditionally before this step. Its own length
    /// doubles as the balance counter `flatten()` checks, the same role
    /// `layer_stack`'s own length plays for `push_layer`/`pop_layer`.
    clip_stack: Vec<ScissorRect>,
    /// The next `Depth ID` a `DrawGeometry` command will receive (Step
    /// 5.1.3) -- a single global, monotonically increasing counter,
    /// never reset by `save`/`push_clip`/`push_layer`/`begin_overlay`.
    /// No widget tree or z-index resolver exists above this imperative
    /// `Canvas` API to derive a richer traversal-order index from
    /// (ARCHITECTURE.md Section 4.1) -- call order is the only ordering
    /// this layer of the stack has.
    ///
    /// Shared (Step 5.2.1: `Arc<AtomicU32>` rather than a plain `u32`)
    /// so every `SubCanvas` `create_sub_canvas()` produces increments
    /// the exact same counter -- `fetch_add`'s own atomicity is what
    /// guarantees no two `DrawGeometry` commands anywhere in the frame
    /// (root canvas or any sub-canvas, on any thread) ever collide on
    /// Depth ID, which `flatten_run`'s sort/merge logic depends on.
    next_depth_id: std::sync::Arc<std::sync::atomic::AtomicU32>,
    /// The overlay Layer ID stack (Step 5.1.3, DESIGN.md Section 7.2).
    /// Empty means standard content (Layer ID `0`); `begin_overlay`
    /// pushes `OVERLAY_LAYER_BASE + priority`, `end_overlay` pops.
    /// Unrelated to `push_layer`/`pop_layer`'s own offscreen-compositing
    /// mechanism -- see `begin_overlay`'s own doc comment for why these
    /// two very differently-named "layer" concepts never interact.
    /// Deliberately *not* shared across sub-canvases the way
    /// `next_depth_id` is -- Layer ID only distinguishes standard
    /// content from the overlay plane, never needs to be unique per
    /// command, so two different sub-canvases both drawing at Layer `0`
    /// (or both calling `begin_overlay` at the same priority) is
    /// completely correct (Step 5.2.1).
    overlay_stack: Vec<u16>,
    /// `begin_overlay`'s own saved copy of whatever `clip_stack` held
    /// before it was reset to "no clip" -- one entry pushed per
    /// `begin_overlay` call, popped and restored by the matching
    /// `end_overlay`.
    saved_clip_stacks: Vec<Vec<ScissorRect>>,
    /// The maximum number of concurrently-live `SubCanvas` instances
    /// (Step 5.2.1, TECHNICAL.md Section 8: "`available_parallelism()`
    /// minus one"). Set once at construction (`RenderingCanvas::new()`,
    /// or the `#[cfg(test)]`-only `new_with_sub_canvas_cap`), copied by
    /// value into every `SubCanvas` -- never mutated after construction,
    /// so it needs no atomic of its own.
    max_sub_canvases: usize,
    /// How many `SubCanvas` instances sharing this canvas's root are
    /// currently alive. Shared with every `SubCanvas` (`Arc::clone`);
    /// `create_sub_canvas` increments it via a compare-exchange loop
    /// that checks `max_sub_canvases` *before* committing the increment,
    /// and `SubCanvas`'s own `Drop` decrements it -- exact even if a
    /// caller panics mid-use and that panic is later caught
    /// (TECHNICAL.md Section 9.4's `catch_unwind` FFI boundary), unlike
    /// a fire-and-forget increment only ever fixed up on the
    /// non-panicking path.
    live_sub_canvases: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// Every node tagged this frame via `tag_accessibility_node` (Step
    /// 5.3.1) -- a flat list, not a tree the engine builds; the UI
    /// framework already owns the real hierarchy and this only reports
    /// each node's rendered spatial position back.
    accessibility_nodes: Vec<AccessibilityNode>,
    /// Every node tagged this frame via `tag_focusable` (Phase 19 Step
    /// 19.2) -- same "flat list, not a tree" shape as
    /// `accessibility_nodes`, and deliberately NOT threaded through
    /// `flatten`/`FrameArena`/`SubCanvas` merging the way that field is:
    /// that plumbing exists for multi-threaded recording, and no real
    /// caller records focusable nodes off the main thread today. Read
    /// directly off the root `RenderingCanvas` via `focusable_nodes()`,
    /// before `render_canvas()`/`flatten()` consumes it -- the same
    /// "read before render" ordering rule Step 18.3 already established
    /// for `accessibility_nodes()`.
    focusable_nodes: Vec<FocusableNode>,
}

/// A worker thread's independently-recordable sub-canvas
/// (`Canvas::create_sub_canvas()`, DESIGN.md Section 6.3/TECHNICAL.md
/// Section 8, Step 5.2.1). Wraps a private `RenderingCanvas` with a
/// fresh, empty `state_stack`/`clip_stack`/`overlay_stack`/`vertices`/
/// `indices`/`commands` -- every existing drawing method (`save`,
/// `push_clip`, `begin_overlay`, `draw_rounded_rect`, `draw_text`)
/// works on it unchanged via `Deref`/`DerefMut`, since a `SubCanvas`
/// records into exactly the same kind of thread-local linear arena a
/// root canvas does. The one thing it shares with its root (and every
/// sibling `SubCanvas`) is the Depth ID counter -- see
/// `RenderingCanvas::next_depth_id`'s own doc comment for why that
/// specific field, and only that one, must be genuinely shared.
///
/// Still cannot be `flatten()`ed directly -- that takes `self` by value,
/// which `Deref`/`DerefMut` cannot forward, and a `SubCanvas` (unlike a
/// root canvas) is never the final destination for a frame's data
/// anyway. Merging a `SubCanvas`'s recorded data into a shared
/// [`FrameArena`] is `stitch_into`'s job (Step 5.2.2), forwarded
/// unchanged via `Deref` since Phase 9 Step 9.2 made it take `&self`
/// rather than consume `self` (REVIEW.md finding #134) -- a `SubCanvas`
/// can therefore now be `reset()` (also forwarded via `DerefMut`) and
/// reused across many frames instead of being dropped and recreated
/// every one, though dropping it (ordinary scope exit) remains a
/// perfectly normal way to end one when reuse isn't wanted.
pub struct SubCanvas {
    canvas: RenderingCanvas,
    live_sub_canvases: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl std::ops::Deref for SubCanvas {
    type Target = RenderingCanvas;

    fn deref(&self) -> &RenderingCanvas {
        &self.canvas
    }
}

impl std::ops::DerefMut for SubCanvas {
    fn deref_mut(&mut self) -> &mut RenderingCanvas {
        &mut self.canvas
    }
}

impl Drop for SubCanvas {
    fn drop(&mut self) {
        self.live_sub_canvases
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

/// Everything `Canvas::draw_text` needs to know about the caller's
/// currently-uploaded shared dynamic texture atlas (IMPLEMENTATION.md
/// Step 5.1.2) -- bundled since all four fields travel together at
/// every call site. `atlas` is the borrowed, `Clone`-able lookup/
/// request handle (ARCHITECTURE.md: one atlas shared by every window,
/// never owned by a per-frame `Canvas`); `texture_handle` is that
/// atlas's current bindless GPU texture index (obtained by the caller
/// once per texture upload, the same way `atlas_concurrency_demo`'s own
/// `texture.bindless_index()` call does); `dimensions` is the atlas's
/// own pixel width/height, needed to normalize a `PackedRect` into UV
/// coordinates; `current_frame` is the caller's own frame counter,
/// forwarded to `AtlasOwnerHandle::lookup`/`request_insert` unchanged.
pub struct GlyphAtlasContext<'a> {
    pub atlas: &'a tre_atlas::AtlasOwnerHandle,
    pub texture_handle: u32,
    pub dimensions: (u32, u32),
    pub current_frame: u64,
}

impl RenderingCanvas {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state_stack: vec![CanvasState {
                transform: tre_math::Affine2::IDENTITY,
                alpha: 1.0,
            }],
            max_sub_canvases: default_max_sub_canvases(),
            ..Self::default()
        }
    }

    /// Test-only: overrides `max_sub_canvases` directly, so
    /// `create_sub_canvas()`'s cap-panic behavior is deterministically
    /// testable regardless of the test runner's real core count (Step
    /// 5.2.1's own scope decision -- a genuinely single-core CI runner
    /// would otherwise make every real `create_sub_canvas()` call panic
    /// immediately, with no way for a test to pick a known-good cap).
    #[cfg(test)]
    pub(crate) fn new_with_sub_canvas_cap(cap: usize) -> Self {
        Self {
            max_sub_canvases: cap,
            ..Self::new()
        }
    }

    /// The maximum number of concurrently-live `SubCanvas` instances
    /// this canvas will allow (TECHNICAL.md Section 8:
    /// `available_parallelism() - 1`, or a smaller test-injected value)
    /// -- Step 5.2.3: a real caller deciding how many worker threads to
    /// actually spawn reads this first, rather than guessing and
    /// risking `create_sub_canvas()`'s own panic.
    #[must_use]
    pub fn max_sub_canvases(&self) -> usize {
        self.max_sub_canvases
    }

    /// Resets this canvas to a freshly-`new()`-like empty state *without*
    /// releasing any of its `Vec`s' own backing allocations -- Phase 9
    /// Step 9.2's own zero-allocation reuse path (REVIEW.md finding
    /// #134: a real caller with a long-running loop can now build one
    /// `RenderingCanvas` and reuse it every frame via `reset()` instead
    /// of constructing a fresh one, which always allocated at least
    /// `state_stack`'s own single entry). `vertices`/`indices`/
    /// `commands`/`accessibility_nodes`/`layer_stack`/`clip_stack`/
    /// `overlay_stack`/`saved_clip_stacks` are all `.clear()`d (kept
    /// capacity, no allocation on a warm canvas); `state_stack` is
    /// cleared and given back exactly the one identity entry `new()`
    /// itself seeds, matching `flatten()`'s own "always has at least one
    /// entry" invariant. `next_depth_id` is reset to 0 -- a reused
    /// canvas must not let this shared, monotonically-increasing counter
    /// grow unbounded across a long-running session the way a genuinely
    /// fresh-every-frame canvas (today's only real caller) never could;
    /// this exactly reproduces `new()`'s own `AtomicU32::new(0)` starting
    /// point. Available on [`SubCanvas`] automatically via its existing
    /// `DerefMut` -- resetting a `SubCanvas` also re-zeroes the shared
    /// counter (harmless if the root or a sibling also resets it to the
    /// same value the same frame; see `create_sub_canvas`'s own doc
    /// comment for why this field, uniquely, is shared via `Arc`).
    /// `max_sub_canvases`/`live_sub_canvases` are untouched -- a reused
    /// canvas's own concurrency-cap bookkeeping does not change just
    /// because a new frame started.
    pub fn reset(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();
        self.accessibility_nodes.clear();
        self.focusable_nodes.clear();
        self.layer_stack.clear();
        self.clip_stack.clear();
        self.overlay_stack.clear();
        self.saved_clip_stacks.clear();
        self.state_stack.clear();
        self.state_stack.push(CanvasState {
            transform: tre_math::Affine2::IDENTITY,
            alpha: 1.0,
        });
        self.next_depth_id
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// Creates an independently-recordable `SubCanvas` sharing this
    /// canvas's Depth ID counter and concurrency-cap bookkeeping (Step
    /// 5.2.1, DESIGN.md Section 6.3). Intended to be moved into a real
    /// worker thread (e.g. via `std::thread::spawn`) and recorded into
    /// with the exact same drawing API this canvas itself has.
    ///
    /// # Panics
    /// Panics if creating this `SubCanvas` would exceed
    /// `max_sub_canvases` (TECHNICAL.md Section 8:
    /// `available_parallelism() - 1`, or a smaller test-injected value)
    /// -- a caller spawning more worker threads than it configured
    /// itself for is a programmer error, not a recoverable runtime
    /// condition (DESIGN.md Section 2.6).
    #[must_use]
    pub fn create_sub_canvas(&self) -> SubCanvas {
        let mut current = self
            .live_sub_canvases
            .load(std::sync::atomic::Ordering::Relaxed);
        loop {
            assert!(
                current < self.max_sub_canvases,
                "create_sub_canvas() would exceed the configured limit of {} concurrent \
                 sub-canvases (TECHNICAL.md Section 8: available_parallelism() - 1)",
                self.max_sub_canvases
            );
            match self.live_sub_canvases.compare_exchange_weak(
                current,
                current + 1,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
        SubCanvas {
            canvas: RenderingCanvas {
                next_depth_id: std::sync::Arc::clone(&self.next_depth_id),
                max_sub_canvases: self.max_sub_canvases,
                live_sub_canvases: std::sync::Arc::clone(&self.live_sub_canvases),
                ..RenderingCanvas::new()
            },
            live_sub_canvases: std::sync::Arc::clone(&self.live_sub_canvases),
        }
    }

    /// Pushes a copy of the current transform/alpha state -- subsequent
    /// `transform()`/`set_alpha()` calls mutate only this new top level,
    /// leaving the saved one intact for `restore()` to return to.
    ///
    /// # Panics
    /// Never in practice: `state_stack` always holds at least one entry
    /// by construction (`new()` seeds it, and only `restore()` -- itself
    /// guarded against popping the last one -- ever removes an entry).
    pub fn save(&mut self) {
        let top = *self
            .state_stack
            .last()
            .expect("state_stack must always have at least one entry");
        self.state_stack.push(top);
    }

    /// Pops back to the previously saved transform/alpha state.
    ///
    /// # Panics
    /// Panics if called without a matching prior `save()` -- popping the
    /// base level would leave no active state at all, a programmer error
    /// (DESIGN.md Section 2.6), matching `pop_layer`'s own precedent of
    /// failing immediately (not just at `flatten()`) on this exact class
    /// of mistake.
    pub fn restore(&mut self) {
        assert!(
            self.state_stack.len() > 1,
            "restore() called without a matching save()"
        );
        self.state_stack.pop();
    }

    /// Composes `matrix` onto the current top-of-stack transform
    /// (`world_child = world_parent * local_child`, DESIGN.md Section
    /// 7.1's convention) -- reuses `Affine2::compose` directly rather
    /// than reimplementing matrix multiplication here.
    ///
    /// # Panics
    /// Never in practice -- see `save()`'s own `# Panics` section for why
    /// `state_stack` is never empty.
    pub fn transform(&mut self, matrix: &tre_math::Affine2) {
        let top = self
            .state_stack
            .last_mut()
            .expect("state_stack must always have at least one entry");
        top.transform = top.transform.compose(matrix);
    }

    /// Multiplies the current top-of-stack's effective alpha by `factor`
    /// -- see `CanvasState::alpha`'s own doc comment for why this
    /// compounds rather than replaces.
    ///
    /// # Panics
    /// Never in practice -- see `save()`'s own `# Panics` section for why
    /// `state_stack` is never empty.
    pub fn set_alpha(&mut self, factor: f32) {
        let top = self
            .state_stack
            .last_mut()
            .expect("state_stack must always have at least one entry");
        top.alpha *= factor;
    }

    /// Intersects `rect` with the current clip (or uses it directly if
    /// nothing is clipped yet), pushes the result, and emits a real
    /// `PushScissor` command -- the variant has existed since Phase 0 but
    /// this is its first real emission.
    pub fn push_clip(&mut self, rect: &ScissorRect) {
        let intersected = match self.clip_stack.last() {
            Some(&current) => intersect_scissor(current, *rect),
            None => *rect,
        };
        self.clip_stack.push(intersected);
        self.commands.push(UiDrawCommand {
            kind: CommandType::PushScissor,
            sort_key: 0,
            pipeline_state_id: 0,
            texture_handle: 0,
            element_count: 0,
            vertex_offset: 0,
            clip_bounds: intersected,
        });
    }

    /// Pops the clip stack and emits a real `PopScissor` command.
    ///
    /// # Panics
    /// Panics if called without a matching prior `push_clip()`, same
    /// immediate-failure precedent as `restore()`/`pop_layer()`.
    pub fn pop_clip(&mut self) {
        self.clip_stack
            .pop()
            .expect("pop_clip() called without a matching push_clip()");
        self.commands.push(UiDrawCommand {
            kind: CommandType::PopScissor,
            sort_key: 0,
            pipeline_state_id: 0,
            texture_handle: 0,
            element_count: 0,
            vertex_offset: 0,
            clip_bounds: ScissorRect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        });
    }

    /// Routes subsequent draws into the overlay plane (DESIGN.md
    /// Section 7.2, ARCHITECTURE.md Section 4.1's Layer ID $\ge$ 10000)
    /// and resets the active clip to the full window -- "decoupled from
    /// the parent container's scissor stack" (DESIGN.md Section 7.2).
    /// Unrelated to `push_layer`/`pop_layer`'s own offscreen
    /// compositing-layer mechanism (DESIGN.md Section 5): this method
    /// only ever changes the sort key's Layer ID field and the clip
    /// stack, never acquires or redirects into a render target.
    ///
    /// # Panics
    /// Panics if `priority.0` would push the Layer ID past `u16::MAX`
    /// (the 16-bit Layer ID field, ARCHITECTURE.md Section 4.1) -- a
    /// caller error, not a recoverable runtime condition.
    pub fn begin_overlay(&mut self, priority: OverlayLayerPriority) {
        let layer_id = OVERLAY_LAYER_BASE
            .checked_add(priority.0)
            .expect("overlay priority overflowed the 16-bit Layer ID field");
        self.overlay_stack.push(layer_id);
        self.saved_clip_stacks
            .push(std::mem::take(&mut self.clip_stack));
        self.commands.push(UiDrawCommand {
            kind: CommandType::PushScissor,
            sort_key: 0,
            pipeline_state_id: 0,
            texture_handle: 0,
            element_count: 0,
            vertex_offset: 0,
            clip_bounds: FULL_WINDOW_CLIP,
        });
    }

    /// Restores the clip stack `begin_overlay` reset and pops the
    /// active overlay Layer ID.
    ///
    /// # Panics
    /// Panics if called without a matching prior `begin_overlay()`,
    /// same immediate-failure precedent as `restore()`/`pop_clip()`/
    /// `pop_layer()`.
    pub fn end_overlay(&mut self) {
        self.overlay_stack
            .pop()
            .expect("end_overlay() called without a matching begin_overlay()");
        self.clip_stack = self
            .saved_clip_stacks
            .pop()
            .expect("end_overlay() called without a matching begin_overlay()");
        self.commands.push(UiDrawCommand {
            kind: CommandType::PopScissor,
            sort_key: 0,
            pipeline_state_id: 0,
            texture_handle: 0,
            element_count: 0,
            vertex_offset: 0,
            clip_bounds: ScissorRect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        });
    }

    /// The active overlay Layer ID, or `0` (standard content) if
    /// nothing is currently inside a `begin_overlay`/`end_overlay`
    /// bracket.
    fn active_layer_id(&self) -> u16 {
        self.overlay_stack.last().copied().unwrap_or(0)
    }

    /// Assigns the next `sort_key` for a `DrawGeometry` command about to
    /// be emitted -- reads the active overlay Layer ID, advances
    /// `next_depth_id` by one (never reset, guaranteeing every command
    /// in a frame gets a distinct key), and packs the result via
    /// `compute_sort_key` (ARCHITECTURE.md Section 4.1).
    ///
    /// # Panics
    /// Never in practice: a single frame would need over a million
    /// `DrawGeometry` calls to overflow `next_depth_id`'s 20-bit field
    /// -- see `compute_sort_key`'s own `# Panics` section. (`fetch_add`
    /// itself never panics -- it wraps on overflow, per atomic
    /// semantics -- but reaching the real, far lower 20-bit Depth ID
    /// threshold `compute_sort_key` checks is already astronomically
    /// unlikely, and wrapping the raw `u32` counter itself at 4 billion
    /// calls was never the meaningful bound.)
    pub(crate) fn next_sort_key(&mut self, pipeline_state_id: u16, texture_handle: u32) -> u64 {
        let depth_id = self
            .next_depth_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        compute_sort_key(
            self.active_layer_id(),
            pipeline_state_id,
            texture_handle,
            depth_id,
        )
    }

    /// Records a `PushLayer` IR marker (DESIGN.md Section 6.2) and pushes
    /// `desc` onto `layer_stack`. Does not itself acquire a transient
    /// render target -- that's `execute_frame`'s job (Step 6.4.2), driven
    /// by Step 6.4.1's RHI capability; `desc.format` rides this command's
    /// otherwise-unused `pipeline_state_id` field (`texture_format_to_u16`,
    /// below) since `TextureFormat` has no integer repr of its own to
    /// reuse directly, and `PushLayer` itself never resolves a pipeline.
    pub fn push_layer(&mut self, desc: &LayerDesc) {
        self.layer_stack.push(*desc);
        self.commands.push(UiDrawCommand {
            kind: CommandType::PushLayer,
            sort_key: 0,
            pipeline_state_id: texture_format_to_u16(desc.format),
            texture_handle: 0,
            element_count: 0,
            vertex_offset: 0,
            clip_bounds: ScissorRect {
                x: desc.x,
                y: desc.y,
                width: desc.width,
                height: desc.height,
            },
        });
    }

    /// Pops `layer_stack` and records a `PopLayer` IR marker that also
    /// bakes real composite-quad geometry (Step 6.4.2): four vertices
    /// covering the popped `LayerDesc`'s own `x`/`y`/`width`/`height`,
    /// sampling a bound texture across its full `(0,0)`-`(1,1)` UV extent
    /// -- the same shape `render_to_texture_demo.rs`'s own hand-built
    /// `textured_quad` proved (Step 6.4.1). Deliberately skips both
    /// `state.transform` and `premultiply_alpha`, unlike `draw_rounded_
    /// rect`: `LayerDesc.x`/`y` are screen-space, matching `ScissorRect`'s
    /// own untransformed semantics (this command's `clip_bounds` above
    /// already uses them raw, with no transform involved), and
    /// `LayerDesc` carries no opacity field yet -- its own doc comment
    /// defers that to a later phase's visual filter pipeline, so this
    /// draw stays a fixed opaque white, letting the sampled texture's own
    /// (already premultiplied, by `end_render_to_texture`'s own layer
    /// content) alpha carry through unmodified.
    ///
    /// The emitted command's `texture_handle` carries the popped
    /// `LayerDesc`'s own `blur` flag (`1` if set, `0` otherwise) -- never
    /// a real bindless index, which only exists once `execute_frame`
    /// renders into the layer and calls `RhiDevice::register_bindless`
    /// at execute time; `execute_frame`'s own `PopLayer` handling
    /// substitutes the real index in directly rather than trusting this
    /// field for that, the same way it already substitutes `full_window`
    /// for `PushScissor`'s `FULL_WINDOW_CLIP` sentinel. Reusing this
    /// field for `blur` (Step 7.2.2) rather than widening `UiDrawCommand`
    /// matches `PushLayer`'s own established precedent of smuggling
    /// `LayerDesc` data through an otherwise-inert IR field
    /// (`texture_format_to_u16`/`pipeline_state_id`, above).
    ///
    /// # Panics
    /// Panics if called without a matching prior `push_layer` -- an
    /// unbalanced push/pop is a programmer error (DESIGN.md Section 2.6),
    /// not a recoverable runtime condition.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a single frame's vertex/index count stays far below u32::MAX, \
                   the same headroom reasoning ARCHITECTURE.md Section 4.1 applies to Depth ID"
    )]
    #[allow(
        clippy::cast_precision_loss,
        reason = "screen-space layer position/size stays far below f32's exact-integer range \
                   for any real window size"
    )]
    pub fn pop_layer(&mut self) {
        let desc = self
            .layer_stack
            .pop()
            .expect("pop_layer called without a matching push_layer");

        let base_vertex = self.vertices.len() as u32;
        let base_index = self.indices.len() as u32;
        let white = rgba8(255, 255, 255, 255);
        let (x, y) = (desc.x as f32, desc.y as f32);
        let (w, h) = (desc.width as f32, desc.height as f32);
        let positions = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        self.vertices.extend(
            positions
                .into_iter()
                .zip(uvs)
                .map(|(position, uv)| UiVertex {
                    position,
                    uv,
                    color: white,
                    params: [0.0; 3],
                }),
        );
        self.indices.extend_from_slice(&[
            base_vertex,
            base_vertex + 1,
            base_vertex + 2,
            base_vertex,
            base_vertex + 2,
            base_vertex + 3,
        ]);

        self.commands.push(UiDrawCommand {
            kind: CommandType::PopLayer,
            sort_key: 0,
            pipeline_state_id: PipelineKind::TexturedQuad as u16,
            texture_handle: u32::from(desc.blur),
            element_count: 6,
            vertex_offset: base_index,
            clip_bounds: ScissorRect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        });
    }

    /// Emits exactly one `UiDrawCommand` per call, backed by a real
    /// analytical SDF rounded rectangle (TECHNICAL.md Section 5.2's "always
    /// exactly 4 vertices / 6 indices per rectangle" rule, evaluated by
    /// IMPLEMENTATION.md Step 3.2's `sdf_rounded_rect` shader). `radius` is
    /// a single uniform corner radius, clamped to
    /// `[0.0, min(w, h) / 2.0]` before use -- an uncapped radius produces a
    /// self-overlapping, visually wrong shape from this exact formula, not
    /// a crash, but a real, easy caller mistake worth guarding against at
    /// the one place it's constructed. Each corner's `uv` is that corner's
    /// offset from the rect's center, in *local* (untransformed) pixel
    /// units (ARCHITECTURE.md Section 3.1's "Texture coordinates or SDF
    /// bounds" convention) -- linear interpolation across the quad's two
    /// triangles reproduces the exact local `(x, y)` offset at every
    /// fragment, the standard technique for evaluating a box SDF from a
    /// single quad. `params` is `[radius, half_width, half_height]`,
    /// uniform across all 4 vertices since the vertex format has no
    /// per-quad channel. `uv`/`params` deliberately stay in local space
    /// even though `position` does not (see below) -- the SDF shader
    /// evaluates the rounded-rect formula against the rect's own local
    /// half-extents, which must stay a true rectangle regardless of
    /// whatever the active transform does to the rect's screen position
    /// (e.g. a rotation).
    ///
    /// IMPLEMENTATION.md Step 5.1.1: `position` is the active
    /// `Canvas::transform()`'s `Affine2` applied to each raw corner (world
    /// space, not local); `rgba`'s alpha channel is scaled by the active
    /// `Canvas::set_alpha()` effective alpha (`premultiply_alpha`, below); the
    /// emitted command's `clip_bounds` is the current `push_clip()` top,
    /// or the previous unconditional "full window" sentinel if nothing is
    /// clipped.
    ///
    /// # Panics
    /// Never in practice -- see `save()`'s own `# Panics` section for why
    /// `state_stack` is never empty.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a single frame's vertex/index count stays far below u32::MAX, \
                   the same headroom reasoning ARCHITECTURE.md Section 4.1 applies to Depth ID"
    )]
    pub fn draw_rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, rgba: u32) {
        let base_vertex = self.vertices.len() as u32;
        let base_index = self.indices.len() as u32;

        let half_width = w / 2.0;
        let half_height = h / 2.0;
        let radius = radius.clamp(0.0, half_width.min(half_height));
        let params = [radius, half_width, half_height];

        let state = *self
            .state_stack
            .last()
            .expect("state_stack must always have at least one entry");
        let color = premultiply_alpha(rgba, state.alpha);
        let positions = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
        let uvs = [
            [-half_width, -half_height],
            [half_width, -half_height],
            [half_width, half_height],
            [-half_width, half_height],
        ];
        self.vertices.extend(
            positions
                .into_iter()
                .zip(uvs)
                .map(|(position, uv)| UiVertex {
                    position: state.transform.transform_point(position),
                    uv,
                    color,
                    params,
                }),
        );
        self.indices.extend_from_slice(&[
            base_vertex,
            base_vertex + 1,
            base_vertex + 2,
            base_vertex,
            base_vertex + 2,
            base_vertex + 3,
        ]);

        let clip_bounds = self.clip_stack.last().copied().unwrap_or(FULL_WINDOW_CLIP);
        let sort_key = self.next_sort_key(0, 0);
        self.commands.push(UiDrawCommand {
            kind: CommandType::DrawGeometry,
            sort_key,
            pipeline_state_id: 0,
            texture_handle: NO_TEXTURE,
            element_count: 6,
            vertex_offset: base_index,
            clip_bounds,
        });
    }

    /// Phase 10 Step 10.2: the non-uniform-corner-radii / bordered /
    /// corner-smoothed rectangle path -- `ShapeRegistry::flatten_into`'s
    /// real caller for any `Rectangle` beyond `draw_rounded_rect`'s
    /// narrower uniform-radius/borderless case. A SEPARATE method (and
    /// pipeline, `PipelineKind::SdfRectStyled`) from `draw_rounded_rect`,
    /// not a second code path inside it, specifically so `draw_rounded_
    /// rect`'s existing callers/tests stay byte-for-byte unaffected.
    ///
    /// Requires a live `device` handle -- unlike every other `Canvas`
    /// method, this one writes a `GpuRectStyle` record into `device`'s
    /// per-frame-segmented `shape_style_buffer` immediately (not deferred
    /// to upload time), since that buffer's "current segment" is decided
    /// by the device's own live frame state at write time; see
    /// `crates/tre-engine/src/gpu_style.rs`'s module doc comment for why
    /// `UiVertex` itself has no room for this data.
    ///
    /// `corner_radii` is `[top_left, top_right, bottom_right,
    /// bottom_left]`, each independently clamped to half the smaller
    /// extent (`draw_rounded_rect`'s own clamp, applied per-corner here).
    /// `border_thickness <= 0.0` disables the border entirely (pure fill,
    /// matching `draw_rounded_rect`'s visual result when `border_rgba` is
    /// irrelevant). `corner_smoothing` is clamped to `[0, 1]` --
    /// `sdf_rect_styled.frag`'s own doc comment on what `0`/`1` mean.
    ///
    /// `fill` (Phase 10 Step 10.2.1, extended Step 10.2.2 for texture
    /// fill) bundles every GPU-side fill-selection field into one value
    /// ([`StyleFill`]'s own doc comment has the full field-by-`fill_kind`
    /// account): [`StyleFill::SOLID`] for a plain solid fill (`fill_rgba`
    /// is what renders, the common case); `fill_kind: 1` with a real
    /// `GpuGradientStyle` word index (from writing one via this same
    /// `device`'s style buffer first) to fill with a gradient instead;
    /// `fill_kind: 2` with a real bindless texture index to fill with a
    /// texture. `fill_rgba` is ignored by the shader for either non-solid
    /// case (still written into the vertex `color` field regardless,
    /// since `UiVertex` always carries one). `ShapeRegistry::flatten_
    /// into`'s own `FillStyle` handling is the real, intended caller for
    /// the non-solid cases; a direct caller passing a non-zero
    /// `fill_kind` is responsible for having already written the
    /// referenced style-buffer record itself.
    ///
    /// # Panics
    /// Panics if the shape style buffer has no room left this frame
    /// (DESIGN.md Section 2.6: ring-buffer starvation is reported, not
    /// silently dropped -- the same policy `RhiDynamicRingBuffer::write`'s
    /// own doc comment already applies to the vertex/index ring buffer).
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a single frame's vertex/index count stays far below u32::MAX, matching \
                   draw_rounded_rect's own identical reasoning"
    )]
    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors Rectangle's own real field set 1:1, plus one bundled StyleFill"
    )]
    pub fn draw_styled_rectangle(
        &mut self,
        device: &dyn RhiDevice,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        corner_radii: [f32; 4],
        fill_rgba: u32,
        border_rgba: u32,
        border_thickness: f32,
        corner_smoothing: f32,
        fill: StyleFill,
    ) {
        let base_vertex = self.vertices.len() as u32;
        let base_index = self.indices.len() as u32;

        let half_width = w / 2.0;
        let half_height = h / 2.0;
        let max_radius = half_width.min(half_height);
        let clamped_radii = corner_radii.map(|r| r.clamp(0.0, max_radius));

        let state = *self
            .state_stack
            .last()
            .expect("state_stack must always have at least one entry");
        let fill_color = premultiply_alpha(fill_rgba, state.alpha);
        let border_color = premultiply_alpha(border_rgba, state.alpha);

        let style = GpuRectStyle {
            corner_radii: clamped_radii,
            border_color,
            border_thickness: border_thickness.max(0.0),
            corner_smoothing: corner_smoothing.clamp(0.0, 1.0),
            fill_kind: fill.fill_kind,
            gradient_word_index: fill.gradient_word_index,
            texture_index: fill.texture_index,
        };
        let byte_offset = device
            .shape_style_buffer()
            .write(bytemuck::bytes_of(&style))
            .expect("shape style buffer starved for this frame");
        let params = [style_index_param(byte_offset), half_width, half_height];

        let positions = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
        let uvs = [
            [-half_width, -half_height],
            [half_width, -half_height],
            [half_width, half_height],
            [-half_width, half_height],
        ];
        self.vertices.extend(
            positions
                .into_iter()
                .zip(uvs)
                .map(|(position, uv)| UiVertex {
                    position: state.transform.transform_point(position),
                    uv,
                    color: fill_color,
                    params,
                }),
        );
        self.indices.extend_from_slice(&[
            base_vertex,
            base_vertex + 1,
            base_vertex + 2,
            base_vertex,
            base_vertex + 2,
            base_vertex + 3,
        ]);

        let clip_bounds = self.clip_stack.last().copied().unwrap_or(FULL_WINDOW_CLIP);
        let pipeline_id = PipelineKind::SdfRectStyled as u16;
        let sort_key = self.next_sort_key(pipeline_id, 0);
        self.commands.push(UiDrawCommand {
            kind: CommandType::DrawGeometry,
            sort_key,
            pipeline_state_id: pipeline_id,
            texture_handle: NO_TEXTURE,
            element_count: 6,
            vertex_offset: base_index,
            clip_bounds,
        });
    }

    /// Phase 10 Step 10.2: Circle/Ellipse rendering --
    /// `ShapeRegistry::flatten_into`'s real caller for `ShapePrimitive::
    /// Circle`. `radius` is `[radius_x, radius_y]` (a uniform circle when
    /// equal); `arc_sweep_angle >= TAU` (`std::f32::consts::TAU`) draws a
    /// complete, unswept ellipse. See `draw_styled_rectangle`'s own doc
    /// comment for why this needs a live `device` handle, and for
    /// `fill`'s own identical `StyleFill` contract.
    ///
    /// # Panics
    /// Panics if the shape style buffer has no room left this frame --
    /// same policy as `draw_styled_rectangle`'s identical case.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "mirrors draw_rounded_rect's own identical reasoning"
    )]
    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors Circle's own real field set 1:1, plus one bundled StyleFill"
    )]
    pub fn draw_ellipse(
        &mut self,
        device: &dyn RhiDevice,
        center_x: f32,
        center_y: f32,
        radius: [f32; 2],
        fill_rgba: u32,
        border_rgba: u32,
        border_thickness: f32,
        arc_start_angle: f32,
        arc_sweep_angle: f32,
        fill: StyleFill,
    ) {
        let base_vertex = self.vertices.len() as u32;
        let base_index = self.indices.len() as u32;

        let state = *self
            .state_stack
            .last()
            .expect("state_stack must always have at least one entry");
        let fill_color = premultiply_alpha(fill_rgba, state.alpha);
        let border_color = premultiply_alpha(border_rgba, state.alpha);

        let style = GpuEllipseStyle {
            border_color,
            border_thickness: border_thickness.max(0.0),
            arc_start_angle,
            arc_sweep_angle: arc_sweep_angle.max(0.0),
            fill_kind: fill.fill_kind,
            gradient_word_index: fill.gradient_word_index,
            texture_index: fill.texture_index,
        };
        let byte_offset = device
            .shape_style_buffer()
            .write(bytemuck::bytes_of(&style))
            .expect("shape style buffer starved for this frame");
        let params = [style_index_param(byte_offset), radius[0], radius[1]];

        let [rx, ry] = radius;
        let positions = [
            [center_x - rx, center_y - ry],
            [center_x + rx, center_y - ry],
            [center_x + rx, center_y + ry],
            [center_x - rx, center_y + ry],
        ];
        let uvs = [[-rx, -ry], [rx, -ry], [rx, ry], [-rx, ry]];
        self.vertices.extend(
            positions
                .into_iter()
                .zip(uvs)
                .map(|(position, uv)| UiVertex {
                    position: state.transform.transform_point(position),
                    uv,
                    color: fill_color,
                    params,
                }),
        );
        self.indices.extend_from_slice(&[
            base_vertex,
            base_vertex + 1,
            base_vertex + 2,
            base_vertex,
            base_vertex + 2,
            base_vertex + 3,
        ]);

        let clip_bounds = self.clip_stack.last().copied().unwrap_or(FULL_WINDOW_CLIP);
        let pipeline_id = PipelineKind::SdfEllipse as u16;
        let sort_key = self.next_sort_key(pipeline_id, 0);
        self.commands.push(UiDrawCommand {
            kind: CommandType::DrawGeometry,
            sort_key,
            pipeline_state_id: pipeline_id,
            texture_handle: NO_TEXTURE,
            element_count: 6,
            vertex_offset: base_index,
            clip_bounds,
        });
    }

    /// Phase 10 Step 10.2: a plain, flat-vertex-color triangle mesh --
    /// `ShapeRegistry::flatten_into`'s real caller for `Polygon`/`Path`
    /// fill (both ultimately reduce to "world-space points plus a
    /// triangle-index list plus one solid color" once generated/
    /// tessellated). `positions` are already in the shape's own LOCAL
    /// space -- each gets the active transform applied here, exactly
    /// like every other `draw_*` method's own corners. `uv`/`params` are
    /// zeroed (`tre_svg::to_ui_vertices`' own established convention for
    /// a plain triangle soup with no SDF to evaluate).
    ///
    /// Emits no command at all if `triangles` is empty (a degenerate
    /// input, e.g. fewer than 3 points) -- no vertices are pushed either,
    /// so this is a true no-op rather than inert unreferenced data.
    ///
    /// # Panics
    /// Never in practice -- see `save()`'s own `# Panics` section for why
    /// `state_stack` is never empty.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a single frame's vertex/index count stays far below u32::MAX, matching \
                   draw_rounded_rect's own identical reasoning"
    )]
    pub fn draw_flat_polygon(&mut self, positions: &[[f32; 2]], triangles: &[[u32; 3]], rgba: u32) {
        if triangles.is_empty() {
            return;
        }

        let base_vertex = self.vertices.len() as u32;
        let base_index = self.indices.len() as u32;

        let state = *self
            .state_stack
            .last()
            .expect("state_stack must always have at least one entry");
        let color = premultiply_alpha(rgba, state.alpha);

        self.vertices
            .extend(positions.iter().map(|&position| UiVertex {
                position: state.transform.transform_point(position),
                uv: [0.0, 0.0],
                color,
                params: [0.0; 3],
            }));
        self.indices.extend(
            triangles
                .iter()
                .flat_map(|&[a, b, c]| [base_vertex + a, base_vertex + b, base_vertex + c]),
        );

        let clip_bounds = self.clip_stack.last().copied().unwrap_or(FULL_WINDOW_CLIP);
        let pipeline_id = PipelineKind::FlatColor as u16;
        let sort_key = self.next_sort_key(pipeline_id, 0);
        self.commands.push(UiDrawCommand {
            kind: CommandType::DrawGeometry,
            sort_key,
            pipeline_state_id: pipeline_id,
            texture_handle: NO_TEXTURE,
            element_count: (triangles.len() * 3) as u32,
            vertex_offset: base_index,
            clip_bounds,
        });
    }

    /// `ShapePrimitive::CustomShaded` (Phase 13 Step 13.8: custom shader
    /// API; `params` added Phase 16 Step 16.1) -- draws an axis-aligned
    /// `(0,0)`-`(w,h)` quad with real `0..1` UVs (identical vertex
    /// layout to `TexturedQuad`'s own draw path), tagged with a
    /// caller-registered `pipeline_id` instead
    /// of one of the eight built-in `PipelineKind`s. `execute_frame`
    /// already resolves any `pipeline_state_id` generically via
    /// `PipelineRegistry::get`, not a hardcoded per-`PipelineKind`
    /// branch (confirmed by reading its own source before adding this),
    /// so no RHI-level change was needed to make an arbitrary
    /// registered pipeline id actually render -- only this real,
    /// disclosed engine-level seam to reach it from a `ShapePrimitive`.
    ///
    /// # Panics
    /// Never in practice -- see `save()`'s own `# Panics` section for why
    /// `state_stack` is never empty.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a single frame's vertex/index count stays far below u32::MAX, matching \
                   draw_flat_polygon's own identical reasoning"
    )]
    #[allow(
        clippy::too_many_arguments,
        reason = "one field per real per-vertex/per-draw input; `params` (Phase 16 Step 16.1) \
                   is the last of them, mirroring CustomShaded's own field list"
    )]
    pub fn draw_custom_shaded_quad(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        pipeline_id: u16,
        rgba: u32,
        params: [f32; 3],
    ) {
        let base_vertex = self.vertices.len() as u32;
        let base_index = self.indices.len() as u32;

        let state = *self
            .state_stack
            .last()
            .expect("state_stack must always have at least one entry");
        let color = premultiply_alpha(rgba, state.alpha);
        let positions = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        self.vertices.extend(
            positions
                .into_iter()
                .zip(uvs)
                .map(|(position, uv)| UiVertex {
                    position: state.transform.transform_point(position),
                    uv,
                    color,
                    params,
                }),
        );
        self.indices.extend_from_slice(&[
            base_vertex,
            base_vertex + 1,
            base_vertex + 2,
            base_vertex,
            base_vertex + 2,
            base_vertex + 3,
        ]);

        let clip_bounds = self.clip_stack.last().copied().unwrap_or(FULL_WINDOW_CLIP);
        let sort_key = self.next_sort_key(pipeline_id, 0);
        self.commands.push(UiDrawCommand {
            kind: CommandType::DrawGeometry,
            sort_key,
            pipeline_state_id: pipeline_id,
            texture_handle: NO_TEXTURE,
            element_count: 6,
            vertex_offset: base_index,
            clip_bounds,
        });
    }

    /// `Polygon`/`Path` gradient fill (Phase 10 Step 10.2.1) --
    /// `ShapeRegistry::flatten_into`'s real caller for a `Polygon`/`Path`
    /// whose `fill` is `FillStyle::Gradient`. `positions` are LOCAL,
    /// pre-transform coordinates, exactly like [`draw_flat_polygon`]'s
    /// own -- but unlike that method (which zeroes `UiVertex::uv`, having
    /// no use for it), this one carries each vertex's own LOCAL position
    /// through `uv` so `gradient_fill.frag` can evaluate the gradient at
    /// the correct point after transform/perspective interpolation --
    /// `Rectangle`/`Circle` get this for free from their own existing
    /// `frag_uv` convention (an SDF shape's fragment shader already
    /// evaluates in local space); `Polygon`/`Path` have no equivalent
    /// per-vertex local-space channel otherwise, since
    /// [`draw_flat_polygon`]'s `position` field is already transformed
    /// into world space by the time it reaches a vertex.
    ///
    /// `gradient_word_index` is a real `GpuGradientStyle` word index
    /// (from writing one via `RhiDevice::shape_style_buffer` first) --
    /// carried to the fragment shader via the SAME per-draw push constant
    /// `PipelineKind::TexturedQuad`'s own `texture_index` already uses
    /// (`execute_frame`'s `cmd_buffer.bind_texture(0, command.
    /// texture_handle)` call runs for every `DrawGeometry` command
    /// regardless of pipeline, and `RhiCommandBuffer::draw_indexed`
    /// always pushes it as part of one shared `PushConstants` struct) --
    /// not a new push-constant range, and not a per-vertex style word
    /// the way `Rectangle`/`Circle` use, since a gradient fill's word
    /// index is constant across one whole draw, exactly like a texture
    /// index already is.
    ///
    /// # Panics
    /// Panics if `state_stack` is empty -- see [`draw_flat_polygon`]'s
    /// own `# Panics` section for why that never happens in practice.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "mirrors draw_flat_polygon's own identical reasoning"
    )]
    pub fn draw_gradient_polygon(
        &mut self,
        positions: &[[f32; 2]],
        triangles: &[[u32; 3]],
        gradient_word_index: u32,
    ) {
        if triangles.is_empty() {
            return;
        }

        let base_vertex = self.vertices.len() as u32;
        let base_index = self.indices.len() as u32;

        let state = *self
            .state_stack
            .last()
            .expect("state_stack must always have at least one entry");

        self.vertices
            .extend(positions.iter().map(|&position| UiVertex {
                position: state.transform.transform_point(position),
                uv: position,
                color: 0xFFFF_FFFF,
                params: [0.0; 3],
            }));
        self.indices.extend(
            triangles
                .iter()
                .flat_map(|&[a, b, c]| [base_vertex + a, base_vertex + b, base_vertex + c]),
        );

        let clip_bounds = self.clip_stack.last().copied().unwrap_or(FULL_WINDOW_CLIP);
        let pipeline_id = PipelineKind::GradientFill as u16;
        let sort_key = self.next_sort_key(pipeline_id, gradient_word_index);
        self.commands.push(UiDrawCommand {
            kind: CommandType::DrawGeometry,
            sort_key,
            pipeline_state_id: pipeline_id,
            texture_handle: gradient_word_index,
            element_count: (triangles.len() * 3) as u32,
            vertex_offset: base_index,
            clip_bounds,
        });
    }

    /// `Polygon`/`Path` texture fill (Phase 10 Step 10.2.2) --
    /// `ShapeRegistry::flatten_into`'s real caller for a `Polygon`/`Path`
    /// whose `fill` is `FillStyle::Texture`. `uvs` are real,
    /// already-computed bounding-box-normalized `[0, 1]` texture
    /// coordinates, one per `positions` entry (`draw_polygon_fill`'s own
    /// bounding-box helper computes them) -- unlike [`draw_gradient_
    /// polygon`]'s own `uv` (repurposed to carry local position), this is
    /// a REAL texture coordinate, so this reuses the EXISTING
    /// `PipelineKind::TexturedQuad`/`bindless_textured.frag` pipeline
    /// directly: no new shader, no new pipeline -- sampling a texture is
    /// not new math the way gradient evaluation was, `bindless_textured.
    /// frag` already does exactly this for its own existing callers
    /// (`PopLayer`'s own compositing quad).
    ///
    /// # Panics
    /// Panics if `positions.len() != uvs.len()` (a real caller
    /// programming error, not a normal runtime condition), or if
    /// `state_stack` is empty (see [`draw_flat_polygon`]'s own `#
    /// Panics` section for why that never happens in practice).
    #[allow(
        clippy::cast_possible_truncation,
        reason = "mirrors draw_flat_polygon's own identical reasoning"
    )]
    pub fn draw_textured_polygon(
        &mut self,
        positions: &[[f32; 2]],
        uvs: &[[f32; 2]],
        triangles: &[[u32; 3]],
        texture_index: u32,
    ) {
        assert_eq!(
            positions.len(),
            uvs.len(),
            "draw_textured_polygon: positions and uvs must be the same length"
        );
        if triangles.is_empty() {
            return;
        }

        let base_vertex = self.vertices.len() as u32;
        let base_index = self.indices.len() as u32;

        let state = *self
            .state_stack
            .last()
            .expect("state_stack must always have at least one entry");

        self.vertices
            .extend(positions.iter().zip(uvs).map(|(&position, &uv)| UiVertex {
                position: state.transform.transform_point(position),
                uv,
                color: 0xFFFF_FFFF,
                params: [0.0; 3],
            }));
        self.indices.extend(
            triangles
                .iter()
                .flat_map(|&[a, b, c]| [base_vertex + a, base_vertex + b, base_vertex + c]),
        );

        let clip_bounds = self.clip_stack.last().copied().unwrap_or(FULL_WINDOW_CLIP);
        let pipeline_id = PipelineKind::TexturedQuad as u16;
        let sort_key = self.next_sort_key(pipeline_id, texture_index);
        self.commands.push(UiDrawCommand {
            kind: CommandType::DrawGeometry,
            sort_key,
            pipeline_state_id: pipeline_id,
            texture_handle: texture_index,
            element_count: (triangles.len() * 3) as u32,
            vertex_offset: base_index,
            clip_bounds,
        });
    }

    /// `Polygon`/`Path` solid fill under a non-`Normal` `BlendMode`
    /// (Phase 10 Step 10.2.3) -- `ShapeRegistry::flatten_into`'s real
    /// caller once it has confirmed `RhiDevice::local_read_blend_
    /// supported()`; callers MUST check that themselves first (this
    /// method has no way to, and would otherwise silently select a
    /// pipeline/descriptor set that was never created on unsupported
    /// hardware). `blend_mode` is `BlendMode as u32` (`Multiply` = 1
    /// through `ColorDodge` = 5 -- `Normal`/`0` has no reason to call
    /// this method at all, since it is exactly what [`draw_flat_
    /// polygon`] already renders via ordinary hardware blending).
    ///
    /// Carried to `flat_color_blend.frag` via the SAME per-draw push
    /// constant [`draw_gradient_polygon`]'s own `gradient_word_index`
    /// and [`draw_textured_polygon`]'s own `texture_index` already use
    /// (`execute_frame`'s `cmd_buffer.bind_texture(0, command.
    /// texture_handle)` call runs for every `DrawGeometry` command
    /// regardless of pipeline) -- not a new push-constant range, since a
    /// blend mode is constant across one whole draw exactly like those
    /// two values already are.
    ///
    /// # Panics
    /// Panics if `state_stack` is empty -- see [`draw_flat_polygon`]'s
    /// own `# Panics` section for why that never happens in practice.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "mirrors draw_flat_polygon's own identical reasoning"
    )]
    pub fn draw_flat_polygon_blended(
        &mut self,
        positions: &[[f32; 2]],
        triangles: &[[u32; 3]],
        rgba: u32,
        blend_mode: u32,
    ) {
        if triangles.is_empty() {
            return;
        }

        let base_vertex = self.vertices.len() as u32;
        let base_index = self.indices.len() as u32;

        let state = *self
            .state_stack
            .last()
            .expect("state_stack must always have at least one entry");
        let color = premultiply_alpha(rgba, state.alpha);

        self.vertices
            .extend(positions.iter().map(|&position| UiVertex {
                position: state.transform.transform_point(position),
                uv: [0.0, 0.0],
                color,
                params: [0.0; 3],
            }));
        self.indices.extend(
            triangles
                .iter()
                .flat_map(|&[a, b, c]| [base_vertex + a, base_vertex + b, base_vertex + c]),
        );

        let clip_bounds = self.clip_stack.last().copied().unwrap_or(FULL_WINDOW_CLIP);
        let pipeline_id = PipelineKind::FlatColorBlend as u16;
        let sort_key = self.next_sort_key(pipeline_id, blend_mode);
        self.commands.push(UiDrawCommand {
            kind: CommandType::DrawGeometry,
            sort_key,
            pipeline_state_id: pipeline_id,
            texture_handle: blend_mode,
            element_count: (triangles.len() * 3) as u32,
            vertex_offset: base_index,
            clip_bounds,
        });
    }

    /// Renders one already-shaped `ShapedRun` (IMPLEMENTATION.md Step
    /// 5.1.2, `tre-engine`'s first wiring into `tre-text`/`tre-atlas`) as
    /// a sequence of atlas-backed MSDF glyph quads. `font_id`
    /// distinguishes fonts sharing one atlas (`AtlasKey::from_glyph`'s
    /// own `(font_id, glyph_id)` packing); `origin` is the pen's
    /// starting position in the active transform's local space
    /// (transformed the same way `draw_rounded_rect`'s corners are);
    /// `px_size` is the target em size in pixels, used both to scale
    /// `shaped`'s font-design-unit advances/offsets (via `font`'s own
    /// `unitsPerEm`) and as the fixed on-screen side length of every
    /// glyph's square MSDF quad -- PLAN.md's documented simplification:
    /// a fixed square, not each glyph's true design-space bounding box,
    /// same as `atlas_concurrency_demo` already renders.
    ///
    /// A glyph whose outline has no real ink is skipped entirely: no
    /// atlas interaction, no emitted quad, pen still advances. "No real
    /// ink" means `tre_text::has_real_ink` returns `false` -- both the
    /// literal empty-outline case (whitespace, e.g. U+0020 SPACE, per
    /// `tre_text::msdf`'s own doc comment) and a non-empty-but-degenerate
    /// outline (all-coincident or non-finite points, real output some
    /// corrupted/truncated font data -- or even an ordinary font's own
    /// single-point contour -- can legitimately produce). Gating on the
    /// narrower `!outline.is_empty()` alone (REVIEW.md finding #139) let
    /// the latter case reach `GlyphRasterSource::rasterize`'s `.expect`,
    /// panicking the shared atlas owner background thread instead of
    /// being skipped here. A glyph not yet resident in the atlas fires a
    /// real `request_insert` (ignoring a `false`/queue-full return --
    /// "report, don't block," DESIGN.md Section 2.6) and renders nothing
    /// this frame; a resident glyph emits one real textured
    /// `DrawGeometry` command, current transform/alpha/clip state applied
    /// exactly as `draw_rounded_rect` already does.
    ///
    /// # Panics
    /// Never in practice -- see `save()`'s own `# Panics` section for why
    /// `state_stack` is never empty.
    #[allow(
        clippy::cast_precision_loss,
        reason = "unitsPerEm and every glyph's own advance/offset stay far below f32's exact-\
                   integer range for any real font/text"
    )]
    #[allow(
        clippy::too_many_arguments,
        reason = "each parameter is independently load-bearing (shaped run, resolved font, \
                   font identity, pen origin/size, color, and the bundled borrowed atlas \
                   context); a fifth or sixth parameter beyond the existing four already got \
                   its own GlyphAtlasContext bundle rather than growing this list further"
    )]
    pub fn draw_text(
        &mut self,
        shaped: &tre_text::ShapedRun,
        font: &skrifa::FontRef,
        font_id: u32,
        origin: [f32; 2],
        px_size: f32,
        rgba: u32,
        atlas_context: &GlyphAtlasContext<'_>,
    ) {
        let units_per_em = skrifa::MetadataProvider::metrics(
            font,
            skrifa::instance::Size::unscaled(),
            skrifa::instance::LocationRef::default(),
        )
        .units_per_em;
        let scale = px_size / f32::from(units_per_em);

        let state = *self
            .state_stack
            .last()
            .expect("state_stack must always have at least one entry");
        let color = premultiply_alpha(rgba, state.alpha);
        let clip_bounds = self.clip_stack.last().copied().unwrap_or(FULL_WINDOW_CLIP);

        let mut pen = origin;
        for glyph in &shaped.glyphs {
            let key = tre_atlas::AtlasKey::from_glyph(font_id, glyph.glyph_id);
            let glyph_origin = [
                pen[0] + glyph.x_offset as f32 * scale,
                pen[1] - glyph.y_offset as f32 * scale,
            ];

            if let Some((rect, _generation)) =
                atlas_context.atlas.lookup(key, atlas_context.current_frame)
            {
                self.emit_glyph_quad(
                    glyph_origin,
                    px_size,
                    rect,
                    atlas_context,
                    color,
                    clip_bounds,
                    state.transform,
                );
            } else if let Ok(outline) =
                tre_text::glyph_outline(font, skrifa::GlyphId::from(glyph.glyph_id))
            {
                // REVIEW.md finding #137 (documented, not fixed): this
                // miss branch re-runs real outline extraction and fires
                // another `request_insert` on every single frame a glyph
                // stays unresolved -- no "already requested" tracking
                // exists anywhere in this stack (`AtlasOwnerHandle::
                // lookup`'s own doc comment states pending-vs-never-
                // requested is deliberately indistinguishable). Latent
                // today (every real demo pre-seeds its atlas, so no
                // glyph here ever stays unresolved for more than one
                // frame); a real live-text-under-load consumer would pay
                // this cost scaling with (pending glyphs) x (frames to
                // resolve). Real fix: track in-flight-requested keys, or
                // extend `lookup`'s own contract to distinguish "pending"
                // from "never requested."
                //
                // REVIEW.md finding #139: `!outline.is_empty()` alone is
                // the wrong (too narrow) guard -- a non-empty-but-
                // degenerate outline (e.g. a lone MoveTo/Close pair, or a
                // real single-point contour some fonts legitimately
                // produce) still fails `generate_msdf`'s own real
                // degeneracy check, and `GlyphRasterSource::rasterize`
                // panics on that `None` on the atlas owner's shared
                // background thread. `has_real_ink` runs the same check
                // `generate_msdf` itself requires, not a weaker one.
                if tre_text::has_real_ink(&outline) {
                    let _ = atlas_context.atlas.request_insert(
                        key,
                        Box::new(tre_text::GlyphRasterSource {
                            contours: outline,
                            size: GLYPH_MSDF_SIZE,
                            range_px: GLYPH_MSDF_RANGE_PX,
                        }),
                        atlas_context.current_frame,
                    );
                }
            }

            pen[0] += glyph.x_advance as f32 * scale;
            pen[1] += glyph.y_advance as f32 * scale;
        }
    }

    /// `draw_text`'s cache-hit path: emits one `px_size`-square textured
    /// quad, anchored with its bottom edge on the baseline and
    /// horizontally centered on `origin` (the shaped pen position plus
    /// the glyph's own scaled `x_offset`/`y_offset`) -- see `draw_text`'s
    /// own doc comment for why a fixed square, not `rect`'s true aspect,
    /// is this step's deliberate simplification. `uv` is normalized
    /// against `atlas_context.dimensions`, the same
    /// rect-over-atlas-size pattern `atlas_concurrency_demo` already
    /// established -- unlike `draw_rounded_rect`'s local SDF-bounds
    /// `uv`, `UiVertex::uv`'s other documented meaning
    /// ("Texture coordinates," ARCHITECTURE.md Section 3.1).
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a single frame's vertex/index count stays far below u32::MAX, \
                   the same headroom reasoning ARCHITECTURE.md Section 4.1 applies to Depth ID"
    )]
    #[allow(
        clippy::cast_precision_loss,
        reason = "atlas/rect coordinates stay far below f32's exact-integer range for any \
                   real atlas size"
    )]
    #[allow(
        clippy::too_many_arguments,
        reason = "a private helper splitting draw_text's own cache-hit path -- every parameter \
                   is a value draw_text already computed once and passes through unchanged"
    )]
    fn emit_glyph_quad(
        &mut self,
        origin: [f32; 2],
        px_size: f32,
        rect: tre_atlas::PackedRect,
        atlas_context: &GlyphAtlasContext<'_>,
        color: u32,
        clip_bounds: ScissorRect,
        transform: tre_math::Affine2,
    ) {
        let base_vertex = self.vertices.len() as u32;
        let base_index = self.indices.len() as u32;

        let (atlas_width, atlas_height) = atlas_context.dimensions;
        let (u0, v0, u1, v1) = (
            rect.x as f32 / atlas_width as f32,
            rect.y as f32 / atlas_height as f32,
            (rect.x + rect.width) as f32 / atlas_width as f32,
            (rect.y + rect.height) as f32 / atlas_height as f32,
        );

        let half = px_size / 2.0;
        let (x0, y0) = (origin[0] - half, origin[1] - px_size);
        let (x1, y1) = (origin[0] + half, origin[1]);
        let positions = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
        let uvs = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
        self.vertices.extend(
            positions
                .into_iter()
                .zip(uvs)
                .map(|(position, uv)| UiVertex {
                    position: transform.transform_point(position),
                    uv,
                    color,
                    params: [0.0; 3],
                }),
        );
        self.indices.extend_from_slice(&[
            base_vertex,
            base_vertex + 1,
            base_vertex + 2,
            base_vertex,
            base_vertex + 2,
            base_vertex + 3,
        ]);

        let sort_key = self.next_sort_key(PIPELINE_MSDF_TEXT, atlas_context.texture_handle);
        self.commands.push(UiDrawCommand {
            kind: CommandType::DrawGeometry,
            sort_key,
            pipeline_state_id: PIPELINE_MSDF_TEXT,
            texture_handle: atlas_context.texture_handle,
            element_count: 6,
            vertex_offset: base_index,
            clip_bounds,
        });
    }

    /// DESIGN.md Section 5.2's `Canvas::tag_accessibility_node(node_id,
    /// bounds, role_flags)` (Step 5.3.1). `x`/`y`/`width`/`height` are
    /// in this canvas's *local* space, matching every other drawing
    /// primitive's own convention; all four corners are transformed by
    /// the active `Affine2` (not just the top-left -- the active
    /// transform can rotate) and the stored bounds are the real
    /// axis-aligned bounding box of those four transformed corners --
    /// a genuine axis-aligned rect an OS accessibility API can consume
    /// directly (AT-SPI2's `Component::GetExtents`, UIA's
    /// `BoundingRectangle`), not a naive reuse of the local
    /// width/height at a transformed origin, which would be wrong the
    /// moment the active transform includes a rotation.
    ///
    /// Deliberately does not intersect against the active clip stack --
    /// a disclosed, deliberate simplification (Step 5.3.1's own scope
    /// decision), not silently assumed away: a node partially scrolled
    /// out of view still reports its full transformed bounds.
    ///
    /// # Panics
    /// Never in practice -- see `save()`'s own `# Panics` section for why
    /// `state_stack` is never empty.
    pub fn tag_accessibility_node(
        &mut self,
        node_id: AccessibilityNodeId,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        role: AccessibilityRole,
    ) {
        let state = *self
            .state_stack
            .last()
            .expect("state_stack must always have at least one entry");
        let (x, y, width, height) = transform_bounds(&state, x, y, width, height);

        self.accessibility_nodes.push(AccessibilityNode {
            node_id,
            x,
            y,
            width,
            height,
            role,
        });
    }

    /// Every node tagged so far this frame via `tag_accessibility_node`
    /// (Phase 18 Step 18.3) -- a real caller publishes this to a real
    /// `tre_a11y::A11yBridge` once per rendered frame. A plain read-only
    /// getter over the field `tag_accessibility_node` already populates;
    /// nothing outside this crate's own internal `FlattenedFrame` path
    /// read it before this.
    #[must_use]
    pub fn accessibility_nodes(&self) -> &[AccessibilityNode] {
        &self.accessibility_nodes
    }

    /// Tags a real, transform-correct focusable widget at
    /// `(x, y, width, height)` (this canvas's own local space,
    /// transformed by whatever `save()`/`clip()`/`layer()` scope is
    /// currently active), for `FocusManager::focus_next`/
    /// `focus_previous` (Phase 19 Step 19.2) to traverse in Tab order.
    /// `node_id` is the same caller-assigned, stable identifier
    /// `tag_accessibility_node` already uses for this widget tree --
    /// deliberately not a second, parallel id system. `tab_index`
    /// follows the HTML `tabindex` convention; see
    /// `tre_engine::focus`'s own module doc for the full rule.
    ///
    /// # Panics
    /// Never in practice -- see `save()`'s own `# Panics` section for
    /// why `state_stack` is never empty.
    pub fn tag_focusable(
        &mut self,
        node_id: AccessibilityNodeId,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        tab_index: Option<i32>,
    ) {
        let state = *self
            .state_stack
            .last()
            .expect("state_stack must always have at least one entry");
        let (x, y, width, height) = transform_bounds(&state, x, y, width, height);

        self.focusable_nodes.push(FocusableNode {
            node_id,
            x,
            y,
            width,
            height,
            tab_index,
        });
    }

    /// Every node tagged so far this frame via `tag_focusable` (Phase 19
    /// Step 19.2) -- a real caller passes this to `FocusManager::
    /// focus_next`/`focus_previous`, read before `render_canvas()`
    /// consumes the canvas (same ordering rule `accessibility_nodes()`
    /// already established).
    #[must_use]
    pub fn focusable_nodes(&self) -> &[FocusableNode] {
        &self.focusable_nodes
    }

    /// Real sort/flatten stage (ARCHITECTURE.md Section 4.2, Step
    /// 5.1.3): every non-`DrawGeometry` command (`PushScissor`/
    /// `PopScissor`/`PushLayer`/`PopLayer`) is a hard barrier -- sorting
    /// and merging never reorders a draw across one (the conservative
    /// reading of Section 4.2's own "single draw call per layer plane"
    /// *soft* target, which explicitly defers cross-clip-boundary
    /// batching to a future, measurement-driven pass). Within each
    /// maximal marker-free run of `DrawGeometry` commands,
    /// [`flatten_run`] sorts by `sort_key` and merges adjacent commands
    /// sharing Layer+Pipeline+Texture (the key's top 44 bits) and
    /// identical `clip_bounds` into one, rewriting `indices` into a
    /// freshly built contiguous buffer (`vertices` never moves -- every
    /// index is an absolute reference into it, unaffected by which
    /// command it originally belonged to).
    ///
    /// # Panics
    /// In debug builds, panics if `push_layer`/`pop_layer` calls, `save`/
    /// `restore` calls, `push_clip`/`pop_clip` calls, or `begin_overlay`/
    /// `end_overlay` calls are unbalanced at frame boundary
    /// (IMPLEMENTATION.md Step 2.2 task 5, extended by Step 5.1.1/5.1.3
    /// to the three new stacks) -- an unreleased transient target,
    /// transform/alpha level, clip rect, or overlay scope otherwise
    /// leaks silently into the next frame instead of failing loudly at
    /// the actual bug. Compiled out in release builds along with the
    /// counters' checks.
    #[must_use]
    pub fn flatten(self) -> FlattenedFrame {
        debug_assert_eq!(
            self.layer_stack.len(),
            0,
            "push_layer/pop_layer calls are unbalanced at frame boundary"
        );
        debug_assert_eq!(
            self.state_stack.len(),
            1,
            "save/restore calls are unbalanced at frame boundary"
        );
        debug_assert!(
            self.clip_stack.is_empty(),
            "push_clip/pop_clip calls are unbalanced at frame boundary"
        );
        debug_assert!(
            self.overlay_stack.is_empty(),
            "begin_overlay/end_overlay calls are unbalanced at frame boundary"
        );

        let RenderingCanvas {
            vertices,
            indices,
            commands,
            accessibility_nodes,
            ..
        } = self;
        segment_and_flatten(vertices, &indices, commands, accessibility_nodes, true)
    }

    /// Identical to [`Self::flatten`] except it skips batch-merging
    /// entirely -- every recorded `DrawGeometry` command becomes its own
    /// separate output command, one draw call each, still ordered by
    /// the exact same real sort. Exists specifically for Phase 9 Step
    /// 9.1's own batching-equivalence test
    /// (`batching_equivalence_demo.rs`): rendering the identical scene
    /// through this and through [`Self::flatten`] isolates *batching*
    /// as the only variable that can differ between the two outputs,
    /// so a pixel mismatch between them indicates a real batching or
    /// sort-key bug, not a performance regression. A validation-only
    /// utility, not a production rendering path -- real callers always
    /// want [`Self::flatten`]'s own merged output.
    ///
    /// # Panics
    /// Same balance-assertion panics as [`Self::flatten`].
    #[must_use]
    pub fn flatten_unbatched(self) -> FlattenedFrame {
        debug_assert_eq!(
            self.layer_stack.len(),
            0,
            "push_layer/pop_layer calls are unbalanced at frame boundary"
        );
        debug_assert_eq!(
            self.state_stack.len(),
            1,
            "save/restore calls are unbalanced at frame boundary"
        );
        debug_assert!(
            self.clip_stack.is_empty(),
            "push_clip/pop_clip calls are unbalanced at frame boundary"
        );
        debug_assert!(
            self.overlay_stack.is_empty(),
            "begin_overlay/end_overlay calls are unbalanced at frame boundary"
        );

        let RenderingCanvas {
            vertices,
            indices,
            commands,
            accessibility_nodes,
            ..
        } = self;
        segment_and_flatten(vertices, &indices, commands, accessibility_nodes, false)
    }

    /// Merges this canvas's locally-recorded data into `arena` (Step
    /// 5.2.2), rebasing every index value and every command's
    /// `vertex_offset` to their new positions within `arena`'s own
    /// shared buffers. Safe to call concurrently with any other
    /// canvas's own `stitch_into` call against the same `arena`,
    /// including from a worker thread as its very last action before
    /// it exits (`tre_memory::ScatterArena::reserve`'s own lock-free
    /// contract is what makes this genuinely concurrent, not just
    /// safe).
    ///
    /// Returns `false` if any of the three reservations this needs
    /// (vertices, then indices rebased by the vertices reservation's
    /// own start, then commands rebased by the indices reservation's
    /// own start) would exceed `arena`'s fixed capacity -- this
    /// canvas's contribution to the frame is then incomplete (some or
    /// all of its content is missing from the final frame), not
    /// corrupted; `arena`'s own data for whatever *did* fit stays
    /// valid. Deciding what to do about an incomplete contribution
    /// (e.g. DESIGN.md Section 2.6's prioritized-degradation policy)
    /// is a future step's job, not this method's.
    ///
    /// Takes `&self`, not `self` -- Phase 9 Step 9.2 (REVIEW.md finding
    /// #134): this method only ever *copies* data out into `arena`'s own
    /// reserved slices (`copy_from_slice`, never a move), so it never
    /// actually needed ownership; the original consuming signature was
    /// incidental, not load-bearing. Borrowing is what lets a real
    /// caller `reset()` and reuse the same canvas across frames instead
    /// of constructing a fresh one every frame -- call `reset()`
    /// afterward to prepare this canvas for the next frame's recording.
    ///
    /// # Panics
    /// In debug builds, panics on the same unbalanced `save`/
    /// `push_clip`/`push_layer`/`begin_overlay` conditions
    /// `flatten`'s own `# Panics` section documents -- an unreleased
    /// state otherwise leaks into `arena`'s shared frame just as
    /// silently as it would have leaked into a lone `flatten()` call.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a single frame's vertex/index count stays far below u32::MAX, the same \
                   headroom reasoning ARCHITECTURE.md Section 4.1 applies to Depth ID"
    )]
    pub fn stitch_into(&self, arena: &FrameArena) -> bool {
        debug_assert_eq!(
            self.layer_stack.len(),
            0,
            "push_layer/pop_layer calls are unbalanced at frame boundary"
        );
        debug_assert_eq!(
            self.state_stack.len(),
            1,
            "save/restore calls are unbalanced at frame boundary"
        );
        debug_assert!(
            self.clip_stack.is_empty(),
            "push_clip/pop_clip calls are unbalanced at frame boundary"
        );
        debug_assert!(
            self.overlay_stack.is_empty(),
            "begin_overlay/end_overlay calls are unbalanced at frame boundary"
        );

        let vertices: &[UiVertex] = &self.vertices;
        let indices: &[u32] = &self.indices;
        let commands: &[UiDrawCommand] = &self.commands;
        let accessibility_nodes: &[AccessibilityNode] = &self.accessibility_nodes;

        let Some(mut vertex_slice) = arena.vertices.reserve(vertices.len()) else {
            return false;
        };
        vertex_slice.copy_from_slice(vertices);
        let vertex_base = vertex_slice.start_index() as u32;

        let Some(mut index_slice) = arena.indices.reserve(indices.len()) else {
            return false;
        };
        for (dest, &source) in index_slice.iter_mut().zip(indices) {
            *dest = source + vertex_base;
        }
        let index_base = index_slice.start_index() as u32;

        let Some(mut command_slice) = arena.commands.reserve(commands.len()) else {
            return false;
        };
        for (dest, &source) in command_slice.iter_mut().zip(commands) {
            *dest = UiDrawCommand {
                vertex_offset: source.vertex_offset + index_base,
                ..source
            };
        }

        // Accessibility nodes reference no position in any other array
        // (unlike indices/vertex_offset), so this is a literal bulk
        // copy -- no rebasing needed at all.
        let Some(mut accessibility_slice) =
            arena.accessibility_nodes.reserve(accessibility_nodes.len())
        else {
            return false;
        };
        accessibility_slice.copy_from_slice(accessibility_nodes);

        true
    }
}

/// Bundles the three shared `tre_memory::ScatterArena`s a real,
/// multi-source frame needs (Step 5.2.2): one for `vertices`, one for
/// `indices`, one for `commands`. Constructed once by the coordinating
/// thread before any worker thread is spawned; every `RenderingCanvas`/
/// `SubCanvas` that should contribute to the same final frame calls
/// `stitch_into` with a shared reference to the same `FrameArena`.
///
/// Capacities are fixed at construction and never grown mid-frame
/// (DESIGN.md Section 2.1) -- the caller decides them, the same way
/// every other pre-allocated pool in this codebase (the transient
/// render-target pool, the MPSC ring buffers) is caller-sized.
pub struct FrameArena {
    vertices: tre_memory::ScatterArena<UiVertex>,
    indices: tre_memory::ScatterArena<u32>,
    commands: tre_memory::ScatterArena<UiDrawCommand>,
    /// Step 5.3.1: `AccessibilityNode` is `Copy`, fitting the existing
    /// `ScatterArena` primitive unchanged -- merging tagged nodes needs
    /// no rebasing at all (unlike vertices/indices/commands), since
    /// nothing about one references a position in another array.
    accessibility_nodes: tre_memory::ScatterArena<AccessibilityNode>,
    /// Phase 9 Step 9.2 (REVIEW.md finding #134): persistent, reused
    /// scratch storage for `flatten_into`'s own sort/segment/merge pass
    /// only -- `flatten`/`flatten_unbatched` (the consuming, single-shot
    /// path) never touch these. `raw_commands`/`raw_indices` hold this
    /// frame's drained-but-not-yet-sorted commands/indices
    /// (`sort_and_batch_into`'s own input); `sort_scratch` is
    /// `radix_sort_by_key`'s own reused scratch buffer; `counts` is
    /// `radix_sort_by_key`'s own reused histogram buffer (a second real,
    /// previously-undetected per-call allocation this step's own new
    /// zero-allocation debug guard found and fixed, beyond the
    /// originally-planned `raw_commands`/`raw_indices`/`sort_scratch`).
    /// All four start empty and grow to this session's steady-state size
    /// across their first few calls, then never reallocate again.
    raw_commands: Vec<UiDrawCommand>,
    raw_indices: Vec<u32>,
    sort_scratch: Vec<UiDrawCommand>,
    counts: Vec<u32>,
}

impl FrameArena {
    #[must_use]
    pub fn with_capacity(
        vertex_capacity: usize,
        index_capacity: usize,
        command_capacity: usize,
        accessibility_capacity: usize,
    ) -> Self {
        Self {
            vertices: tre_memory::ScatterArena::with_capacity(vertex_capacity),
            indices: tre_memory::ScatterArena::with_capacity(index_capacity),
            commands: tre_memory::ScatterArena::with_capacity(command_capacity),
            accessibility_nodes: tre_memory::ScatterArena::with_capacity(accessibility_capacity),
            raw_commands: Vec::new(),
            raw_indices: Vec::new(),
            sort_scratch: Vec::new(),
            counts: Vec::new(),
        }
    }

    /// Consumes every stitched source's data and produces the real,
    /// sorted-and-merged `FlattenedFrame` -- the exact same Step 5.1.3
    /// algorithm `RenderingCanvas::flatten` uses (shared via
    /// `segment_and_flatten`), just fed from `arena`'s already-merged
    /// buffers instead of one canvas's own. Call only after every
    /// `stitch_into` call that could contribute to this frame has
    /// already returned -- typically "after every worker thread has
    /// been joined."
    #[must_use]
    pub fn flatten(self) -> FlattenedFrame {
        segment_and_flatten(
            self.vertices.into_vec(),
            &self.indices.into_vec(),
            self.commands.into_vec(),
            self.accessibility_nodes.into_vec(),
            true,
        )
    }

    /// The non-consuming, zero-allocation-in-steady-state sibling of
    /// `flatten()` -- Phase 9 Step 9.2 (REVIEW.md finding #134). Drains
    /// each internal `ScatterArena` (via `ScatterArena::drain_into`,
    /// which also resets it so this same `FrameArena` can be `reserve`d
    /// into again for the next frame) into `out`'s own fields and this
    /// arena's own persistent `raw_commands`/`raw_indices` scratch, then
    /// runs the identical sort/segment/merge pass `flatten()` does
    /// (`sort_and_batch_into`, the same core `segment_and_flatten`
    /// shares) into `out.commands`/`out.indices` (cleared first, kept
    /// capacity) using `sort_scratch` as the reused radix-sort scratch
    /// buffer. Always merges (`flatten()`'s own default) -- unlike
    /// `flatten`/`flatten_unbatched`, there is no unbatched sibling of
    /// this method: it exists for Step 9.2's own real, production reuse
    /// path, not Step 9.1's validation-only batching-equivalence test.
    ///
    /// A real caller builds one `FrameArena` and one `FlattenedFrame`
    /// once, before its own loop begins, and calls `flatten_into` every
    /// frame instead of reconstructing either -- once every internal
    /// buffer has grown to this session's steady-state size (typically
    /// within the first frame or two), no further call allocates at
    /// all. Call only after every `stitch_into` call that could
    /// contribute to this frame has already returned, exactly like
    /// `flatten()`.
    pub fn flatten_into(&mut self, out: &mut FlattenedFrame) {
        self.vertices.drain_into(&mut out.vertices);
        self.accessibility_nodes
            .drain_into(&mut out.accessibility_nodes);
        self.commands.drain_into(&mut self.raw_commands);
        self.indices.drain_into(&mut self.raw_indices);

        out.commands.clear();
        out.indices.clear();
        sort_and_batch_into(
            &mut self.raw_commands,
            &self.raw_indices,
            &mut self.sort_scratch,
            &mut self.counts,
            &mut out.commands,
            &mut out.indices,
            true,
        );
    }
}

/// `Canvas::draw_text`'s fixed MSDF atlas-entry resolution -- deliberately
/// independent of any on-screen `px_size` (the whole point of the MSDF
/// technique: one rasterized atlas entry serves any final display size),
/// matching the value `atlas_concurrency_demo`/`atlas_eviction_demo`
/// already established (Step 4.2.4/4.3.3).
const GLYPH_MSDF_SIZE: u32 = 32;
/// Pixels of margin around each glyph on every side within its
/// `GLYPH_MSDF_SIZE`-square atlas entry -- `generate_msdf`'s own
/// `range_px` parameter, same value as `GLYPH_MSDF_SIZE` above.
const GLYPH_MSDF_RANGE_PX: f64 = 4.0;

/// Scales all four of `color`'s channels (`UiVertex::color`'s established
/// little-endian `[r, g, b, a]` layout, see `rgba8`'s own doc comment) by
/// `factor`, rounding each to the nearest `u8` and clamping to `[0, 255]`
/// rather than wrapping -- `factor` is a product of possibly many nested
/// `set_alpha()` calls and could in principle exceed `1.0` or go negative
/// from a caller mistake; clamping keeps that a visually wrong but
/// harmless result, not a wrapped-around color channel.
///
/// Scaling R/G/B too, not just A, is load-bearing, not an approximation:
/// `sdf_rounded_rect.frag`'s own comment states "ARCHITECTURE.md Section
/// 6.1's blend state expects premultiplied alpha," and its output is
/// `vec4(frag_color.rgb * coverage, frag_color.a * coverage)` -- coverage
/// (the SDF anti-aliasing term) is the only factor ever multiplied into
/// `frag_color.rgb` there. A vertex color's *own* alpha reduction has to
/// already be premultiplied into its own RGB before it ever reaches that
/// shader, or the result is a genuinely over-bright premultiplied color
/// (`rgb / a > 1.0`) that the GPU silently clamps back to fully opaque --
/// confirmed by actually running `canvas_state_stack_demo` during this
/// step's own implementation: scaling only the alpha byte rendered Rect
/// B's 50%-alpha square as indistinguishable from fully opaque, not the
/// visible blend `Canvas::set_alpha` is supposed to produce.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "explicitly clamped to [0.0, 255.0] immediately before this cast"
)]
pub(crate) fn premultiply_alpha(color: u32, factor: f32) -> u32 {
    let [r, g, b, a] = color.to_le_bytes();
    let scale = |channel: u8| (f32::from(channel) * factor).round().clamp(0.0, 255.0) as u8;
    u32::from_le_bytes([scale(r), scale(g), scale(b), scale(a)])
}

/// The overlapping region of two scissor rects, in the same coordinate
/// space -- `Canvas::push_clip`'s own narrowing operation. An empty
/// (zero-area) result if the two rects don't overlap at all, not a
/// negative-size rect: `(x1 - x0)`/`(y1 - y0)` are clamped to `0` before
/// the final cast, so a caller pushing two disjoint clips gets a real,
/// harmless "clip everything" rect rather than a `u32` underflow.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "scissor rect coordinates and extents stay far below i32::MAX for any real window \
               size, and the width/height subtraction below is clamped to 0 before the final cast"
)]
fn intersect_scissor(a: ScissorRect, b: ScissorRect) -> ScissorRect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width as i32).min(b.x + b.width as i32);
    let y1 = (a.y + a.height as i32).min(b.y + b.height as i32);
    ScissorRect {
        x: x0,
        y: y0,
        width: (x1 - x0).max(0) as u32,
        height: (y1 - y0).max(0) as u32,
    }
}

/// TECHNICAL.md Section 8: "Max concurrent sub-canvases constrained to
/// `available_parallelism()` minus one." Falls back to a sane default
/// (4 cores) if the query itself errors -- rare, platform-specific, and
/// still preferable to silently forbidding every `SubCanvas` outright.
fn default_max_sub_canvases() -> usize {
    std::thread::available_parallelism()
        .map_or(4, std::num::NonZeroUsize::get)
        .saturating_sub(1)
}

/// `ARCHITECTURE.md` Section 4.1's Texture/Bindless ID field width (12
/// bits, 4,096 concurrent slots).
const TEXTURE_ID_MASK: u32 = 0xFFF;
/// `ARCHITECTURE.md` Section 4.1's Depth ID field width (20 bits,
/// 1,048,576 slots -- widened from 16 in the September 2026
/// documentation review).
pub(crate) const DEPTH_ID_MASK: u32 = 0xF_FFFF;

/// ARCHITECTURE.md Section 4.1's canonical 64-bit sort key:
/// `(LayerID<<48)|(PipelineID<<32)|(TextureID<<20)|(DepthID)`.
/// `layer_id`/`pipeline_state_id` are `u16` and always fit their 16-bit
/// fields exactly; `texture_handle`/`depth_id` are wider (`u32`) and
/// masked down to their documented 12-/20-bit ranges.
///
/// # Panics
/// In debug builds, panics if `texture_handle` exceeds the 12-bit
/// Texture ID field, or if `depth_id` exceeds the 20-bit Depth ID field
/// -- the latter is the exact debug assert ARCHITECTURE.md Section 4.1
/// itself documents as required ("the Canvas asserts in debug builds...
/// if a single frame's node count would still overflow 20 bits") before
/// either field would otherwise silently bleed into Layer/Pipeline's own
/// bits. Compiled out in release builds, matching this crate's
/// established balance-assertion precedent; the release-mode "splits
/// the offending layer's content into two sequential sub-frame passes"
/// behavior the same paragraph describes is not implemented here.
pub(crate) fn compute_sort_key(
    layer_id: u16,
    pipeline_state_id: u16,
    texture_handle: u32,
    depth_id: u32,
) -> u64 {
    debug_assert!(
        texture_handle <= TEXTURE_ID_MASK,
        "texture_handle ({texture_handle}) exceeds the 12-bit Texture ID field \
         (ARCHITECTURE.md Section 4.1)"
    );
    debug_assert!(
        depth_id <= DEPTH_ID_MASK,
        "depth_id ({depth_id}) exceeds the 20-bit Depth ID field (ARCHITECTURE.md Section 4.1) \
         -- a single frame's DrawGeometry count has overflowed the documented capacity"
    );
    (u64::from(layer_id) << 48)
        | (u64::from(pipeline_state_id) << 32)
        | (u64::from(texture_handle & TEXTURE_ID_MASK) << 20)
        | u64::from(depth_id & DEPTH_ID_MASK)
}

/// The shared segmentation-sort-merge core of both `RenderingCanvas::
/// flatten` and `FrameArena::flatten` (Step 5.2.2) -- takes plain,
/// already-assembled `vertices`/`indices`/`commands` rather than
/// `self`, so it doesn't care whether they came from one canvas's own
/// recording or from several sources a `FrameArena` already merged
/// together. Segments `commands` into maximal runs bounded by any
/// non-`DrawGeometry` command, running `flatten_run` over each --
/// identical to `RenderingCanvas::flatten`'s own Step 5.1.3 logic, just
/// extracted so it has exactly one implementation instead of two. Every
/// marker with no geometry of its own (`PushScissor`/`PopScissor`/
/// `PushLayer`, `element_count == 0`) passes through unchanged, as
/// before Step 6.4.2; a marker that *does* carry real geometry
/// (`PopLayer`'s own baked composite quad, Step 6.4.2) gets the exact
/// same indices-copy-and-rebase `flatten_run` already does for
/// `DrawGeometry` commands -- its `vertex_offset` is a position in the
/// caller's own raw `indices`, which this function's whole job is to
/// translate into a position in the freshly built `out_indices` the
/// returned `FlattenedFrame` actually carries; leaving it unrebased
/// would have `execute_frame`'s `draw_indexed` read from the wrong
/// buffer entirely.
/// The radix width `radix_sort_by_key` processes per pass -- 16 bits,
/// matching TECHNICAL.md Section 4 / ARCHITECTURE.md Section 4.1's own
/// "4-pass Radix Sort" for a 64-bit key (4 passes * 16 bits = 64 bits).
const RADIX_BITS: u32 = 16;
const RADIX_BUCKETS: usize = 1 << RADIX_BITS;
const RADIX_PASSES: u32 = 64 / RADIX_BITS;

/// A real least-significant-digit radix sort, ascending by `key_fn(item)`
/// -- 4 passes of a 16-bit digit each, $O(N)$ per pass (a counting sort:
/// one histogram pass, one prefix-sum pass, one scatter pass, all
/// linear in `items.len()` plus the fixed $2^{16}$-bucket overhead).
/// Replaces `flatten_run`'s own former `sort_unstable_by_key` call
/// (Phase 9 Step 9.1, REVIEW.md): TECHNICAL.md Section 4 has always
/// specified this exact algorithm for the 64-bit draw-command sort key
/// -- `UiDrawCommand::sort_key`'s own doc comment even already says
/// "64-bit Radix Sort Key" -- but no prior step actually built it.
///
/// `scratch` must have the same length as `items`; every pass scatters
/// into it and the two halves swap roles, so after an even number of
/// passes (4) the fully-sorted result ends up back in `items` with no
/// final copy needed. Callers own `scratch` and are expected to reuse
/// one buffer across many calls (`segment_and_flatten` allocates one
/// per frame, sized to the frame's own total command count, an upper
/// bound for any single run) rather than allocating fresh scratch space
/// per call -- this function itself never allocates.
///
/// Deliberately unconditional: no small-`N` fallback to a comparison
/// sort. TECHNICAL.md's own specification names this algorithm
/// unconditionally, and Step 9.1 is a correctness pass, not a
/// performance-tuning one (TECHNICAL.md Section 9.2's own benchmark
/// suite owns that) -- a hybrid crossover threshold would be a real
/// performance decision with no measured data behind it yet. A real,
/// deliberate, disclosed scope boundary, not an oversight.
///
/// Stable (ties keep their original relative order): each pass's
/// scatter step preserves relative order among equal digits, and LSD
/// radix sort's own correctness proof relies on every pass being
/// stable, from the least-significant digit up. `flatten_run`'s own
/// real keys are always unique within one frame in practice
/// (`RenderingCanvas::next_depth_id` never repeats or resets), so no
/// real caller depends on this today -- it falls out of the algorithm
/// for free, not because anything requires it.
///
/// # Panics
/// Panics if `scratch.len() != items.len()`.
#[allow(
    clippy::cast_possible_truncation,
    reason = "digit_of's own result is always masked to RADIX_BUCKETS - 1 (0xFFFF) before the \
               cast, provably in range for usize on every real target this project builds for"
)]
pub(crate) fn radix_sort_by_key<T: Copy>(
    items: &mut [T],
    scratch: &mut [T],
    counts: &mut Vec<u32>,
    key_fn: impl Fn(&T) -> u64,
) {
    assert_eq!(
        items.len(),
        scratch.len(),
        "radix_sort_by_key: scratch must be exactly as long as items"
    );
    if items.len() <= 1 {
        return;
    }

    // `counts[d]` becomes, after the prefix-sum step, the first output
    // index for digit `d` -- one extra slot isn't needed since digits
    // run `0..RADIX_BUCKETS` and the prefix sum is computed in place,
    // left to right, before any scatter reads it.
    //
    // Phase 9 Step 9.2 (real bug found by the new zero-allocation debug
    // guard, TECHNICAL.md Section 3.4): this used to be `vec![0u32;
    // RADIX_BUCKETS]`, allocated fresh on *every single call* -- a real,
    // previously-undetected per-run heap allocation Step 9.1's own
    // "allocate once, reuse across the frame" discipline had already
    // applied to `scratch` but missed here. `counts` is now caller-
    // provided and grown at most once ever (`RADIX_BUCKETS` is a fixed
    // compile-time constant, so a persistent `counts` `Vec` reaches its
    // final length on its very first call and never resizes again).
    if counts.len() < RADIX_BUCKETS {
        counts.resize(RADIX_BUCKETS, 0);
    }

    let mut src: &mut [T] = items;
    let mut dst: &mut [T] = scratch;
    for pass in 0..RADIX_PASSES {
        let shift = pass * RADIX_BITS;
        let digit_of = |item: &T| ((key_fn(item) >> shift) & (RADIX_BUCKETS as u64 - 1)) as usize;

        counts.fill(0);
        for item in src.iter() {
            counts[digit_of(item)] += 1;
        }
        let mut running = 0u32;
        for count in counts.iter_mut() {
            let this_bucket = *count;
            *count = running;
            running += this_bucket;
        }
        for item in src.iter() {
            let digit = digit_of(item);
            dst[counts[digit] as usize] = *item;
            counts[digit] += 1;
        }

        std::mem::swap(&mut src, &mut dst);
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "a single frame's index count stays far below u32::MAX, the same headroom \
               reasoning ARCHITECTURE.md Section 4.1 applies to Depth ID"
)]
fn segment_and_flatten(
    vertices: Vec<UiVertex>,
    indices: &[u32],
    mut commands: Vec<UiDrawCommand>,
    accessibility_nodes: Vec<AccessibilityNode>,
    merge: bool,
) -> FlattenedFrame {
    let mut out_commands = Vec::with_capacity(commands.len());
    let mut out_indices = Vec::with_capacity(indices.len());
    let mut scratch = Vec::new();
    let mut counts = Vec::new();
    sort_and_batch_into(
        &mut commands,
        indices,
        &mut scratch,
        &mut counts,
        &mut out_commands,
        &mut out_indices,
        merge,
    );

    FlattenedFrame {
        vertices,
        indices: out_indices,
        commands: out_commands,
        accessibility_nodes,
    }
}

/// The neutral placeholder `flatten_run`'s own scratch buffer is filled
/// with before `radix_sort_by_key` overwrites every element -- factored
/// out since Phase 9 Step 9.2's `sort_and_batch_into` needs it wherever
/// its own `scratch` buffer must grow, the same reason `segment_and_
/// flatten` (Step 9.1) originally needed it inline.
fn neutral_draw_command() -> UiDrawCommand {
    UiDrawCommand {
        kind: CommandType::DrawGeometry,
        sort_key: 0,
        pipeline_state_id: 0,
        texture_handle: 0,
        element_count: 0,
        vertex_offset: 0,
        clip_bounds: ScissorRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        },
    }
}

/// The real sort/segment/merge core (Step 5.1.3/9.1), factored out here
/// (Phase 9 Step 9.2, REVIEW.md finding #134) so it can be shared by
/// both the consuming, single-shot `segment_and_flatten` and the
/// reusable, zero-allocation-in-steady-state `FrameArena::flatten_into`
/// -- the algorithm itself is identical either way; only whether
/// `out_commands`/`out_indices`/`scratch` start genuinely empty or
/// pre-cleared-but-warm (kept capacity from a prior call) differs.
///
/// `scratch` is grown (`Vec::resize`, which only ever truncates or
/// extends -- never shrinks its own backing capacity) to at least
/// `commands.len()` if it isn't already that long; a caller that reuses
/// the same `scratch` across many calls at a stable-or-shrinking
/// command count therefore only ever reallocates on the call that first
/// reaches this session's steady-state command count, never again after.
#[allow(
    clippy::cast_possible_truncation,
    reason = "a single frame's index count stays far below u32::MAX, the same headroom \
               reasoning ARCHITECTURE.md Section 4.1 applies to Depth ID"
)]
fn sort_and_batch_into(
    commands: &mut [UiDrawCommand],
    indices: &[u32],
    scratch: &mut Vec<UiDrawCommand>,
    counts: &mut Vec<u32>,
    out_commands: &mut Vec<UiDrawCommand>,
    out_indices: &mut Vec<u32>,
    merge: bool,
) {
    if scratch.len() < commands.len() {
        scratch.resize(commands.len(), neutral_draw_command());
    }
    let mut run_start = 0;
    for i in 0..=commands.len() {
        let at_boundary = i == commands.len() || commands[i].kind != CommandType::DrawGeometry;
        if !at_boundary {
            continue;
        }
        let run_len = i - run_start;
        flatten_run(
            &mut commands[run_start..i],
            &mut scratch[..run_len],
            counts,
            indices,
            out_commands,
            out_indices,
            merge,
        );
        if i < commands.len() {
            let mut boundary_command = commands[i];
            if boundary_command.element_count > 0 {
                let rebased_offset = out_indices.len() as u32;
                out_indices.extend_from_slice(command_indices(indices, &boundary_command));
                boundary_command.vertex_offset = rebased_offset;
            }
            out_commands.push(boundary_command);
        }
        run_start = i + 1;
    }
}

/// `source_indices[command.vertex_offset..][..command.element_count]` --
/// `flatten_run`'s own accessor for one command's original index slice,
/// named to make each call site read as "this command's indices," not a
/// bare range expression.
fn command_indices<'a>(source_indices: &'a [u32], command: &UiDrawCommand) -> &'a [u32] {
    let start = command.vertex_offset as usize;
    let end = start + command.element_count as usize;
    &source_indices[start..end]
}

/// Sorts one marker-free run of `DrawGeometry` commands by `sort_key`
/// via `radix_sort_by_key` (Phase 9 Step 9.1 -- TECHNICAL.md Section
/// 4's own always-specified algorithm, real as of this step; stable
/// regardless, though nothing relies on that -- see its own doc
/// comment), then, when `merge` is set, merges adjacent commands
/// sharing Layer+Pipeline+Texture (the key's top 44 bits, `sort_key >>
/// 20`) and identical `clip_bounds` into one -- ARCHITECTURE.md Section
/// 4.2's own three-step batch-flattening algorithm. When `merge` is
/// unset, every command is emitted as its own separate output instead
/// -- `RenderingCanvas::flatten_unbatched`'s own real consumer (Phase 9
/// Step 9.1's batching-equivalence test), isolating *batching*
/// specifically as the only variable that differs from the real
/// `flatten()` path, since both still use the identical real sort.
/// Either way, every emitted command's original (possibly
/// non-contiguous, when merged) index slice is concatenated into
/// `out_indices`, the actual contiguous buffer the RHI will read from
/// -- each output command's `vertex_offset` refers to a position in
/// `out_indices`, never `source_indices`.
///
/// `sort_scratch` must be exactly `run.len()` long -- see
/// `radix_sort_by_key`'s own doc comment for why it's the caller's own
/// reused buffer, not allocated here. `counts` is `radix_sort_by_key`'s
/// own reused histogram buffer (Phase 9 Step 9.2) -- same reasoning.
#[allow(
    clippy::cast_possible_truncation,
    reason = "a single frame's index count stays far below u32::MAX, the same headroom \
               reasoning ARCHITECTURE.md Section 4.1 applies to Depth ID"
)]
pub(crate) fn flatten_run(
    run: &mut [UiDrawCommand],
    sort_scratch: &mut [UiDrawCommand],
    counts: &mut Vec<u32>,
    source_indices: &[u32],
    out_commands: &mut Vec<UiDrawCommand>,
    out_indices: &mut Vec<u32>,
    merge: bool,
) {
    radix_sort_by_key(run, sort_scratch, counts, |command| command.sort_key);

    let mut remaining = run.iter();
    let Some(&first) = remaining.next() else {
        return;
    };
    let mut current = first;
    let mut current_offset = out_indices.len() as u32;
    out_indices.extend_from_slice(command_indices(source_indices, &current));

    for &command in remaining {
        let same_batch = merge
            && command.sort_key >> 20 == current.sort_key >> 20
            && command.clip_bounds == current.clip_bounds;
        if same_batch {
            out_indices.extend_from_slice(command_indices(source_indices, &command));
            current.element_count += command.element_count;
        } else {
            out_commands.push(UiDrawCommand {
                vertex_offset: current_offset,
                ..current
            });
            current = command;
            current_offset = out_indices.len() as u32;
            out_indices.extend_from_slice(command_indices(source_indices, &current));
        }
    }
    out_commands.push(UiDrawCommand {
        vertex_offset: current_offset,
        ..current
    });
}
