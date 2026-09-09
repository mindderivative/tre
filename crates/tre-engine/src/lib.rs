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

/// "No clip, full window" -- `draw_rounded_rect`/`draw_text`/
/// `begin_overlay`'s shared sentinel for "nothing is actively clipped,"
/// reused instead of each call site constructing its own equivalent
/// literal (Step 5.1.3).
const FULL_WINDOW_CLIP: ScissorRect = ScissorRect {
    x: 0,
    y: 0,
    width: u32::MAX,
    height: u32::MAX,
};

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
/// commands keep the implicit `0` (the SDF-rect pipeline). Step 5.1.3's
/// real batch flattening does read this value (it is part of the top 44
/// merge-key bits); a real, generic consumer that resolves it to an
/// actual pipeline object via [`PipelineRegistry`] is Phase 6 Step 6.2's
/// job -- today the only two readers are two demos' own hardcoded
/// `if pipeline_state_id == PIPELINE_MSDF_TEXT` branches.
pub const PIPELINE_MSDF_TEXT: u16 = 1;

/// Real, type-safe names for [`UiDrawCommand::pipeline_state_id`]'s
/// `Canvas`-emittable values (IMPLEMENTATION.md Phase 6 Step 6.1,
/// extended Step 6.4.2) -- `PIPELINE_MSDF_TEXT` stays defined above as a
/// plain `u16` for existing call sites and the sort-key-packing code,
/// which only ever wants a bare 16-bit numeric field (ARCHITECTURE.md
/// Section 4.1), not this enum. `TexturedQuad` was added at Step 6.4.2
/// once `pop_layer` became the first real `Canvas` method to emit a
/// bindless-textured composite draw -- until then this comment explained
/// the plain-textured-quad pipeline was deliberately excluded because
/// nothing in `Canvas` could reach it yet; that stopped being true.
/// Flat-vertex-color and stencil/cover pipelines still have no real
/// `Canvas` caller and stay unrepresented -- see
/// `planning/archive/PLAN_PHASE6_STEP6_1.md`'s "Scope decisions" for the
/// original reasoning, which still applies to those two.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineKind {
    SdfRoundedRect = 0,
    MsdfText = 1,
    TexturedQuad = 2,
}

/// Packs a [`TextureFormat`] into the `u16` [`UiDrawCommand::PushLayer`]
/// commands carry it as (Step 6.4.2) -- `TextureFormat` itself has no
/// `#[repr]` (it's a plain enum also used by
/// [`RhiDevice::create_texture`]/[`RhiDevice::acquire_transient_target`]'s
/// public signatures, so giving it one would be a wider, unrelated
/// change); these two private conversions are the smaller fix, local to
/// the one place in the IR that needs a numeric encoding at all.
fn texture_format_to_u16(format: TextureFormat) -> u16 {
    match format {
        TextureFormat::Bgra8Srgb => 0,
        TextureFormat::Rgba16Float => 1,
        TextureFormat::Rgba8Unorm => 2,
    }
}

/// Reverses [`texture_format_to_u16`].
///
/// # Panics
/// Panics if `value` is not one of the three values
/// [`texture_format_to_u16`] ever produces -- every real `PushLayer`
/// command's `pipeline_state_id` was written by `texture_format_to_u16`
/// itself (`push_layer`'s own body), so an unrecognized value here means
/// the IR was corrupted or hand-constructed incorrectly, not a normal
/// runtime condition.
fn u16_to_texture_format(value: u16) -> TextureFormat {
    match value {
        0 => TextureFormat::Bgra8Srgb,
        1 => TextureFormat::Rgba16Float,
        2 => TextureFormat::Rgba8Unorm,
        other => panic!("u16_to_texture_format: unrecognized encoded value {other}"),
    }
}

/// The real "no texture bound" sentinel for
/// [`UiDrawCommand::texture_handle`] on a `DrawGeometry` command whose
/// pipeline doesn't sample a texture (`draw_rounded_rect`'s own) --
/// IMPLEMENTATION.md Phase 6 Step 6.1. Not `0`: a real bindless index
/// `0` is a legitimate value a future textured pipeline could validly
/// use, so `0` cannot double as "nothing bound" without an ambiguity a
/// future caller could hit for real. Matches the value
/// `canvas_batch_flattening_demo.rs`/`canvas_sub_canvas_demo.rs` already
/// each independently defined as their own local `NO_TEXTURE` constant
/// for RHI-side binding -- promoted here into one real, shared
/// definition both the IR-emission side (this constant) and any future
/// RHI-execution side can agree on, rather than two sentinels for the
/// same concept reconciled only by a hardcoded branch.
pub const NO_TEXTURE: u32 = u32::MAX;

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

/// A real, hardware-backed monotonic frame timer (IMPLEMENTATION.md Phase 8
/// Step 8.1 task 1; TECHNICAL.md Section 7.1's `QueryPerformanceCounter`/
/// `clock_gettime(CLOCK_MONOTONIC)` requirement -- satisfied by
/// `std::time::Instant` itself, which wraps exactly these platform APIs
/// internally, so no per-platform `#[cfg]` code is needed here). Owns real,
/// mutable frame-to-frame state (the previous tick's own timestamp), an
/// engine-lifecycle concern -- distinct from `tre-math::spring_decay`'s
/// pure, state-free formula, which a caller feeds this clock's own output
/// `dt` into.
pub struct FrameClock {
    previous_tick: Option<std::time::Instant>,
}

impl FrameClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            previous_tick: None,
        }
    }

    /// Returns the real elapsed seconds since the previous call to `tick`,
    /// as an `f32`. The first call has no prior tick to measure a delta
    /// from, so it returns `0.0` rather than an arbitrary or undefined
    /// value.
    pub fn tick(&mut self) -> f32 {
        let now = std::time::Instant::now();
        let delta = match self.previous_tick {
            Some(previous) => now.duration_since(previous).as_secs_f32(),
            None => 0.0,
        };
        self.previous_tick = Some(now);
        delta
    }
}

impl Default for FrameClock {
    fn default() -> Self {
        Self::new()
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
/// now -- opacity/blend-mode fields belong here once a later phase
/// implements those visual filters (DESIGN.md Section 6.2's "Visual
/// Filter Pipeline"); this step only needs enough to acquire a
/// correctly-sized, correctly-formatted transient render target from the
/// pool, plus (Step 6.4.2) where its composited result lands on screen,
/// plus (Step 7.2.2) whether to blur it before compositing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerDesc {
    /// Screen-space position the composited layer is drawn back at
    /// (Step 6.4.2) -- `i32`, matching `ScissorRect::x`/`y`'s own
    /// coordinate type, not `u32` like `width`/`height`.
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    /// Applies a real Dual-Kawase blur (`RhiCommandBuffer::
    /// apply_layer_blur`, IMPLEMENTATION.md Step 7.2.2) to this layer's
    /// own content before compositing -- own-content blur only, not a
    /// true backdrop blur of whatever is visually behind the layer
    /// (`planning/archive/PLAN_PHASE7_STEP7_2_1.md`'s own explicit scope
    /// choice). A fixed chain depth, matching Step 7.2.1's own proven
    /// demo -- no tunable radius/quality yet; real, separate future work
    /// once a real caller needs it.
    pub blur: bool,
}

/// A frame's fully-recorded, sorted-and-flattened batch: one contiguous
/// vertex/index stream plus the (currently trivial, Phase 0) list of
/// draw commands describing how to slice it into RHI draw calls, plus
/// (Step 5.3.1) every tagged accessibility node recorded this frame.
pub struct FlattenedFrame {
    pub vertices: Vec<UiVertex>,
    pub indices: Vec<u32>,
    pub commands: Vec<UiDrawCommand>,
    pub accessibility_nodes: Vec<AccessibilityNode>,
}

/// DESIGN.md Section 5.2's `Canvas::tag_accessibility_node`'s own
/// `node_id` parameter (Step 5.3.1) -- an opaque, caller-assigned
/// stable key. The UI framework already owns the real widget tree and
/// its hierarchy; this engine only reports each tagged node's
/// *rendered* spatial position back, keyed by whatever id the
/// framework itself already tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccessibilityNodeId(pub u64);

/// DESIGN.md Section 5.2's `role_flags` parameter, concretized (Step
/// 5.3.1) -- a small, real, useful starter set rather than an attempt
/// at AT-SPI2's own roughly 130-role taxonomy or literal bitflags; no
/// consumer exists yet to demand more than "what kind of element is
/// this," and this is trivially extensible once Step 5.3.2's real OS
/// bridge reveals which additional roles it actually needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityRole {
    Generic,
    Button,
    TextLabel,
    Image,
}

/// One tagged node's real, transform-correct world-space bounds (Step
/// 5.3.1) -- `x`/`y`/`width`/`height` are the axis-aligned bounding box
/// of the local rect `Canvas::tag_accessibility_node` was given, after
/// the active transform (see that method's own doc comment for why a
/// full bounding-box computation, not a naive corner offset, is
/// necessary once rotation is involved). Stored as `f32`, not yet
/// rounded to whatever integer convention a real OS accessibility
/// bridge (Step 5.3.2) will ultimately need.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccessibilityNode {
    pub node_id: AccessibilityNodeId,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub role: AccessibilityRole,
}

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
/// Cannot be flattened or have its recorded data extracted yet --
/// `RenderingCanvas::flatten` takes `self` by value, which `Deref`/
/// `DerefMut` cannot forward, and this sub-step deliberately adds no
/// escape hatch of its own. Merging a `SubCanvas`'s output back into a
/// real frame is Step 5.2.2's job, not this one's -- dropping a
/// `SubCanvas` (ordinary scope exit) is the only way to end one today.
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

impl SubCanvas {
    /// Delegates to the inner canvas's own `RenderingCanvas::
    /// stitch_into` (Step 5.2.2). Extracts it via `std::mem::take`
    /// first (leaving a throwaway, never-used-again placeholder behind)
    /// rather than destructuring `self` directly -- `SubCanvas`
    /// implements `Drop`, and Rust forbids partially moving fields out
    /// of any type that does. `self`'s own `Drop` still runs normally
    /// once this function returns, releasing this sub-canvas's slot in
    /// `live_sub_canvases` exactly as it would for any other drop.
    #[must_use]
    pub fn stitch_into(mut self, arena: &FrameArena) -> bool {
        let canvas = std::mem::take(&mut self.canvas);
        canvas.stitch_into(arena)
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
    fn new_with_sub_canvas_cap(cap: usize) -> Self {
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
    fn next_sort_key(&mut self, pipeline_state_id: u16, texture_handle: u32) -> u64 {
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

        self.accessibility_nodes.push(AccessibilityNode {
            node_id,
            x: min[0],
            y: min[1],
            width: max[0] - min[0],
            height: max[1] - min[1],
            role,
        });
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
    pub fn stitch_into(self, arena: &FrameArena) -> bool {
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

        let Some(mut vertex_slice) = arena.vertices.reserve(vertices.len()) else {
            return false;
        };
        vertex_slice.copy_from_slice(&vertices);
        let vertex_base = vertex_slice.start_index() as u32;

        let Some(mut index_slice) = arena.indices.reserve(indices.len()) else {
            return false;
        };
        for (dest, &source) in index_slice.iter_mut().zip(&indices) {
            *dest = source + vertex_base;
        }
        let index_base = index_slice.start_index() as u32;

        let Some(mut command_slice) = arena.commands.reserve(commands.len()) else {
            return false;
        };
        for (dest, &source) in command_slice.iter_mut().zip(&commands) {
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
        accessibility_slice.copy_from_slice(&accessibility_nodes);

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
const DEPTH_ID_MASK: u32 = 0xF_FFFF;

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
fn compute_sort_key(
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
fn radix_sort_by_key<T: Copy>(items: &mut [T], scratch: &mut [T], key_fn: impl Fn(&T) -> u64) {
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
    let mut counts = vec![0u32; RADIX_BUCKETS];

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
        for count in &mut counts {
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
    // Phase 9 Step 9.1: one scratch buffer, sized to the whole frame's
    // own command count (an upper bound for any single run below),
    // allocated once here and reused across every `flatten_run` call --
    // `radix_sort_by_key`'s own doc comment explains why it needs
    // caller-provided scratch space rather than allocating its own.
    let mut sort_scratch = vec![
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
        };
        commands.len()
    ];
    let mut run_start = 0;
    for i in 0..=commands.len() {
        let at_boundary = i == commands.len() || commands[i].kind != CommandType::DrawGeometry;
        if !at_boundary {
            continue;
        }
        let run_len = i - run_start;
        flatten_run(
            &mut commands[run_start..i],
            &mut sort_scratch[..run_len],
            indices,
            &mut out_commands,
            &mut out_indices,
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

    FlattenedFrame {
        vertices,
        indices: out_indices,
        commands: out_commands,
        accessibility_nodes,
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
/// reused buffer, not allocated here.
#[allow(
    clippy::cast_possible_truncation,
    reason = "a single frame's index count stays far below u32::MAX, the same headroom \
               reasoning ARCHITECTURE.md Section 4.1 applies to Depth ID"
)]
fn flatten_run(
    run: &mut [UiDrawCommand],
    sort_scratch: &mut [UiDrawCommand],
    source_indices: &[u32],
    out_commands: &mut Vec<UiDrawCommand>,
    out_indices: &mut Vec<u32>,
    merge: bool,
) {
    radix_sort_by_key(run, sort_scratch, |command| command.sort_key);

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

/// A GPU buffer paired with the byte offset to bind it at
/// (IMPLEMENTATION.md Phase 8 Step 8.1.2) -- lets [`execute_frame`]
/// accept either a one-shot-uploaded whole-frame buffer (`offset: 0`)
/// or a real per-frame `RhiDynamicRingBuffer`-backed segment (whatever
/// offset that buffer's own `write` call returned) through the same
/// parameter, without `execute_frame` itself needing to know which.
/// Bundled rather than left as two separate parameters -- matching this
/// crate's own `GlyphAtlasContext` precedent for `draw_text` -- since a
/// buffer and its own offset are always meant to travel together; two
/// bare `u32` parameters next to each other would risk a caller
/// transposing a vertex offset and an index offset with no compiler
/// error either way.
#[derive(Clone, Copy)]
pub struct BufferBinding<'a> {
    pub buffer: &'a dyn RhiBuffer,
    pub offset: u32,
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
    ///
    /// # Note (REVIEW.md finding #142)
    /// `Option<u32>`, not `Result<u32, EngineError>`, is a narrower
    /// contract than DESIGN.md Section 2.6's own blanket "every fallible
    /// operation returns `Result<T, EngineError>`" rule -- and that
    /// section's own "ring buffer / transient pool starvation" bullet
    /// describes graceful degradation (dropping the lowest-priority
    /// pending draws and reporting a frame-budget diagnostic), not a bare
    /// `None` for the caller to do whatever it likes with.
    /// `main_loop_demo.rs` (Step 8.1.2), the first real per-frame caller,
    /// currently `.expect()`s this -- i.e. starvation crashes the
    /// process today, not graceful degradation. Implementing the real
    /// policy is future work belonging with the overlay-priority/
    /// depth-sorting machinery, not a signature tweak; flagged here so a
    /// future reader doesn't assume this already matches policy.
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

/// Maps a [`UiDrawCommand::pipeline_state_id`] to the real
/// [`RhiPipelineState`] object it names (IMPLEMENTATION.md Phase 6 Step
/// 6.1) -- replaces the hardcoded `if pipeline_state_id ==
/// PIPELINE_MSDF_TEXT {...} else {...}` branch
/// `canvas_batch_flattening_demo.rs`/`canvas_sub_canvas_demo.rs`
/// currently each duplicate. A backend builds its real pipeline objects
/// exactly as it does today (`VulkanDevice::create_pipeline`, unchanged)
/// and registers them here once at startup; this type owns no
/// Vulkan-specific knowledge at all, generic purely over the
/// `RhiPipelineState` trait. `HashMap` over a fixed-size array: only two
/// real entries exist today (`PipelineKind::SdfRoundedRect`/`MsdfText`)
/// against a 16-bit id space far too sparse for an array to make sense,
/// and frame-time `get()` is not a measured hot path (Step 6.2's real
/// executor is the first thing that will call it at all).
pub struct PipelineRegistry {
    pipelines: std::collections::HashMap<u16, Box<dyn RhiPipelineState>>,
}

impl PipelineRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pipelines: std::collections::HashMap::new(),
        }
    }

    /// Registers `pipeline` under `id` (typically `PipelineKind::* as
    /// u16`, though any `u16` is accepted -- the registry itself has no
    /// opinion on where ids come from).
    ///
    /// # Panics
    /// Panics if `id` is already registered -- two pipelines silently
    /// sharing one id is a programmer error, not a recoverable runtime
    /// condition, matching this crate's established `pop_layer`/
    /// `restore`-style precedent for unbalanced/invalid caller state.
    pub fn register(&mut self, id: u16, pipeline: Box<dyn RhiPipelineState>) {
        assert!(
            self.pipelines.insert(id, pipeline).is_none(),
            "PipelineRegistry: id {id} was already registered"
        );
    }

    /// Resolves `id` to its registered pipeline, or `None` if nothing is
    /// registered under it -- distinct from `register`'s panic-on-
    /// duplicate above, since a command referencing an unknown pipeline
    /// id is a real runtime condition a caller (Step 6.2's executor)
    /// should be able to detect and report, not necessarily a programmer
    /// error caught at registration time.
    #[must_use]
    pub fn get(&self, id: u16) -> Option<&dyn RhiPipelineState> {
        self.pipelines.get(&id).map(std::convert::AsRef::as_ref)
    }
}

impl Default for PipelineRegistry {
    fn default() -> Self {
        Self::new()
    }
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

    /// Registers an already-created texture -- typically one just
    /// rendered into via [`RhiCommandBuffer::begin_render_to_texture`]/
    /// [`RhiCommandBuffer::end_render_to_texture`] -- into the RHI's
    /// persistent bindless texture array, returning the allocated slot
    /// so it can be passed directly to [`RhiCommandBuffer::bind_texture`]
    /// (IMPLEMENTATION.md Phase 6 Step 6.4.1). Unlike
    /// [`RhiDevice::create_texture`], this performs no pixel upload at
    /// all -- `texture` is already GPU-resident; this only allocates a
    /// bindless slot and points it at the texture's own existing view.
    /// Returns the raw index rather than mutating `texture.bindless_
    /// index()` itself: `RhiTexture` exposes no setter (deliberately --
    /// every other method on it is a read of state fixed at construction
    /// time), so the caller is responsible for remembering the returned
    /// index for as long as it needs it, exactly as it already must for
    /// any other value this trait returns.
    ///
    /// A texture returned by [`RhiDevice::acquire_transient_target`] is
    /// *not* bindless-registered by default ("written to, not sampled
    /// from" is the common case that never needs a slot at all) -- this
    /// is the explicit opt-in for the one real case that does:
    /// compositing a rendered-into transient target back as a sampled
    /// quad.
    ///
    /// # Errors
    /// Returns [`EngineError::BindlessArrayExhausted`] if the bindless
    /// texture array has no free slots left -- the same failure mode
    /// [`RhiDevice::create_texture`] can hit, for the same underlying
    /// array.
    fn register_bindless(&self, texture: &dyn RhiTexture) -> Result<u32, EngineError>;

    /// Reverses [`RhiDevice::register_bindless`] -- frees `bindless_index`
    /// (that call's own return value) back to the registry's free list.
    /// Takes the raw index alone, not a texture reference: the free-list
    /// release itself only ever needed the index (the descriptor slot's
    /// contents are simply overwritten, harmlessly, whenever it's next
    /// allocated to something else). Callers that bindless-register a
    /// texture acquired via [`RhiDevice::acquire_transient_target`] must
    /// call this before [`RhiDevice::release_transient_target`], not
    /// after: that function's own safety guard (Phase 2 Code Review
    /// finding #70) rejects any texture whose `bindless_index()` is
    /// `Some`, since a *genuinely* `create_texture`-sourced texture
    /// reaching it that way would otherwise be pooled as if it had
    /// `COLOR_ATTACHMENT` usage it never actually has -- deregistering
    /// first, here, keeps that guard's own logic completely untouched.
    /// This crate's own `register_bindless` never mutates a transient
    /// target's own `bindless_index()` field at all (see that method's
    /// own doc comment), so that guard's check is unaffected by this
    /// method's use either way.
    fn deregister_bindless(&self, bindless_index: u32);

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

    // Offscreen render targets (IMPLEMENTATION.md Phase 6 Step 6.4.1) --
    // real render-to-texture, the RHI capability real `PushLayer`/
    // `PopLayer` execution (Step 6.4.2) is built on. Scoped to exactly
    // one level of redirection: `begin_render_to_texture`/
    // `end_render_to_texture` bracket rendering into one texture at a
    // time, and `resume_swapchain_rendering` returns to the swapchain
    // `RhiDevice::begin_frame` originally set up -- resuming an *outer*
    // layer's own target (true nested layers) is real, separate future
    // work with no real scene to prove it against yet.
    /// Ends whatever rendering scope is currently active and begins a
    /// new one targeting `texture`, cleared to transparent black, with
    /// no stencil attachment (transient targets don't have one).
    ///
    /// `logical_width`/`logical_height` are the caller's own *intended*
    /// size -- exactly what it originally passed to `RhiDevice::
    /// acquire_transient_target` -- not necessarily `texture`'s own real
    /// physical size. REVIEW.md finding #152: `acquire_transient_
    /// target`'s documented "oversized borrow" fallback can hand back a
    /// texture larger than requested, and every subsequent `draw_indexed`
    /// call's own NDC-mapping push constant must be computed against the
    /// caller's *intended* size, not the texture's real one, or content
    /// recorded assuming the smaller size (every `PushLayer` inner draw,
    /// baked at `Canvas` record time before the real texture is ever
    /// acquired) silently confines itself to a small corner of the
    /// oversized image. Viewport/scissor/render area stay driven by the
    /// texture's own real size regardless -- content simply draws
    /// "stretched" to fill it, a stretch exactly undone later when it's
    /// sampled back through a normalized `(0,0)`-`(1,1)` UV read and
    /// redrawn at its own real, requested on-screen size (`PopLayer`'s
    /// own composite quad). No other caller-side change is needed.
    fn begin_render_to_texture(
        &mut self,
        texture: &dyn RhiTexture,
        logical_width: u32,
        logical_height: u32,
    );
    /// Ends the rendering scope `begin_render_to_texture` began and
    /// transitions `texture` to a layout suitable for sampling
    /// afterward (e.g. via `RhiDevice::register_bindless` then
    /// `bind_texture`).
    fn end_render_to_texture(&mut self, texture: &dyn RhiTexture);
    /// Begins a new rendering scope targeting `texture`, cleared to
    /// transparent black -- identical to `begin_render_to_texture`
    /// except it never calls `cmd_end_rendering` first, because nothing
    /// is currently active to end. `logical_width`/`logical_height` carry
    /// the same meaning as `begin_render_to_texture`'s own parameters of
    /// the same name -- see that method's own doc comment.
    ///
    /// Exists specifically for chaining multiple render-to-texture
    /// passes back to back (IMPLEMENTATION.md Step 7.2.1's own
    /// Dual-Kawase downsample/upsample levels), used together with a
    /// plain `end_render_to_texture(previous)` call immediately before
    /// it -- **not** `begin_render_to_texture(texture)` directly, which
    /// would call `cmd_end_rendering` a second time for the one scope
    /// `end_render_to_texture` already ended, a real Vulkan validation
    /// error found by actually running Step 7.2.1's own first demo.
    ///
    /// A first, combined `chain_render_to_texture(ending, beginning)`
    /// design (one call doing both the end-and-barrier and the next
    /// begin) was tried and reverted during this same step's own
    /// implementation: it left no point between "the previous texture is
    /// in a sampling-ready layout" and "the next render pass is already
    /// active" to call `RhiDevice::register_bindless` -- every proven
    /// working caller of that method (Step 6.4.1's own single-level
    /// flow) calls it with *no* render pass active, and calling it while
    /// one *is* active (as the combined design forced) produced fully
    /// transparent/wrong sampled output on real hardware, not a
    /// validation error -- caught only by a real GPU pixel check, not
    /// design review. This split keeps every call's own preconditions
    /// identical to the already-proven single-level usage.
    fn begin_render_to_texture_no_end(
        &mut self,
        texture: &dyn RhiTexture,
        logical_width: u32,
        logical_height: u32,
    );
    /// Resumes rendering into the swapchain image `RhiDevice::begin_frame`
    /// originally set up, preserving whatever it already had drawn --
    /// unlike `begin_render_to_texture`, this never clears.
    fn resume_swapchain_rendering(&mut self);

    /// Applies a real Dual-Kawase blur to `source` -- a texture already
    /// `end_render_to_texture`'d (sampling-ready), `width`/`height` its
    /// own real, intended/logical size (the same convention as
    /// `begin_render_to_texture`'s own `logical_width`/`logical_height`
    /// above) -- and returns a *new*, separately-owned, already
    /// sampling-ready blurred texture. IMPLEMENTATION.md Step 7.2.2:
    /// backs `LayerDesc::blur`; own-content blur only (blurs `source`'s
    /// own already-rendered pixels, not whatever is visually behind it),
    /// a fixed chain depth matching Step 7.2.1's own proven demo.
    ///
    /// Deliberately one opaque, purpose-built operation rather than
    /// several smaller primitives the caller would orchestrate itself --
    /// the real mechanism (a non-bindless downsample/upsample chain,
    /// REVIEW.md finding #130's own real fix) needs a custom pipeline
    /// layout/descriptor set incompatible with every other trait method
    /// here, which deliberately assumes the universal bindless layout;
    /// exposing that mismatch to callers has no benefit over hiding it
    /// entirely behind one call, matching this trait's own precedent for
    /// `begin_render_to_texture_no_end` (added narrowly for the one real
    /// need it served, not as a speculative primitive family).
    ///
    /// The caller owns the returned texture exactly as if it had called
    /// `RhiDevice::acquire_transient_target` itself -- release it the
    /// same way once done. `source` itself is untouched (still owned by
    /// the caller, still sampling-ready) -- this does not consume or
    /// release it.
    fn apply_layer_blur(
        &mut self,
        device: &dyn RhiDevice,
        source: &dyn RhiTexture,
        width: u32,
        height: u32,
    ) -> Box<dyn RhiTexture>;

    fn raw_handle(&self) -> u64;
}

/// The real, generic frame executor (IMPLEMENTATION.md Phase 6 -- Step
/// 6.2 built the `DrawGeometry` half, Step 6.3 the real `PushScissor`/
/// `PopScissor` half; renamed from `execute_draw_geometry_batches` at
/// Step 6.3 since its scope is no longer just draw batches). Drives
/// every command in `frame.commands` through the RHI: a `DrawGeometry`
/// resolves its own `pipeline_state_id` via `registry` (Step 6.1's
/// [`PipelineRegistry`]) instead of a hardcoded per-pipeline branch,
/// exactly as Step 6.2 left it; a `PushScissor`/`PopScissor` applies a
/// real `RhiCommandBuffer::set_scissor` via a runtime clip stack, since
/// a `PopScissor` command's own `clip_bounds` carries no restore data
/// (`push_clip`/`pop_clip`'s own real source).
///
/// `full_window` is the real framebuffer extent, in real pixels -- the
/// only thing the caller (not `tre-engine`, which has no notion of
/// framebuffer size) knows. It stands in for [`FULL_WINDOW_CLIP`]'s own
/// `u32::MAX`-sized sentinel wherever that sentinel would otherwise
/// reach a real `set_scissor` call (`begin_overlay`'s own `PushScissor`
/// command carries the raw sentinel directly; the clip stack emptying
/// after a `PopScissor` represents the same "no active clip" concept) --
/// passing the raw sentinel to a real GPU call would be an invalid,
/// out-of-bounds scissor rect. `RhiDevice::begin_frame` already applies
/// a real, correct full-framebuffer scissor before returning the command
/// buffer, so a frame with no `PushScissor` at all needs no extra call
/// here to stay correct.
///
/// `PushLayer`/`PopLayer` commands drive real transient-target
/// acquisition and compositing (Step 6.4.2), built on Step 6.4.1's RHI
/// capability: `PushLayer` decodes `desc.format` back out of
/// `command.pipeline_state_id` (`u16_to_texture_format`, reversing
/// `push_layer`'s own `texture_format_to_u16`), `device.
/// acquire_transient_target`s a target sized to `command.clip_bounds`'
/// `width`/`height`, and `cmd_buffer.begin_render_to_texture`s into it.
/// `PopLayer` ends that render -- then, if the popped `LayerDesc`'s own
/// `blur` flag was set (smuggled through `command.texture_handle`,
/// `1`/`0`, `pop_layer`'s own doc comment; Step 7.2.2), calls
/// `cmd_buffer.apply_layer_blur` and releases the original, now-
/// unneeded layer texture, compositing the *returned* blurred one
/// instead. Either way, `device.register_bindless`s whichever texture
/// is actually being composited, `cmd_buffer.resume_swapchain_
/// rendering`s, then draws the `PopLayer` command's own baked
/// composite-quad geometry (`element_count`/`vertex_offset`, `pop_
/// layer`'s own doc comment) against the pipeline `command.pipeline_
/// state_id` names (`PipelineKind::TexturedQuad`) -- binding the
/// just-registered index directly rather than trusting `command.
/// texture_handle` (which carries the blur flag here, never a texture
/// reference), the same substitution `PushScissor` already does for
/// `FULL_WINDOW_CLIP`, below. `resume_swapchain_rendering` unconditionally resets the GPU
/// scissor to the full swapchain extent (Step 6.4.1's own REVIEW.md
/// #128 fix), so `PopLayer` re-applies `clip_stack`'s current top
/// afterward -- otherwise a layer popped from inside an active
/// `push_clip` would incorrectly escape that clip for its own composite
/// draw. Finally `device.deregister_bindless`/`release_transient_target`
/// return the target to the pool, mirroring `render_to_texture_demo.rs`'s
/// own hand-written sequence exactly, just driven by the IR instead of
/// hand-written calls. Scoped to one level: a nested `PushLayer` (while
/// another is already active) panics -- see `# Panics`.
///
/// `vertex_buffer`/`index_buffer` are [`BufferBinding`]s the caller
/// already populated -- via a one-shot backend upload helper (`offset:
/// 0`), or via a real per-frame `RhiDynamicRingBuffer::write` call
/// (whatever offset it returned). Either way, "Buffer Packing"
/// (DESIGN.md's own frame-lifecycle item 7) is a distinct stage this
/// function deliberately does not perform -- it only binds at whatever
/// offset the caller's own packing already produced (IMPLEMENTATION.md
/// Phase 8 Step 8.1.2).
///
/// # Panics
/// Panics if a `DrawGeometry` or `PopLayer` command's
/// `pipeline_state_id` was never registered in `registry`; if a
/// `PushLayer` is encountered while another is already active (true
/// nested layers are real, separate future work -- no real scene needs
/// them yet, matching `RhiCommandBuffer::resume_swapchain_rendering`'s
/// own single-level scope); if a `PopLayer` is encountered with no
/// active `PushLayer`; or if `device.acquire_transient_target`/
/// `register_bindless` return `Err` (this function has no `Result`
/// return type to propagate a genuinely mid-frame-recoverable failure
/// through, even though `EngineError::TransientPoolBudgetExceeded`'s own
/// doc comment calls that specific failure "recoverable" -- REVIEW.md
/// finding #141: this is a real, undisclosed gap, not yet the honest,
/// documented limit an earlier draft of this comment incorrectly cited
/// IMPLEMENTATION.md's Step 6.4.2 write-up as already covering. The real
/// fix is giving this function a `Result<(), EngineError>` return type
/// and propagating both `Err`s instead of `.expect()`-ing them, updating
/// every real call site -- substantial enough to be its own future work,
/// not attempted opportunistically inside this review). For this
/// function's real callers, every pipeline
/// `Canvas` can emit is always registered before a frame is rendered, so
/// an unresolved id is a static setup bug, not a transient,
/// recoverable-mid-frame condition (matching this crate's established
/// `pop_layer`/`restore`/`PipelineRegistry::register`-style precedent
/// for invalid caller state, not `EngineError`'s own device/resource
/// failure modes).
pub fn execute_frame(
    frame: &FlattenedFrame,
    registry: &PipelineRegistry,
    vertex_buffer: BufferBinding<'_>,
    index_buffer: BufferBinding<'_>,
    full_window: &ScissorRect,
    device: &dyn RhiDevice,
    cmd_buffer: &mut dyn RhiCommandBuffer,
) {
    // REVIEW.md finding #135: bound once, here, rather than inside the
    // loop below -- `vertex_buffer`/`index_buffer` are this whole call's
    // own parameters, invariant for every command in `frame`, and a
    // vertex/index buffer binding is command-buffer state that persists
    // across `PopLayer`'s own render-target switch (`begin_render_to_
    // texture`/`resume_swapchain_rendering` never touch it -- unlike
    // viewport/scissor, finding #128), so rebinding it per command was
    // pure redundant driver overhead, not a correctness requirement.
    cmd_buffer.bind_vertex_buffer(vertex_buffer.buffer, vertex_buffer.offset);
    cmd_buffer.bind_index_buffer(index_buffer.buffer, index_buffer.offset);

    let mut clip_stack: Vec<ScissorRect> = Vec::new();
    // The layer's own requested (logical) width/height ride alongside
    // the texture itself -- REVIEW.md finding #152: `PopLayer`'s own
    // `apply_layer_blur` call (Step 7.2.2) needs the *requested* size,
    // not whatever the acquired texture's own real dimensions are.
    let mut active_layer: Option<(Box<dyn RhiTexture>, u32, u32)> = None;
    for command in &frame.commands {
        match command.kind {
            CommandType::DrawGeometry => {
                let pipeline = registry.get(command.pipeline_state_id).unwrap_or_else(|| {
                    panic!(
                        "execute_frame: no pipeline registered for id {}",
                        command.pipeline_state_id
                    )
                });
                cmd_buffer.set_pipeline(pipeline);
                cmd_buffer.bind_texture(0, command.texture_handle);
                cmd_buffer.draw_indexed(command.element_count, command.vertex_offset, 0);
            }
            CommandType::PushScissor => {
                let resolved = if command.clip_bounds == FULL_WINDOW_CLIP {
                    *full_window
                } else {
                    command.clip_bounds
                };
                clip_stack.push(resolved);
                cmd_buffer.set_scissor(&resolved);
            }
            CommandType::PopScissor => {
                clip_stack.pop();
                let restored = clip_stack.last().copied().unwrap_or(*full_window);
                cmd_buffer.set_scissor(&restored);
            }
            CommandType::PushLayer => {
                assert!(
                    active_layer.is_none(),
                    "execute_frame: nested PushLayer is not supported yet"
                );
                let format = u16_to_texture_format(command.pipeline_state_id);
                let texture = device
                    .acquire_transient_target(
                        command.clip_bounds.width,
                        command.clip_bounds.height,
                        format,
                    )
                    .expect("execute_frame: acquire_transient_target failed for PushLayer");
                // REVIEW.md finding #152: pass the *requested* size, not
                // whatever `texture` itself reports -- `acquire_transient_
                // target`'s oversized-borrow fallback can return something
                // larger, and `begin_render_to_texture`'s own doc comment
                // is explicit that its `logical_width`/`logical_height`
                // parameters must be the caller's original intent.
                cmd_buffer.begin_render_to_texture(
                    &*texture,
                    command.clip_bounds.width,
                    command.clip_bounds.height,
                );
                active_layer = Some((
                    texture,
                    command.clip_bounds.width,
                    command.clip_bounds.height,
                ));
            }
            CommandType::PopLayer => {
                let (texture, layer_width, layer_height) = active_layer
                    .take()
                    .expect("execute_frame: PopLayer with no active PushLayer");
                cmd_buffer.end_render_to_texture(&*texture);

                // `pop_layer`'s own doc comment: `texture_handle` carries
                // the popped `LayerDesc`'s own `blur` flag here, never a
                // real bindless index (Step 7.2.2).
                let composited_texture = if command.texture_handle != 0 {
                    let blurred =
                        cmd_buffer.apply_layer_blur(device, &*texture, layer_width, layer_height);
                    device.release_transient_target(texture);
                    // REVIEW.md finding #153: `apply_layer_blur`'s own
                    // internal hops rebind the command buffer's vertex/
                    // index buffers to its own small, unit-quad ones --
                    // finding #135's own "bound once, at the top"
                    // invariant otherwise leaves this frame's real
                    // vertex/index buffers un-bound for the composite
                    // draw below, a real bug a real GPU run caught
                    // immediately (an out-of-bounds index read, since the
                    // composite quad's own real `vertex_offset` doesn't
                    // exist in `apply_layer_blur`'s own tiny buffer).
                    // Restored here, the same "undo whatever
                    // this call disturbed" responsibility `resume_
                    // swapchain_rendering`'s own scissor-restore already
                    // established.
                    cmd_buffer.bind_vertex_buffer(vertex_buffer.buffer, vertex_buffer.offset);
                    cmd_buffer.bind_index_buffer(index_buffer.buffer, index_buffer.offset);
                    blurred
                } else {
                    texture
                };
                let bindless_index = device
                    .register_bindless(&*composited_texture)
                    .expect("execute_frame: register_bindless failed for PopLayer");
                cmd_buffer.resume_swapchain_rendering();
                let restored = clip_stack.last().copied().unwrap_or(*full_window);
                cmd_buffer.set_scissor(&restored);

                let pipeline = registry.get(command.pipeline_state_id).unwrap_or_else(|| {
                    panic!(
                        "execute_frame: no pipeline registered for id {}",
                        command.pipeline_state_id
                    )
                });
                cmd_buffer.set_pipeline(pipeline);
                cmd_buffer.bind_texture(0, bindless_index);
                cmd_buffer.draw_indexed(command.element_count, command.vertex_offset, 0);

                device.deregister_bindless(bindless_index);
                device.release_transient_target(composited_texture);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

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
    fn frame_clock_first_tick_returns_exactly_zero() {
        let mut clock = FrameClock::new();
        assert!(clock.tick().abs() <= f32::EPSILON);
    }

    #[test]
    fn frame_clock_reports_real_elapsed_time_between_ticks() {
        let mut clock = FrameClock::new();
        clock.tick();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let delta = clock.tick();
        assert!(
            delta >= 0.015,
            "a real ~20ms sleep must report a delta of at least 15ms, got {delta}s"
        );
        assert!(
            delta < 1.0,
            "a real ~20ms sleep must not report a wildly inflated delta, got {delta}s"
        );
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
        // Step 6.1: NO_TEXTURE, not 0 -- 0 is a legitimate real bindless
        // index a future textured pipeline could use, so it can't double
        // as "nothing bound."
        assert_eq!(frame.commands[0].texture_handle, NO_TEXTURE);
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
            x: 10,
            y: 20,
            width: 256,
            height: 256,
            format: TextureFormat::Bgra8Srgb,
            blur: false,
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
            x: 0,
            y: 0,
            width: 64,
            height: 64,
            format: TextureFormat::Bgra8Srgb,
            blur: false,
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
    fn draw_text_merges_two_resolved_glyphs_sharing_pipeline_and_texture_into_one_command() {
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

        // Step 5.1.3: both glyphs share Layer 0 (standard content),
        // Pipeline PIPELINE_MSDF_TEXT, texture 7, and the same
        // full-window clip_bounds (nothing between the two glyphs
        // changed the clip stack) -- real batch flattening merges them
        // into a single command instead of the naive one-per-glyph
        // mapping Step 5.1.2 originally shipped with.
        assert_eq!(
            frame.commands.len(),
            1,
            "two glyphs sharing Layer+Pipeline+Texture+clip_bounds must merge into one command"
        );
        let command = frame.commands[0];
        assert_eq!(command.kind, CommandType::DrawGeometry);
        assert_eq!(command.pipeline_state_id, PIPELINE_MSDF_TEXT);
        assert_eq!(command.texture_handle, 7);
        assert_eq!(
            command.element_count, 12,
            "6 indices per glyph, 2 glyphs merged"
        );

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

    #[test]
    fn compute_sort_key_packs_each_field_into_its_documented_bit_range() {
        let key = compute_sort_key(0x1234, 0x5678, 0x9AB, 0xC_DEF0);
        assert_eq!(
            key,
            (0x1234_u64 << 48) | (0x5678_u64 << 32) | (0x9AB_u64 << 20) | 0xC_DEF0_u64
        );
    }

    #[test]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "the overflow assertion is a debug_assert, compiled out in release"
    )]
    #[should_panic(expected = "exceeds the 12-bit Texture ID field")]
    fn compute_sort_key_panics_on_texture_handle_overflow() {
        let _ = compute_sort_key(0, 0, 0x1000, 0);
    }

    #[test]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "the overflow assertion is a debug_assert, compiled out in release"
    )]
    #[should_panic(expected = "exceeds the 20-bit Depth ID field")]
    fn compute_sort_key_panics_on_depth_id_overflow() {
        let _ = compute_sort_key(0, 0, 0, 0x10_0000);
    }

    #[test]
    fn next_sort_key_advances_depth_id_monotonically_and_never_resets() {
        let mut canvas = RenderingCanvas::new();
        let first = canvas.next_sort_key(0, 0);
        let second = canvas.next_sort_key(0, 0);
        assert_eq!(first & 0xF_FFFF, 0);
        assert_eq!(second & 0xF_FFFF, 1);

        // begin_overlay/end_overlay must not reset the counter.
        canvas.begin_overlay(OverlayLayerPriority(0));
        canvas.end_overlay();
        let third = canvas.next_sort_key(0, 0);
        assert_eq!(third & 0xF_FFFF, 2);
    }

    #[test]
    fn begin_overlay_sets_the_active_layer_id_and_resets_clip_to_full_window() {
        let mut canvas = RenderingCanvas::new();
        canvas.push_clip(&ScissorRect {
            x: 10,
            y: 10,
            width: 20,
            height: 20,
        });
        canvas.begin_overlay(OverlayLayerPriority(5));
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        canvas.end_overlay();
        canvas.pop_clip();
        let frame = canvas.flatten();

        let overlay_draw = frame
            .commands
            .iter()
            .find(|c| c.kind == CommandType::DrawGeometry)
            .expect("the overlay rect must have been recorded");
        assert_eq!(
            overlay_draw.sort_key >> 48,
            10_005,
            "Layer ID must be OVERLAY_LAYER_BASE + priority"
        );
        assert_eq!(
            overlay_draw.clip_bounds, FULL_WINDOW_CLIP,
            "content inside begin_overlay/end_overlay must not inherit the outer clip"
        );
    }

    #[test]
    fn end_overlay_restores_the_exact_clip_that_was_active_before_begin_overlay() {
        let mut canvas = RenderingCanvas::new();
        let outer_clip = ScissorRect {
            x: 10,
            y: 10,
            width: 20,
            height: 20,
        };
        canvas.push_clip(&outer_clip);
        canvas.begin_overlay(OverlayLayerPriority(0));
        canvas.end_overlay();
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        canvas.pop_clip();
        let frame = canvas.flatten();

        let draw = frame
            .commands
            .iter()
            .find(|c| c.kind == CommandType::DrawGeometry)
            .expect("the rect must have been recorded");
        assert_eq!(
            draw.clip_bounds, outer_clip,
            "clip must be restored exactly after end_overlay"
        );
    }

    #[test]
    #[should_panic(expected = "without a matching begin_overlay()")]
    fn end_overlay_without_begin_panics_immediately() {
        let mut canvas = RenderingCanvas::new();
        canvas.end_overlay();
    }

    #[test]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "the balance assertion is a debug_assert, compiled out in release"
    )]
    #[should_panic(expected = "begin_overlay/end_overlay calls are unbalanced")]
    fn unbalanced_begin_overlay_panics_at_flatten() {
        let mut canvas = RenderingCanvas::new();
        canvas.begin_overlay(OverlayLayerPriority(0));
        let _ = canvas.flatten();
    }

    #[test]
    #[should_panic(expected = "overlay priority overflowed")]
    fn begin_overlay_panics_on_priority_overflow() {
        let mut canvas = RenderingCanvas::new();
        canvas.begin_overlay(OverlayLayerPriority(u16::MAX));
    }

    #[test]
    fn flatten_reproduces_design_doc_section_8s_worked_example() {
        // DESIGN.md Section 8: Rect1(P1,Tex0) -> Text(P2,AtlasA) ->
        // Rect2(P1,Tex0) -> OverlayRect(P1,Tex0) collapses into exactly
        // 3 batches: Rect1+Rect2 merged (same Layer+Pipeline+Texture+
        // clip), Text alone (different pipeline), OverlayRect alone
        // (different Layer ID despite sharing Rect1/Rect2's own
        // pipeline+texture).
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
            texture_handle: 9,
            dimensions: (256, 256),
            current_frame: 0,
        };

        let mut canvas = RenderingCanvas::new();
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF); // Rect1
        canvas.draw_text(
            shaped,
            &font,
            0,
            [100.0, 32.0],
            32.0,
            0xFFFF_FFFF,
            &atlas_context,
        ); // Text
        canvas.draw_rounded_rect(50.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF); // Rect2
        canvas.begin_overlay(OverlayLayerPriority(0));
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF); // OverlayRect
        canvas.end_overlay();
        let frame = canvas.flatten();
        drop(owner.join());

        let draws: Vec<_> = frame
            .commands
            .iter()
            .filter(|c| c.kind == CommandType::DrawGeometry)
            .collect();
        assert_eq!(draws.len(), 3, "expected exactly 3 batches, got {draws:?}");

        let rect_batch = draws
            .iter()
            .find(|c| c.pipeline_state_id == 0 && c.sort_key >> 48 == 0)
            .expect("standard-plane rect batch must exist");
        assert_eq!(
            rect_batch.element_count, 12,
            "Rect1+Rect2 must merge into one 12-index batch"
        );

        let text_batch = draws
            .iter()
            .find(|c| c.pipeline_state_id == PIPELINE_MSDF_TEXT)
            .expect("text batch must exist");
        assert_eq!(text_batch.element_count, 6);

        let overlay_batch = draws
            .iter()
            .find(|c| c.pipeline_state_id == 0 && c.sort_key >> 48 == 10_000)
            .expect("overlay-plane rect batch must exist");
        assert_eq!(overlay_batch.element_count, 6);
    }

    #[test]
    fn flatten_run_does_not_merge_two_commands_sharing_key_but_differing_clip_bounds() {
        // Unreachable via the public Canvas API today (clip_bounds only
        // ever changes alongside a marker command, which already acts
        // as a hard barrier on its own) -- this exercises flatten_run's
        // own clip_bounds check directly as a regression guard, in case
        // a future caller ever changes clip_bounds without a marker.
        let a = UiDrawCommand {
            kind: CommandType::DrawGeometry,
            sort_key: compute_sort_key(0, 0, 0, 0),
            pipeline_state_id: 0,
            texture_handle: 0,
            element_count: 6,
            vertex_offset: 0,
            clip_bounds: ScissorRect {
                x: 0,
                y: 0,
                width: 50,
                height: 50,
            },
        };
        let b = UiDrawCommand {
            sort_key: compute_sort_key(0, 0, 0, 1),
            vertex_offset: 6,
            clip_bounds: ScissorRect {
                x: 60,
                y: 0,
                width: 50,
                height: 50,
            },
            ..a
        };
        let source_indices: Vec<u32> = (0..12).collect();
        let mut run = [a, b];
        let mut sort_scratch = run;
        let mut out_commands = Vec::new();
        let mut out_indices = Vec::new();
        flatten_run(
            &mut run,
            &mut sort_scratch,
            &source_indices,
            &mut out_commands,
            &mut out_indices,
            true,
        );

        assert_eq!(
            out_commands.len(),
            2,
            "differing clip_bounds must prevent merging even with identical \
             Layer+Pipeline+Texture"
        );
    }

    /// A small, deterministic xorshift64 PRNG -- no new dependency
    /// needed for one differential property test (Phase 9 Step 9.1),
    /// and fully reproducible run to run, unlike a system-entropy seed.
    struct Xorshift64(u64);
    impl Xorshift64 {
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    /// Sorts `keys` via `radix_sort_by_key` and returns the resulting
    /// key order -- every adversarial test below only cares about the
    /// resulting *order*, not any other `UiDrawCommand` field, so this
    /// keeps each test's own assertion reading as a plain `Vec<u64>`
    /// comparison rather than constructing full commands throughout.
    fn radix_sorted_keys(keys: &[u64]) -> Vec<u64> {
        let mut items: Vec<u64> = keys.to_vec();
        let mut scratch = items.clone();
        radix_sort_by_key(&mut items, &mut scratch, |&k| k);
        items
    }

    #[test]
    fn radix_sort_handles_all_identical_keys() {
        let keys = vec![42u64; 500];
        assert_eq!(radix_sorted_keys(&keys), keys);
    }

    #[test]
    fn radix_sort_handles_fully_reverse_sorted_input() {
        let keys: Vec<u64> = (0..500).rev().collect();
        let mut expected = keys.clone();
        expected.sort_unstable();
        assert_eq!(radix_sorted_keys(&keys), expected);
    }

    #[test]
    fn radix_sort_handles_keys_clustered_at_every_field_boundary() {
        // ARCHITECTURE.md Section 4.1's own field layout: Layer (63:48),
        // Pipeline (47:32), Texture (31:20), Depth (19:0). One key
        // exactly at 0 and one at the max value of each field alone,
        // plus every field maxed simultaneously and every field zeroed
        // simultaneously -- the exact boundary values a byte/digit-wise
        // radix sort is most likely to get wrong if a shift/mask is off
        // by even one bit.
        let keys: Vec<u64> = vec![
            0,
            u64::from(u16::MAX) << 48,              // Layer ID maxed alone
            u64::from(u16::MAX) << 32,              // Pipeline ID maxed alone
            0xFFF << 20,                            // Texture ID maxed alone
            0xF_FFFF,                               // Depth ID maxed alone
            u64::MAX,                               // every field maxed
            (u64::from(u16::MAX) << 48) | 0xF_FFFF, // Layer + Depth maxed, rest zero
        ];
        let mut expected = keys.clone();
        expected.sort_unstable();
        assert_eq!(radix_sorted_keys(&keys), expected);
    }

    #[test]
    fn radix_sort_handles_maximum_depth_id_values() {
        // Depth ID's own real 20-bit budget (ARCHITECTURE.md Section
        // 4.1) -- every value packed into the field's own low 20 bits,
        // including the field's own real maximum, `0xFFFFF`.
        let keys: Vec<u64> = vec![0xF_FFFF, 0, 0xF_FFFE, 1, 0x8_0000];
        let mut expected = keys.clone();
        expected.sort_unstable();
        assert_eq!(radix_sorted_keys(&keys), expected);
    }

    #[test]
    fn radix_sort_handles_an_empty_run_without_panicking() {
        let keys: Vec<u64> = Vec::new();
        assert_eq!(radix_sorted_keys(&keys), Vec::<u64>::new());
    }

    #[test]
    fn radix_sort_handles_a_single_element_run() {
        let keys = vec![777u64];
        assert_eq!(radix_sorted_keys(&keys), keys);
    }

    #[test]
    #[should_panic(expected = "scratch must be exactly as long as items")]
    fn radix_sort_panics_on_a_mismatched_scratch_length() {
        let mut items = vec![3u64, 1, 2];
        let mut scratch = vec![0u64; 2];
        radix_sort_by_key(&mut items, &mut scratch, |&k| k);
    }

    #[test]
    fn radix_sort_agrees_with_sort_unstable_on_many_randomized_inputs() {
        // Differential property test (Phase 9 Step 9.1): rather than
        // trusting a handful of specific cases above to cover every
        // real bug shape, generate many pseudo-random key sets of
        // varying size and distribution and check the real radix sort
        // produces exactly the same *sorted order* std's own
        // comparison sort would -- a mismatch on any single run would
        // be a real correctness bug in the new algorithm.
        let mut rng = Xorshift64(0x9E37_79B9_7F4A_7C15);
        for run in 0..200u32 {
            let len = (rng.next_u64() % 300) as usize;
            // Every few runs, bias toward small key ranges (heavy
            // duplicate/clustering pressure) instead of the full u64
            // range, matching the "adversarial distribution" spirit of
            // the specific cases above with real randomized coverage.
            let mask: u64 = if run % 3 == 0 { 0xFF } else { u64::MAX };
            let keys: Vec<u64> = (0..len).map(|_| rng.next_u64() & mask).collect();

            let mut expected = keys.clone();
            expected.sort_unstable();
            assert_eq!(
                radix_sorted_keys(&keys),
                expected,
                "radix sort disagreed with sort_unstable on randomized run {run} (len {len}, \
                 mask {mask:#x})"
            );
        }
    }

    #[test]
    fn flatten_does_not_merge_across_a_push_clip_pop_clip_pair_even_when_the_net_clip_is_unchanged()
    {
        let mut canvas = RenderingCanvas::new();
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        canvas.push_clip(&ScissorRect {
            x: 0,
            y: 0,
            width: 1000,
            height: 1000,
        });
        canvas.pop_clip();
        canvas.draw_rounded_rect(50.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        let frame = canvas.flatten();

        let draws: Vec<_> = frame
            .commands
            .iter()
            .filter(|c| c.kind == CommandType::DrawGeometry)
            .collect();
        assert_eq!(
            draws.len(),
            2,
            "an intervening push_clip/pop_clip pair must remain a hard barrier even when its \
             net clip effect matches what was already active"
        );
    }

    #[test]
    fn sub_canvases_recording_concurrently_never_collide_on_depth_id() {
        const THREADS: usize = 4;
        const DRAWS_PER_THREAD: usize = 50;

        let root = RenderingCanvas::new_with_sub_canvas_cap(THREADS);
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let mut sub = root.create_sub_canvas();
                std::thread::spawn(move || {
                    let mut depth_ids = Vec::with_capacity(DRAWS_PER_THREAD);
                    for _ in 0..DRAWS_PER_THREAD {
                        sub.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
                        let command = sub.commands.last().expect("just pushed a command");
                        depth_ids.push(command.sort_key & u64::from(DEPTH_ID_MASK));
                    }
                    depth_ids
                })
            })
            .collect();

        let mut all_depth_ids: Vec<u64> = handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("worker thread panicked"))
            .collect();
        assert_eq!(all_depth_ids.len(), THREADS * DRAWS_PER_THREAD);

        let unique_count = {
            all_depth_ids.sort_unstable();
            all_depth_ids.dedup();
            all_depth_ids.len()
        };
        assert_eq!(
            unique_count,
            THREADS * DRAWS_PER_THREAD,
            "every DrawGeometry command across every concurrently-recording sub-canvas must \
             receive a distinct Depth ID"
        );
    }

    #[test]
    #[should_panic(expected = "would exceed the configured limit of 2 concurrent sub-canvases")]
    fn create_sub_canvas_panics_once_the_cap_is_exceeded() {
        let root = RenderingCanvas::new_with_sub_canvas_cap(2);
        let _first = root.create_sub_canvas();
        let _second = root.create_sub_canvas();
        let _third = root.create_sub_canvas();
    }

    #[test]
    fn dropping_a_sub_canvas_frees_its_slot_for_reuse() {
        let root = RenderingCanvas::new_with_sub_canvas_cap(1);
        let first = root.create_sub_canvas();
        drop(first);
        // Must not panic: the cap was freed by the drop above.
        let _second = root.create_sub_canvas();
    }

    #[test]
    fn two_sub_canvases_sharing_layer_zero_is_not_an_error() {
        // Depth ID must be unique per DrawGeometry command (proven
        // above); Layer ID has no such requirement -- two independent
        // sub-canvases both drawing standard content, or both opening
        // the exact same overlay priority, is completely correct.
        let root = RenderingCanvas::new_with_sub_canvas_cap(2);
        let mut a = root.create_sub_canvas();
        let mut b = root.create_sub_canvas();

        a.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        b.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        assert_eq!(a.commands[0].sort_key >> 48, 0);
        assert_eq!(b.commands[0].sort_key >> 48, 0);

        a.begin_overlay(OverlayLayerPriority(0));
        b.begin_overlay(OverlayLayerPriority(0));
        a.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        b.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        let a_overlay_draw = a.commands[a.commands.len() - 1];
        let b_overlay_draw = b.commands[b.commands.len() - 1];
        assert_eq!(a_overlay_draw.sort_key >> 48, 10_000);
        assert_eq!(b_overlay_draw.sort_key >> 48, 10_000);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s (whole-number translation, no rounding), \
                   same reasoning as save_transform_restore_translates_then_reverts_to_untransformed"
    )]
    fn sub_canvas_drawing_methods_work_identically_to_the_root_canvas_via_delegation() {
        let root = RenderingCanvas::new_with_sub_canvas_cap(1);
        let mut sub = root.create_sub_canvas();

        sub.save();
        sub.transform(&tre_math::Affine2::from_translation(100.0, 200.0));
        sub.push_clip(&ScissorRect {
            x: 0,
            y: 0,
            width: 50,
            height: 50,
        });
        sub.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        sub.pop_clip();
        sub.restore();

        // The draw's own vertex position confirms transform() really
        // ran on this SubCanvas, not a no-op stand-in -- the same
        // delegated-through-Deref call as the root canvas's own
        // save_transform_restore test.
        assert_eq!(sub.vertices[0].position, [100.0, 200.0]);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s (whole-number positions, no rounding), same \
                   reasoning as this crate's other exact-arithmetic tests"
    )]
    fn stitch_into_rebases_indices_and_vertex_offset_correctly() {
        // Two sub-canvases sharing one root's Depth ID counter (so
        // their sort keys never tie -- see next_sort_key's own
        // uniqueness guarantee), each drawing one rect, stitched into
        // the same arena in order.
        let arena = FrameArena::with_capacity(20, 20, 20, 0);
        let root = RenderingCanvas::new();

        let mut first = root.create_sub_canvas();
        first.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        assert!(first.stitch_into(&arena));

        let mut second = root.create_sub_canvas();
        second.draw_rounded_rect(50.0, 50.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        assert!(second.stitch_into(&arena));

        let frame = arena.flatten();

        // Vertices never move regardless of merging: the first source's
        // 4 vertices land first, the second source's right after.
        assert_eq!(frame.vertices.len(), 8);
        assert_eq!(frame.vertices[0].position, [0.0, 0.0]);
        assert_eq!(frame.vertices[4].position, [50.0, 50.0]);

        // Both rects share Layer/Pipeline/Texture/clip_bounds with
        // nothing between them, so they merge into one 12-index
        // command -- and the exact rebased index values prove the
        // second source's own local indices (recorded relative to its
        // own vertex buffer starting at 0) were correctly shifted by
        // +4 once its vertices landed at that offset in the shared
        // arena.
        assert_eq!(frame.commands.len(), 1);
        assert_eq!(frame.commands[0].element_count, 12);
        assert_eq!(frame.indices, vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]);
    }

    #[test]
    fn stitch_into_reports_false_when_the_arena_is_too_small() {
        let arena = FrameArena::with_capacity(2, 2, 2, 0);
        let mut canvas = RenderingCanvas::new();
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        assert!(
            !canvas.stitch_into(&arena),
            "a 4-vertex/6-index/1-command rect cannot fit in a 2/2/2 arena"
        );
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s (whole-number x offsets, no rounding), same \
                   reasoning as this crate's other exact-arithmetic tests"
    )]
    fn many_real_worker_threads_stitch_concurrently_into_one_arena_correctly() {
        const THREADS: usize = 4;

        let root = RenderingCanvas::new_with_sub_canvas_cap(THREADS);
        let arena = std::sync::Arc::new(FrameArena::with_capacity(
            THREADS * 4,
            THREADS * 6,
            THREADS,
            0,
        ));

        let handles: Vec<_> = (0..THREADS)
            .map(|i| {
                let mut sub = root.create_sub_canvas();
                let arena = std::sync::Arc::clone(&arena);
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "THREADS is a small constant, far below f32's exact-integer range"
                )]
                let x = i as f32 * 20.0;
                std::thread::spawn(move || {
                    sub.draw_rounded_rect(x, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
                    assert!(
                        sub.stitch_into(&arena),
                        "arena was sized exactly for this test"
                    );
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("worker thread panicked");
        }

        let arena = std::sync::Arc::try_unwrap(arena)
            .unwrap_or_else(|_| panic!("all worker threads have joined"));
        let frame = arena.flatten();

        assert_eq!(frame.vertices.len(), THREADS * 4);
        // All 4 rects share Layer/Pipeline/Texture/clip_bounds with no
        // marker anywhere -- real batch flattening must merge them all
        // into one command regardless of which thread's reservation
        // landed first.
        assert_eq!(frame.commands.len(), 1);
        assert_eq!(
            frame.commands[0].element_count,
            u32::try_from(THREADS * 6).unwrap()
        );

        for i in 0..THREADS {
            #[allow(
                clippy::cast_precision_loss,
                reason = "THREADS is a small constant, far below f32's exact-integer range"
            )]
            let expected_x = i as f32 * 20.0;
            let found = frame
                .vertices
                .iter()
                .any(|v| v.position == [expected_x, 0.0]);
            assert!(
                found,
                "thread {i}'s rect at x={expected_x} must appear somewhere in the merged \
                 vertices, regardless of scheduling order"
            );
        }
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s (whole-number positions, no rounding), same \
                   reasoning as this crate's other exact-arithmetic tests"
    )]
    fn tag_accessibility_node_translates_local_bounds_into_world_space() {
        let mut canvas = RenderingCanvas::new();
        canvas.transform(&tre_math::Affine2::from_translation(10.0, 5.0));
        canvas.tag_accessibility_node(
            AccessibilityNodeId(1),
            0.0,
            0.0,
            20.0,
            8.0,
            AccessibilityRole::Button,
        );

        let frame = canvas.flatten();
        assert_eq!(frame.accessibility_nodes.len(), 1);
        let node = frame.accessibility_nodes[0];
        assert_eq!(node.node_id, AccessibilityNodeId(1));
        assert_eq!(node.role, AccessibilityRole::Button);
        assert_eq!(
            (node.x, node.y, node.width, node.height),
            (10.0, 5.0, 20.0, 8.0)
        );
    }

    #[test]
    fn tag_accessibility_node_under_rotation_reports_the_real_axis_aligned_bounding_box() {
        // A 90-degree CCW rotation maps (x, y) -> (-y, x) (up to f32
        // sin/cos rounding), so a 10x4 local rect's four corners land
        // near (0,0), (0,10), (-4,10), (-4,0). A naive implementation
        // that only transforms the top-left corner and reuses the
        // local width/height would wrongly report (0, 0, 10, 4) -- the
        // real axis-aligned bounding box of all four rotated corners is
        // (-4, 0, 4, 10), the opposite aspect ratio, which is this
        // sub-step's whole reason for existing. `sin_cos` on
        // `FRAC_PI_2` doesn't land on exactly 0.0/1.0 in f32, so this
        // compares within a small epsilon rather than asserting exact
        // equality.
        const EPSILON: f32 = 1e-4;
        let mut canvas = RenderingCanvas::new();
        canvas.transform(&tre_math::Affine2::from_rotation(
            std::f32::consts::FRAC_PI_2,
        ));
        canvas.tag_accessibility_node(
            AccessibilityNodeId(7),
            0.0,
            0.0,
            10.0,
            4.0,
            AccessibilityRole::Generic,
        );

        let frame = canvas.flatten();
        assert_eq!(frame.accessibility_nodes.len(), 1);
        let node = frame.accessibility_nodes[0];
        for (actual, expected, label) in [
            (node.x, -4.0, "x"),
            (node.y, 0.0, "y"),
            (node.width, 4.0, "width"),
            (node.height, 10.0, "height"),
        ] {
            assert!(
                (actual - expected).abs() < EPSILON,
                "{label}: expected the real bounding box of all four rotated corners \
                 (~{expected}), got {actual} -- not the naive untransformed rect"
            );
        }
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s (whole-number positions, no rounding), same \
                   reasoning as this crate's other exact-arithmetic tests"
    )]
    fn tagging_from_a_sub_canvas_carries_the_node_into_a_frame_arena_via_stitch_into() {
        let arena = FrameArena::with_capacity(20, 20, 20, 4);
        let root = RenderingCanvas::new();

        let mut sub = root.create_sub_canvas();
        sub.tag_accessibility_node(
            AccessibilityNodeId(42),
            1.0,
            2.0,
            3.0,
            4.0,
            AccessibilityRole::TextLabel,
        );
        assert!(sub.stitch_into(&arena));

        let frame = arena.flatten();
        assert_eq!(frame.accessibility_nodes.len(), 1);
        let node = frame.accessibility_nodes[0];
        assert_eq!(node.node_id, AccessibilityNodeId(42));
        assert_eq!(node.role, AccessibilityRole::TextLabel);
        assert_eq!(
            (node.x, node.y, node.width, node.height),
            (1.0, 2.0, 3.0, 4.0)
        );
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s (whole-number x offsets, no rounding), same \
                   reasoning as this crate's other exact-arithmetic tests"
    )]
    fn many_real_worker_threads_tag_accessibility_nodes_alongside_their_rects_concurrently() {
        const THREADS: usize = 4;

        let root = RenderingCanvas::new_with_sub_canvas_cap(THREADS);
        let arena = std::sync::Arc::new(FrameArena::with_capacity(
            THREADS * 4,
            THREADS * 6,
            THREADS,
            THREADS,
        ));

        let handles: Vec<_> = (0..THREADS)
            .map(|i| {
                let mut sub = root.create_sub_canvas();
                let arena = std::sync::Arc::clone(&arena);
                let node_id = AccessibilityNodeId(u64::try_from(i).unwrap());
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "THREADS is a small constant, far below f32's exact-integer range"
                )]
                let x = i as f32 * 20.0;
                std::thread::spawn(move || {
                    sub.draw_rounded_rect(x, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
                    sub.tag_accessibility_node(
                        node_id,
                        x,
                        0.0,
                        10.0,
                        10.0,
                        AccessibilityRole::Button,
                    );
                    assert!(
                        sub.stitch_into(&arena),
                        "arena was sized exactly for this test"
                    );
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("worker thread panicked");
        }

        let arena = std::sync::Arc::try_unwrap(arena)
            .unwrap_or_else(|_| panic!("all worker threads have joined"));
        let frame = arena.flatten();

        assert_eq!(frame.accessibility_nodes.len(), THREADS);
        for i in 0..THREADS {
            let expected_id = AccessibilityNodeId(u64::try_from(i).unwrap());
            #[allow(
                clippy::cast_precision_loss,
                reason = "THREADS is a small constant, far below f32's exact-integer range"
            )]
            let expected_x = i as f32 * 20.0;
            let found = frame.accessibility_nodes.iter().any(|node| {
                node.node_id == expected_id
                    && node.x == expected_x
                    && node.y == 0.0
                    && node.width == 10.0
                    && node.height == 10.0
            });
            assert!(
                found,
                "thread {i}'s tagged node at x={expected_x} must survive concurrent \
                 stitching with its correct bounds, regardless of thread scheduling"
            );
        }
    }

    /// A minimal `RhiPipelineState` double for `PipelineRegistry` tests --
    /// this crate has no Vulkan dependency and shouldn't gain one just to
    /// test a registry that is itself generic over the trait, not over
    /// any concrete backend.
    struct FakePipeline {
        raw_handle: u64,
    }

    impl RhiPipelineState for FakePipeline {
        fn raw_handle(&self) -> u64 {
            self.raw_handle
        }

        fn layout_handle(&self) -> u64 {
            self.raw_handle
        }
    }

    #[test]
    fn pipeline_registry_resolves_registered_ids_back_to_the_exact_object() {
        let mut registry = PipelineRegistry::new();
        registry.register(
            PipelineKind::SdfRoundedRect as u16,
            Box::new(FakePipeline { raw_handle: 111 }),
        );
        registry.register(
            PipelineKind::MsdfText as u16,
            Box::new(FakePipeline { raw_handle: 222 }),
        );

        assert_eq!(
            registry
                .get(PipelineKind::SdfRoundedRect as u16)
                .unwrap()
                .raw_handle(),
            111
        );
        assert_eq!(
            registry
                .get(PipelineKind::MsdfText as u16)
                .unwrap()
                .raw_handle(),
            222
        );
    }

    #[test]
    fn pipeline_registry_get_on_an_unregistered_id_returns_none_not_a_panic() {
        let registry = PipelineRegistry::new();
        assert!(registry.get(PipelineKind::SdfRoundedRect as u16).is_none());
    }

    #[test]
    #[should_panic(expected = "id 1 was already registered")]
    fn pipeline_registry_register_panics_on_a_duplicate_id() {
        let mut registry = PipelineRegistry::new();
        registry.register(
            PipelineKind::MsdfText as u16,
            Box::new(FakePipeline { raw_handle: 1 }),
        );
        registry.register(
            PipelineKind::MsdfText as u16,
            Box::new(FakePipeline { raw_handle: 2 }),
        );
    }

    /// A minimal `RhiBuffer` double for `execute_frame` tests, mirroring
    /// `FakePipeline`'s own reasoning above.
    struct FakeBuffer {
        raw_handle: u64,
    }

    impl RhiBuffer for FakeBuffer {
        fn raw_handle(&self) -> u64 {
            self.raw_handle
        }
    }

    /// Every call `execute_frame` can make against a
    /// `RhiCommandBuffer`, recorded with its real arguments (by the
    /// callee's own `raw_handle()`, not by object identity, since a
    /// `&dyn Trait` reference itself isn't comparable) -- lets tests
    /// assert the exact call sequence and its exact arguments, not just
    /// "some calls happened."
    #[derive(Debug, PartialEq)]
    enum RecordedCall {
        SetPipeline(u64),
        SetScissor(ScissorRect),
        BindTexture(u32, u32),
        BindVertexBuffer(u64, u32),
        BindIndexBuffer(u64, u32),
        DrawIndexed(u32, u32, i32),
        BeginRenderToTexture(u64, u32, u32),
        EndRenderToTexture(u64),
        BeginRenderToTextureNoEnd(u64, u32, u32),
        ResumeSwapchainRendering,
        AcquireTransientTarget(u32, u32),
        RegisterBindless(u64),
        DeregisterBindless(u32),
        ReleaseTransientTarget(u64),
        ApplyLayerBlur(u64, u32, u32),
    }

    #[derive(Default)]
    struct FakeCommandBuffer {
        calls: Vec<RecordedCall>,
    }

    impl RhiCommandBuffer for FakeCommandBuffer {
        fn set_pipeline(&mut self, pipeline: &dyn RhiPipelineState) {
            self.calls
                .push(RecordedCall::SetPipeline(pipeline.raw_handle()));
        }

        fn set_scissor(&mut self, rect: &ScissorRect) {
            self.calls.push(RecordedCall::SetScissor(*rect));
        }

        fn bind_vertex_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32) {
            self.calls
                .push(RecordedCall::BindVertexBuffer(buffer.raw_handle(), offset));
        }

        fn bind_index_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32) {
            self.calls
                .push(RecordedCall::BindIndexBuffer(buffer.raw_handle(), offset));
        }

        fn bind_texture(&mut self, slot: u32, bindless_index: u32) {
            self.calls
                .push(RecordedCall::BindTexture(slot, bindless_index));
        }

        fn draw_indexed(&mut self, index_count: u32, start_index: u32, base_vertex: i32) {
            self.calls.push(RecordedCall::DrawIndexed(
                index_count,
                start_index,
                base_vertex,
            ));
        }

        fn begin_render_to_texture(
            &mut self,
            texture: &dyn RhiTexture,
            logical_width: u32,
            logical_height: u32,
        ) {
            self.calls.push(RecordedCall::BeginRenderToTexture(
                texture.raw_handle(),
                logical_width,
                logical_height,
            ));
        }

        fn end_render_to_texture(&mut self, texture: &dyn RhiTexture) {
            self.calls
                .push(RecordedCall::EndRenderToTexture(texture.raw_handle()));
        }

        fn begin_render_to_texture_no_end(
            &mut self,
            texture: &dyn RhiTexture,
            logical_width: u32,
            logical_height: u32,
        ) {
            self.calls.push(RecordedCall::BeginRenderToTextureNoEnd(
                texture.raw_handle(),
                logical_width,
                logical_height,
            ));
        }

        fn resume_swapchain_rendering(&mut self) {
            self.calls.push(RecordedCall::ResumeSwapchainRendering);
        }

        fn apply_layer_blur(
            &mut self,
            _device: &dyn RhiDevice,
            source: &dyn RhiTexture,
            width: u32,
            height: u32,
        ) -> Box<dyn RhiTexture> {
            self.calls.push(RecordedCall::ApplyLayerBlur(
                source.raw_handle(),
                width,
                height,
            ));
            // A distinct raw_handle (888) from every other fake texture in
            // this test module -- lets a test assert the *blurred*
            // texture, not the original, is what gets composited.
            Box::new(FakeTexture {
                raw_handle: 888,
                width,
                height,
                format: TextureFormat::Rgba16Float,
            })
        }

        fn raw_handle(&self) -> u64 {
            0
        }
    }

    /// A minimal `RhiTexture` double, mirroring `FakePipeline`/
    /// `FakeBuffer`'s own reasoning above -- `bindless_index` starts
    /// `None`, matching `acquire_transient_target`'s own real contract
    /// (not bindless-registered by default).
    struct FakeTexture {
        raw_handle: u64,
        width: u32,
        height: u32,
        format: TextureFormat,
    }

    impl RhiTexture for FakeTexture {
        fn raw_handle(&self) -> u64 {
            self.raw_handle
        }
        fn image_handle(&self) -> u64 {
            self.raw_handle
        }
        fn memory_handle(&self) -> u64 {
            self.raw_handle
        }
        fn dimensions(&self) -> (u32, u32) {
            (self.width, self.height)
        }
        fn format(&self) -> TextureFormat {
            self.format
        }
        fn bindless_index(&self) -> Option<u32> {
            None
        }
        fn size_bytes(&self) -> u64 {
            u64::from(self.width) * u64::from(self.height) * 4
        }
    }

    /// A minimal `RhiDevice` double for `execute_frame`'s `PushLayer`/
    /// `PopLayer` tests (Step 6.4.2) -- only implements the resource-
    /// management methods those tests actually exercise
    /// (`acquire_transient_target`/`release_transient_target`/
    /// `register_bindless`/`deregister_bindless`); `begin_frame`/
    /// `submit_and_present`/`create_dynamic_ring_buffer`/`create_texture`
    /// are `unimplemented!()` since no `execute_frame` test needs them.
    /// Records its own calls into `calls` (a separate list from
    /// `FakeCommandBuffer`'s own -- `RhiDevice`'s methods all take `&self`,
    /// not `&mut self`, so a single shared list would need `RefCell`
    /// interior mutability on both fakes; two independently-ordered lists
    /// are simpler and just as conclusive, since `execute_frame` is
    /// single-threaded and each fake's own call order is what actually
    /// needs proving).
    #[derive(Default)]
    struct FakeDevice {
        calls: RefCell<Vec<RecordedCall>>,
        next_bindless_index: Cell<u32>,
        /// REVIEW.md finding #152's own regression test: when `Some`,
        /// `acquire_transient_target` returns a texture of *this* size
        /// instead of whatever was actually requested -- simulating
        /// `RhiDevice::acquire_transient_target`'s real, documented
        /// "oversized borrow" fallback, which this fake would otherwise
        /// have no way to reproduce (it always echoes the requested size
        /// back by default, unlike the real transient pool).
        oversized_borrow: Cell<Option<(u32, u32)>>,
    }

    impl RhiDevice for FakeDevice {
        fn create_dynamic_ring_buffer(&self, _capacity: usize) -> Box<dyn RhiDynamicRingBuffer> {
            unimplemented!("not exercised by any execute_frame test")
        }

        fn acquire_transient_target(
            &self,
            width: u32,
            height: u32,
            format: TextureFormat,
        ) -> Result<Box<dyn RhiTexture>, EngineError> {
            self.calls
                .borrow_mut()
                .push(RecordedCall::AcquireTransientTarget(width, height));
            let (returned_width, returned_height) =
                self.oversized_borrow.get().unwrap_or((width, height));
            Ok(Box::new(FakeTexture {
                raw_handle: 777,
                width: returned_width,
                height: returned_height,
                format,
            }))
        }

        fn release_transient_target(&self, texture: Box<dyn RhiTexture>) {
            self.calls
                .borrow_mut()
                .push(RecordedCall::ReleaseTransientTarget(texture.raw_handle()));
        }

        fn create_texture(
            &self,
            _width: u32,
            _height: u32,
            _format: TextureFormat,
            _pixels: &[u8],
        ) -> Result<Box<dyn RhiTexture>, EngineError> {
            unimplemented!("not exercised by any execute_frame test")
        }

        fn register_bindless(&self, texture: &dyn RhiTexture) -> Result<u32, EngineError> {
            self.calls
                .borrow_mut()
                .push(RecordedCall::RegisterBindless(texture.raw_handle()));
            let index = self.next_bindless_index.get();
            self.next_bindless_index.set(index + 1);
            Ok(index)
        }

        fn deregister_bindless(&self, bindless_index: u32) {
            self.calls
                .borrow_mut()
                .push(RecordedCall::DeregisterBindless(bindless_index));
        }

        fn begin_frame(
            &self,
            _swapchain: &dyn RhiSwapchain,
        ) -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError> {
            unimplemented!("not exercised by any execute_frame test")
        }

        fn submit_and_present(
            &self,
            _cmd_buffer: Box<dyn RhiCommandBuffer>,
            _swapchain: &dyn RhiSwapchain,
            _image: AcquiredImage,
        ) -> Result<(), EngineError> {
            unimplemented!("not exercised by any execute_frame test")
        }
    }

    /// A hand-built `UiDrawCommand`, defaulting every field this test
    /// module doesn't care about to an inert value -- extended per test
    /// with the specific fields under test, matching this crate's own
    /// established style for hand-constructed IR fixtures.
    fn marker_command(kind: CommandType) -> UiDrawCommand {
        UiDrawCommand {
            kind,
            sort_key: 0,
            pipeline_state_id: 0,
            texture_handle: 0,
            element_count: 0,
            vertex_offset: 0,
            clip_bounds: FULL_WINDOW_CLIP,
        }
    }

    #[test]
    fn execute_frame_dispatches_draw_geometry_and_scissor_markers_correctly() {
        let mut registry = PipelineRegistry::new();
        registry.register(
            PipelineKind::SdfRoundedRect as u16,
            Box::new(FakePipeline { raw_handle: 111 }),
        );
        registry.register(
            PipelineKind::MsdfText as u16,
            Box::new(FakePipeline { raw_handle: 222 }),
        );
        let vertex_buffer = FakeBuffer { raw_handle: 1000 };
        let index_buffer = FakeBuffer { raw_handle: 2000 };
        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };

        // Both markers here carry FULL_WINDOW_CLIP (marker_command's own
        // default), reproducing begin_overlay's real emission -- so this
        // also exercises the sentinel-substitution path, not just marker
        // vs. draw dispatch.
        let frame = FlattenedFrame {
            vertices: Vec::new(),
            indices: Vec::new(),
            commands: vec![
                marker_command(CommandType::PushScissor),
                UiDrawCommand {
                    kind: CommandType::DrawGeometry,
                    pipeline_state_id: PipelineKind::SdfRoundedRect as u16,
                    texture_handle: NO_TEXTURE,
                    element_count: 6,
                    vertex_offset: 0,
                    ..marker_command(CommandType::DrawGeometry)
                },
                marker_command(CommandType::PopScissor),
                UiDrawCommand {
                    kind: CommandType::DrawGeometry,
                    pipeline_state_id: PipelineKind::MsdfText as u16,
                    texture_handle: 7,
                    element_count: 24,
                    vertex_offset: 6,
                    ..marker_command(CommandType::DrawGeometry)
                },
            ],
            accessibility_nodes: Vec::new(),
        };

        let mut cmd_buffer = FakeCommandBuffer::default();
        let device = FakeDevice::default();
        execute_frame(
            &frame,
            &registry,
            BufferBinding {
                buffer: &vertex_buffer,
                offset: 0,
            },
            BufferBinding {
                buffer: &index_buffer,
                offset: 0,
            },
            &full_window,
            &device,
            &mut cmd_buffer,
        );

        assert_eq!(
            cmd_buffer.calls,
            vec![
                RecordedCall::BindVertexBuffer(1000, 0),
                RecordedCall::BindIndexBuffer(2000, 0),
                RecordedCall::SetScissor(full_window),
                RecordedCall::SetPipeline(111),
                RecordedCall::BindTexture(0, NO_TEXTURE),
                RecordedCall::DrawIndexed(6, 0, 0),
                RecordedCall::SetScissor(full_window),
                RecordedCall::SetPipeline(222),
                RecordedCall::BindTexture(0, 7),
                RecordedCall::DrawIndexed(24, 6, 0),
            ],
            "the vertex/index buffer must be bound exactly once, up front (finding #135) -- \
             not rebound before every DrawGeometry command -- and each PushScissor/PopScissor \
             must set the real full_window rect (not FULL_WINDOW_CLIP's own u32::MAX-sized \
             sentinel), with each DrawGeometry command resolving its own pipeline id to the \
             exact registered pipeline object and passing through its own real texture/\
             element_count/vertex_offset unchanged"
        );
    }

    #[test]
    fn execute_frame_binds_the_real_ring_buffer_offsets_it_was_given() {
        // IMPLEMENTATION.md Phase 8 Step 8.1.2: `execute_frame` used to
        // hardcode a `0` literal at both bind call sites, so a real
        // `RhiDynamicRingBuffer::write` offset (always non-zero once the
        // ring has advanced past its first segment) could never be
        // passed through. Non-zero, non-equal offsets here would have
        // failed against the old hardcoded-0 behavior.
        let mut registry = PipelineRegistry::new();
        registry.register(
            PipelineKind::SdfRoundedRect as u16,
            Box::new(FakePipeline { raw_handle: 111 }),
        );
        let vertex_buffer = FakeBuffer { raw_handle: 1000 };
        let index_buffer = FakeBuffer { raw_handle: 2000 };
        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };

        let frame = FlattenedFrame {
            vertices: Vec::new(),
            indices: Vec::new(),
            commands: vec![UiDrawCommand {
                kind: CommandType::DrawGeometry,
                pipeline_state_id: PipelineKind::SdfRoundedRect as u16,
                texture_handle: NO_TEXTURE,
                element_count: 6,
                vertex_offset: 0,
                ..marker_command(CommandType::DrawGeometry)
            }],
            accessibility_nodes: Vec::new(),
        };

        let mut cmd_buffer = FakeCommandBuffer::default();
        let device = FakeDevice::default();
        execute_frame(
            &frame,
            &registry,
            BufferBinding {
                buffer: &vertex_buffer,
                offset: 768,
            },
            BufferBinding {
                buffer: &index_buffer,
                offset: 256,
            },
            &full_window,
            &device,
            &mut cmd_buffer,
        );

        assert!(
            cmd_buffer
                .calls
                .contains(&RecordedCall::BindVertexBuffer(1000, 768)),
            "the real vertex_offset argument must reach RhiCommandBuffer::bind_vertex_buffer \
             verbatim, not the old hardcoded 0"
        );
        assert!(
            cmd_buffer
                .calls
                .contains(&RecordedCall::BindIndexBuffer(2000, 256)),
            "the real index_offset argument must reach RhiCommandBuffer::bind_index_buffer \
             verbatim, not the old hardcoded 0"
        );
    }

    #[test]
    fn execute_frame_scissor_stack_restores_the_correct_nested_rect_and_full_window_when_empty() {
        let registry = PipelineRegistry::new();
        let vertex_buffer = FakeBuffer { raw_handle: 1 };
        let index_buffer = FakeBuffer { raw_handle: 2 };
        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };
        let rect_a = ScissorRect {
            x: 10,
            y: 10,
            width: 100,
            height: 100,
        };
        let rect_b = ScissorRect {
            x: 20,
            y: 20,
            width: 30,
            height: 30,
        };

        let frame = FlattenedFrame {
            vertices: Vec::new(),
            indices: Vec::new(),
            commands: vec![
                UiDrawCommand {
                    clip_bounds: rect_a,
                    ..marker_command(CommandType::PushScissor)
                },
                UiDrawCommand {
                    clip_bounds: rect_b,
                    ..marker_command(CommandType::PushScissor)
                },
                marker_command(CommandType::PopScissor),
                marker_command(CommandType::PopScissor),
            ],
            accessibility_nodes: Vec::new(),
        };

        let mut cmd_buffer = FakeCommandBuffer::default();
        let device = FakeDevice::default();
        execute_frame(
            &frame,
            &registry,
            BufferBinding {
                buffer: &vertex_buffer,
                offset: 0,
            },
            BufferBinding {
                buffer: &index_buffer,
                offset: 0,
            },
            &full_window,
            &device,
            &mut cmd_buffer,
        );

        assert_eq!(
            cmd_buffer.calls,
            vec![
                RecordedCall::BindVertexBuffer(1, 0),
                RecordedCall::BindIndexBuffer(2, 0),
                RecordedCall::SetScissor(rect_a),
                RecordedCall::SetScissor(rect_b),
                RecordedCall::SetScissor(rect_a),
                RecordedCall::SetScissor(full_window),
            ],
            "popping the inner push must restore the outer rect from the runtime stack, not \
             full_window -- and only popping the outermost push, emptying the stack, must \
             restore full_window"
        );
    }

    #[test]
    #[should_panic(expected = "no pipeline registered for id 99")]
    fn execute_frame_panics_on_an_unregistered_pipeline_id() {
        let registry = PipelineRegistry::new();
        let vertex_buffer = FakeBuffer { raw_handle: 1 };
        let index_buffer = FakeBuffer { raw_handle: 2 };
        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };
        let frame = FlattenedFrame {
            vertices: Vec::new(),
            indices: Vec::new(),
            commands: vec![UiDrawCommand {
                kind: CommandType::DrawGeometry,
                pipeline_state_id: 99,
                ..marker_command(CommandType::DrawGeometry)
            }],
            accessibility_nodes: Vec::new(),
        };
        let mut cmd_buffer = FakeCommandBuffer::default();
        let device = FakeDevice::default();

        execute_frame(
            &frame,
            &registry,
            BufferBinding {
                buffer: &vertex_buffer,
                offset: 0,
            },
            BufferBinding {
                buffer: &index_buffer,
                offset: 0,
            },
            &full_window,
            &device,
            &mut cmd_buffer,
        );
    }

    #[test]
    fn execute_frame_with_no_commands_only_binds_buffers_once_and_draws_nothing() {
        let registry = PipelineRegistry::new();
        let vertex_buffer = FakeBuffer { raw_handle: 1 };
        let index_buffer = FakeBuffer { raw_handle: 2 };
        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };
        let frame = FlattenedFrame {
            vertices: Vec::new(),
            indices: Vec::new(),
            commands: Vec::new(),
            accessibility_nodes: Vec::new(),
        };
        let mut cmd_buffer = FakeCommandBuffer::default();
        let device = FakeDevice::default();

        execute_frame(
            &frame,
            &registry,
            BufferBinding {
                buffer: &vertex_buffer,
                offset: 0,
            },
            BufferBinding {
                buffer: &index_buffer,
                offset: 0,
            },
            &full_window,
            &device,
            &mut cmd_buffer,
        );

        // Finding #135: the vertex/index buffer bind is unconditional and
        // happens once up front, regardless of whether `frame` has any
        // commands at all -- cheap, and simpler than special-casing an
        // empty frame.
        assert_eq!(
            cmd_buffer.calls,
            vec![
                RecordedCall::BindVertexBuffer(1, 0),
                RecordedCall::BindIndexBuffer(2, 0),
            ]
        );
    }

    #[test]
    fn pop_layer_bakes_composite_quad_vertices_at_the_descs_own_screen_position() {
        let mut canvas = RenderingCanvas::new();
        canvas.push_layer(&LayerDesc {
            x: 50,
            y: 40,
            width: 100,
            height: 80,
            format: TextureFormat::Rgba16Float,
            blur: false,
        });
        canvas.pop_layer();
        let frame = canvas.flatten();

        assert_eq!(frame.vertices.len(), 4);
        let positions: Vec<[f32; 2]> = frame.vertices.iter().map(|v| v.position).collect();
        assert_eq!(
            positions,
            vec![[50.0, 40.0], [150.0, 40.0], [150.0, 120.0], [50.0, 120.0]],
            "the composite quad's own vertices must cover the popped LayerDesc's own x/y/width/ \
             height exactly, in raw screen space -- unlike draw_rounded_rect, never passed \
             through the active Canvas transform (LayerDesc.x/y match ScissorRect's own \
             untransformed semantics)"
        );
        let uvs: Vec<[f32; 2]> = frame.vertices.iter().map(|v| v.uv).collect();
        assert_eq!(
            uvs,
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            "a textured-quad composite samples its full (0,0)-(1,1) UV extent, not \
             draw_rounded_rect's own center-relative SDF convention"
        );

        let pop_command = frame
            .commands
            .iter()
            .find(|c| c.kind == CommandType::PopLayer)
            .expect("flatten must still emit a PopLayer command");
        assert_eq!(
            pop_command.pipeline_state_id,
            PipelineKind::TexturedQuad as u16
        );
        assert_eq!(
            pop_command.texture_handle, 0,
            "texture_handle carries the popped LayerDesc's own blur flag (Step 7.2.2), not \
             NO_TEXTURE -- this LayerDesc requested blur: false"
        );
        assert_eq!(pop_command.element_count, 6);
        assert_eq!(
            &frame.indices[pop_command.vertex_offset as usize..][..6],
            &[0, 1, 2, 0, 2, 3],
            "PopLayer's own vertex_offset must be rebased into the flattened frame's real \
             indices buffer (segment_and_flatten's fix), not left pointing at the canvas's own \
             pre-flatten raw index positions"
        );
    }

    #[test]
    fn execute_frame_push_pop_layer_drives_the_full_render_to_texture_round_trip_in_order() {
        let mut registry = PipelineRegistry::new();
        registry.register(
            PipelineKind::TexturedQuad as u16,
            Box::new(FakePipeline { raw_handle: 333 }),
        );
        let vertex_buffer = FakeBuffer { raw_handle: 1000 };
        let index_buffer = FakeBuffer { raw_handle: 2000 };
        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };

        let mut canvas = RenderingCanvas::new();
        canvas.push_layer(&LayerDesc {
            x: 10,
            y: 20,
            width: 64,
            height: 48,
            format: TextureFormat::Rgba16Float,
            blur: false,
        });
        canvas.pop_layer();
        let frame = canvas.flatten();

        let mut cmd_buffer = FakeCommandBuffer::default();
        let device = FakeDevice::default();
        execute_frame(
            &frame,
            &registry,
            BufferBinding {
                buffer: &vertex_buffer,
                offset: 0,
            },
            BufferBinding {
                buffer: &index_buffer,
                offset: 0,
            },
            &full_window,
            &device,
            &mut cmd_buffer,
        );

        assert_eq!(
            device.calls.into_inner(),
            vec![
                RecordedCall::AcquireTransientTarget(64, 48),
                RecordedCall::RegisterBindless(777),
                RecordedCall::DeregisterBindless(0),
                RecordedCall::ReleaseTransientTarget(777),
            ],
            "PushLayer must acquire a target sized to the LayerDesc, and PopLayer must \
             register/deregister bindless and release the target back to the pool, in that order"
        );
        assert_eq!(
            cmd_buffer.calls,
            vec![
                RecordedCall::BindVertexBuffer(1000, 0),
                RecordedCall::BindIndexBuffer(2000, 0),
                RecordedCall::BeginRenderToTexture(777, 64, 48),
                RecordedCall::EndRenderToTexture(777),
                RecordedCall::ResumeSwapchainRendering,
                RecordedCall::SetScissor(full_window),
                RecordedCall::SetPipeline(333),
                RecordedCall::BindTexture(0, 0),
                RecordedCall::DrawIndexed(6, 0, 0),
            ],
            "the vertex/index buffer is bound exactly once up front (finding #135); PopLayer \
             must resume swapchain rendering, re-apply the current clip stack's top (full_window \
             here, since no PushScissor is active), then draw the composite quad using the \
             just-registered bindless index (0) -- not the IR's own NO_TEXTURE placeholder"
        );
    }

    #[test]
    fn execute_frame_push_layer_passes_the_requested_size_not_an_oversized_borrowed_textures_own() {
        // REVIEW.md finding #152: `RhiDevice::acquire_transient_target`'s
        // real, documented "oversized borrow" fallback can hand back a
        // texture larger than requested. `FakeDevice::oversized_borrow`
        // simulates exactly that -- a 50x40 layer is requested, but the
        // fake returns a 200x150 texture, mirroring the real pool handing
        // back a larger, already-freed bucket. `execute_frame`'s own
        // `PushLayer` handling must still tell `begin_render_to_texture`
        // the *requested* 50x40 size, not the oversized texture's own
        // 200x150 -- this is the exact defect a real GPU repro found in
        // this session (a genuinely smaller layer's content vanishing
        // from the composited frame after a larger one was released).
        let mut registry = PipelineRegistry::new();
        registry.register(
            PipelineKind::TexturedQuad as u16,
            Box::new(FakePipeline { raw_handle: 333 }),
        );
        let vertex_buffer = FakeBuffer { raw_handle: 1000 };
        let index_buffer = FakeBuffer { raw_handle: 2000 };
        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };

        let mut canvas = RenderingCanvas::new();
        canvas.push_layer(&LayerDesc {
            x: 220,
            y: 20,
            width: 50,
            height: 40,
            format: TextureFormat::Rgba16Float,
            blur: false,
        });
        canvas.pop_layer();
        let frame = canvas.flatten();

        let mut cmd_buffer = FakeCommandBuffer::default();
        let device = FakeDevice::default();
        device.oversized_borrow.set(Some((200, 150)));
        execute_frame(
            &frame,
            &registry,
            BufferBinding {
                buffer: &vertex_buffer,
                offset: 0,
            },
            BufferBinding {
                buffer: &index_buffer,
                offset: 0,
            },
            &full_window,
            &device,
            &mut cmd_buffer,
        );

        assert_eq!(
            device.calls.into_inner()[0],
            RecordedCall::AcquireTransientTarget(50, 40),
            "PushLayer must still request the LayerDesc's own real size from the pool, \
             regardless of what it's handed back"
        );
        assert!(
            cmd_buffer
                .calls
                .contains(&RecordedCall::BeginRenderToTexture(777, 50, 40)),
            "begin_render_to_texture must be told the requested 50x40 size, not the oversized \
             texture's own 200x150 -- got {:?}",
            cmd_buffer.calls
        );
    }

    #[test]
    fn execute_frame_pop_layer_with_blur_composites_the_blurred_texture_not_the_raw_one() {
        // Step 7.2.2: LayerDesc.blur, smuggled through PopLayer's own
        // command.texture_handle field, must make execute_frame call
        // apply_layer_blur and composite *its* returned texture (raw_
        // handle 888 in FakeCommandBuffer's own double) instead of the
        // layer's own raw content (raw_handle 777, FakeDevice's own
        // fixed acquire_transient_target return value) -- and release
        // the original, now-unneeded layer texture along the way.
        let mut registry = PipelineRegistry::new();
        registry.register(
            PipelineKind::TexturedQuad as u16,
            Box::new(FakePipeline { raw_handle: 333 }),
        );
        let vertex_buffer = FakeBuffer { raw_handle: 1000 };
        let index_buffer = FakeBuffer { raw_handle: 2000 };
        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };

        let mut canvas = RenderingCanvas::new();
        canvas.push_layer(&LayerDesc {
            x: 10,
            y: 20,
            width: 64,
            height: 48,
            format: TextureFormat::Rgba16Float,
            blur: true,
        });
        canvas.pop_layer();
        let frame = canvas.flatten();

        let mut cmd_buffer = FakeCommandBuffer::default();
        let device = FakeDevice::default();
        execute_frame(
            &frame,
            &registry,
            BufferBinding {
                buffer: &vertex_buffer,
                offset: 0,
            },
            BufferBinding {
                buffer: &index_buffer,
                offset: 0,
            },
            &full_window,
            &device,
            &mut cmd_buffer,
        );

        assert!(
            cmd_buffer
                .calls
                .contains(&RecordedCall::ApplyLayerBlur(777, 64, 48)),
            "apply_layer_blur must be called with the layer's own raw texture and its real \
             requested size -- got {:?}",
            cmd_buffer.calls
        );
        assert_eq!(
            device.calls.into_inner(),
            vec![
                RecordedCall::AcquireTransientTarget(64, 48),
                RecordedCall::ReleaseTransientTarget(777),
                RecordedCall::RegisterBindless(888),
                RecordedCall::DeregisterBindless(0),
                RecordedCall::ReleaseTransientTarget(888),
            ],
            "the original layer texture (777) must be released right after blurring, and the \
             blurred texture (888) must be what gets registered bindless, composited, and \
             released -- not the original"
        );
    }

    #[test]
    #[should_panic(expected = "nested PushLayer is not supported")]
    fn execute_frame_panics_on_a_nested_push_layer() {
        let registry = PipelineRegistry::new();
        let vertex_buffer = FakeBuffer { raw_handle: 1 };
        let index_buffer = FakeBuffer { raw_handle: 2 };
        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };
        let frame = FlattenedFrame {
            vertices: Vec::new(),
            indices: Vec::new(),
            commands: vec![
                UiDrawCommand {
                    clip_bounds: ScissorRect {
                        x: 0,
                        y: 0,
                        width: 10,
                        height: 10,
                    },
                    ..marker_command(CommandType::PushLayer)
                },
                UiDrawCommand {
                    clip_bounds: ScissorRect {
                        x: 0,
                        y: 0,
                        width: 20,
                        height: 20,
                    },
                    ..marker_command(CommandType::PushLayer)
                },
            ],
            accessibility_nodes: Vec::new(),
        };
        let mut cmd_buffer = FakeCommandBuffer::default();
        let device = FakeDevice::default();

        execute_frame(
            &frame,
            &registry,
            BufferBinding {
                buffer: &vertex_buffer,
                offset: 0,
            },
            BufferBinding {
                buffer: &index_buffer,
                offset: 0,
            },
            &full_window,
            &device,
            &mut cmd_buffer,
        );
    }

    #[test]
    #[should_panic(expected = "PopLayer with no active PushLayer")]
    fn execute_frame_panics_on_a_pop_layer_with_no_active_push_layer() {
        let registry = PipelineRegistry::new();
        let vertex_buffer = FakeBuffer { raw_handle: 1 };
        let index_buffer = FakeBuffer { raw_handle: 2 };
        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };
        let frame = FlattenedFrame {
            vertices: Vec::new(),
            indices: Vec::new(),
            commands: vec![marker_command(CommandType::PopLayer)],
            accessibility_nodes: Vec::new(),
        };
        let mut cmd_buffer = FakeCommandBuffer::default();
        let device = FakeDevice::default();

        execute_frame(
            &frame,
            &registry,
            BufferBinding {
                buffer: &vertex_buffer,
                offset: 0,
            },
            BufferBinding {
                buffer: &index_buffer,
                offset: 0,
            },
            &full_window,
            &device,
            &mut cmd_buffer,
        );
    }
}
