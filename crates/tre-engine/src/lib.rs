//! Core engine crate: the `Canvas` API, intermediate representation,
//! sort/batch pipeline, dynamic texture atlas, and SVG/MSDF tessellation.
//!
//! Pure safe Rust -- see TECHNICAL.md Section 9.1 for the workspace's
//! `unsafe` policy. Raw graphics-API FFI lives in the `tre-rhi-*` crates,
//! and zero-allocation buffer/arena/atlas-concurrency primitives live in
//! `tre-memory`; this crate depends on both but contains no `unsafe` itself.
#![forbid(unsafe_code)]

/// Recoverable engine failure (DESIGN.md Section 2.6). Every fallible
/// engine operation returns `Result<T, EngineError>`; panics are reserved
/// for programmer errors, never for these expected failure modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineError {
    /// GPU device removal, driver TDR, or an out-of-date swapchain
    /// (DESIGN.md Section 2.6, "Device loss / swapchain acquire failure").
    DeviceLost,
    /// The swapchain no longer matches the window (e.g. after a resize)
    /// and must be recreated before rendering can continue.
    SwapchainOutOfDate,
    /// A graphics pipeline failed to create (DESIGN.md Section 2.6,
    /// "Shader compilation / pipeline creation failure").
    PipelineCreationFailed,
    /// `RhiDevice::create_texture`'s `pixels` slice length doesn't match
    /// what `width`/`height`/`format` implies, or `width`/`height` is zero
    /// (Phase 2 Code Review finding #66) -- caught before any GPU call, so
    /// no out-of-bounds read into `pixels` or its staging buffer can occur.
    InvalidTextureData,
    /// The RHI's persistent bindless texture array (IMPLEMENTATION.md
    /// Step 2.1) has no free slots left (Phase 2 Code Review finding #67;
    /// DESIGN.md Section 2.6's "atlas exhaustion beyond LRU capacity"
    /// failure class). Recoverable in principle -- a caller can release
    /// textures and retry -- even though no eviction policy exists yet.
    BindlessArrayExhausted,
    /// `RhiDevice::acquire_transient_target` would need to cold-allocate a
    /// genuinely novel size while the transient pool's already-idle free
    /// bytes alone are at or past the dynamic-VRAM budget (Phase 2
    /// Step 2.3 Code Review finding #80; TECHNICAL.md Section 3.3's
    /// generational GC only reclaims idle entries -- it cannot claw back
    /// budget from a caller that keeps enough distinct sizes in rotation
    /// to never go idle, so admission needs its own check). Recoverable:
    /// a caller can release outstanding textures, wait for the GC thread
    /// to catch up, and retry.
    TransientPoolBudgetExceeded,
}

/// A clip rectangle in the coordinate space `Canvas::push_clip`/scissor
/// operations use. Referenced but never defined by ARCHITECTURE.md's
/// `UiDrawCommand`/`RhiCommandBuffer::set_scissor` sketch -- defined here.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct ScissorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The canonical 32-byte UI vertex (ARCHITECTURE.md Section 3.1).
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    pub position: [f32; 2], // 8 bytes:  Screen-Space X, Y
    pub uv: [f32; 2],       // 8 bytes:  Texture coordinates or SDF bounds
    pub color: u32,         // 4 bytes:  Packed RGBA8 (sRGB converted to Linear in shader)
    pub params: [f32; 3],   // 12 bytes: Shader params (Corner Radii, Stroke Width, etc.)
} // 32 Bytes Total

const _: () = assert!(std::mem::size_of::<UiVertex>() == 32);

/// Packs 8-bit RGBA channels into `UiVertex::color`'s `u32` in the byte
/// order the vertex format (`R8G8B8A8_UNORM`) expects in memory.
///
/// This exists because a `u32` hex literal does NOT give you this for
/// free: `0xE0_A0_40_FFu32` stored little-endian on `x86_64`/`ARM64` places
/// `0xFF` (the *last* two hex digits) at the *lowest* memory address, so a
/// literal written in visual "RRGGBBAA" order actually produces memory
/// bytes `[AA, BB, GG, RR]` -- the reverse of what `R8G8B8A8` expects.
/// `rgba8` does the correct packing so callers never have to hand-reverse
/// the byte order themselves.
#[must_use]
pub const fn rgba8(r: u8, g: u8, b: u8, a: u8) -> u32 {
    u32::from_le_bytes([r, g, b, a])
}

/// The IR command kind (ARCHITECTURE.md Section 3.2).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandType {
    DrawGeometry,
    PushScissor,
    PopScissor,
    PushLayer,
    PopLayer,
}

/// The canonical intermediate-representation draw command
/// (ARCHITECTURE.md Section 3.2).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UiDrawCommand {
    pub kind: CommandType, // `type` is a reserved keyword in Rust
    pub sort_key: u64,     // 64-bit Radix Sort Key
    pub pipeline_state_id: u16,
    pub texture_handle: u32, // Bindless array index or atlas handle
    pub element_count: u32,  // Index count
    pub vertex_offset: u32,  // Offset into the dynamic ring buffer
    pub clip_bounds: ScissorRect,
}

/// `UiDrawCommand::pipeline_state_id` for `Canvas::draw_text`'s MSDF
/// glyph quads (IMPLEMENTATION.md Step 5.1.2) -- the first real
/// distinction between pipeline ids in the IR; `draw_rounded_rect`'s own
/// commands keep the implicit `0` (the SDF-rect pipeline). Nothing
/// downstream reads either value yet (Step 5.1.3's real batch flattening
/// is what would), the same up-front honesty already established for
/// `sort_key: 0`.
pub const PIPELINE_MSDF_TEXT: u16 = 1;

/// Opaque identifier for a platform window, assigned by
/// `tre-platform`'s `PlatformConnection` when a window is created
/// (IMPLEMENTATION.md Step 1.2). Stable for that window's lifetime and
/// never reused while the owning connection is alive, so it is safe to use
/// as a stable map key (e.g. per-window swapchain lookup) rather than a
/// raw pointer or index that could be invalidated by window closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u64);

/// A pointer button (TECHNICAL.md Section 8's input event model).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    /// A raw platform button code for buttons beyond the three common
    /// ones (e.g. side/forward-back buttons), passed through unchanged.
    Other(u16),
}

/// Whether a button or key was pressed or released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementState {
    Pressed,
    Released,
}

/// A single translated, backend-agnostic event flowing from the platform
/// layer to the engine (TECHNICAL.md Section 8): the SPSC ring buffer's
/// payload type. Every variant carries the [`WindowId`] it originated
/// from so a multi-window application can route events without querying
/// per-backend state -- this is also why window lifecycle events
/// (`CloseRequested`, `Resized`) live here rather than in a separate
/// per-window enum: `PlatformConnection` now owns multiple windows behind
/// one shared connection, so every event it produces needs the same
/// window-tagging regardless of category.
///
/// `PointerMoved` events are coalesced by [`InputEventQueue`]
/// (IMPLEMENTATION.md Step 1.2): a burst of raw OS motion events for the
/// same window collapses to the single most recent position, so a slow
/// consumer never falls behind on stale mouse positions the way it could
/// on discrete clicks or key presses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputEvent {
    PointerMoved {
        window: WindowId,
        x: f64,
        y: f64,
    },
    PointerButton {
        window: WindowId,
        button: MouseButton,
        state: ElementState,
    },
    /// `key_code` is the raw platform key code (Linux evdev keycode on
    /// both Wayland and X11, per `wl_keyboard`'s and X11 `KeyCode`'s
    /// shared evdev-based numbering) -- layout-aware translation is a UI
    /// framework concern (DESIGN.md Section 2.7), out of scope here.
    KeyboardKey {
        window: WindowId,
        key_code: u32,
        state: ElementState,
    },
    CloseRequested {
        window: WindowId,
    },
    Resized {
        window: WindowId,
        width: u32,
        height: u32,
    },
}

/// Producer-side queue wrapping `tre_memory::SpscRingBuffer<InputEvent>`
/// with pointer-move coalescing (TECHNICAL.md Section 8, IMPLEMENTATION.md
/// Step 1.2): a `PointerMoved` for the same window as the currently
/// staged pending move overwrites that staged value instead of being
/// published as a new queue entry, so a burst of high-frequency raw OS
/// motion events collapses to the single most recent position by the
/// time a consumer drains the queue.
///
/// The staged value lives in this producer-exclusive struct field, never
/// in an already-published ring-buffer slot -- overwriting a *published*
/// slot in place would race a concurrent consumer that might be mid-read
/// of that exact slot (true whenever the queue holds exactly one
/// unconsumed item). Staging it here instead keeps the underlying
/// `SpscRingBuffer` itself untouched by this coalescing logic, so it
/// stays sound if a real second consumer thread is ever introduced,
/// matching that type's own "no redesign needed" design goal.
pub struct InputEventQueue {
    queue: tre_memory::SpscRingBuffer<InputEvent>,
    pending_move: Option<InputEvent>, // always `PointerMoved` when `Some`
}

impl InputEventQueue {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: tre_memory::SpscRingBuffer::with_capacity(capacity),
            pending_move: None,
        }
    }

    /// Producer-side: enqueues `event`, coalescing consecutive
    /// `PointerMoved`s for the same window per this type's doc comment.
    /// A full underlying queue silently drops the event rather than
    /// blocking or panicking (DESIGN.md Section 2.6): input events are a
    /// UI convenience, never something worth stalling a render frame for.
    pub fn push(&mut self, event: InputEvent) {
        if let InputEvent::PointerMoved { window, .. } = event {
            let coalesces = matches!(
                self.pending_move,
                Some(InputEvent::PointerMoved { window: pending_window, .. })
                    if pending_window == window
            );
            if !coalesces {
                self.flush_pending_move();
            }
            self.pending_move = Some(event);
            return;
        }
        self.flush_pending_move();
        let _ = self.queue.push(event);
    }

    /// Publishes the currently staged pending move, if any. Callers
    /// should call this once per polling cycle after translating all
    /// available raw OS events, so a move isn't left stuck in staging
    /// with nothing left to flush it this cycle.
    pub fn flush_pending_move(&mut self) {
        if let Some(event) = self.pending_move.take() {
            let _ = self.queue.push(event);
        }
    }

    /// Non-blocking drain (this step's stand-in for a real cross-thread
    /// consumer, per `PLAN.md`'s scope decision): flushes any pending
    /// move, then pops every currently queued event into a `Vec`.
    #[must_use]
    pub fn drain(&mut self) -> Vec<InputEvent> {
        self.flush_pending_move();
        std::iter::from_fn(|| self.queue.pop()).collect()
    }
}

/// An engine-level, backend-agnostic pixel format for transient render
/// targets (TECHNICAL.md Section 3.2's `(Width, Height, Format)` pool
/// key), swapchains, and (Phase 4 Step 4.2.3) regular uploaded textures.
/// `Bgra8Srgb`/`Rgba16Float` match TECHNICAL.md Section 6.1's two
/// swapchain formats -- SDR and HDR -- since transient offscreen targets
/// need to match whichever pipeline is compositing them; `Rgba8Unorm` is
/// for texture data that is never a color at all (an MSDF texel is a
/// distance encoding) and must never pass through an `_SRGB` format's
/// automatic gamma transform, which would corrupt it at every value
/// except the two endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    /// Standard dynamic range: `VK_FORMAT_B8G8R8A8_SRGB` /
    /// `DXGI_FORMAT_B8G8R8A8_UNORM_SRGB`.
    Bgra8Srgb,
    /// High dynamic range / wide gamut: `VK_FORMAT_R16G16B16A16_SFLOAT` /
    /// `DXGI_FORMAT_R16G16B16A16_FLOAT`.
    Rgba16Float,
    /// Linear, no gamma anywhere in the read path: `VK_FORMAT_R8G8B8A8_UNORM`
    /// / `DXGI_FORMAT_R8G8B8A8_UNORM`. For non-color texture data --
    /// Step 4.2.3's MSDF glyph textures are the first user.
    Rgba8Unorm,
}

/// Which pixels inside a (possibly self-intersecting) path's boundary
/// count as "filled" (IMPLEMENTATION.md Step 3.3 task 3). Backend-agnostic
/// -- a stencil-and-cover renderer encodes each rule as different GPU
/// stencil-buffer operations, but the rule itself is a property of the
/// path, not of any one backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule {
    /// A point is inside if the path's signed winding number around it is
    /// nonzero. Encoded in stencil as a genuine per-triangle increment/
    /// decrement counter (two-sided: opposite ops for front- and
    /// back-facing fan triangles).
    NonZero,
    /// A point is inside if a ray from it to infinity crosses the path's
    /// boundary an odd number of times. Encoded in stencil as a single
    /// `INVERT` op per fan triangle, regardless of triangle winding.
    EvenOdd,
}

/// Describes an offscreen compositing layer requested via
/// `RenderingCanvas::push_layer` (DESIGN.md Section 6.2). Minimal for
/// now -- opacity/blend-mode/blur-radius fields belong here once a later
/// phase implements those visual filters (DESIGN.md Section 6.2's
/// "Visual Filter Pipeline"); this step only needs enough to acquire a
/// correctly-sized, correctly-formatted transient render target from the
/// pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerDesc {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

/// A frame's fully-recorded, sorted-and-flattened batch: one contiguous
/// vertex/index stream plus the (currently trivial, Phase 0) list of
/// draw commands describing how to slice it into RHI draw calls.
pub struct FlattenedFrame {
    pub vertices: Vec<UiVertex>,
    pub indices: Vec<u32>,
    pub commands: Vec<UiDrawCommand>,
}

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
    vertices: Vec<UiVertex>,
    indices: Vec<u32>,
    commands: Vec<UiDrawCommand>,
    /// Debug-only balance counter for `push_layer`/`pop_layer`
    /// (IMPLEMENTATION.md Step 2.2 task 5): incremented/decremented on
    /// each call, asserted zero at `flatten()`. Compiles to a plain
    /// unused `u32` in release builds rather than being cfg'd out
    /// entirely, so `push_layer`/`pop_layer`'s own bodies don't need
    /// separate debug/release code paths.
    layer_depth: u32,
    /// The Drawing Context's transform/alpha stack (Step 5.1.1). Always
    /// has at least one entry -- the base level `save`/`restore` can
    /// never pop past -- so every read of `.last()` is infallible by
    /// construction, not just by convention.
    state_stack: Vec<CanvasState>,
    /// The independent scissor-clip stack (Step 5.1.1). Empty means "no
    /// clip, full window" -- the same sentinel `draw_rounded_rect`
    /// already used unconditionally before this step. Its own length
    /// doubles as the balance counter `flatten()` checks; no separate
    /// counter field is needed the way `layer_depth` needs one, since
    /// `push_layer`/`pop_layer` record no per-call stack data of their
    /// own to count instead.
    clip_stack: Vec<ScissorRect>,
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
            ..Self::default()
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

    /// Records a `PushLayer` IR marker (DESIGN.md Section 6.2) and
    /// increments the debug balance counter. Does not itself acquire a
    /// transient render target -- see this struct's doc comment for why
    /// that wiring is deferred; `desc` is recorded for a future RHI
    /// execution stage to act on.
    pub fn push_layer(&mut self, desc: &LayerDesc) {
        self.layer_depth += 1;
        self.commands.push(UiDrawCommand {
            kind: CommandType::PushLayer,
            sort_key: 0,
            pipeline_state_id: 0,
            texture_handle: 0,
            element_count: 0,
            vertex_offset: 0,
            clip_bounds: ScissorRect {
                x: 0,
                y: 0,
                width: desc.width,
                height: desc.height,
            },
        });
    }

    /// Records a `PopLayer` IR marker and decrements the debug balance
    /// counter.
    ///
    /// # Panics
    /// Panics if called without a matching prior `push_layer` -- an
    /// unbalanced push/pop is a programmer error (DESIGN.md Section 2.6),
    /// not a recoverable runtime condition.
    pub fn pop_layer(&mut self) {
        self.layer_depth = self
            .layer_depth
            .checked_sub(1)
            .expect("pop_layer called without a matching push_layer");
        self.commands.push(UiDrawCommand {
            kind: CommandType::PopLayer,
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

        let clip_bounds = self.clip_stack.last().copied().unwrap_or(ScissorRect {
            x: 0,
            y: 0,
            width: u32::MAX,
            height: u32::MAX,
        });
        self.commands.push(UiDrawCommand {
            kind: CommandType::DrawGeometry,
            sort_key: 0, // Phase 0: single command, sorts itself.
            pipeline_state_id: 0,
            texture_handle: 0,
            element_count: 6,
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
    /// A glyph whose outline has no real ink (whitespace) is skipped
    /// entirely: no atlas interaction, no emitted quad, pen still
    /// advances -- `tre_text::msdf`'s own doc comment already
    /// establishes that this is the common case for e.g. U+0020 SPACE,
    /// not an error. A glyph not yet resident in the atlas fires a real
    /// `request_insert` (ignoring a `false`/queue-full return -- "report,
    /// don't block," DESIGN.md Section 2.6) and renders nothing this
    /// frame; a resident glyph emits one real textured `DrawGeometry`
    /// command, current transform/alpha/clip state applied exactly as
    /// `draw_rounded_rect` already does.
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
        let clip_bounds = self.clip_stack.last().copied().unwrap_or(ScissorRect {
            x: 0,
            y: 0,
            width: u32::MAX,
            height: u32::MAX,
        });

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
                if !outline.is_empty() {
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

        self.commands.push(UiDrawCommand {
            kind: CommandType::DrawGeometry,
            sort_key: 0, // Step 5.1.3: real sort key.
            pipeline_state_id: PIPELINE_MSDF_TEXT,
            texture_handle: atlas_context.texture_handle,
            element_count: 6,
            vertex_offset: base_index,
            clip_bounds,
        });
    }

    /// Phase 0 stub for the sort/flatten stage (ARCHITECTURE.md Section 4):
    /// a trivial pass-through for the single-command case. The real 4-pass
    /// radix sort (IMPLEMENTATION.md Phase 6) has nothing to do yet when
    /// there is only ever one element -- "one element sorts itself"
    /// (IMPLEMENTATION.md Phase 0, task 3).
    /// # Panics
    /// In debug builds, panics if `push_layer`/`pop_layer` calls, `save`/
    /// `restore` calls, or `push_clip`/`pop_clip` calls are unbalanced at
    /// frame boundary (IMPLEMENTATION.md Step 2.2 task 5, extended by Step
    /// 5.1.1 to the two new stacks) -- an unreleased transient target,
    /// transform/alpha level, or clip rect otherwise leaks silently into
    /// the next frame instead of failing loudly at the actual bug.
    /// Compiled out in release builds along with the counters' checks.
    #[must_use]
    pub fn flatten(self) -> FlattenedFrame {
        debug_assert_eq!(
            self.layer_depth, 0,
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
        FlattenedFrame {
            vertices: self.vertices,
            indices: self.indices,
            commands: self.commands,
        }
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
fn premultiply_alpha(color: u32, factor: f32) -> u32 {
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

/// An acquired swapchain image, handed from `RhiSwapchain::acquire_next_image`
/// through `RhiDevice::begin_frame` to the caller and back to
/// `RhiDevice::submit_and_present`.
///
/// `target_view_handle` is a backend-specific opaque handle (e.g. a Vulkan
/// `vk::ImageView` reinterpreted via `ash::vk::Handle::as_raw`). This is
/// deliberately an opaque integer, not a trait-object downcast: passing an
/// already-opaque handle through a generic interface is not the "dynamic
/// type inspection" TECHNICAL.md Section 9.1 bans from the per-frame path
/// -- no runtime type identification happens anywhere in this exchange,
/// only a backend re-interpreting a handle it produced itself moments
/// earlier. This mirrors how Vulkan itself represents every object as an
/// opaque `u64`.
#[derive(Debug, Clone, Copy)]
pub struct AcquiredImage {
    pub index: u32,
    pub target_view_handle: u64,
    /// The raw target image itself (distinct from its view), needed for
    /// layout-transition barriers around dynamic rendering.
    pub target_image_handle: u64,
    /// Opaque handle of the semaphore `acquire_next_image` signaled;
    /// `RhiDevice::submit_and_present`'s queue submit waits on it.
    pub image_available_semaphore_handle: u64,
    /// Opaque handle of the *per-swapchain-image* semaphore
    /// `RhiDevice::submit_and_present`'s queue submit signals, and
    /// `RhiSwapchain::present` waits on before showing this image.
    /// Per-image, not shared across frames: reusing one semaphore for
    /// every frame's present is a real hazard the Vulkan validation layer
    /// catches (VUID-vkQueueSubmit-pSignalSemaphores-00067) -- the CPU-side
    /// fence this engine waits on covers the queue submit's completion,
    /// not the separate, asynchronous present operation's.
    pub render_finished_semaphore_handle: u64,
}

/// A GPU buffer (vertex, index, or uniform). ARCHITECTURE.md Section 6
/// references `&dyn RhiBuffer` in `RhiCommandBuffer` but never defines
/// this trait's own methods -- defined here using the same opaque-handle
/// pattern as `AcquiredImage`.
pub trait RhiBuffer {
    fn raw_handle(&self) -> u64;
}

/// A GPU texture (atlas page or offscreen render target). Referenced but
/// undefined by ARCHITECTURE.md Section 6; defined here.
///
/// Exposes every raw handle a backend needs to reconstruct its own
/// concrete texture type from a `Box<dyn RhiTexture>` -- e.g.
/// `RhiDevice::release_transient_target` receives one back from a caller
/// and must recover enough to store/eventually destroy it. This is the
/// same opaque-handle-reinterpretation pattern `AcquiredImage` already
/// uses (multiple named `u64` fields, not one), not a downcast:
/// TECHNICAL.md Section 9.1 bans dynamic type inspection in the per-frame
/// path, but a backend re-interpreting a handle it produced itself
/// moments earlier is not that.
pub trait RhiTexture {
    /// The image view -- what a shader binds/samples.
    fn raw_handle(&self) -> u64;
    /// The underlying image (distinct from its view), needed for layout
    /// barriers and for destroying the image itself.
    fn image_handle(&self) -> u64;
    /// The backing device memory, needed to free it on destruction.
    fn memory_handle(&self) -> u64;
    /// The texture's actual dimensions -- may be *larger* than what a
    /// caller requested from `RhiDevice::acquire_transient_target` on a
    /// transient-pool cache miss (TECHNICAL.md Section 3.2's next-larger
    /// fallback, DESIGN.md Section 2.6), so callers must consult this
    /// rather than assume it matches their request.
    fn dimensions(&self) -> (u32, u32);
    fn format(&self) -> TextureFormat;
    /// This texture's slot in the RHI's persistent bindless texture array
    /// (IMPLEMENTATION.md Step 2.1), usable directly as
    /// `RhiCommandBuffer::bind_texture`'s `bindless_index` argument. `None`
    /// for a transient render target (`RhiDevice::acquire_transient_target`)
    /// -- those are written to, not sampled from, and are not registered
    /// into the bindless array this step (see the Step 2.1 plan's "out of
    /// scope" section).
    fn bindless_index(&self) -> Option<u32>;
    /// This texture's real GPU allocation size in bytes (its own
    /// `VkMemoryRequirements::size`, or backend equivalent). Added
    /// IMPLEMENTATION.md Step 2.3 so `RhiDevice::release_transient_target`
    /// can maintain the transient pool's total-free-bytes accounting (the
    /// generational GC's 85%-of-budget trigger) without re-querying it.
    fn size_bytes(&self) -> u64;
}

/// A triple-buffered, host-mapped dynamic buffer (TECHNICAL.md Section
/// 3.1) for per-frame vertex/index/uniform data written directly by the
/// CPU, without a staging upload. A distinct trait from the plain
/// `RhiBuffer` rather than additional methods bolted onto it, since
/// callers use a fundamentally different pattern: bump-allocate into the
/// current frame's segment every frame, rather than upload-once-and-keep.
pub trait RhiDynamicRingBuffer: RhiBuffer {
    /// Bump-allocates `bytes.len()` (rounded up to the RHI's minimum
    /// dynamic-offset alignment) from the current frame's segment and
    /// copies `bytes` into it, returning the byte offset `bytes` was
    /// written at -- usable directly as a
    /// `RhiCommandBuffer::bind_vertex_buffer`/`bind_index_buffer` offset.
    /// Returns `None` if the segment has no room left this frame
    /// (DESIGN.md Section 2.6: ring-buffer starvation is reported, never
    /// grown dynamically mid-frame).
    fn write(&self, bytes: &[u8]) -> Option<u32>;
}

/// A compiled graphics pipeline state object. Referenced but undefined by
/// ARCHITECTURE.md Section 6; defined here.
pub trait RhiPipelineState {
    fn raw_handle(&self) -> u64;
    /// Opaque handle of this pipeline's layout, needed by
    /// `RhiCommandBuffer::set_pipeline` implementations that push
    /// constants/descriptors keyed by layout (e.g. `vkCmdPushConstants`).
    /// Same opaque-handle pattern as `AcquiredImage` -- not a downcast.
    fn layout_handle(&self) -> u64;
}

/// A per-window presentation surface. Referenced (as `&dyn RhiSwapchain`)
/// but never defined by ARCHITECTURE.md Section 6; defined here with the
/// minimum needed to make `RhiDevice::begin_frame`/`submit_and_present`
/// actually implementable without `Any`-downcasting (TECHNICAL.md Section
/// 9.1's per-frame-loop ban).
pub trait RhiSwapchain {
    fn extent(&self) -> (u32, u32);

    /// Opaque handle (e.g. a Vulkan `vk::ImageView` reinterpreted via
    /// `ash::vk::Handle::as_raw`, same pattern as `AcquiredImage`'s own
    /// handles -- not a downcast) of this swapchain's own stencil image
    /// view, sized to match its `extent()`. IMPLEMENTATION.md Step 3.3.3:
    /// every swapchain owns its own stencil image, mirroring how it
    /// already owns its own color image(s), since different swapchains
    /// (e.g. two windows in `multi_window`) can have different extents.
    fn stencil_view_handle(&self) -> u64;

    /// The same stencil image's underlying `vk::Image` (distinct from its
    /// view, exactly like `AcquiredImage::target_image_handle` vs.
    /// `target_view_handle`) -- needed for the layout-transition barrier
    /// `RhiDevice::begin_frame` issues before rendering can use it.
    fn stencil_image_handle(&self) -> u64;

    /// # Errors
    /// Returns [`EngineError::SwapchainOutOfDate`] if the surface no longer
    /// matches the window (DESIGN.md Section 2.6) or
    /// [`EngineError::DeviceLost`] on any other acquisition failure.
    fn acquire_next_image(&self) -> Result<AcquiredImage, EngineError>;

    /// Waits on `image.render_finished_semaphore_handle` before showing
    /// the image (DESIGN.md Section 2.6 -- surfaces failures rather than
    /// stalling or panicking).
    ///
    /// # Errors
    /// Returns [`EngineError::SwapchainOutOfDate`] if the surface no longer
    /// matches the window, or [`EngineError::DeviceLost`] on any other
    /// presentation failure.
    fn present(&self, image: AcquiredImage) -> Result<(), EngineError>;
}

/// The Render Hardware Interface device trait (ARCHITECTURE.md Section 6).
///
/// `begin_frame`/`submit_and_present` return `Result<_, EngineError>`,
/// which ARCHITECTURE.md's original sketch omitted -- DESIGN.md Section
/// 2.6 explicitly requires device-loss/swapchain-out-of-date conditions to
/// be "detected at `RhiDevice::begin_frame` and surfaced as a recoverable
/// error," which is impossible with a bare, infallible return type. This
/// is exactly the kind of interface mismatch Phase 0 exists to catch
/// while it's still cheap to change (IMPLEMENTATION.md Phase 0 rationale).
pub trait RhiDevice {
    // Resource Management
    /// `capacity` is the ring buffer's TOTAL size in bytes (TECHNICAL.md
    /// Section 3.1's $16\text{-}32\text{MB}$), divided evenly across the
    /// 3 frame-in-flight segments -- not the per-segment size.
    fn create_dynamic_ring_buffer(&self, capacity: usize) -> Box<dyn RhiDynamicRingBuffer>;
    /// # Errors
    /// Returns [`EngineError::TransientPoolBudgetExceeded`] if a genuinely
    /// novel size would need cold-allocating while the pool's idle free
    /// bytes are already at or past the dynamic-VRAM budget (Phase 2
    /// Step 2.3 Code Review finding #80) -- a reuse of an already-pooled
    /// size (the common case) never fails this way.
    fn acquire_transient_target(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
    ) -> Result<Box<dyn RhiTexture>, EngineError>;
    fn release_transient_target(&self, texture: Box<dyn RhiTexture>);
    /// Uploads `pixels` (tightly packed, row-major, matching `format`'s
    /// byte layout) as a new GPU-resident sampled texture and registers it
    /// into the RHI's persistent bindless texture array (IMPLEMENTATION.md
    /// Step 2.1), so `texture.bindless_index()` can immediately be passed to
    /// `RhiCommandBuffer::bind_texture`. Unlike `acquire_transient_target`,
    /// this is a genuine one-time GPU upload, not a pool checkout -- callers
    /// own the returned texture for as long as they need it and simply drop
    /// it when done (`Drop` tears down the GPU resources and frees the
    /// bindless slot).
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidTextureData`] if `pixels.len()` doesn't
    /// match `width * height * bytes_per_pixel(format)`, or `width`/`height`
    /// is zero. Returns [`EngineError::BindlessArrayExhausted`] if the
    /// bindless texture array has no free slots left. Added in Phase 2 Code
    /// Review findings #66/#67 -- both were previously unconditional panics.
    fn create_texture(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
        pixels: &[u8],
    ) -> Result<Box<dyn RhiTexture>, EngineError>;

    // Command Submission
    /// # Errors
    /// Returns [`EngineError::DeviceLost`] on GPU device removal or driver
    /// TDR, or [`EngineError::SwapchainOutOfDate`] if `swapchain` no longer
    /// matches its window -- surfaced here per DESIGN.md Section 2.6.
    fn begin_frame(
        &self,
        swapchain: &dyn RhiSwapchain,
    ) -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError>;

    /// # Errors
    /// Returns [`EngineError::DeviceLost`] or
    /// [`EngineError::SwapchainOutOfDate`] under the same conditions as
    /// [`RhiDevice::begin_frame`].
    fn submit_and_present(
        &self,
        cmd_buffer: Box<dyn RhiCommandBuffer>,
        swapchain: &dyn RhiSwapchain,
        image: AcquiredImage,
    ) -> Result<(), EngineError>;
}

/// The Render Hardware Interface command-buffer trait (ARCHITECTURE.md
/// Section 6), with one addition beyond the original sketch: `raw_handle`,
/// needed so `RhiDevice::submit_and_present` can recover the concrete
/// backend's submittable handle from a `Box<dyn RhiCommandBuffer>` -- via
/// the same opaque-handle pattern as `AcquiredImage`, not downcasting.
pub trait RhiCommandBuffer {
    // State Tracking
    fn set_pipeline(&mut self, pipeline: &dyn RhiPipelineState);
    fn set_scissor(&mut self, rect: &ScissorRect);

    // Bindings (Leveraging Bindless where available)
    fn bind_vertex_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32);
    fn bind_index_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32);
    fn bind_texture(&mut self, slot: u32, bindless_index: u32);

    // Execution
    fn draw_indexed(&mut self, index_count: u32, start_index: u32, base_vertex: i32);

    fn raw_handle(&self) -> u64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba8_packs_bytes_in_memory_order_not_hex_literal_order() {
        // R=0xE0, G=0xA0, B=0x40, A=0xFF must land at byte offsets 0,1,2,3
        // respectively -- verified by reading back through `to_le_bytes`,
        // not by asserting a specific u32 numeric value, so this test
        // still documents intent even to a reader who doesn't want to
        // mentally byte-reverse a hex literal.
        let packed = rgba8(0xE0, 0xA0, 0x40, 0xFF);
        assert_eq!(packed.to_le_bytes(), [0xE0, 0xA0, 0x40, 0xFF]);
    }

    #[test]
    fn input_event_queue_coalesces_consecutive_moves_for_the_same_window() {
        let mut queue = InputEventQueue::with_capacity(8);
        let window = WindowId(0);
        queue.push(InputEvent::PointerMoved {
            window,
            x: 1.0,
            y: 1.0,
        });
        queue.push(InputEvent::PointerMoved {
            window,
            x: 2.0,
            y: 2.0,
        });
        queue.push(InputEvent::PointerMoved {
            window,
            x: 3.0,
            y: 3.0,
        });

        let drained = queue.drain();
        assert_eq!(
            drained,
            vec![InputEvent::PointerMoved {
                window,
                x: 3.0,
                y: 3.0
            }],
            "three same-window moves must collapse to only the latest position"
        );
    }

    #[test]
    fn input_event_queue_does_not_coalesce_moves_across_different_windows() {
        let mut queue = InputEventQueue::with_capacity(8);
        let (window_a, window_b) = (WindowId(0), WindowId(1));
        queue.push(InputEvent::PointerMoved {
            window: window_a,
            x: 1.0,
            y: 1.0,
        });
        queue.push(InputEvent::PointerMoved {
            window: window_b,
            x: 2.0,
            y: 2.0,
        });

        let drained = queue.drain();
        assert_eq!(
            drained,
            vec![
                InputEvent::PointerMoved { window: window_a, x: 1.0, y: 1.0 },
                InputEvent::PointerMoved { window: window_b, x: 2.0, y: 2.0 },
            ],
            "switching windows must flush the first window's pending move rather than dropping or merging it"
        );
    }

    #[test]
    fn input_event_queue_flushes_pending_move_before_a_non_move_event() {
        let mut queue = InputEventQueue::with_capacity(8);
        let window = WindowId(0);
        queue.push(InputEvent::PointerMoved {
            window,
            x: 5.0,
            y: 5.0,
        });
        queue.push(InputEvent::PointerButton {
            window,
            button: MouseButton::Left,
            state: ElementState::Pressed,
        });

        let drained = queue.drain();
        assert_eq!(
            drained,
            vec![
                InputEvent::PointerMoved {
                    window,
                    x: 5.0,
                    y: 5.0
                },
                InputEvent::PointerButton {
                    window,
                    button: MouseButton::Left,
                    state: ElementState::Pressed,
                },
            ],
            "a click must not be reordered ahead of the motion that preceded it"
        );
    }

    #[test]
    fn input_event_queue_drain_is_empty_when_nothing_was_pushed() {
        let mut queue = InputEventQueue::with_capacity(8);
        assert_eq!(queue.drain(), Vec::new());
    }

    #[test]
    fn draw_rounded_rect_emits_one_command_with_four_vertices_six_indices() {
        let mut canvas = RenderingCanvas::new();
        canvas.draw_rounded_rect(0.0, 0.0, 100.0, 40.0, 0.0, 0xFF00_FFFF);
        let frame = canvas.flatten();

        assert_eq!(frame.commands.len(), 1);
        assert_eq!(frame.vertices.len(), 4);
        assert_eq!(frame.indices.len(), 6);
        assert_eq!(frame.commands[0].element_count, 6);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s (halving/negating whole numbers with no \
                   rounding), not an epsilon-worthy computed value -- same reasoning as \
                   tre-math's Step 3.1 exact-arithmetic tests"
    )]
    fn draw_rounded_rect_encodes_uv_as_center_relative_offset_and_params_as_radius_half_extents() {
        let mut canvas = RenderingCanvas::new();
        // A 100x40 rect at (10, 20): half_width=50, half_height=20.
        canvas.draw_rounded_rect(10.0, 20.0, 100.0, 40.0, 8.0, 0xFF00_FFFF);
        let frame = canvas.flatten();

        let expected_uv = [
            [-50.0, -20.0], // top-left
            [50.0, -20.0],  // top-right
            [50.0, 20.0],   // bottom-right
            [-50.0, 20.0],  // bottom-left
        ];
        for (vertex, uv) in frame.vertices.iter().zip(expected_uv) {
            assert_eq!(vertex.uv, uv);
            assert_eq!(vertex.params, [8.0, 50.0, 20.0]);
        }
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s, not an epsilon-worthy computed value -- \
                   same reasoning as tre-math's Step 3.1 exact-arithmetic tests"
    )]
    fn draw_rounded_rect_clamps_an_oversized_radius_to_half_the_smaller_extent() {
        let mut canvas = RenderingCanvas::new();
        // A 100x40 rect: half_width=50, half_height=20, so any radius above
        // 20.0 must be clamped to 20.0 rather than stored as requested.
        canvas.draw_rounded_rect(0.0, 0.0, 100.0, 40.0, 1000.0, 0xFF00_FFFF);
        let frame = canvas.flatten();

        for vertex in &frame.vertices {
            assert_eq!(
                vertex.params[0], 20.0,
                "radius must be clamped to min(half_width, half_height)"
            );
        }
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s, not an epsilon-worthy computed value -- \
                   same reasoning as tre-math's Step 3.1 exact-arithmetic tests"
    )]
    fn draw_rounded_rect_clamps_a_negative_radius_to_zero() {
        let mut canvas = RenderingCanvas::new();
        canvas.draw_rounded_rect(0.0, 0.0, 100.0, 40.0, -5.0, 0xFF00_FFFF);
        let frame = canvas.flatten();

        for vertex in &frame.vertices {
            assert_eq!(
                vertex.params[0], 0.0,
                "a negative radius must be clamped to zero"
            );
        }
    }

    #[test]
    fn balanced_push_pop_layer_does_not_panic_at_flatten() {
        let mut canvas = RenderingCanvas::new();
        let desc = LayerDesc {
            width: 256,
            height: 256,
            format: TextureFormat::Bgra8Srgb,
        };
        canvas.push_layer(&desc);
        canvas.pop_layer();
        let frame = canvas.flatten();
        assert_eq!(frame.commands.len(), 2);
        assert_eq!(frame.commands[0].kind, CommandType::PushLayer);
        assert_eq!(frame.commands[1].kind, CommandType::PopLayer);
    }

    #[test]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "the balance assertion is a debug_assert, compiled out in release"
    )]
    #[should_panic(expected = "unbalanced")]
    fn unbalanced_push_layer_panics_at_flatten() {
        let mut canvas = RenderingCanvas::new();
        canvas.push_layer(&LayerDesc {
            width: 64,
            height: 64,
            format: TextureFormat::Bgra8Srgb,
        });
        let _ = canvas.flatten();
    }

    #[test]
    #[should_panic(expected = "without a matching push_layer")]
    fn pop_layer_without_push_panics_immediately() {
        let mut canvas = RenderingCanvas::new();
        canvas.pop_layer();
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s (whole-number translation, no rounding), \
                   same reasoning as tre-math's Step 3.1 exact-arithmetic tests"
    )]
    fn save_transform_restore_translates_then_reverts_to_untransformed() {
        let mut canvas = RenderingCanvas::new();
        canvas.save();
        canvas.transform(&tre_math::Affine2::from_translation(100.0, 200.0));
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFF00_FFFF);
        canvas.restore();
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFF00_FFFF);
        let frame = canvas.flatten();

        // First rect: translated by (100, 200).
        assert_eq!(frame.vertices[0].position, [100.0, 200.0]);
        assert_eq!(frame.vertices[2].position, [110.0, 210.0]);
        // Second rect, after restore(): back to raw, untransformed positions.
        assert_eq!(frame.vertices[4].position, [0.0, 0.0]);
        assert_eq!(frame.vertices[6].position, [10.0, 10.0]);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s (whole-number translations, no rounding), \
                   same reasoning as tre-math's Step 3.1 exact-arithmetic tests"
    )]
    fn nested_save_transform_composes_both_translations() {
        let mut canvas = RenderingCanvas::new();
        canvas.save();
        canvas.transform(&tre_math::Affine2::from_translation(100.0, 0.0));
        canvas.save();
        canvas.transform(&tre_math::Affine2::from_translation(0.0, 50.0));
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFF00_FFFF);
        canvas.restore();
        canvas.restore();
        let frame = canvas.flatten();

        // The combined offset (100, 50) predicts the top-left corner.
        assert_eq!(frame.vertices[0].position, [100.0, 50.0]);
    }

    #[test]
    fn nested_set_alpha_compounds_multiplicatively() {
        let mut canvas = RenderingCanvas::new();
        canvas.save();
        canvas.set_alpha(0.5);
        canvas.save();
        canvas.set_alpha(0.5);
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, rgba8(255, 255, 255, 255));
        canvas.restore();
        canvas.restore();
        let frame = canvas.flatten();

        // Effective alpha 0.5 * 0.5 = 0.25 -> byte 64 (255 * 0.25, rounded).
        let [_, _, _, a] = frame.vertices[0].color.to_le_bytes();
        assert_eq!(a, 64);
    }

    #[test]
    #[should_panic(expected = "without a matching save()")]
    fn restore_without_save_panics_immediately() {
        let mut canvas = RenderingCanvas::new();
        canvas.restore();
    }

    #[test]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "the balance assertion is a debug_assert, compiled out in release"
    )]
    #[should_panic(expected = "save/restore calls are unbalanced")]
    fn unbalanced_save_panics_at_flatten() {
        let mut canvas = RenderingCanvas::new();
        canvas.save();
        let _ = canvas.flatten();
    }

    #[test]
    fn push_clip_intersects_a_narrower_rect_and_pop_clip_restores_the_wider_one() {
        let mut canvas = RenderingCanvas::new();
        canvas.push_clip(&ScissorRect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        });
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFF00_FFFF);
        canvas.push_clip(&ScissorRect {
            x: 20,
            y: 20,
            width: 30,
            height: 30,
        });
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFF00_FFFF);
        canvas.pop_clip();
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFF00_FFFF);
        canvas.pop_clip();
        let frame = canvas.flatten();

        // Commands: PushScissor, DrawGeometry, PushScissor, DrawGeometry,
        // PopScissor, DrawGeometry, PopScissor.
        assert_eq!(
            frame.commands[0].clip_bounds,
            ScissorRect {
                x: 0,
                y: 0,
                width: 100,
                height: 100
            }
        );
        assert_eq!(
            frame.commands[1].clip_bounds,
            ScissorRect {
                x: 0,
                y: 0,
                width: 100,
                height: 100
            },
            "the first rect must be clipped to the outer 100x100 region"
        );
        let intersected = ScissorRect {
            x: 20,
            y: 20,
            width: 30,
            height: 30,
        };
        assert_eq!(frame.commands[2].clip_bounds, intersected);
        assert_eq!(
            frame.commands[3].clip_bounds, intersected,
            "the nested rect must be clipped to the intersected 30x30 region"
        );
        assert_eq!(
            frame.commands[5].clip_bounds,
            ScissorRect {
                x: 0,
                y: 0,
                width: 100,
                height: 100
            },
            "after pop_clip(), the wider clip must be restored exactly"
        );
    }

    #[test]
    #[should_panic(expected = "without a matching push_clip()")]
    fn pop_clip_without_push_panics_immediately() {
        let mut canvas = RenderingCanvas::new();
        canvas.pop_clip();
    }

    #[test]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "the balance assertion is a debug_assert, compiled out in release"
    )]
    #[should_panic(expected = "push_clip/pop_clip calls are unbalanced")]
    fn unbalanced_push_clip_panics_at_flatten() {
        let mut canvas = RenderingCanvas::new();
        canvas.push_clip(&ScissorRect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        });
        let _ = canvas.flatten();
    }

    /// A real, installed system font's bytes via `fontconfig` (the same
    /// "verify against a real font, not a synthetic stub" precedent
    /// `tre-text`'s own tests already establish, e.g.
    /// `fallback::tests::read_family`).
    fn discover_test_font_bytes() -> Vec<u8> {
        let cascade =
            tre_text::FontCascade::discover().expect("fontconfig cascade discovery failed");
        std::fs::read(&cascade.entries[0]).expect("failed to read the primary cascade font")
    }

    fn test_units_per_em(font: &skrifa::FontRef) -> f32 {
        f32::from(
            skrifa::MetadataProvider::metrics(
                font,
                skrifa::instance::Size::unscaled(),
                skrifa::instance::LocationRef::default(),
            )
            .units_per_em,
        )
    }

    /// Requests `glyph_id`'s atlas space and blocks (real `sleep`-based
    /// polling -- this genuinely waits on a different thread, the atlas
    /// owner, to make progress, same reasoning as
    /// `atlas_concurrency_demo`'s own identical polling loop) until it
    /// resolves, so a later `draw_text` call against the same `handle`
    /// sees a real cache hit instead of firing its own `request_insert`.
    fn seed_atlas(
        handle: &tre_atlas::AtlasOwnerHandle,
        font: &skrifa::FontRef,
        glyph_id: u32,
        current_frame: u64,
    ) {
        let outline = tre_text::glyph_outline(font, skrifa::GlyphId::from(glyph_id))
            .expect("outline extraction failed");
        assert!(!outline.is_empty(), "test glyph must have real ink");
        let key = tre_atlas::AtlasKey::from_glyph(0, glyph_id);
        assert!(
            handle.request_insert(
                key,
                Box::new(tre_text::GlyphRasterSource {
                    contours: outline,
                    size: 32,
                    range_px: 4.0,
                }),
                current_frame,
            ),
            "request_insert failed -- queue unexpectedly full"
        );
        for _ in 0..500 {
            if handle.lookup(key, current_frame).is_some() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("glyph {glyph_id} never resolved");
    }

    #[test]
    #[allow(
        clippy::cast_precision_loss,
        reason = "a glyph's own x_advance stays far below f32's exact-integer range for any \
                   real font/text, same reasoning as draw_text's own production-code allow"
    )]
    fn draw_text_emits_one_command_per_resolved_glyph_with_correct_pen_advance() {
        // Plain ASCII Latin glyphs shape with zero x_offset/y_offset (no
        // GPOS mark repositioning applies), so each quad's expected
        // top-left reduces to the running pen position minus half the
        // fixed square side.
        const EPSILON: f32 = 1e-3;

        let font_bytes = discover_test_font_bytes();
        let font = skrifa::FontRef::new(&font_bytes).expect("font invalid for skrifa");
        let face = rustybuzz::Face::from_slice(&font_bytes, 0).expect("font invalid for rustybuzz");
        let runs = tre_text::shape_text(&face, "ab").expect("shaping failed");
        assert_eq!(runs.len(), 1, "plain Latin text must be a single run");
        let shaped = &runs[0];
        assert_eq!(
            shaped.glyphs.len(),
            2,
            "\"ab\" must shape to exactly 2 glyphs"
        );

        let owner = tre_atlas::AtlasOwner::spawn(256, 256, 8, 8);
        let handle = owner.handle();
        for glyph in &shaped.glyphs {
            seed_atlas(&handle, &font, glyph.glyph_id, 0);
        }

        let atlas_context = GlyphAtlasContext {
            atlas: &handle,
            texture_handle: 7,
            dimensions: (256, 256),
            current_frame: 0,
        };
        let px_size = 32.0;
        let mut canvas = RenderingCanvas::new();
        canvas.draw_text(
            shaped,
            &font,
            0,
            [0.0, 0.0],
            px_size,
            0xFFFF_FFFF,
            &atlas_context,
        );
        let frame = canvas.flatten();
        drop(owner.join());

        assert_eq!(frame.commands.len(), 2, "one command per resolved glyph");
        for command in &frame.commands {
            assert_eq!(command.kind, CommandType::DrawGeometry);
            assert_eq!(command.pipeline_state_id, PIPELINE_MSDF_TEXT);
            assert_eq!(command.texture_handle, 7);
            assert_eq!(command.element_count, 6);
        }

        let scale = px_size / test_units_per_em(&font);
        assert!(
            (frame.vertices[0].position[0] - (-px_size / 2.0)).abs() <= EPSILON,
            "first glyph's quad must start at the pen origin: {:?}",
            frame.vertices[0].position
        );
        let expected_second_x0 = shaped.glyphs[0].x_advance as f32 * scale - px_size / 2.0;
        assert!(
            (frame.vertices[4].position[0] - expected_second_x0).abs() <= EPSILON,
            "second glyph's quad must be offset by the first glyph's scaled x_advance: got {}, \
             expected {expected_second_x0}",
            frame.vertices[4].position[0]
        );
    }

    #[test]
    fn draw_text_cache_miss_fires_a_real_request_insert_and_renders_nothing_this_frame() {
        let font_bytes = discover_test_font_bytes();
        let font = skrifa::FontRef::new(&font_bytes).expect("font invalid for skrifa");
        let face = rustybuzz::Face::from_slice(&font_bytes, 0).expect("font invalid for rustybuzz");
        let runs = tre_text::shape_text(&face, "c").expect("shaping failed");
        let shaped = &runs[0];

        let owner = tre_atlas::AtlasOwner::spawn(256, 256, 8, 8);
        let handle = owner.handle();
        let atlas_context = GlyphAtlasContext {
            atlas: &handle,
            texture_handle: 0,
            dimensions: (256, 256),
            current_frame: 0,
        };
        let mut canvas = RenderingCanvas::new();
        canvas.draw_text(
            shaped,
            &font,
            0,
            [0.0, 0.0],
            32.0,
            0xFFFF_FFFF,
            &atlas_context,
        );
        let frame = canvas.flatten();
        assert!(
            frame.commands.is_empty(),
            "a glyph not yet resident in the atlas must render nothing this frame"
        );

        let key = tre_atlas::AtlasKey::from_glyph(0, shaped.glyphs[0].glyph_id);
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
            "draw_text's cache miss must have fired a real request_insert -- the glyph should \
             eventually resolve"
        );
        drop(owner.join());
    }

    #[test]
    fn draw_text_skips_a_whitespace_glyph_without_touching_the_atlas() {
        let font_bytes = discover_test_font_bytes();
        let font = skrifa::FontRef::new(&font_bytes).expect("font invalid for skrifa");
        let face = rustybuzz::Face::from_slice(&font_bytes, 0).expect("font invalid for rustybuzz");
        let runs = tre_text::shape_text(&face, " ").expect("shaping failed");
        let shaped = &runs[0];
        assert_eq!(
            shaped.glyphs.len(),
            1,
            "a single space must shape to exactly one glyph"
        );

        let owner = tre_atlas::AtlasOwner::spawn(256, 256, 8, 8);
        let handle = owner.handle();
        let atlas_context = GlyphAtlasContext {
            atlas: &handle,
            texture_handle: 0,
            dimensions: (256, 256),
            current_frame: 0,
        };
        let mut canvas = RenderingCanvas::new();
        canvas.draw_text(
            shaped,
            &font,
            0,
            [0.0, 0.0],
            32.0,
            0xFFFF_FFFF,
            &atlas_context,
        );
        let frame = canvas.flatten();
        assert!(
            frame.commands.is_empty(),
            "whitespace must never emit a command"
        );

        let key = tre_atlas::AtlasKey::from_glyph(0, shaped.glyphs[0].glyph_id);
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(2));
            assert!(
                handle.lookup(key, 0).is_none(),
                "whitespace must never be requested from the atlas at all"
            );
        }
        drop(owner.join());
    }

    #[test]
    #[allow(
        clippy::similar_names,
        reason = "expected_x0/expected_y0 are the clearest names for this test's own \
                   translated top-left corner pair"
    )]
    fn draw_text_respects_active_transform_alpha_and_clip_state() {
        const EPSILON: f32 = 1e-3;

        let font_bytes = discover_test_font_bytes();
        let font = skrifa::FontRef::new(&font_bytes).expect("font invalid for skrifa");
        let face = rustybuzz::Face::from_slice(&font_bytes, 0).expect("font invalid for rustybuzz");
        let runs = tre_text::shape_text(&face, "a").expect("shaping failed");
        let shaped = &runs[0];

        let owner = tre_atlas::AtlasOwner::spawn(256, 256, 8, 8);
        let handle = owner.handle();
        seed_atlas(&handle, &font, shaped.glyphs[0].glyph_id, 0);
        let atlas_context = GlyphAtlasContext {
            atlas: &handle,
            texture_handle: 3,
            dimensions: (256, 256),
            current_frame: 0,
        };

        let px_size = 32.0;
        let mut canvas = RenderingCanvas::new();
        canvas.save();
        canvas.transform(&tre_math::Affine2::from_translation(100.0, 200.0));
        canvas.set_alpha(0.5);
        canvas.push_clip(&ScissorRect {
            x: 5,
            y: 5,
            width: 50,
            height: 50,
        });
        canvas.draw_text(
            shaped,
            &font,
            0,
            [0.0, 0.0],
            px_size,
            rgba8(255, 255, 255, 255),
            &atlas_context,
        );
        canvas.pop_clip();
        canvas.restore();
        let frame = canvas.flatten();
        drop(owner.join());

        // Commands: PushScissor, DrawGeometry (the glyph), PopScissor.
        assert_eq!(frame.commands.len(), 3);
        let glyph_command = &frame.commands[1];
        assert_eq!(
            glyph_command.clip_bounds,
            ScissorRect {
                x: 5,
                y: 5,
                width: 50,
                height: 50
            }
        );

        let expected_color = premultiply_alpha(rgba8(255, 255, 255, 255), 0.5);
        assert_eq!(frame.vertices[0].color, expected_color);

        let expected_x0 = 100.0 - px_size / 2.0;
        let expected_y0 = 200.0 - px_size;
        assert!(
            (frame.vertices[0].position[0] - expected_x0).abs() <= EPSILON
                && (frame.vertices[0].position[1] - expected_y0).abs() <= EPSILON,
            "the glyph quad's top-left must reflect the active translation: {:?}",
            frame.vertices[0].position
        );
    }
}
