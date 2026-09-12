# Architecture

TRE is a pipeline: a retained-mode drawing API records commands into a compact intermediate representation, which is sorted, batched, and finally executed against a graphics API. Every stage is designed around one constraint -- **zero heap allocation during the steady-state render tick** -- which shapes almost every data structure described below.

## Crate layout

| Crate | Role | `unsafe` |
|---|---|---|
| `tre-engine` | The `Canvas` API, the IR, the sort/batch pipeline, the glyph/vector atlas frontend, SVG/MSDF tessellation | forbidden |
| `tre-math` | Vector/matrix math, SIMD via the `wide` crate | forbidden |
| `tre-memory` | Ring arenas, the transient GPU pool, atlas lock-free concurrency primitives | permitted |
| `tre-text` | Font shaping (`skrifa`/`rustybuzz`), caret hit-testing, line-breaking | forbidden |
| `tre-atlas` | Guillotine bin-packing and the multi-window-safe atlas concurrency model | forbidden |
| `tre-svg` | SVG parsing/tessellation, vertex-morph interpolation | -- |
| `tre-tween` / `tre-animation` | Easing curves, the spring integrator, and the timeline sequencer | forbidden |
| `tre-a11y` | The Linux AT-SPI2 accessibility bridge | -- |
| `tre-rhi-vulkan` | The Vulkan 1.2+ backend | permitted |
| `tre-rhi-dx12` / `tre-rhi-metal` | DirectX 12 (Windows) / Metal (macOS) backends -- scaffolded, not yet implemented | permitted |
| `tre-platform` | Native windowing and input, via `winit`; clipboard, file dialogs, tray | forbidden |
| `tre-ffi` | The engine's entire public surface as a stable, panic-safe `extern "C"` boundary, for every language *other* than Python | permitted |
| `tre-python` | Direct PyO3 bindings to `tre-engine`'s native Rust API -- bypasses `tre-ffi` entirely | permitted (one call) |

`unsafe` is confined to exactly four crates (`tre-memory`'s concurrency primitives, the RHI backends' raw graphics API FFI, `tre-ffi`'s handle/pointer conversion, and one specific teardown call in `tre-python`) -- every other crate, including `tre-engine` itself, carries `#![forbid(unsafe_code)]`. `tre-python` binds directly to `tre-engine`, never through `tre-ffi`: two independent build outputs, chosen specifically to avoid a double marshalling round-trip through a C-compatible shadow representation on every high-frequency call (e.g. a shape's own per-frame property mutation).

## The pipeline, end to end

```
Canvas API  --records-->  Intermediate Representation  --sorts/batches-->  RHI execution
(save/restore,             (UiVertex, UiDrawCommand,        (radix sort by      (Vulkan draw
 clip/layer,                 FlattenedFrame)                  64-bit key,         calls, one
 draw_*, tag_*)                                                merge adjacent      swapchain
                                                                runs)               present)
```

A `Canvas` never touches the GPU directly. Every `draw_*`/`tag_*` call appends to plain, pre-sized `Vec`s (`vertices`, `indices`, `commands`); `flatten()` sorts and merges those into a `FlattenedFrame`; `execute_frame` walks that frame's commands and issues the real graphics-API calls. This separation is what makes headless rendering, multi-threaded recording, and three different graphics backends all possible without duplicating drawing logic three times.

## The Canvas API

`RenderingCanvas` owns two genuinely separate stacks -- `state_stack` (transform + alpha, pushed/popped by `save()`/`restore()`) and `clip_stack` (scissor rects, pushed/popped by `push_clip()`/`pop_clip()`) -- plus a `layer_stack` for offscreen compositing and an `overlay_stack` for z-order routing above normal content. Every drawing method reads the top of `state_stack` to transform its own local-space coordinates into world space before recording a command -- this is where `tag_accessibility_node`/`tag_focusable`'s own "transform-correct world-space bounds" comes from (see [Accessibility & Focus](python-api/accessibility-and-focus.md)).

## The intermediate representation

Every vertex the engine ever produces is exactly 32 bytes:

```rust
#[repr(C)]
pub struct UiVertex {
    pub position: [f32; 2], // 8 bytes: screen-space X, Y
    pub uv: [f32; 2],       // 8 bytes: texture coordinates or SDF bounds
    pub color: u32,         // 4 bytes: packed RGBA8
    pub params: [f32; 3],   // 12 bytes: shader params (corner radii, stroke width, custom-shader data, ...)
}
```

That fixed 32-byte budget is deliberately tight -- growing it would bloat every pipeline in the system. It's also the real reason [Shadow v2](python-api/rendering.md)'s SDF shadow packs seven logical values into `params`' three float slots via normalization instead of a straightforward fourth field.

A draw command is equally compact:

```rust
pub enum CommandType { DrawGeometry, PushScissor, PopScissor, PushLayer, PopLayer }

pub struct UiDrawCommand {
    pub kind: CommandType,
    pub sort_key: u64,        // see below
    pub pipeline_state_id: u16,
    pub texture_handle: u32,  // bindless array index, or atlas handle
    pub element_count: u32,   // index count
    pub vertex_offset: u32,   // offset into the dynamic ring buffer
    pub clip_bounds: ScissorRect,
}
```

Flattening a canvas produces a `FlattenedFrame { vertices, indices, commands, accessibility_nodes }` -- the one value that crosses from "recorded IR" into "ready to submit."

## The 64-bit sort key and batching

Every `DrawGeometry` command carries one packed key:

```
(LayerID << 48) | (PipelineID << 32) | (TextureID << 20) | DepthID
```

| Field | Width | Range | Purpose |
|---|---|---|---|
| Layer ID | 16 bits | 0-65535 | Overlay routing -- normal content is layer `0`; `begin_overlay(priority)` content sorts above it |
| Pipeline ID | 16 bits | 0-65535 | Which GPU pipeline a command needs (SDF rect, MSDF text, custom shader, ...) |
| Texture ID | 12 bits | 0-4095 | The bindless array slot or atlas handle a command samples |
| Depth ID | 20 bits | 0-1,048,575 | Monotonically increasing per canvas, per frame -- preserves real painter's-algorithm draw order within an otherwise-identical batch |

Radix-sorting the whole command list by this one `u64` key (a real, from-scratch 4-pass LSD radix sort, 16 bits per pass) means commands that share a layer, pipeline, and texture end up adjacent -- and adjacent, compatible commands merge into a single indexed draw call. This is the whole mechanism behind the project's own "single-digit draw calls per frame" goal: batching falls out of the sort, rather than needing a separate batching pass. In debug builds, exceeding the Texture ID or Depth ID field width panics immediately rather than silently corrupting a neighboring field.

## Multi-threaded recording

A `SubCanvas` lets multiple OS threads record geometry concurrently without a lock: it's a thread-local linear arena that shares exactly one thing with its root canvas -- the atomic Depth ID counter each `next_sort_key()` call draws from, so every command across every thread still gets a globally distinct sort position. Every existing drawing method works on a `SubCanvas` unchanged (it `Deref`s to the same recording surface a root `Canvas` uses); the one thing it can't do is `flatten()` itself, since it's never a frame's final destination.

Merging happens through a `FrameArena`: each worker's `SubCanvas` is `stitch_into`'d via a lock-free atomic fetch-add that reserves its own slice of the arena's pre-sized buffers, so N threads recording concurrently never contend on a mutex -- they just claim disjoint index ranges and write into them independently. `render_parallel` in the Python API (see [Rendering](python-api/rendering.md)) is this mechanism exposed directly: pass several `ShapeRegistry` objects, get genuinely parallel flattening with no manual thread management.

## The atlas

Glyphs (MSDF-rendered for crisp edges at any scale) and other small raster content share one dynamic atlas, packed with a Guillotine bin-packer (best-area-fit selection, always splitting leftover space into exactly two non-overlapping rectangles). A dedicated background thread (`AtlasOwner`) drains insertion requests from any number of producer threads over a lock-free MPSC queue, does the real packing and rasterization, and publishes results into a single-writer/multi-reader slot table any renderer thread can read without ever blocking -- the mechanism that makes multi-window text rendering safe without a shared mutex. Tombstone deletion and LRU eviction reclaim atlas space once a real app's glyph vocabulary grows past what fits; `tre-python`'s own text atlas (a separate instance from the engine's internal one, sized 1024x1024) refreshes its GPU texture every 15 frames and is disclosed as exhaustible -- a real app whose text keeps introducing new glyphs forever will eventually hit the fixed-size bindless array's own 4096-slot ceiling.

## The RHI abstraction

`RhiDevice`/`RhiCommandBuffer`/`RhiSwapchain`/`RhiBuffer`/`RhiTexture`/`RhiPipelineState` are the trait surface every backend implements; `execute_frame` and `submit_frame` are the two free functions that drive them generically, resolving each command's `pipeline_state_id` through a `PipelineRegistry` rather than a hardcoded per-shape branch -- this is exactly the seam that let [custom shaders](python-api/rendering.md#custom-shaders) get added later with zero changes to the execution path itself.

Only `tre-rhi-vulkan` is implemented today (Vulkan 1.2+, requiring `VK_KHR_dynamic_rendering` and `VK_EXT_descriptor_indexing` for bindless textures -- see [Getting Started](getting-started.md) for the exact hardware floor). It maintains one persistent bindless descriptor set covering up to 4096 textures, selected per draw call via a push constant rather than a per-vertex index. Its Dual-Kawase blur (behind `Canvas.layer(..., blur=True)`) is a real, fixed 4-hop downsample/upsample chain against one reused unit quad -- see [Canvas & Shapes](python-api/canvas-and-shapes.md) and [Rendering](python-api/rendering.md#built-in-sdf-soft-shadows) for the two different shadow techniques built on top of it. `tre-rhi-dx12`/`tre-rhi-metal` exist as scaffolded, empty crates -- Windows and macOS support is not yet implemented.

## The full frame loop

A real windowed application's per-frame sequence, as proven end to end by the engine's own continuous main-loop demo:

**Drain Events → Wait Fences → Multi-Thread Canvas → Sub-Canvas Stitch → Tessellation/Atlas Check → Radix Sort & Batch → Ring Buffer Packing → RHI Submit & Present**

Every stage above corresponds to a real, independently-tested mechanism described in this page -- the loop is the proof they compose correctly together, every frame, not a new mechanism of its own. From Python, `WindowedRenderer.render(window, registry)` (see [Rendering](python-api/rendering.md)) runs this same sequence for you.

## What's deliberately not here

Layout computation and styling/theming are explicitly out of scope for this engine -- they're the responsibility of the Python UI framework consuming it (pySilver). `tre-engine` positions shapes strictly via caller-supplied transforms; it has no flex/grid/constraint solver and no theme/token system. See the Python API's [Overview](python-api/index.md) for the boundary this implies for every shape's own `x`/`y`/`scale`/`rotation` fields.