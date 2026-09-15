# TRE (Tesserae Render Engine) — API Reference

Generated 2026-09-10, covering every crate in the Cargo workspace as of
commit `b9b1f9e`. This document is a **reference**, not a tutorial: every
public item (`pub struct`/`enum`/`trait`/`fn`/`const`/`type`/field) is
listed with its real signature, a short description, and any real
`# Errors`/`# Panics`/platform-specific caveats. It reflects the *current*
code, verified by direct source reading and a four-agent parallel audit,
not the (sometimes stale) prose in source doc comments — see
`documentation/REVIEW.md` findings #180-189 for what was found wrong and
fixed as part of producing this document.

For architecture/design rationale (the *why*), see `documentation/
ARCHITECTURE.md`, `documentation/DESIGN.md`, and `documentation/
TECHNICAL.md`. For build-by-build history, see `documentation/
IMPLEMENTATION.md` and `documentation/REVIEW.md`.

## Workspace map

| Crate | Status | What it is |
|---|---|---|
| [`tre-engine`](#tre-engine) | Real, core | Canvas/IR, shape primitives, RHI traits, input events, accessibility node types |
| [`tre-platform`](#tre-platform) | Real | Native window creation/input, Linux only (Wayland + X11), `winit`-backed |
| [`tre-memory`](#tre-memory) | Real | Lock-free zero-allocation ring buffers, arenas, slot tables, debug alloc guard |
| [`tre-math`](#tre-math) | Real | 2D affine transforms, SIMD batch ops, tone mapping, spring decay |
| [`tre-svg`](#tre-svg) | Real | SVG ingestion, Bezier flattening, `lyon` tessellation, path morphing |
| [`tre-text`](#tre-text) | Real | HarfBuzz/FreeType-equivalent shaping (`rustybuzz`/`skrifa`), MSDF glyph generation |
| [`tre-atlas`](#tre-atlas) | Real | 2D bin-packing, LRU eviction, background-thread atlas ownership |
| [`tre-rhi-vulkan`](#tre-rhi-vulkan) | Real | The only real RHI backend — Vulkan 1.2+ via `ash` |
| [`tre-rhi-dx12`](#tre-rhi-dx12--tre-rhi-metal--tre-ffi-stub-crates) | **Empty stub** | DirectX 12 backend — not yet implemented |
| [`tre-rhi-metal`](#tre-rhi-dx12--tre-rhi-metal--tre-ffi-stub-crates) | **Empty stub** | Metal backend — not yet implemented |
| [`tre-a11y`](#tre-a11y) | Real | Linux AT-SPI2 accessibility bridge |
| [`tre-ffi`](#tre-rhi-dx12--tre-rhi-metal--tre-ffi-stub-crates) | **Empty stub** | C-ABI boundary for non-Python languages — not yet implemented |

All crates except `tre-rhi-vulkan`, `tre-rhi-dx12`, `tre-rhi-metal`, and
`tre-ffi` carry `#![forbid(unsafe_code)]`. Those four are the closed set
permitted `unsafe` (TECHNICAL.md Section 9.1); `tre-rhi-dx12`/`tre-rhi-metal`/
`tre-ffi` currently contain no code beyond a doc comment, so the `unsafe`
permission is unexercised until they're built.

---

## `tre-engine`

The core crate: `Canvas`/IR types, the shape-primitive retained-mode layer,
RHI trait definitions, input events, accessibility node types. `#![forbid(unsafe_code)]`.

### Error type

```rust
pub enum EngineError {
    DeviceLost,
    SwapchainOutOfDate,
    PipelineCreationFailed,
    InvalidTextureData,
    BindlessArrayExhausted,
    TransientPoolBudgetExceeded,
}
```
Recoverable engine failures — panics are reserved for programmer errors,
never these.

### Color / math / IR primitives

```rust
pub const fn rgba8(r: u8, g: u8, b: u8, a: u8) -> u32
```
Packs RGBA8 into `UiVertex::color`'s little-endian layout — use this, not
a hand-written hex literal, to avoid reversed byte order.

```rust
#[repr(C)]
pub struct ScissorRect { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }
```

```rust
#[repr(C, align(16))]
pub struct UiVertex {
    pub position: [f32; 2], // Screen-space X, Y
    pub uv: [f32; 2],       // Texture coords or SDF bounds
    pub color: u32,         // Packed RGBA8 (sRGB, converted to linear in-shader)
    pub params: [f32; 3],   // Shader params (corner radii, stroke width, etc.)
} // 32 bytes total, statically asserted
```

```rust
#[repr(u8)]
pub enum CommandType { DrawGeometry, PushScissor, PopScissor, PushLayer, PopLayer }
```

```rust
#[repr(C)]
pub struct UiDrawCommand {
    pub kind: CommandType,
    pub sort_key: u64,
    pub pipeline_state_id: u16,
    pub texture_handle: u32,   // NO_TEXTURE (u32::MAX) sentinel = none
    pub element_count: u32,
    pub vertex_offset: u32,
    pub clip_bounds: ScissorRect,
}
```
The canonical intermediate-representation draw command.

```rust
pub const PIPELINE_MSDF_TEXT: u16 = 1;
pub const NO_TEXTURE: u32 = u32::MAX;
```

```rust
#[repr(u16)]
pub enum PipelineKind {
    SdfRoundedRect = 0,
    MsdfText = 1,
    TexturedQuad = 2,
    SdfRectStyled = 3,   // non-uniform corners/border/smoothing, GpuRectStyle-sourced
    SdfEllipse = 4,      // Circle/Ellipse, GpuEllipseStyle-sourced
    FlatColor = 5,       // Polygon/Path flat fill
    GradientFill = 6,    // Polygon/Path gradient fill
    FlatColorBlend = 7,  // Polygon/Path solid fill under non-Normal BlendMode
}
```

### Input events

```rust
pub struct WindowId(pub u64);   // opaque, stable, never reused while the owning connection lives
pub enum MouseButton { Left, Right, Middle, Other(u16) }
pub enum ElementState { Pressed, Released }

pub enum InputEvent {
    PointerMoved { window: WindowId, x: f64, y: f64 },
    PointerButton { window: WindowId, button: MouseButton, state: ElementState },
    /// key_code: Linux evdev keycode, same numbering on Wayland and X11.
    /// Layout-aware translation is a UI-framework concern, out of scope here.
    KeyboardKey { window: WindowId, key_code: u32, state: ElementState },
    CloseRequested { window: WindowId },
    Resized { window: WindowId, width: u32, height: u32 },
}
```
`PointerMoved` events are coalesced by `InputEventQueue` — a burst of raw
motion events for the same window collapses to the single most recent
position.

```rust
pub struct InputEventQueue { /* private */ }
impl InputEventQueue {
    pub fn with_capacity(capacity: usize) -> Self
    pub fn push(&mut self, event: InputEvent)              // coalesces PointerMoved; drops on full queue, never blocks/panics
    pub fn flush_pending_move(&mut self)                   // publishes a staged pending move
    pub fn drain(&mut self) -> Vec<InputEvent>              // flushes pending move, then pops everything
}
```

```rust
pub struct FrameClock { /* private */ }
impl FrameClock {
    pub fn new() -> Self
    pub fn tick(&mut self) -> f32   // real elapsed seconds since the previous call
}
impl Default for FrameClock { .. }
```

### Texture / layer types

```rust
pub enum TextureFormat { Bgra8Srgb, Rgba16Float, Rgba8Unorm }
```

```rust
pub struct LayerDesc {
    pub x: i32, pub y: i32, pub width: u32, pub height: u32,
    pub format: TextureFormat,
    pub blur: bool,   // Dual-Kawase blur, own-content only
}
```

```rust
#[derive(Default)]
pub struct FlattenedFrame {
    pub vertices: Vec<UiVertex>,
    pub indices: Vec<u32>,
    pub commands: Vec<UiDrawCommand>,
    pub accessibility_nodes: Vec<AccessibilityNode>,
}
```

### Accessibility types

```rust
pub struct AccessibilityNodeId(pub u64);   // caller-assigned stable key
pub enum AccessibilityRole { Generic, Button, TextLabel, Image }
pub struct AccessibilityNode {
    pub node_id: AccessibilityNodeId,
    pub x: f32, pub y: f32, pub width: f32, pub height: f32,
    pub role: AccessibilityRole,
}
pub struct OverlayLayerPriority(pub u16);   // offset added to an internal OVERLAY_LAYER_BASE
```

### Canvas API — `RenderingCanvas`

The immediate-mode drawing surface every other API (including
`ShapeRegistry`) ultimately compiles down to.

```rust
pub struct RenderingCanvas { /* private */ }
impl RenderingCanvas {
    pub fn new() -> Self
    pub fn max_sub_canvases(&self) -> usize
    pub fn reset(&mut self)                                  // clears for reuse without reallocating (zero-alloc steady state)

    /// # Panics
    /// If more than `max_sub_canvases` are ever alive at once.
    pub fn create_sub_canvas(&self) -> SubCanvas

    pub fn save(&mut self)
    /// # Panics — if `restore()` is called with no matching `save()`.
    pub fn restore(&mut self)
    pub fn transform(&mut self, matrix: &tre_math::Affine2)
    pub fn set_alpha(&mut self, factor: f32)
    pub fn push_clip(&mut self, rect: &ScissorRect)
    /// # Panics — if called with no matching `push_clip()`.
    pub fn pop_clip(&mut self)

    /// # Panics — on layer-ID overflow.
    pub fn begin_overlay(&mut self, priority: OverlayLayerPriority)
    /// # Panics — if called with no matching `begin_overlay()`.
    pub fn end_overlay(&mut self)

    pub fn push_layer(&mut self, desc: &LayerDesc)
    /// # Panics — if called with no matching `push_layer()`.
    pub fn pop_layer(&mut self)

    pub fn draw_rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, rgba: u32)

    /// # Panics — if the GPU shape-style ring buffer is starved (see `RhiDynamicRingBuffer::write`).
    pub fn draw_styled_rectangle(
        &mut self, device: &dyn RhiDevice,
        x: f32, y: f32, w: f32, h: f32,
        corner_radii: [f32; 4], fill_rgba: u32, border_rgba: u32,
        border_thickness: f32, corner_smoothing: f32, fill: StyleFill,
    )

    /// # Panics — same style-buffer-starvation case as `draw_styled_rectangle`.
    pub fn draw_ellipse(
        &mut self, device: &dyn RhiDevice,
        center_x: f32, center_y: f32, radius: [f32; 2],
        fill_rgba: u32, border_rgba: u32, border_thickness: f32,
        arc_start_angle: f32, arc_sweep_angle: f32, fill: StyleFill,
    )

    pub fn draw_flat_polygon(&mut self, positions: &[[f32; 2]], triangles: &[[u32; 3]], rgba: u32)
    pub fn draw_gradient_polygon(&mut self, positions: &[[f32; 2]], triangles: &[[u32; 3]], gradient_word_index: u32)

    /// # Panics — if `positions.len() != uvs.len()`.
    pub fn draw_textured_polygon(&mut self, positions: &[[f32; 2]], uvs: &[[f32; 2]], triangles: &[[u32; 3]], texture_index: u32)

    pub fn draw_flat_polygon_blended(&mut self, positions: &[[f32; 2]], triangles: &[[u32; 3]], rgba: u32, blend_mode: u32)

    pub fn draw_text(
        &mut self, shaped: &tre_text::ShapedRun, font: &skrifa::FontRef, font_id: u32,
        origin: [f32; 2], px_size: f32, rgba: u32, atlas_context: &GlyphAtlasContext<'_>,
    )

    pub fn tag_accessibility_node(&mut self, node_id: AccessibilityNodeId, x: f32, y: f32, width: f32, height: f32, role: AccessibilityRole)

    /// # Panics (debug builds only) — unbalanced save/clip/overlay/layer stack.
    pub fn flatten(self) -> FlattenedFrame
    pub fn flatten_unbatched(self) -> FlattenedFrame   // pixel-equivalence testing against the batched path

    pub fn stitch_into(&self, arena: &FrameArena) -> bool   // lock-free merge into a shared FrameArena
}
```

```rust
pub struct SubCanvas { /* Deref/DerefMut to RenderingCanvas, forwarding every method above */ }
pub struct GlyphAtlasContext<'a> {
    pub atlas: &'a tre_atlas::AtlasOwnerHandle,
    pub texture_handle: u32,
    pub dimensions: (u32, u32),
    pub current_frame: u64,
}
```

### `FrameArena`

Per-frame scatter-arena aggregation target for multi-threaded recording
(Phase 5 Step 5.2).

```rust
pub struct FrameArena { /* private */ }
impl FrameArena {
    pub fn with_capacity(vertex_capacity: usize, index_capacity: usize, command_capacity: usize, accessibility_capacity: usize) -> Self
    pub fn flatten(self) -> FlattenedFrame
    pub fn flatten_into(&mut self, out: &mut FlattenedFrame)   // zero-alloc reuse sibling of flatten()
}
```

### RHI (Render Hardware Interface) — trait definitions

These traits are **defined here** in `tre-engine`; the only real
implementor today is `tre-rhi-vulkan` (see [that section](#tre-rhi-vulkan)
for the concrete `Vulkan*` types).

```rust
pub struct AcquiredImage {
    pub index: u32,
    pub target_view_handle: u64,
    pub target_image_handle: u64,
    pub image_available_semaphore_handle: u64,
    pub render_finished_semaphore_handle: u64,
}

pub trait RhiBuffer { fn raw_handle(&self) -> u64; }

pub struct BufferBinding<'a> { pub buffer: &'a dyn RhiBuffer, pub offset: u32 }

pub trait RhiTexture {
    fn raw_handle(&self) -> u64;
    fn image_handle(&self) -> u64;
    fn memory_handle(&self) -> u64;
    fn dimensions(&self) -> (u32, u32);
    fn format(&self) -> TextureFormat;
    fn bindless_index(&self) -> Option<u32>;
    fn size_bytes(&self) -> u64;
}

pub trait RhiDynamicRingBuffer: RhiBuffer {
    /// Returns `None` (not an Err) on segment overflow — starvation is
    /// reported, not grown mid-frame (DESIGN.md Section 2.6). Note: this
    /// Option-not-Result signature is narrower than the project's own
    /// blanket "fallible ops return Result" policy (REVIEW.md #142) —
    /// documented as a real, disclosed inconsistency, not fixed.
    fn write(&self, bytes: &[u8]) -> Option<u32>;
}

pub trait RhiPipelineState {
    fn raw_handle(&self) -> u64;
    fn layout_handle(&self) -> u64;
}

pub struct PipelineRegistry { /* private */ }
impl PipelineRegistry {
    pub fn new() -> Self
    /// # Panics — on a duplicate `id`.
    pub fn register(&mut self, id: u16, pipeline: Box<dyn RhiPipelineState>)
    pub fn get(&self, id: u16) -> Option<&dyn RhiPipelineState>
}
impl Default for PipelineRegistry { .. }

pub trait RhiSwapchain {
    fn extent(&self) -> (u32, u32);
    fn stencil_view_handle(&self) -> u64;
    fn stencil_image_handle(&self) -> u64;
    fn supports_local_read_input_attachment(&self) -> bool;
    /// # Errors — EngineError::SwapchainOutOfDate on a stale swapchain, else DeviceLost.
    fn acquire_next_image(&self) -> Result<AcquiredImage, EngineError>;
    /// # Errors — same as acquire_next_image.
    fn present(&self, image: AcquiredImage) -> Result<(), EngineError>;
}

pub trait RhiDevice {
    fn create_dynamic_ring_buffer(&self, capacity: usize) -> Box<dyn RhiDynamicRingBuffer>;
    fn shape_style_buffer(&self) -> &dyn RhiDynamicRingBuffer;
    fn local_read_blend_supported(&self) -> bool;
    /// # Errors — EngineError::TransientPoolBudgetExceeded under real VRAM pressure.
    fn acquire_transient_target(&self, width: u32, height: u32, format: TextureFormat) -> Result<Box<dyn RhiTexture>, EngineError>;
    fn release_transient_target(&self, texture: Box<dyn RhiTexture>);
    /// # Errors — EngineError::InvalidTextureData on a pixel/dimension mismatch.
    fn create_texture(&self, width: u32, height: u32, format: TextureFormat, pixels: &[u8]) -> Result<Box<dyn RhiTexture>, EngineError>;
    /// # Errors — EngineError::BindlessArrayExhausted.
    fn register_bindless(&self, texture: &dyn RhiTexture) -> Result<u32, EngineError>;
    fn deregister_bindless(&self, bindless_index: u32);
    /// # Errors — see RhiSwapchain::acquire_next_image.
    fn begin_frame(&self, swapchain: &dyn RhiSwapchain) -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError>;
    /// # Errors — see RhiSwapchain::present.
    fn submit_and_present(&self, cmd_buffer: Box<dyn RhiCommandBuffer>, swapchain: &dyn RhiSwapchain, image: AcquiredImage) -> Result<(), EngineError>;
}

pub trait RhiCommandBuffer {
    fn set_pipeline(&mut self, pipeline: &dyn RhiPipelineState);
    fn set_scissor(&mut self, rect: &ScissorRect);
    fn bind_vertex_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32);
    fn bind_index_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32);
    fn bind_texture(&mut self, slot: u32, bindless_index: u32);   // only slot == 0 is supported today
    /// # Panics (Vulkan impl) — if called before `set_pipeline`.
    fn draw_indexed(&mut self, index_count: u32, start_index: u32, base_vertex: i32);
    /// # Panics — if called with no pipeline bound, or blending unsupported.
    fn insert_blend_read_barrier(&mut self);
    fn begin_render_to_texture(&mut self, texture: &dyn RhiTexture, logical_width: u32, logical_height: u32);
    fn end_render_to_texture(&mut self, texture: &dyn RhiTexture);
    fn begin_render_to_texture_no_end(&mut self, texture: &dyn RhiTexture, logical_width: u32, logical_height: u32);
    fn resume_swapchain_rendering(&mut self);
    /// (Vulkan impl) Can panic on a real, recoverable transient-pool
    /// exhaustion instead of propagating it — REVIEW.md #189, disclosed
    /// not fixed (would require widening this trait's signature).
    fn apply_layer_blur(&mut self, device: &dyn RhiDevice, source: &dyn RhiTexture, width: u32, height: u32) -> Box<dyn RhiTexture>;
    fn raw_handle(&self) -> u64;
}
```

```rust
pub fn execute_frame(
    frame: &FlattenedFrame, registry: &PipelineRegistry,
    vertex_buffer: BufferBinding<'_>, index_buffer: BufferBinding<'_>,
    full_window: &ScissorRect, device: &dyn RhiDevice, cmd_buffer: &mut dyn RhiCommandBuffer,
)
```
The generic frame executor — turns a `FlattenedFrame` into real RHI draw
calls by walking `commands` and dispatching on `CommandType`.
**# Panics**: unregistered `pipeline_state_id`; nested `PushLayer`;
unmatched `PopLayer`; `acquire_transient_target`/`register_bindless`
returning `Err` (both `.expect()`-ed internally rather than propagated —
a known, disclosed gap; fixing it needs `execute_frame` to return `Result`).

### GPU style buffer types (`gpu_style` module, re-exported at crate root)

Raw-buffer layouts written into `RhiDevice::shape_style_buffer()`, read
back by the corresponding fragment shader.

```rust
pub const RECT_STYLE_WORDS: u32 = 10;
#[repr(C)]
pub struct GpuRectStyle {
    pub corner_radii: [f32; 4],
    pub border_color: u32,
    pub border_thickness: f32,
    pub corner_smoothing: f32,
    pub fill_kind: u32,           // 0 = solid, 1 = gradient, 2 = texture
    pub gradient_word_index: u32,
    pub texture_index: u32,
}   // matches sdf_rect_styled.frag's word layout exactly

pub const ELLIPSE_STYLE_WORDS: u32 = 7;
#[repr(C)]
pub struct GpuEllipseStyle {
    pub border_color: u32,
    pub border_thickness: f32,
    pub arc_start_angle: f32,
    pub arc_sweep_angle: f32,
    pub fill_kind: u32,
    pub gradient_word_index: u32,
    pub texture_index: u32,
}   // matches sdf_ellipse.frag

pub struct StyleFill {
    pub fill_kind: u32,
    pub gradient_word_index: u32,
    pub texture_index: u32,
}
impl StyleFill { pub const SOLID: Self = ..; }

pub const GRADIENT_MAX_STOPS: usize = 8;
pub const GRADIENT_STYLE_WORDS: u32 = 6 + (GRADIENT_MAX_STOPS as u32) * 2;
#[repr(C)]
pub struct GpuGradientStyle {
    pub kind: u32,                                  // 0 = linear, 1 = radial
    pub point0: [f32; 2],
    pub point1_or_radius: [f32; 2],
    pub stop_count: u32,
    pub stop_positions: [f32; GRADIENT_MAX_STOPS],
    pub stop_colors: [u32; GRADIENT_MAX_STOPS],       // sRGB packed; interpolated in linear space in-shader
}

/// # Panics (debug builds only) — if `byte_offset` is not 4-byte-aligned.
#[must_use]
pub fn style_index_param(byte_offset: u32) -> f32
```
Bitcasts a byte offset into a numeric (not raw-bitcast) `f32` word index —
numeric encoding, not `f32::from_bits`, specifically to avoid a real
denormal flush-to-zero GPU bug found during development.

### Shape primitive layer (`shapes` module, re-exported at crate root)

The retained-mode convenience layer — every shape here compiles down to
the exact same `RenderingCanvas` calls above; never a second render path.
**All rendering support is real and complete**: `Rectangle` (any corner
radii, borders, corner smoothing), `Circle`/`Ellipse` (borders,
partial-arc sweep with rounded stroke caps), `Polygon`/`Path` (real fill
including compound shapes with holes, and real stroke, via `lyon`), and
every `FillStyle` variant (`Solid`/`Gradient`/`Texture`) for all four
shape kinds.

```rust
pub type Vec2 = [f32; 2];
pub type Color = u32;   // packed RGBA8

pub struct Transform2D { pub position: Vec2, pub scale: Vec2, pub rotation: f32 /* radians */ }
impl Transform2D {
    pub const IDENTITY: Self;
    pub fn to_affine2(&self) -> tre_math::Affine2
}
impl Default for Transform2D { .. }   // == IDENTITY

/// Only real for Polygon/Path solid fill, gated behind
/// RhiDevice::local_read_blend_supported().
pub enum BlendMode { #[default] Normal, Multiply, Screen, Overlay, SoftLight, ColorDodge }

pub enum Visibility { #[default] Visible, Hidden, Collapsed }

pub struct PrimitiveCommon {
    pub transform: Transform2D,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub visibility: Visibility,
    pub hit_testable: bool,
}
impl PrimitiveCommon { pub const fn new() -> Self; }   // identity transform, opacity 1.0, Normal, Visible, hit_testable true

pub trait Primitive {
    fn common(&self) -> &PrimitiveCommon;
    fn common_mut(&mut self) -> &mut PrimitiveCommon;
}   // object-safe accessor; never used as `dyn Primitive` in practice

pub enum FillStyle { Solid(Color), Gradient(GradientId), Texture(u32) }
```

#### Gradients

```rust
pub struct GradientId(pub u32);   // no generational reuse/removal, by design

pub struct GradientDef { pub kind: GradientKind, pub stops: Vec<GradientStop> }
pub enum GradientKind {
    Linear { start: Vec2, end: Vec2 },
    Radial { center: Vec2, radius: f32 },
}
pub struct GradientStop { pub position: f32, pub color: Color }

pub enum GradientError {
    NoStops,
    TooManyStops { count: usize, max: usize },        // max == GRADIENT_MAX_STOPS (8)
    StopPositionOutOfRange { index: usize, position: f32 },
    StopsNotAscending { index: usize },
    NonPositiveRadius(f32),
}   // implements Display + std::error::Error
```

#### Shape kinds

```rust
pub struct CornerRadii { pub top_left: f32, pub top_right: f32, pub bottom_right: f32, pub bottom_left: f32 }
impl CornerRadii {
    pub const fn uniform(radius: f32) -> Self
    pub fn is_uniform(&self) -> bool
}

pub struct Rectangle {
    pub common: PrimitiveCommon,
    pub size: Vec2,
    pub fill: FillStyle,
    pub border_color: Color,
    pub border_thickness: f32,
    pub corner_radius: CornerRadii,
    pub corner_smoothing: f32,   // 0.0 = pure circular-arc rounding, 1.0 = full squircle
}
impl Rectangle {
    /// Corner-radius-free, borderless rectangle of `size`, filled `color`, identity transform.
    pub fn new(size: Vec2, color: Color) -> Self
}

pub struct Circle {
    pub common: PrimitiveCommon,
    pub radius: Vec2,             // radius[0] == radius[1]: circle; else: ellipse
    pub fill: FillStyle,
    pub border_color: Color,
    pub border_thickness: f32,
    pub arc_length: f32,          // degrees, 0.0..=360.0, partial sweep from 12 o'clock clockwise
}
impl Circle {
    /// Full (360°), borderless circle/ellipse of `radius`, filled `color`, identity transform.
    pub fn new(radius: Vec2, color: Color) -> Self
}

pub struct Polygon {
    pub common: PrimitiveCommon,
    pub sides: u32,
    pub radius: f32,
    pub vertex_radius: f32,        // only used when star_points is Some (inner/odd-index vertex radius)
    pub star_points: Option<u32>,  // None: regular sides-gon; Some(k): 2k-vertex star
    pub fill: FillStyle,
    pub border_color: Color,
    pub border_thickness: f32,
}
impl Polygon {
    /// Regular sides-gon (not a star) of `radius`, filled `color`, borderless, identity transform.
    pub fn new(sides: u32, radius: f32, color: Color) -> Self
}

pub enum PathCommand {
    MoveTo(Vec2), LineTo(Vec2),
    QuadraticTo { control: Vec2, to: Vec2 },
    CubicTo { control1: Vec2, control2: Vec2, to: Vec2 },
    Close,
}
pub enum LineCap { Butt, Round, Square }     // SVG stroke-linecap
pub enum LineJoin { Miter, Round, Bevel }    // SVG stroke-linejoin

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
    /// Borderless path of `commands`, filled `color`, Butt/Miter caps, identity transform.
    pub fn new(commands: Vec<PathCommand>, color: Color) -> Self
}

pub enum ShapePrimitive { Rectangle(Rectangle), Circle(Circle), Polygon(Polygon), Path(Path) }
impl ShapePrimitive {
    pub fn common(&self) -> &PrimitiveCommon
    pub fn common_mut(&mut self) -> &mut PrimitiveCommon
}

pub fn flatten_path(commands: &[PathCommand]) -> Vec<Vec<Vec2>>
```
Flattens a `Path`'s commands into local-space polylines (one per
subpath) — used by `ShapeRegistry::hit_test`'s `Path` case.

#### `ShapeRegistry` — the generational shape store

```rust
pub struct ShapeId { /* private: index: u32, generation: u32 */ }
pub struct AnimationId(pub u64);
pub struct ShapeSlot {
    pub shape: ShapePrimitive,
    pub active_animations: Vec<AnimationId>,
    pub layout_dirty: bool,                  // caller must set this true after mutating a shape's fields
    pub clip_bounds: Option<ScissorRect>,
}

pub struct ShapeRegistry { /* private */ }
impl ShapeRegistry {
    pub fn new() -> Self
    pub fn insert(&mut self, shape: ShapePrimitive) -> ShapeId
    pub fn remove(&mut self, id: ShapeId) -> bool                       // false on stale id, never panics
    pub fn get(&self, id: ShapeId) -> Option<&ShapeSlot>
    pub fn get_mut(&mut self, id: ShapeId) -> Option<&mut ShapeSlot>
    pub fn len(&self) -> usize
    pub fn is_empty(&self) -> bool

    /// # Errors — see GradientError.
    /// # Panics — never in practice (only past u32::MAX gradients created).
    pub fn create_gradient(&mut self, def: GradientDef) -> Result<GradientId, GradientError>

    /// Mutable access to an existing gradient's own definition. Does NOT
    /// mark any shape using it as layout_dirty — the caller must also
    /// call get_mut on the shape itself for the mutation to take effect
    /// on the next flatten_into.
    pub fn gradient_mut(&mut self, id: GradientId) -> Option<&mut GradientDef>

    /// Read-only counterpart to gradient_mut.
    pub fn gradient(&self, id: GradientId) -> Option<&GradientDef>

    /// The per-frame flattening pass: every layout_dirty-or-animated slot
    /// is re-flattened into `canvas` via the same immediate-mode calls
    /// any other caller would use; Hidden/Collapsed shapes are skipped.
    /// # Panics — only if a FillStyle::Gradient names a GradientId this
    /// registry never issued (a stale/foreign handle, a real programmer error).
    pub fn flatten_into(&mut self, canvas: &mut RenderingCanvas, device: &dyn RhiDevice)

    #[must_use]
    pub fn hit_test(&self, point: Vec2) -> Option<ShapeId>
}
```

---

## `tre-platform`

Native OS window creation and input. **Linux only** (Wayland primary, X11
fallback), backed by `winit` 0.30 since Phase 11 Step 11.1 (previously two
hand-rolled protocol integrations). `#![forbid(unsafe_code)]` — needed none
after the `winit` migration.

```rust
pub use tre_engine::{ElementState, InputEvent, MouseButton, WindowId};

pub enum PlatformError {
    ConnectionFailed,
    ProtocolMissing(&'static str),
    UnknownWindow,     // window not created by this connection, or already closed
    Other(String),
}   // implements Display + std::error::Error

/// RGBA8 pixel data for PlatformConnection::set_icon.
/// rgba.len() must equal width * height * 4.
pub struct WindowIcon { pub rgba: Vec<u8>, pub width: u32, pub height: u32 }

pub enum PlatformConnection {
    Wayland(/* private */),
    X11(/* private */),
}
```

```rust
impl PlatformConnection {
    /// Picks Wayland if WAYLAND_DISPLAY is set, else X11.
    /// # Errors — connection failure or missing protocol/extension.
    pub fn new() -> Result<Self, PlatformError>
    pub fn new_wayland() -> Result<Self, PlatformError>
    pub fn new_x11() -> Result<Self, PlatformError>

    /// # Errors — compositor/WM rejection, or protocol object unavailable.
    pub fn create_window(&mut self, title: &str, width: u32, height: u32) -> Result<WindowId, PlatformError>

    /// Drains pending window-lifecycle + input events. Call once per frame; never blocks.
    pub fn poll_events(&mut self) -> Vec<InputEvent>

    /// Real per-window DPI scale factor (f64 — Wayland wp-fractional-scale
    /// falling back to integer; X11 Xft.dpi/RandR).
    pub fn scale_factor(&self, window: WindowId) -> f64

    /// # Errors — HandleError::Unavailable if `window` wasn't created here.
    pub fn window_handle(&self, window: WindowId) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>

    /// Changes `window`'s title after creation.
    /// # Errors — PlatformError::UnknownWindow.
    pub fn set_title(&self, window: WindowId, title: &str) -> Result<(), PlatformError>

    /// Requests minimize/un-minimize. Only sends the request — an
    /// immediately-following is_minimized() will NOT yet reflect it (a
    /// real compositor round trip via poll_events() is required; verified
    /// by live testing). On Wayland, un-minimizing is a protocol-level
    /// no-op winit cannot lift; minimizing works on both backends.
    /// # Errors — PlatformError::UnknownWindow.
    pub fn set_minimized(&self, window: WindowId, minimized: bool) -> Result<(), PlatformError>

    /// Current minimized state as of the last processed compositor
    /// update. None if unknown window, or state undeterminable — always
    /// None on Wayland (the protocol has no query for it).
    pub fn is_minimized(&self, window: WindowId) -> Option<bool>

    /// Requests maximize/restore. Same "request only, real round trip
    /// needed before is_maximized() reflects it" caveat as set_minimized.
    /// Works fully on both backends.
    /// # Errors — PlatformError::UnknownWindow.
    pub fn set_maximized(&self, window: WindowId, maximized: bool) -> Result<(), PlatformError>

    /// Current maximized state as of the last processed update. False if unknown window.
    pub fn is_maximized(&self, window: WindowId) -> bool

    /// Sets/clears (None) window icon. Unsupported on Wayland (no
    /// client-side icon mechanism at all — icons come from desktop-file
    /// metadata matched by app_id) — the call still succeeds there, as a
    /// no-op. Works on X11, subject to the WM's own icon-size conventions.
    /// # Errors — UnknownWindow, or Other if rgba doesn't match width*height*4.
    pub fn set_icon(&self, window: WindowId, icon: Option<WindowIcon>) -> Result<(), PlatformError>
}
impl raw_window_handle::HasDisplayHandle for PlatformConnection { .. }
```

**What's still NOT possible**: programmatic window *position*/movement
(a genuine Wayland protocol-level restriction, not a `tre-platform` gap —
the protocol gives clients zero control over their own toplevel position,
confirmed by real observed same-position-window-stacking during earlier
development); programmatic resize after creation (only inbound `Resized`
notifications exist); explicit `close_window()` (a window closes only via
user/OS action or dropping the connection). Also disclosed: `EventLoop`
(one per `PlatformConnection`) can only be constructed once per OS
process, ever — a real, permanent `winit` restriction, harmless for every
current caller (every demo is a separate process).

---

## `tre-memory`

Lock-free, zero-allocation-after-construction primitives.
`#![forbid(unsafe_code)]` is *not* set here — this is one of the three
crates permitted `unsafe` (TECHNICAL.md Section 9.1), for the actual
lock-free ring-buffer/arena/slot-table internals.

```rust
pub use alloc_guard::{DebugAllocGuard, RenderTickGuard};
pub use mpsc::MpscRingBuffer;
pub use scatter::{ScatterArena, ScatterSlice};
pub use spsc::SpscRingBuffer;
pub use swmr::SwmrSlotTable;
```

```rust
pub struct SpscRingBuffer<T> { /* private */ }
impl<T> SpscRingBuffer<T> {
    /// # Panics — if `capacity` is zero.
    pub fn with_capacity(capacity: usize) -> Self
    pub fn push(&self, item: T) -> Result<(), T>     // Err returns the item back if full
    pub fn pop(&self) -> Option<T>
    pub fn is_empty(&self) -> bool
    pub fn capacity(&self) -> usize                  // the real usable capacity (sentinel slot excluded)
}
```
Single-producer single-consumer. Backing storage allocated once, at
construction — `push`/`pop` never allocate.

```rust
pub struct MpscRingBuffer<T> { /* private */ }
impl<T> MpscRingBuffer<T> {
    /// # Panics — if `capacity` is zero.
    pub fn with_capacity(capacity: usize) -> Self
    pub fn push(&self, item: T) -> Result<(), T>     // safe from any number of producer threads
    pub fn pop(&self) -> Option<T>                    // single consumer thread ONLY
    pub fn is_empty(&self) -> bool                    // best-effort snapshot under concurrent producers
    pub fn capacity(&self) -> usize
}
```
Dmitry Vyukov's bounded MPMC ring buffer design, used here as MPSC.

```rust
pub struct DebugAllocGuard;   // GlobalAlloc wrapper; panics on allocation while a RenderTickGuard is active on the calling thread
impl DebugAllocGuard { pub const fn new() -> Self; }
impl Default for DebugAllocGuard { .. }
// unsafe impl GlobalAlloc for DebugAllocGuard { .. }  -- no-op in release builds

pub struct RenderTickGuard { /* private */ }
impl RenderTickGuard {
    /// # Panics (debug builds only) — if a guard is already active on this thread (guards don't nest).
    pub fn begin() -> Self
}
// impl Drop clears the thread-local flag. No-op in release builds.
```
Thread-local by design — a multi-threaded recording scheme needs its own
guard started on each thread whose allocations should be checked.

```rust
pub struct ScatterArena<T> { /* private */ }
impl<T: Copy> ScatterArena<T> {
    pub fn with_capacity(capacity: usize) -> Self
    pub fn capacity(&self) -> usize
    /// Reserves `count` contiguous slots; None (not grown) if it wouldn't
    /// fit. Safe from any number of concurrent threads; never blocks.
    pub fn reserve(&self, count: usize) -> Option<ScatterSlice<'_, T>>
    pub fn into_vec(self) -> Vec<T>       // only the actually-written prefix
    pub fn reset(&mut self)               // back to empty, no reallocation
    pub fn drain_into(&mut self, out: &mut Vec<T>)   // zero-alloc reuse sibling of into_vec + reset
}

pub struct ScatterSlice<'a, T> { /* Deref/DerefMut to [T] */ }
impl<T> ScatterSlice<'_, T> {
    pub fn start_index(&self) -> usize    // where this reservation landed within the arena
}
```
Any number of threads can concurrently reserve disjoint output ranges via
one atomic `fetch_update` (not a bare `fetch_add` — see the source's own
REVIEW.md #132 note on why the update-vs-add distinction matters for
correctness after a failed reservation).

```rust
pub struct SwmrSlotTable<K> { /* private */ }
impl<K: Copy + Eq + Into<u64>> SwmrSlotTable<K> {
    /// # Panics — if `capacity` is zero.
    pub fn with_capacity(capacity: usize) -> Self
    pub fn insert(&self, key: K, value: u64) -> bool    // single writer thread only; false = table full
    pub fn remove(&self, key: K) -> bool                 // single writer thread only
    pub fn get(&self, key: K) -> Option<u64>              // any number of reader threads
    pub fn get_and_touch(&self, key: K, frame: u64) -> Option<u64>   // get + records recency for LRU
    pub fn scan_older_than(&self, cutoff_frame: u64, visit: impl FnMut(u64, u64))   // single writer thread only
    pub fn capacity(&self) -> usize
}
```
Safe for one writer, any number of concurrent readers. Uses a per-slot
epoch/seqlock counter to prevent an ABA hazard across an evict-then-reuse
race (REVIEW.md #131).

---

## `tre-math`

2D affine transforms, SIMD batch ops, built on the `wide` crate's safe
portable-SIMD API. `#![forbid(unsafe_code)]`.

```rust
#[repr(C)]
pub struct Affine2 { pub a: f32, pub b: f32, pub tx: f32, pub c: f32, pub d: f32, pub ty: f32 }
// Canonical [[a,b,tx],[c,d,ty],[0,0,1]] matrix, bottom row implicit.

impl Affine2 {
    pub const IDENTITY: Self;
    pub const fn from_translation(tx: f32, ty: f32) -> Self
    pub fn from_rotation(theta: f32) -> Self             // radians, counterclockwise
    pub const fn from_scale(sx: f32, sy: f32) -> Self     // negative = legitimate flip
    pub fn from_translation_rotation_scale(translation: [f32; 2], rotation: f32, scale: [f32; 2]) -> Self
    pub fn compose(&self, child: &Self) -> Self            // self.compose(&child) applies child first, then self; not commutative
    pub fn transform_point(&self, point: [f32; 2]) -> [f32; 2]
    pub fn invert(&self) -> Option<Self>                    // None if degenerate (near-zero determinant)
    pub const fn to_array(self) -> [f32; 6]                 // [a, b, tx, c, d, ty], matches #[repr(C)] field order
    pub const fn from_array(v: [f32; 6]) -> Self
}
```

```rust
/// # Panics — if parents/children/out don't all have the same length.
pub fn compose_batch(parents: &[Affine2], children: &[Affine2], out: &mut [Affine2])
```
SIMD-batched `Affine2::compose`, 8-wide via `wide::f32x8`, scalar
remainder for `len % 8`. Writes into `out` (zero-alloc), no return `Vec`.

```rust
/// # Panics — if from/to/out don't all have the same length.
pub fn lerp_points_batch(from: &[[f32; 2]], to: &[[f32; 2]], t: f32, out: &mut [[f32; 2]])
```
SIMD-batched per-point linear interpolation, same structure as
`compose_batch`.

```rust
/// # Panics — never in practice (pure arithmetic, headroom expected positive).
pub fn tone_map(linear: f32, headroom: f32) -> f32
```
HDR-to-SDR tone-mapping curve — identity at/below standard white,
Reinhard-style compression above it. A pure primitive, not yet wired into
any real render path (no HDR-capable swapchain surface is actually
selected on any hardware this project has tested against).

```rust
/// # Panics — never (pure arithmetic).
pub fn spring_decay(current: f32, target: f32, lambda: f32, dt: f32) -> f32
```
Frame-rate-independent exponential decay toward `target`: pure
exponential smoothing (never overshoots), not a mass-spring-damper ODE.

---

## `tre-svg`

SVG ingestion, Bezier flattening, `lyon`-backed tessellation, path
morphing. `#![forbid(unsafe_code)]`.

```rust
pub use flatten::{flatten_cubic, flatten_quad};
pub use morph::{morph, morph_into};
pub use tessellate::{tessellate_fill, FillRule};

pub struct Polygon { pub points: Vec<[f32; 2]> }   // one closed contour, last point implicitly connects to first

pub enum SvgError {
    TooLarge { size: usize, max: usize },
    TooManyPoints { count: usize, max: usize },
    Parse(String),
    TessellationFailed,
    TopologyMismatch { from_points: usize, to_points: usize },
}   // implements Display + std::error::Error

/// # Errors
/// TooLarge if source exceeds max_bytes; Parse if usvg rejects the
/// document; TooManyPoints if the total resolved point count exceeds
/// max_points (usvg does not itself enforce this — a bounded document
/// can still resolve to unbounded points).
pub fn parse_svg(source: &[u8], max_bytes: usize, max_points: usize) -> Result<Vec<Polygon>, SvgError>
```
`max_points` bounds peak memory across the whole document, but NOT
worst-case CPU time for a single adversarially-shaped path within budget.

```rust
/// Appends flattened line segments for a cubic Bezier, NOT including p0
/// (caller already has it as the current point).
pub fn flatten_cubic(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], out: &mut Vec<[f32; 2]>)
pub fn flatten_quad(p0: [f32; 2], control: [f32; 2], p1: [f32; 2], out: &mut Vec<[f32; 2]>)
```
`pub` (not `pub(crate)`) since `tre-text` reuses these for glyph outlines.

```rust
/// # Errors — SvgError::TopologyMismatch if point counts differ.
pub fn morph(from: &Polygon, to: &Polygon, t: f32) -> Result<Polygon, SvgError>
/// Zero-alloc reuse sibling of morph — writes into `out` instead of returning a Vec.
/// # Errors — same as morph. `out` unchanged on error.
pub fn morph_into(from: &Polygon, to: &Polygon, t: f32, out: &mut Vec<[f32; 2]>) -> Result<(), SvgError>
```
"Topological equivalence" between flattened polygons means equal vertex
counts — mismatches are rejected, never auto-resampled.

```rust
pub enum FillRule { NonZero, EvenOdd }   // SVG fill-rule: nonzero | evenodd

/// # Errors — SvgError::TessellationFailed (rare; lyon handles self-
/// intersecting/multi-contour input, unlike the retired ear-clipper).
pub fn tessellate_fill(contours: &[Polygon], fill_rule: FillRule, rgba: u32) -> Result<(Vec<tre_engine::UiVertex>, Vec<u32>), SvgError>
```
Tessellates directly into flat-colored vertex/index buffers, ready for
`upload_buffer`/`draw_indexed`.

---

## `tre-text`

HarfBuzz/FreeType-equivalent shaping (`rustybuzz`/`skrifa`), MSDF glyph
generation. `#![forbid(unsafe_code)]`.

```rust
pub use error::TextError;
#[cfg(target_os = "linux")]
pub use fallback::{covers, resolve_font_index, resolve_run, FontCascade};
pub use msdf::{generate_msdf, has_real_ink, MsdfBitmap};
pub use outline::{glyph_outline, Contour, OutlineSegment};
pub use raster::GlyphRasterSource;
pub use shape::{segment_runs, shape_text, ShapedGlyph, ShapedRun, TextRun};
```
The `fallback` module's re-exports are Linux-only — not part of the
public API on other targets.

```rust
pub enum TextError {
    InvalidFontForShaping,          // rustybuzz rejected the font bytes
    InvalidFontForOutlines,         // skrifa rejected the bytes, or the glyph has no outline entry
    OutlineDrawFailed,              // skrifa's draw call failed on a malformed glyf/CFF table
    FontDiscoveryUnavailable,       // fontconfig unavailable, or no cascade family resolved
}   // implements Display + std::error::Error
```

```rust
/// Rasterizes one glyph's MSDF on demand for AtlasOwnerHandle::request_insert.
/// Caller must only construct for a glyph already confirmed to have real ink.
pub struct GlyphRasterSource { pub contours: Vec<Contour>, pub size: u32, pub range_px: f64 }
impl RasterSource for GlyphRasterSource {
    fn size(&self) -> (u32, u32)
    /// # Panics — if `contours` has no real ink (see struct doc).
    fn rasterize(&self) -> Vec<u8>
}
```

```rust
pub struct FontCascade { pub entries: Vec<PathBuf> }   // primary sans -> broad fallback -> emoji
impl FontCascade {
    /// # Errors — FontDiscoveryUnavailable.
    pub fn discover() -> Result<Self, TextError>
}

/// # Errors — InvalidFontForOutlines if font_bytes isn't parseable.
pub fn covers(font_bytes: &[u8], text: &str) -> Result<bool, TextError>
/// # Errors — propagates `covers`'s error.
pub fn resolve_font_index(font_candidates: &[&[u8]], run_text: &str) -> Result<usize, TextError>
/// # Errors — propagates resolve_font_index, or InvalidFontForShaping.
pub fn resolve_run(font_candidates: &[&[u8]], text: &str, run: &TextRun) -> Result<(usize, ShapedRun), TextError>
```
All Linux-only (`#[cfg(target_os = "linux")]`).

```rust
pub enum OutlineSegment {
    MoveTo([f32; 2]), LineTo([f32; 2]),
    QuadTo { control: [f32; 2], end: [f32; 2] },
    CubicTo { control1: [f32; 2], control2: [f32; 2], end: [f32; 2] },
    Close,
}
pub type Contour = Vec<OutlineSegment>;

/// # Errors — InvalidFontForOutlines (no outline entry), OutlineDrawFailed (malformed table).
pub fn glyph_outline(font: &FontRef, glyph_id: GlyphId) -> Result<Vec<Contour>, TextError>
```

```rust
pub struct ShapedGlyph {
    pub glyph_id: u32, pub cluster: u32,   // byte offset into the input string
    pub x_advance: i32, pub y_advance: i32, pub x_offset: i32, pub y_offset: i32,
}   // font design units, not pixels
pub struct ShapedRun { pub text_range: Range<usize>, pub direction: Direction, pub glyphs: Vec<ShapedGlyph> }   // already in visual order
pub struct TextRun { pub text_range: Range<usize>, pub level: Level, pub script: unicode_script::Script }

#[must_use]
pub fn segment_runs(text: &str) -> Vec<TextRun>   // bidi- and script-uniform runs, in visual order
/// # Errors — InvalidFontForShaping only in a degenerate constructed-Face case.
pub fn shape_text(face: &Face, text: &str) -> Result<Vec<ShapedRun>, TextError>
```

```rust
pub struct MsdfBitmap { pub width: u32, pub height: u32, pub pixels: Vec<u8> }   // RGB8, row-major

#[must_use]
pub fn generate_msdf(contours: &[Contour], size: u32, range_px: f64) -> Option<MsdfBitmap>
```
`None` if `contours` has no real ink (empty, e.g. U+0020 SPACE — not an
error) or is degenerate (coincident/non-finite points). Previously
`panic!`ed on both cases; fixed as a real security/typography finding
(REVIEW.md #139) since neither is a caller-programming error.

```rust
#[must_use]
pub fn has_real_ink(contours: &[Contour]) -> bool
```
The exact condition `generate_msdf` requires to succeed — callers should
gate `GlyphRasterSource` construction on this, not just
`!contours.is_empty()` (a narrower, wrong condition; REVIEW.md #139).

---

## `tre-atlas`

2D Guillotine bin-packing (Best-Area-Fit + guillotine split) plus a
background-thread atlas-owner concurrency model. `#![forbid(unsafe_code)]`.

```rust
pub use key::{pack_slot_value, unpack_slot_value, AtlasKey};
pub use owner::{AtlasOwner, AtlasOwnerHandle};
pub use raster::{AtlasInsertRequest, RasterSource};

pub struct PackedRect { pub x: u32, pub y: u32, pub width: u32, pub height: u32 }
impl PackedRect {
    pub fn overlaps(&self, other: &PackedRect) -> bool   // test-only in practice, but pub
}
```

```rust
pub struct AtlasPacker { /* private */ }
impl AtlasPacker {
    pub fn new(width: u32, height: u32) -> Self
    pub fn insert(&mut self, width: u32, height: u32) -> Option<PackedRect>   // None if no space fits; never panics
    /// `rect` must be a value THIS packer previously returned from
    /// insert(); NOT validated — a foreign/duplicate rect is a caller
    /// bug this method cannot detect (safe only because of the
    /// single-atlas-owner design elsewhere in this crate).
    pub fn remove(&mut self, rect: PackedRect)
    pub fn used_fraction(&self) -> f64   // fraction of total area currently NOT free
}
```
No free-rectangle merging on `remove` — real coalescing deferred until a
concrete fragmentation problem shows up.

```rust
pub trait RasterSource: Send {
    fn size(&self) -> (u32, u32);       // called before any pixel work, so the owner can check space first
    fn rasterize(&self) -> Vec<u8>;     // RGBA8, width*height*4 bytes, row-major
}
pub struct AtlasInsertRequest {
    pub key: AtlasKey,
    pub raster_source: Box<dyn RasterSource>,
    pub current_frame: u64,             // the requester's own frame-number notion, for LRU eviction weighing
}
```

```rust
pub struct AtlasKey(/* private u64 */);   // deliberately opaque -- this crate is content-agnostic
impl AtlasKey {
    pub fn from_glyph(font_id: u32, glyph_id: u32) -> Self
}
impl From<AtlasKey> for u64 { .. }
impl From<u64> for AtlasKey { .. }        // reconstructs from a raw key SwmrSlotTable::scan_older_than yields

#[must_use]
pub fn fits_packed_range(rect: PackedRect) -> bool   // check before pack_slot_value to avoid its own panic
/// # Panics — if any of rect's fields or `generation` exceeds its packed bit allotment.
#[must_use]
pub fn pack_slot_value(rect: PackedRect, generation: u16) -> u64
#[must_use]
pub fn unpack_slot_value(value: u64) -> (PackedRect, u16)
```

```rust
pub struct AtlasOwnerHandle { /* Clone, private fields */ }
impl AtlasOwnerHandle {
    /// Never blocks — false if the request queue is full, or the owner
    /// has already been joined (a request that raced a shutdown must not
    /// silently vanish into "still pending" forever).
    pub fn request_insert(&self, key: AtlasKey, raster_source: Box<dyn RasterSource>, current_frame: u64) -> bool
    /// None if not yet requested, requested-but-not-processed, or evicted
    /// -- deliberately indistinguishable (all three get the same
    /// placeholder-fallback response from a real caller).
    pub fn lookup(&self, key: AtlasKey, current_frame: u64) -> Option<(PackedRect, u16)>
}

pub struct AtlasOwner { /* private */ }
impl AtlasOwner {
    pub fn spawn(atlas_width: u32, atlas_height: u32, request_capacity: usize, slot_capacity: usize) -> Self
    /// A new Clone-able handle to this same owner.
    pub fn handle(&self) -> AtlasOwnerHandle
    /// # Panics — if the background thread itself panicked.
    pub fn join(self) -> Vec<u8>   // signals shutdown, waits, returns the finished RGBA8 atlas buffer
}
```

---

## `tre-rhi-vulkan`

The only real RHI backend — Vulkan 1.2+ via `ash`. One of the three
crates permitted `unsafe` (raw Vulkan FFI). Cross-platform wherever
Vulkan is available (not OS-gated, unlike the DX12/Metal stubs). Defines
**no traits of its own** — implements the `Rhi*` traits from `tre-engine`
(see [that section](#rhi-render-hardware-interface--trait-definitions)
for the full contract).

```rust
pub use headless::{HeadlessSwapchain, HEADLESS_FORMAT};
```

### Device / instance

```rust
pub struct VulkanDevice {
    pub instance: ash::Instance,
    pub physical_device: vk::PhysicalDevice,
    pub stencil_format: vk::Format,      // the real depth/stencil format this GPU supports
    pub device: ash::Device,
    pub queue_family_index: u32,
    /* private: graphics_queue, pools, transient pool, bindless apparatus, GC thread, etc. */
}
impl VulkanDevice {
    /// Creates the instance, a temporary probe surface, the logical
    /// device/queue, the shape-style buffer, and the full bindless
    /// descriptor apparatus; spawns the background GC thread.
    /// # Errors — EngineError on any setup failure.
    pub fn new(display_handle: raw_window_handle::RawDisplayHandle, window_handle: raw_window_handle::RawWindowHandle)
        -> Result<(Self, ash::khr::surface::Instance, vk::SurfaceKHR), EngineError>

    pub fn graphics_queue(&self) -> vk::Queue
    pub fn transient_pool_stats(&self) -> TransientPoolStats

    /// A new surface against this already-selected device — the
    /// multi-window path (all windows share the one device chosen at startup).
    /// # Errors — EngineError::DeviceLost.
    pub fn create_surface(&self, display_handle: raw_window_handle::RawDisplayHandle, window_handle: raw_window_handle::RawWindowHandle)
        -> Result<(ash::khr::surface::Instance, vk::SurfaceKHR), EngineError>

    /// Normal-blend-only pipeline from SPIR-V bytecode.
    /// # Errors — EngineError::PipelineCreationFailed.
    pub fn create_pipeline(&self, vertex_spv: &[u8], fragment_spv: &[u8], color_format: vk::Format)
        -> Result<VulkanPipelineState, EngineError>

    /// Non-Normal-blend-capable pipeline (VK_KHR_dynamic_rendering_local_read).
    /// # Panics — if local_read_blend_supported() is false; callers must check first.
    /// # Errors — EngineError::PipelineCreationFailed.
    pub fn create_blend_mode_pipeline(&self, vertex_spv: &[u8], fragment_spv: &[u8], color_format: vk::Format)
        -> Result<VulkanPipelineState, EngineError>

    /// One-shot vertex/index upload into a host-visible, host-coherent buffer.
    /// # Errors — EngineError::DeviceLost.
    pub fn upload_buffer(&self, bytes: &[u8], usage: vk::BufferUsageFlags) -> Result<VulkanBuffer, EngineError>
}
// impl RhiDevice for VulkanDevice — the trait methods listed under tre-engine above.
// impl Drop — stops/joins GC thread, waits idle, tears down every owned resource in dependency order.
```

```rust
pub struct TransientPoolStats {
    pub hits: u64,        // requests satisfied by reuse
    pub misses: u64,      // requests requiring a cold allocation
    pub evictions: u64,   // moved to the deferred-release queue by the GC thread
    pub destroyed: u64,   // actually destroyed after their 3-frame grace period
}
```

### Swapchain (windowed)

```rust
pub struct VulkanPipelineState { pub layout: vk::PipelineLayout, /* private */ }
// impl RhiPipelineState — raw_handle, layout_handle. impl Drop — destroys pipeline + layout.

pub struct VulkanSwapchain { /* private */ }
impl VulkanSwapchain {
    /// Real VkSwapchainKHR + dedicated stencil image, sized width x height.
    /// # Errors — EngineError::DeviceLost.
    pub fn new(device: &VulkanDevice, surface_loader: ash::khr::surface::Instance, surface: vk::SurfaceKHR, width: u32, height: u32)
        -> Result<Self, EngineError>
    pub fn format(&self) -> vk::Format
}
// impl RhiSwapchain — extent, stencil_view_handle, stencil_image_handle,
// supports_local_read_input_attachment, acquire_next_image, present.
// impl Drop — destroys semaphores, image views, stencil image, swapchain, surface.
```

### Headless rendering (`headless` module, re-exported at crate root)

```rust
pub const HEADLESS_FORMAT: vk::Format = vk::Format::B8G8R8A8_SRGB;   // same format VulkanSwapchain prefers

pub struct HeadlessSwapchain { /* private */ }
impl HeadlessSwapchain {
    /// Real manually-allocated color+stencil images (no VkSurfaceKHR at
    /// all), host-visible staging buffer, own command pool/fence.
    /// # Errors — EngineError::DeviceLost.
    pub fn new(device: &VulkanDevice, width: u32, height: u32) -> Result<Self, EngineError>

    /// Reads back the last presented frame as tightly-packed B8G8R8A8 bytes.
    /// # Errors — EngineError::DeviceLost if the memory map fails.
    pub fn read_pixels_bgra8(&self) -> Result<Vec<u8>, EngineError>

    pub fn width(&self) -> u32
    pub fn height(&self) -> u32
}
// impl RhiSwapchain — supports_local_read_input_attachment always true
// (a manually allocated image, nothing to query); present synchronously
// waits on its own readback_fence before returning.
// impl Drop — waits idle, destroys everything owned.
```

### Command buffer / draw submission

```rust
pub struct VulkanCommandBuffer { /* private, no inherent pub methods, no Drop */ }
// impl RhiCommandBuffer for VulkanCommandBuffer — every method listed
// under tre-engine's RhiCommandBuffer trait above. Real notes:
//   - set_pipeline always binds the bindless descriptor set at set 0 too.
//   - bind_texture: only slot == 0 is supported (silent no-op on release
//     for any other slot); out-of-range bindless_index falls back to a sentinel.
//   - apply_layer_blur: the real 4-hop Dual-Kawase chain (L0->L1->L2->U1->U0),
//     lazily initializes its own resources on first call.
```

### Buffers / ring buffers

```rust
pub struct VulkanBuffer { /* private */ }
// impl RhiBuffer — raw_handle. impl Drop — destroys buffer, frees memory.
// (Constructed only via VulkanDevice::upload_buffer.)

pub struct VulkanRingBuffer { /* private */ }
```
TECHNICAL.md Section 3.1's triple-buffered dynamic ring buffer: one
host-coherent `VkBuffer`, persistently mapped, divided into 3 equal
segments. Constructed internally only, reachable via
`RhiDevice::create_dynamic_ring_buffer`. `impl RhiBuffer`/
`RhiDynamicRingBuffer` — `write` returns `None` (not an error) on segment
overflow. `impl Drop` unmaps, destroys, frees.

### Bindless textures

```rust
pub struct VulkanTexture { /* private */ }
```
Constructed internally only (via `RhiDevice::acquire_transient_target`/
`create_texture`). `impl RhiTexture` — `raw_handle`, `image_handle`,
`memory_handle`, `dimensions`, `format`, `bindless_index`, `size_bytes`.
`impl Drop` destroys the image/view/memory and releases its bindless slot
if one was ever assigned. `from_pixels`'s internal construction validates
`pixels.len()` against `width*height*bytes_per_pixel` using `u128`
arithmetic specifically to avoid `u64` overflow wraparound on
attacker/caller-supplied dimensions.

---

## `tre-rhi-dx12` / `tre-rhi-metal` / `tre-ffi` (stub crates)

**All three currently contain zero real code** — confirmed by reading
each file in full. Each is exactly a crate-level doc comment plus
`#![deny(unsafe_op_in_unsafe_fn)]`, describing aspirational future scope:

- `tre-rhi-dx12`: DirectX 12 backend via the `windows` crate, Windows-only
  (target-gated in `Cargo.toml`; builds as a no-op elsewhere).
- `tre-rhi-metal`: Metal 2.4+ backend via `objc2-metal`, macOS-only.
- `tre-ffi`: the intended sole C-ABI export boundary (`#[repr(C)]` opaque
  handles + `extern "C"` functions, every exported function wrapped in
  `std::panic::catch_unwind`). **No `#[repr(C)]` types, no `extern "C"`
  functions, and no `catch_unwind` wrapping exist yet** — matches
  IMPLEMENTATION.md Phase 10 Step 10.3's own "NEXT", not "DONE", status.

None of the three have any public API to document beyond their own doc
comments. Do not write code assuming any symbol from these crates exists.

---

## `tre-a11y`

Publishes `tre-engine`'s tagged accessibility nodes to the real Linux
AT-SPI2 accessibility bus, via `accesskit`/`accesskit_unix`. Decoupled
from rendering — no RHI dependency, no render-loop hook of its own.
`#![forbid(unsafe_code)]`.

```rust
pub struct A11yBridge { /* private */ }
impl A11yBridge {
    /// Connects to the real Linux accessibility bus. Infallible:
    /// gracefully degrades to a permanently-inactive adapter if no AT-SPI2
    /// registry is reachable (confirmed by reading accesskit_unix's own
    /// source) — publish() becomes a real no-op in that case, not an error.
    pub fn connect(app_name: impl Into<String>, toolkit_name: impl Into<String>, toolkit_version: impl Into<String>) -> Self

    /// Publishes one frame's tagged nodes. Never blocks on D-Bus I/O
    /// (accesskit_unix owns its own background thread/channel for that).
    /// # Concurrency — single-writer only; two threads calling publish
    /// concurrently with different node sets could leave internal state
    /// out of sync with the live AT-SPI2 tree (two separate lock
    /// acquisitions, not one atomic step). Every real caller today
    /// publishes from one thread only.
    /// # Panics — if either internal Mutex is poisoned (a prior panic
    /// while holding the lock); not expected in normal operation.
    pub fn publish(&self, nodes: &[tre_engine::AccessibilityNode])
}
```
Note: `A11yBridge` consumes but does not re-export `tre_engine::
{AccessibilityNode, AccessibilityNodeId, AccessibilityRole}` — import
those from `tre_engine` directly, not from `tre_a11y`.

---

## Known, honestly disclosed gaps (do not build around these as if they were bugs to route around silently)

- **Window position/movement** (`tre-platform`): impossible by Wayland
  protocol design, not a library limitation. X11 also has no explicit
  positioning code here.
- **Swapchain resize recovery**: `acquire_next_image`/`present` correctly
  surface `EngineError::SwapchainOutOfDate`, but nothing in this codebase
  recovers from it yet — a real mid-run window resize panics
  `main_loop_demo`'s own reference main loop today (REVIEW.md #116, #151).
- **`RhiDynamicRingBuffer::write`** returns `Option<u32>`, narrower than
  the project's own blanket `Result`-for-fallible-ops policy (REVIEW.md #142).
- **`RhiCommandBuffer::apply_layer_blur`** and **`RhiDevice::
  create_dynamic_ring_buffer`** can panic on a real, recoverable resource
  exhaustion instead of propagating it (REVIEW.md #189) — would require
  widening their trait signatures to fix.
- **HDR/`tone_map`**: a real, tested primitive with no real caller yet —
  no HDR-capable swapchain surface is ever actually selected on any
  hardware this project has tested against.
- **DirectX 12 / Metal / non-Python C-ABI**: entirely unbuilt (see the
  stub-crates section above). Do not assume any RHI backend besides
  Vulkan exists, and do not assume a C-ABI/FFI boundary exists at all yet.
- **Python bindings**: also not yet built (IMPLEMENTATION.md Phase 10
  Step 10.4, "PLANNED") — when built, will bind directly to `tre-engine`
  via PyO3, not through `tre-ffi`.
