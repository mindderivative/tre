//! Core engine crate: the `Canvas` API, intermediate representation,
//! sort/batch pipeline, dynamic texture atlas, and SVG/MSDF tessellation.
//!
//! Pure safe Rust -- see TECHNICAL.md Section 9.1 for the workspace's
//! `unsafe` policy. Raw graphics-API FFI lives in the `tre-rhi-*` crates,
//! and zero-allocation buffer/arena/atlas-concurrency primitives live in
//! `tre-memory`; this crate depends on both but contains no `unsafe` itself.
#![forbid(unsafe_code)]

mod gpu_style;
pub use gpu_style::{
    style_index_param, GpuEllipseStyle, GpuGradientStyle, GpuRectStyle, StyleFill,
    ELLIPSE_STYLE_WORDS, GRADIENT_MAX_STOPS, GRADIENT_STYLE_WORDS, RECT_STYLE_WORDS,
};

mod shapes;
pub use shapes::{
    flatten_path, AnimationId, BlendMode, Circle, Color as ShapeColor, CornerRadii, FillStyle,
    GradientDef, GradientError, GradientId, GradientKind, GradientStop, LineCap, LineJoin, Path,
    PathCommand, Polygon, Primitive, PrimitiveCommon, Rectangle, ShapeId, ShapePrimitive,
    ShapeRegistry, ShapeSlot, Svg, Transform2D, Vec2 as ShapeVec2, Visibility,
};

mod input;
pub use input::{ElementState, FrameClock, InputEvent, InputEventQueue, MouseButton, WindowId};

mod text;
pub use text::{FontError, FontId, FontRegistry, Text, TextFlattenContext};

mod canvas;
#[cfg(test)]
pub(crate) use canvas::{
    compute_sort_key, flatten_run, premultiply_alpha, radix_sort_by_key, DEPTH_ID_MASK,
};
pub use canvas::{FrameArena, GlyphAtlasContext, OverlayLayerPriority, RenderingCanvas, SubCanvas};

mod rhi;
pub use rhi::{
    execute_frame, submit_frame, AcquiredImage, BufferBinding, PipelineRegistry, RhiBuffer,
    RhiCommandBuffer, RhiDevice, RhiDynamicRingBuffer, RhiPipelineState, RhiSwapchain, RhiTexture,
};

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

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceLost => write!(
                f,
                "GPU device lost (removal, driver TDR, or a stale swapchain)"
            ),
            Self::SwapchainOutOfDate => write!(f, "swapchain is out of date and must be recreated"),
            Self::PipelineCreationFailed => write!(f, "graphics pipeline creation failed"),
            Self::InvalidTextureData => {
                write!(f, "texture pixel data doesn't match width/height/format")
            }
            Self::BindlessArrayExhausted => {
                write!(f, "bindless texture array has no free slots left")
            }
            Self::TransientPoolBudgetExceeded => {
                write!(f, "transient render target pool's VRAM budget exceeded")
            }
        }
    }
}

impl std::error::Error for EngineError {}

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
    /// Phase 10 Step 10.2: non-uniform corner radii / border / corner
    /// smoothing, sourced from a `GpuRectStyle` record
    /// (`RenderingCanvas::draw_styled_rectangle`). Deliberately a
    /// separate pipeline/shader from `SdfRoundedRect`, not a second code
    /// path inside it -- `draw_rounded_rect`'s existing callers and tests
    /// stay byte-for-byte unaffected.
    SdfRectStyled = 3,
    /// Phase 10 Step 10.2: Circle/Ellipse, sourced from a
    /// `GpuEllipseStyle` record (`RenderingCanvas::draw_ellipse`).
    SdfEllipse = 4,
    /// Phase 10 Step 10.2: plain flat-vertex-color triangle fill
    /// (`RenderingCanvas::draw_flat_polygon`) -- `Polygon`/`Path` fill,
    /// via `walking_skeleton.vert`/`.frag` (Phase 0's own placeholder
    /// shader, unmodified). This is that shader's first real `Canvas`
    /// caller; `planning/archive/PLAN_PHASE6_STEP6_1.md`'s "Scope
    /// decisions" note it as deliberately unrepresented until one existed
    /// (see also REVIEW.md's Phase 10 Step 10.2 finding on
    /// `walking_skeleton.frag`'s own non-premultiplied output, disclosed
    /// but not fixed here).
    FlatColor = 5,
    /// Phase 10 Step 10.2.1: `Polygon`/`Path` gradient fill
    /// (`RenderingCanvas::draw_gradient_polygon`). A separate pipeline
    /// from `FlatColor`, not a branch inside `walking_skeleton.frag` --
    /// `FlatColor`'s existing solid-fill callers stay byte-for-byte
    /// unaffected, matching every other real style variant's own
    /// "separate pipeline" precedent in this enum (`SdfRectStyled` next
    /// to `SdfRoundedRect`, `SdfEllipse` next to nothing before it). Its
    /// own `gradient_fill.vert`/`.frag` read a gradient's word index from
    /// the SAME per-draw push constant `TexturedQuad`'s `texture_index`
    /// already uses (`RenderingCanvas::draw_gradient_polygon`'s own doc
    /// comment has the full account of why Polygon/Path can't use a
    /// per-vertex style record the way `Rectangle`/`Circle` do).
    GradientFill = 6,
    /// Phase 10 Step 10.2.3: `Polygon`/`Path` solid fill under a
    /// non-`Normal` `BlendMode` (`RenderingCanvas::draw_flat_polygon_
    /// blended`). Reads the destination pixel a PRECEDING draw already
    /// wrote via `VK_KHR_dynamic_rendering_local_read` (a real framebuffer
    /// read, not a `VkBlendOp` selection -- `VK_EXT_blend_operation_
    /// advanced`, this project's own original assumption, is not
    /// implemented by RADV, this project's own real dev GPU driver, as
    /// of Mesa 26.1; confirmed both locally via `vulkaninfo` and via
    /// Mesa's own release notes, REVIEW.md's own account of this
    /// finding has the full story), computes the requested blend formula
    /// itself, and writes the already-composited result with hardware
    /// blending DISABLED. `RhiDevice::local_read_blend_supported` is the
    /// real, disclosed capability gate this pipeline is only ever
    /// selected behind -- unsupported hardware falls back to plain
    /// `FlatColor` (`Normal` blending), never a silent wrong render.
    FlatColorBlend = 7,
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

/// Computes the screen-space `(x, y, width, height)` a shadow's own
/// [`LayerDesc`] should use for a shape at `(x, y, width, height)`,
/// casting a shadow offset by `(offset_x, offset_y)`, given
/// `blur_margin` extra pixels on every side for the Dual-Kawase blur to
/// spread into without being clipped at the layer's own edge (Phase 13
/// Step 13.4: shadows, built entirely on `LayerDesc.blur`'s already-real
/// Dual-Kawase blur -- no new GPU pipeline).
///
/// The result is the union of the shape's own bounding box and its
/// offset copy, expanded by `blur_margin` on every side -- large enough
/// to contain both the shape's real position and its shifted shadow
/// silhouette, with room around the shadow's own edges for the blur.
///
/// # Panics
/// In debug builds, panics if `blur_margin` is negative -- a negative
/// margin would shrink the layer below the shadow's own extent, silently
/// clipping it, which is never the real intent of a caller passing one.
#[must_use]
pub fn shadow_layer_bounds(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    offset_x: f32,
    offset_y: f32,
    blur_margin: f32,
) -> (i32, i32, u32, u32) {
    debug_assert!(
        blur_margin >= 0.0,
        "shadow_layer_bounds: blur_margin must be >= 0.0, got {blur_margin}"
    );
    let left = x.min(x + offset_x) - blur_margin;
    let top = y.min(y + offset_y) - blur_margin;
    let right = (x + width).max(x + offset_x + width) + blur_margin;
    let bottom = (y + height).max(y + offset_y + height) + blur_margin;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "screen-space layer bounds are always well within i32/u32 range for any real \
                  UI, and width/height are real spans (right >= left, bottom >= top always hold \
                  since blur_margin >= 0.0 is enforced above) so never actually negative"
    )]
    {
        (
            left.floor() as i32,
            top.floor() as i32,
            (right - left).ceil() as u32,
            (bottom - top).ceil() as u32,
        )
    }
}

/// A frame's fully-recorded, sorted-and-flattened batch: one contiguous
/// vertex/index stream plus the (currently trivial, Phase 0) list of
/// draw commands describing how to slice it into RHI draw calls, plus
/// (Step 5.3.1) every tagged accessibility node recorded this frame.
///
/// `Default` (Phase 9 Step 9.2): a real caller reusing one `FlattenedFrame`
/// across many frames via [`FrameArena::flatten_into`] needs an empty
/// starting value to construct once, before its own loop begins.
#[derive(Default)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::sync::Mutex;

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
    fn frame_clock_elapsed_accumulates_independently_of_tick() {
        let clock = FrameClock::new();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let elapsed = clock.elapsed();
        assert!(
            elapsed >= 0.015,
            "a real ~20ms sleep must report at least 15ms elapsed, got {elapsed}s"
        );
        assert!(
            elapsed < 1.0,
            "a real ~20ms sleep must not report a wildly inflated elapsed time, got {elapsed}s"
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
    fn draw_styled_rectangle_emits_one_command_using_the_styled_pipeline() {
        let device = FakeDevice::default();
        let mut canvas = RenderingCanvas::new();
        canvas.draw_styled_rectangle(
            &device,
            0.0,
            0.0,
            100.0,
            40.0,
            [4.0, 8.0, 12.0, 16.0],
            0xFF00_FFFF,
            0xFF00_00FF,
            2.0,
            0.5,
            StyleFill::SOLID,
        );
        let frame = canvas.flatten();

        assert_eq!(frame.commands.len(), 1);
        assert_eq!(frame.vertices.len(), 4);
        assert_eq!(frame.indices.len(), 6);
        assert_eq!(
            frame.commands[0].pipeline_state_id,
            PipelineKind::SdfRectStyled as u16
        );
        assert_eq!(frame.commands[0].texture_handle, NO_TEXTURE);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s, same reasoning as draw_rounded_rect's own \
                   exact-arithmetic tests"
    )]
    fn draw_styled_rectangle_writes_a_real_style_record_and_embeds_its_word_index() {
        let device = FakeDevice::default();
        let mut canvas = RenderingCanvas::new();
        canvas.draw_styled_rectangle(
            &device,
            10.0,
            20.0,
            100.0,
            40.0,
            [4.0, 8.0, 12.0, 16.0],
            0xFF00_FFFF,
            0xAABB_CCDD,
            3.0,
            0.25,
            StyleFill::SOLID,
        );
        let frame = canvas.flatten();

        // Every vertex carries the SAME style word index (uniform per
        // quad, matching draw_rounded_rect's own "no per-quad channel"
        // convention) -- word index 0, since this is the first (only)
        // write into a fresh FakeStyleBuffer.
        for vertex in &frame.vertices {
            assert_eq!(floatBitsToUint_test_helper(vertex.params[0]), 0);
            assert_eq!(vertex.params[1], 50.0, "half_width");
            assert_eq!(vertex.params[2], 20.0, "half_height");
        }

        let bytes = device
            .style_buffer
            .bytes
            .lock()
            .expect("FakeStyleBuffer mutex poisoned");
        assert_eq!(bytes.len(), 40, "one GpuRectStyle record (10 words)");
        let style: &GpuRectStyle = bytemuck::from_bytes(&bytes);
        assert_eq!(style.corner_radii, [4.0, 8.0, 12.0, 16.0]);
        assert_eq!(style.border_color, 0xAABB_CCDD);
        assert_eq!(style.border_thickness, 3.0);
        assert_eq!(style.corner_smoothing, 0.25);
        assert_eq!(style.fill_kind, 0);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s, same reasoning as draw_rounded_rect's own \
                   exact-arithmetic tests"
    )]
    fn draw_styled_rectangle_clamps_each_corner_radius_independently() {
        let device = FakeDevice::default();
        let mut canvas = RenderingCanvas::new();
        // half_width=50, half_height=20 -> max radius 20.0.
        canvas.draw_styled_rectangle(
            &device,
            0.0,
            0.0,
            100.0,
            40.0,
            [1000.0, -5.0, 10.0, 1000.0],
            0xFF00_FFFF,
            0,
            0.0,
            0.0,
            StyleFill::SOLID,
        );
        let bytes = device
            .style_buffer
            .bytes
            .lock()
            .expect("FakeStyleBuffer mutex poisoned");
        let style: &GpuRectStyle = bytemuck::from_bytes(&bytes);
        assert_eq!(style.corner_radii, [20.0, 0.0, 10.0, 20.0]);
    }

    #[test]
    fn draw_ellipse_emits_one_command_using_the_ellipse_pipeline() {
        let device = FakeDevice::default();
        let mut canvas = RenderingCanvas::new();
        canvas.draw_ellipse(
            &device,
            50.0,
            50.0,
            [30.0, 20.0],
            0xFF00_FFFF,
            0,
            0.0,
            0.0,
            std::f32::consts::TAU,
            StyleFill::SOLID,
        );
        let frame = canvas.flatten();

        assert_eq!(frame.commands.len(), 1);
        assert_eq!(frame.vertices.len(), 4);
        assert_eq!(
            frame.commands[0].pipeline_state_id,
            PipelineKind::SdfEllipse as u16
        );
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s, same reasoning as draw_rounded_rect's own \
                   exact-arithmetic tests"
    )]
    fn draw_ellipse_writes_a_real_style_record_with_arc_angles() {
        let device = FakeDevice::default();
        let mut canvas = RenderingCanvas::new();
        canvas.draw_ellipse(
            &device,
            0.0,
            0.0,
            [30.0, 30.0],
            0xFF00_FFFF,
            0x1122_3344,
            2.5,
            0.1,
            1.5,
            StyleFill::SOLID,
        );
        let frame = canvas.flatten();

        for vertex in &frame.vertices {
            assert_eq!(vertex.params[1], 30.0);
            assert_eq!(vertex.params[2], 30.0);
        }

        let bytes = device
            .style_buffer
            .bytes
            .lock()
            .expect("FakeStyleBuffer mutex poisoned");
        assert_eq!(bytes.len(), 28, "one GpuEllipseStyle record (7 words)");
        let style: &GpuEllipseStyle = bytemuck::from_bytes(&bytes);
        assert_eq!(style.border_color, 0x1122_3344);
        assert_eq!(style.border_thickness, 2.5);
        assert_eq!(style.arc_start_angle, 0.1);
        assert_eq!(style.arc_sweep_angle, 1.5);
        assert_eq!(style.fill_kind, 0);
    }

    /// A tiny standalone `floatBitsToUint` mirror for asserting a
    /// `UiVertex.params[0]` style-index encoding in tests without
    /// depending on `gpu_style::style_index_param`'s own inverse (which
    /// would make the test tautological against the function it's meant
    /// to check).
    #[allow(
        non_snake_case,
        reason = "mirrors the GLSL intrinsic it stands in for by name"
    )]
    fn floatBitsToUint_test_helper(value: f32) -> u32 {
        value.to_bits()
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
        let mut counts = Vec::new();
        let mut out_commands = Vec::new();
        let mut out_indices = Vec::new();
        flatten_run(
            &mut run,
            &mut sort_scratch,
            &mut counts,
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
        let mut counts = Vec::new();
        radix_sort_by_key(&mut items, &mut scratch, &mut counts, |&k| k);
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
        let mut counts = Vec::new();
        radix_sort_by_key(&mut items, &mut scratch, &mut counts, |&k| k);
    }

    #[test]
    fn radix_sort_reuses_the_same_counts_buffer_across_calls_without_reallocating() {
        // Phase 9 Step 9.2: `counts` used to be a fresh `vec![0u32;
        // RADIX_BUCKETS]` allocated on *every* radix_sort_by_key call --
        // a real, previously-undetected per-run heap allocation the new
        // zero-allocation debug guard (tre-memory) caught in
        // main_loop_demo.rs. This proves the fix directly: the same
        // `counts` Vec, reused across two independent sort calls, must
        // never change its own backing allocation after the first call
        // grows it to RADIX_BUCKETS.
        let mut counts = Vec::new();

        let mut first_items = vec![5u64, 3, 1];
        let mut first_scratch = first_items.clone();
        radix_sort_by_key(&mut first_items, &mut first_scratch, &mut counts, |&k| k);
        assert_eq!(first_items, vec![1, 3, 5]);
        let counts_ptr_after_first_call = counts.as_ptr();

        let mut second_items = vec![9u64, 2, 7, 4];
        let mut second_scratch = second_items.clone();
        radix_sort_by_key(&mut second_items, &mut second_scratch, &mut counts, |&k| k);
        assert_eq!(second_items, vec![2, 4, 7, 9]);
        assert_eq!(
            counts.as_ptr(),
            counts_ptr_after_first_call,
            "counts must reuse its existing backing allocation on a second call, not reallocate \
             -- this is the whole point of threading it through as a caller-provided buffer"
        );
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
        reason = "exact arithmetic on literal f32s (whole-number x/y offsets, no rounding), \
                   same reasoning as this crate's other exact-arithmetic tests"
    )]
    fn stitch_into_no_longer_consumes_the_canvas_and_can_be_called_again_after_reset() {
        // Phase 9 Step 9.2 (REVIEW.md finding #134): stitch_into takes
        // &self now, so the same canvas can be reset() and reused for a
        // second frame instead of being constructed fresh every time.
        let mut canvas = RenderingCanvas::new();
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);

        let first_arena = FrameArena::with_capacity(4, 6, 1, 0);
        assert!(canvas.stitch_into(&first_arena));
        let first_frame = first_arena.flatten();
        assert_eq!(first_frame.vertices.len(), 4);

        canvas.reset();
        assert!(
            canvas.stitch_into(&FrameArena::with_capacity(4, 6, 1, 0)),
            "a freshly reset() canvas has recorded nothing -- stitching it into any arena, \
             even a zero-capacity-shaped one that only fits nothing, must trivially succeed"
        );

        canvas.draw_rounded_rect(50.0, 50.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        let second_arena = FrameArena::with_capacity(4, 6, 1, 0);
        assert!(canvas.stitch_into(&second_arena));
        let second_frame = second_arena.flatten();
        assert_eq!(
            second_frame.vertices[0].position,
            [50.0, 50.0],
            "the reused canvas's second frame must contain only the second rect, not a stale \
             leftover copy of the first"
        );
    }

    #[test]
    fn reset_restores_the_exact_new_state_including_the_shared_depth_id_counter() {
        let mut root = RenderingCanvas::new();
        root.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        root.draw_rounded_rect(10.0, 10.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        root.reset();

        // A fresh RenderingCanvas::new() draws its first command with
        // Depth ID 0 -- if reset() genuinely reproduces that starting
        // point, drawing once more here and flattening must show the
        // exact same sort_key a lone fresh canvas's own first draw would.
        root.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        let reset_then_drawn = root.flatten();

        let mut fresh = RenderingCanvas::new();
        fresh.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        let fresh_drawn = fresh.flatten();

        assert_eq!(
            reset_then_drawn.commands[0].sort_key, fresh_drawn.commands[0].sort_key,
            "reset() must reproduce new()'s own Depth ID counter starting point exactly, or a \
             long-running reused canvas would drift from a fresh canvas's own sort order"
        );
        assert_eq!(
            reset_then_drawn.vertices.len(),
            4,
            "only the post-reset draw must remain"
        );
    }

    #[test]
    fn flatten_into_produces_the_same_result_as_flatten_across_repeated_reused_frames() {
        // Phase 9 Step 9.2 (REVIEW.md finding #134): flatten_into is the
        // reusable, non-consuming sibling of flatten() -- feeding it the
        // exact same real scene across several simulated "frames," reused
        // via reset()/flatten_into() throughout, must produce identical
        // output to a lone, fresh flatten() call every single time.
        let mut out = FlattenedFrame::default();
        let mut arena = FrameArena::with_capacity(8, 12, 2, 0);

        for round in 0..3 {
            let mut root = RenderingCanvas::new();
            root.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
            let mut sub = root.create_sub_canvas();
            sub.draw_rounded_rect(20.0, 20.0, 10.0, 10.0, 0.0, 0xAABB_CCDD);
            assert!(sub.stitch_into(&arena), "round {round}: sub stitch failed");
            assert!(
                root.stitch_into(&arena),
                "round {round}: root stitch failed"
            );

            arena.flatten_into(&mut out);

            let mut expected_root = RenderingCanvas::new();
            expected_root.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
            let mut expected_sub = expected_root.create_sub_canvas();
            expected_sub.draw_rounded_rect(20.0, 20.0, 10.0, 10.0, 0.0, 0xAABB_CCDD);
            let expected_arena = FrameArena::with_capacity(8, 12, 2, 0);
            assert!(expected_sub.stitch_into(&expected_arena));
            assert!(expected_root.stitch_into(&expected_arena));
            let expected = expected_arena.flatten();

            assert_eq!(out.vertices.len(), expected.vertices.len(), "round {round}");
            assert_eq!(out.indices, expected.indices, "round {round}");
            assert_eq!(out.commands.len(), expected.commands.len(), "round {round}");
            assert_eq!(
                out.accessibility_nodes.len(),
                expected.accessibility_nodes.len(),
                "round {round}"
            );
        }
    }

    #[test]
    fn flatten_into_reuses_out_and_arena_scratch_buffers_across_calls_without_reallocating() {
        // The whole point of flatten_into over flatten(): once every
        // internal buffer has grown to this session's steady-state size,
        // a later call at the same or smaller scene size must not
        // reallocate any of out's own Vecs.
        let mut out = FlattenedFrame::default();
        let mut arena = FrameArena::with_capacity(4, 6, 1, 0);

        let mut canvas = RenderingCanvas::new();
        canvas.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        assert!(canvas.stitch_into(&arena));
        arena.flatten_into(&mut out);

        let vertices_ptr = out.vertices.as_ptr();
        let indices_ptr = out.indices.as_ptr();
        let commands_ptr = out.commands.as_ptr();

        let mut canvas2 = RenderingCanvas::new();
        canvas2.draw_rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0, 0xFFFF_FFFF);
        assert!(canvas2.stitch_into(&arena));
        arena.flatten_into(&mut out);

        assert_eq!(
            out.vertices.as_ptr(),
            vertices_ptr,
            "out.vertices must reuse its existing backing allocation on a same-size second frame"
        );
        assert_eq!(
            out.indices.as_ptr(),
            indices_ptr,
            "out.indices must reuse its existing backing allocation on a same-size second frame"
        );
        assert_eq!(
            out.commands.as_ptr(),
            commands_ptr,
            "out.commands must reuse its existing backing allocation on a same-size second frame"
        );
    }

    #[test]
    #[should_panic(expected = "save/restore calls are unbalanced")]
    fn flatten_into_still_enforces_the_balance_assertion_on_the_reused_path() {
        // Phase 9 Step 9.2 task 2: the balance-assertion gate must keep
        // working on flatten_into's own new, reused-arena path, not just
        // the original consuming flatten().
        let mut out = FlattenedFrame::default();
        let mut arena = FrameArena::with_capacity(4, 6, 1, 0);
        let mut canvas = RenderingCanvas::new();
        canvas.save();
        assert!(canvas.stitch_into(&arena));
        arena.flatten_into(&mut out);
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
        InsertBlendReadBarrier,
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

        fn insert_blend_read_barrier(&mut self) {
            self.calls.push(RecordedCall::InsertBlendReadBarrier);
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
    /// Phase 10 Step 10.2: a minimal `RhiDynamicRingBuffer` double for
    /// `draw_styled_rectangle`/`draw_ellipse` tests -- a plain,
    /// alignment-agnostic bump allocator. Real segment-rotation/alignment
    /// behavior is `VulkanRingBuffer`'s own concern (`tre-rhi-vulkan`),
    /// not exercised here; these tests only need something real to write
    /// into so the returned byte offset (and its `style_index_param`
    /// numeric encoding) can be asserted against.
    #[derive(Default)]
    struct FakeStyleBuffer {
        // `Mutex`, not `RefCell` (REVIEW.md #203/#204's own fallout):
        // `RhiBuffer` gained a real `Send + Sync` bound this fix needed,
        // so every fake implementing it must be `Sync` too -- a test
        // double never actually touched from more than one thread, but
        // still has to satisfy the same trait bound real implementors do.
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
        /// Phase 10 Step 10.2: backs `shape_style_buffer()` for
        /// `draw_styled_rectangle`/`draw_ellipse` tests.
        style_buffer: FakeStyleBuffer,
        /// Phase 10 Step 10.2.3: `false` by default (`Default`'s own
        /// zero value), matching real hardware that lacks
        /// `VK_KHR_dynamic_rendering_local_read` -- tests that need the
        /// "supported" branch set this via `Cell::set` before use.
        local_read_blend_supported: Cell<bool>,
    }

    impl RhiDevice for FakeDevice {
        fn create_dynamic_ring_buffer(&self, _capacity: usize) -> Box<dyn RhiDynamicRingBuffer> {
            unimplemented!("not exercised by any execute_frame test")
        }

        fn shape_style_buffer(&self) -> &dyn RhiDynamicRingBuffer {
            &self.style_buffer
        }

        fn local_read_blend_supported(&self) -> bool {
            self.local_read_blend_supported.get()
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
    fn shadow_layer_bounds_covers_the_shape_and_its_offset_shadow_plus_margin() {
        // A 100x50 rect at (10, 20), shadow offset (8, 12), 5px blur
        // margin. left = min(10, 18) - 5 = 5. top = min(20, 32) - 5 =
        // 15. right = max(110, 118) + 5 = 123. bottom = max(70, 82) + 5
        // = 87. width = 123 - 5 = 118. height = 87 - 15 = 72.
        let (x, y, width, height) = shadow_layer_bounds(10.0, 20.0, 100.0, 50.0, 8.0, 12.0, 5.0);
        assert_eq!((x, y, width, height), (5, 15, 118, 72));
    }

    #[test]
    fn shadow_layer_bounds_with_zero_offset_and_margin_matches_the_shapes_own_aabb_exactly() {
        let (x, y, width, height) = shadow_layer_bounds(10.0, 20.0, 100.0, 50.0, 0.0, 0.0, 0.0);
        assert_eq!((x, y, width, height), (10, 20, 100, 50));
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
