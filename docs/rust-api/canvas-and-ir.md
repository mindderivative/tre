# Canvas & Intermediate Representation

`tre-engine`'s `canvas` module (`RenderingCanvas`/`SubCanvas`/`FrameArena`) is the Drawing Context: an imperative, immediate-mode-style API that records every draw call into a compact intermediate representation, which a separate sort/batch/execute pipeline later turns into real GPU draw calls. This page also covers the IR types defined in `lib.rs` (`UiVertex`, `UiDrawCommand`, the sort key) and the GPU-side style records in `gpu_style.rs`.

## The intermediate representation

### `UiVertex` -- the canonical 32-byte vertex

```rust
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    pub position: [f32; 2], // 8 bytes: screen-space X, Y
    pub uv: [f32; 2],       // 8 bytes: texture coordinates or SDF bounds
    pub color: u32,         // 4 bytes: packed RGBA8 (sRGB, converted to linear in-shader)
    pub params: [f32; 3],   // 12 bytes: shader params (corner radii, stroke width, a style word index, ...)
}
// const _: () = assert!(std::mem::size_of::<UiVertex>() == 32);
```

`rgba8(r: u8, g: u8, b: u8, a: u8) -> u32` packs channels in the exact byte order `R8G8B8A8_UNORM` expects in memory -- a plain hex `u32` literal does **not** give you this for free: written in visual "RRGGBBAA" order, a literal like `0xE0A040FFu32` is stored little-endian as `[FF, 40, A0, E0]` in memory, the reverse of what the vertex format expects. Always construct vertex colors via `rgba8`, never a raw literal.

### `UiDrawCommand` / `CommandType` -- the IR command stream

```rust
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandType { DrawGeometry, PushScissor, PopScissor, PushLayer, PopLayer }

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UiDrawCommand {
    pub kind: CommandType,
    pub sort_key: u64,
    pub pipeline_state_id: u16,
    pub texture_handle: u32,  // bindless array index, or NO_TEXTURE (u32::MAX)
    pub element_count: u32,   // index count
    pub vertex_offset: u32,   // offset into the dynamic ring buffer
    pub clip_bounds: ScissorRect,
}
```

`NO_TEXTURE: u32 = u32::MAX` is the real "nothing bound" sentinel for a `DrawGeometry` command whose pipeline doesn't sample a texture -- deliberately not `0`, since a real bindless index `0` is a legitimate value a textured pipeline could use.

### The 64-bit sort key

```rust
// crate-private compute_sort_key -- shown for reference, not directly callable
fn compute_sort_key(layer_id: u16, pipeline_state_id: u16, texture_handle: u32, depth_id: u32) -> u64
```

The canonical key: `(LayerID << 48) | (PipelineID << 32) | (TextureID << 20) | DepthID`.

| Field | Width | Notes |
|---|---|---|
| Layer ID | 16 bits | `0`-`9999` standard content; `10000+` overlays/modals/popups (`OverlayLayerPriority`). |
| Pipeline ID | 16 bits | Which `PipelineKind` (below) this draw uses. |
| Texture ID | 12 bits | `TEXTURE_ID_MASK = 0xFFF` -- 4,096 concurrent bindless slots. |
| Depth ID | 20 bits | `DEPTH_ID_MASK = 0xF_FFFF` -- 1,048,576 slots (widened from 16 bits in a later documentation review); a single, global, monotonically increasing counter, shared (via `Arc<AtomicU32>`) across a root canvas and every `SubCanvas` so no two `DrawGeometry` commands anywhere in a frame ever collide. |

In debug builds, `compute_sort_key` panics if `texture_handle`/`depth_id` overflow their field widths -- compiled out entirely in release.

### `PipelineKind`

```rust
#[repr(u16)]
pub enum PipelineKind {
    SdfRoundedRect = 0, MsdfText = 1, TexturedQuad = 2,
    SdfRectStyled = 3, SdfEllipse = 4, FlatColor = 5,
    GradientFill = 6, FlatColorBlend = 7,
}
```

Real, type-safe names for `UiDrawCommand::pipeline_state_id`'s `Canvas`-emittable values. `FlatColorBlend` is notable: it's the pipeline behind non-`Normal` `BlendMode` fills, reading the destination pixel a *preceding* draw already wrote via `VK_KHR_dynamic_rendering_local_read` (a real framebuffer read, not hardware blend-op selection -- `VK_EXT_blend_operation_advanced`, the original plan, turned out not to be implemented by this project's own real dev GPU driver). `RhiDevice::local_read_blend_supported()` is the real, disclosed capability gate this pipeline is only ever selected behind; unsupported hardware falls back to plain `FlatColor` (`Normal` blending) rather than a silent wrong render.

### `TextureFormat` / `LayerDesc`

```rust
pub enum TextureFormat { Bgra8Srgb, Rgba16Float, Rgba8Unorm }

pub struct LayerDesc {
    pub x: i32, pub y: i32, pub width: u32, pub height: u32,
    pub format: TextureFormat,
    pub blur: bool,  // applies a fixed-chain Dual-Kawase blur to the layer's own content before compositing
}
```

`Bgra8Srgb`/`Rgba16Float` match the engine's SDR/HDR swapchain formats; `Rgba8Unorm` is for texture data that is never a color at all (e.g. an MSDF texel is a distance encoding) and must never pass through an `_SRGB` format's automatic gamma transform.

### GPU-side style records (`gpu_style`)

`UiVertex.params` has room for only 3 floats -- not enough for a rectangle's non-uniform corner radii, border, and fill selection all at once. Rather than bloat every pipeline's vertex layout, that data is bump-allocated as a fixed-layout record into the RHI's shape-style buffer (bound once, at device construction, to the bindless descriptor set), and referenced per-vertex by a single **word index** carried through one `params` float slot via `style_index_param(byte_offset: u32) -> f32`.

```rust
pub const RECT_STYLE_WORDS: u32 = 10;
pub struct GpuRectStyle {
    pub corner_radii: [f32; 4], pub border_color: u32, pub border_thickness: f32,
    pub corner_smoothing: f32, pub fill_kind: u32, pub gradient_word_index: u32, pub texture_index: u32,
}

pub const ELLIPSE_STYLE_WORDS: u32 = 7;
pub struct GpuEllipseStyle {
    pub border_color: u32, pub border_thickness: f32,
    pub arc_start_angle: f32, pub arc_sweep_angle: f32,
    pub fill_kind: u32, pub gradient_word_index: u32, pub texture_index: u32,
}

pub const GRADIENT_MAX_STOPS: usize = 8;
pub const GRADIENT_STYLE_WORDS: u32 = 6 + (GRADIENT_MAX_STOPS as u32) * 2;
pub struct GpuGradientStyle {
    pub kind: u32,  // 0 = linear, 1 = radial
    pub point0: [f32; 2], pub point1_or_radius: [f32; 2],
    pub stop_count: u32,
    pub stop_positions: [f32; GRADIENT_MAX_STOPS],
    pub stop_colors: [u32; GRADIENT_MAX_STOPS],  // packed sRGB; the shader interpolates in linear space
}

pub struct StyleFill {
    pub fill_kind: u32,           // 0 = solid, 1 = gradient, 2 = texture
    pub gradient_word_index: u32, // valid only when fill_kind == 1
    pub texture_index: u32,       // valid only when fill_kind == 2
}
impl StyleFill { pub const SOLID: Self = /* all-zero */; }
```

Each struct's exact word layout is documented field-by-field (a raw `readonly buffer { uint words[]; }` binding on the GLSL side reads fixed relative offsets, not GLSL struct-array indexing, so the CPU and GPU agree on plain byte layout rather than `std430`'s auto-computed stride) -- the GLSL side must be kept in lockstep by hand; nothing automatically checks it.

!!! warning "A real, found-and-fixed GPU bug"
    `style_index_param` bit-*casts* nothing -- it's a plain numeric `as f32` cast, deliberately not `f32::from_bits`. A bit-cast was tried first, and broke for real: a small word index like `64`, bit-cast to `f32`, produces a subnormal float (its exponent bits are all zero), and the real GPU silently flushed it to `0.0` somewhere between the vertex and fragment stage -- a common, real GPU behavior for subnormals -- so the fragment shader read back the *wrong* style record entirely. A numeric cast has no such failure mode: every real word index is a small integer, exactly representable as a normal `f32` (`f32` represents every integer up to 2^24 exactly), so the value round-trips exactly through `value as f32` / `uint(value)` with no denormal ever in play.

## `RenderingCanvas`

The Drawing Context itself: two genuinely separate stacks (`save`/`restore` for transform + alpha; `push_clip`/`pop_clip` for scissor rects), plus an overlay-layer stack and an offscreen-compositing layer stack.

```rust
impl RenderingCanvas {
    pub fn new() -> Self;
    pub fn max_sub_canvases(&self) -> usize;
    pub fn reset(&mut self);
    pub fn create_sub_canvas(&self) -> SubCanvas;

    pub fn save(&mut self);
    pub fn restore(&mut self);
    pub fn transform(&mut self, matrix: &tre_math::Affine2);
    pub fn set_alpha(&mut self, factor: f32);
    pub fn push_clip(&mut self, rect: &ScissorRect);
    pub fn pop_clip(&mut self);
    pub fn begin_overlay(&mut self, priority: OverlayLayerPriority);
    pub fn end_overlay(&mut self);
    pub fn push_layer(&mut self, desc: &LayerDesc);
    pub fn pop_layer(&mut self);

    pub fn draw_rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, rgba: u32);
    pub fn draw_styled_rectangle(&mut self, device: &dyn RhiDevice, x: f32, y: f32, w: f32, h: f32,
        corner_radii: [f32; 4], fill_rgba: u32, border_rgba: u32, border_thickness: f32,
        corner_smoothing: f32, fill: StyleFill);
    pub fn draw_ellipse(&mut self, device: &dyn RhiDevice, center_x: f32, center_y: f32, radius: [f32; 2],
        fill_rgba: u32, border_rgba: u32, border_thickness: f32,
        arc_start_angle: f32, arc_sweep_angle: f32, fill: StyleFill);
    pub fn draw_flat_polygon(&mut self, positions: &[[f32; 2]], triangles: &[[u32; 3]], rgba: u32);
    pub fn draw_flat_polygon_blended(&mut self, positions: &[[f32; 2]], triangles: &[[u32; 3]],
        rgba: u32, blend_mode: u32);
    pub fn draw_gradient_polygon(&mut self, positions: &[[f32; 2]], triangles: &[[u32; 3]],
        gradient_word_index: u32);
    pub fn draw_textured_polygon(&mut self, positions: &[[f32; 2]], uvs: &[[f32; 2]],
        triangles: &[[u32; 3]], texture_index: u32);
    pub fn draw_custom_shaded_quad(&mut self, x: f32, y: f32, w: f32, h: f32,
        pipeline_id: u16, rgba: u32, params: [f32; 3]);
    pub fn draw_text(&mut self, shaped: &tre_text::ShapedRun, font: &skrifa::FontRef, font_id: u32,
        origin: [f32; 2], px_size: f32, rgba: u32, atlas_context: &GlyphAtlasContext<'_>);

    pub fn tag_accessibility_node(&mut self, /* ... */);   // see Accessibility
    pub fn accessibility_nodes(&self) -> &[AccessibilityNode];
    pub fn tag_focusable(&mut self, /* ... */);            // see Input & Focus
    pub fn focusable_nodes(&self) -> &[FocusableNode];

    pub fn flatten(self) -> FlattenedFrame;
    pub fn flatten_unbatched(self) -> FlattenedFrame;
    pub fn stitch_into(&self, arena: &FrameArena) -> bool;
}
```

A few things worth knowing about this API's shape:

- **`draw_flat_polygon`/`draw_gradient_polygon`/`draw_textured_polygon`/`draw_flat_polygon_blended`** are each a genuinely separate pipeline (`FlatColor`/`GradientFill`/`TexturedQuad`/`FlatColorBlend`), not branches inside one shader -- existing callers of the simpler variant stay byte-for-byte unaffected as each richer one was added.
- **`begin_overlay`/`end_overlay`** and **`push_layer`/`pop_layer`** are two unrelated "layer" concepts that never interact: the former only distinguishes standard content (Layer ID `0`) from the overlay plane (`10000+`) in the sort key; the latter is real offscreen compositing (acquiring a transient render target, later baking a composite quad in `pop_layer`).
- **`draw_text`** needs a `GlyphAtlasContext<'a>` bundling the shared dynamic texture atlas handle, its current bindless GPU texture index, its pixel dimensions, and the caller's frame counter -- all four travel together at every call site.
- **`flatten`/`flatten_unbatched`/`stitch_into`** all `debug_assert!` that every stack (`layer_stack`, `state_stack`, `clip_stack`) is balanced before consuming the canvas -- an unbalanced `save`/`push_clip`/`push_layer` at frame boundary is a programmer error caught immediately in debug builds.

### `SubCanvas`

```rust
pub struct SubCanvas { /* private */ }
// Deref/DerefMut to RenderingCanvas -- every drawing method above works unchanged
```

A worker thread's independently-recordable canvas (`RenderingCanvas::create_sub_canvas()`), with its own fresh `state_stack`/`clip_stack`/`overlay_stack`/vertex/index/command buffers. The **one** thing it shares with its root (and every sibling `SubCanvas`) is the Depth ID counter -- exactly the field that must be globally unique across threads for the sort key to work. Cannot be `flatten()`ed directly (that takes `self` by value, which `Deref` can't forward, and a sub-canvas is never a frame's final destination); merging its recorded data into a shared `FrameArena` is `stitch_into`'s job. `Drop` decrements the live-sub-canvas counter, correctly even if a caller panics mid-use.

## `FrameArena` -- multi-threaded recording's merge target

```rust
pub struct FrameArena { /* private: ScatterArena-backed */ }

impl FrameArena {
    pub fn with_capacity(vertex_capacity: usize, index_capacity: usize,
        command_capacity: usize, accessibility_capacity: usize) -> Self;
    pub fn flatten(self) -> FlattenedFrame;
    pub fn flatten_into(&mut self, out: &mut FlattenedFrame);  // zero-allocation-in-steady-state sibling
}
```

Backed by `tre-memory::ScatterArena` (see [Math & Memory](math-and-memory.md)) for each of its vertex/index/command/accessibility-node buffers -- any number of `SubCanvas::stitch_into` calls from different worker threads can merge into one `FrameArena` lock-free, each reserving its own disjoint output range. Call `flatten`/`flatten_into` only after every contributing `stitch_into` call has returned -- typically "after every worker thread has been joined." `flatten_into` is the reusable, non-consuming sibling: it drains each internal `ScatterArena` (which also resets it for the next frame) into persistent scratch storage, so a long-lived `FrameArena` never reallocates once its steady-state size is reached.

## `FlattenedFrame`

```rust
#[derive(Default)]
pub struct FlattenedFrame {
    pub vertices: Vec<UiVertex>,
    pub indices: Vec<u32>,
    pub commands: Vec<UiDrawCommand>,
    pub accessibility_nodes: Vec<AccessibilityNode>,
}
```

A frame's fully-recorded, sorted-and-merged batch: one contiguous vertex/index stream plus the list of draw commands describing how to slice it into RHI draw calls. This is what `RhiCommandBuffer`/`execute_frame` (see [RHI Trait & Backends](rhi-and-backends.md)) actually consumes.
