# Rendering

Two renderers share almost the same API: `HeadlessRenderer` renders to an in-memory pixel buffer (no window, no display needed beyond a reachable GPU/display-server connection); `WindowedRenderer` renders to one or more real OS windows and also owns their event queues.

Both release the GIL for the real, blocking GPU work inside every render call, so other Python threads keep running while a frame renders.

## `HeadlessRenderer`

```python
renderer = tre.HeadlessRenderer(width, height)
frame: bytes = renderer.render(registry)  # tightly-packed BGRA8 bytes, width*height*4 long
```

| Method | Signature | Raises |
|---|---|---|
| `__init__` | `(width: int, height: int)` | `ValueError` if `width`/`height` are zero or exceed 8192; `TreError`/`RuntimeError` on real setup failure |
| `width`, `height` | properties, `int` | |
| `render` | `(registry: ShapeRegistry) -> bytes` | `TreError` on a recoverable engine failure |
| `flatten_into` | `(canvas: Canvas, registry: ShapeRegistry)` | `TreError` if a due text-atlas texture refresh fails |
| `render_canvas` | `(canvas: Canvas) -> bytes` | `TreError`. Consumes the canvas's recorded content, leaving it empty for reuse |
| `render_parallel` | `(registries: list[ShapeRegistry]) -> bytes` | `ValueError` if `len(registries)` exceeds this machine's available parallelism minus one; `TreError` on a real failure. Flattens every registry on its own OS thread, genuinely in parallel, then composites them into one frame |
| `create_texture` | `(width, height, format: TextureFormat, pixels: bytes) -> Texture` | `TreError` if `pixels` length doesn't match `width*height*bytes_per_pixel(format)`, dimensions are zero, or the bindless texture array (4096 slots) is exhausted |
| `create_custom_shader` | `(fragment_source: str) -> CustomShaderId` | See **Custom shaders** below |

`render`/`render_canvas`/`render_parallel` all take `&mut self` on the Rust side -- calling one from two threads concurrently (rather than the documented single-threaded-per-instance use) raises a clean error rather than corrupting GPU state.

## `WindowedRenderer`

**Must stay on its constructing thread** (see the [Overview](index.md)'s thread-affinity note) -- it owns a real platform connection.

```python
renderer = tre.WindowedRenderer("My App", 800, 600)
window = renderer.main_window

while True:
    for event in renderer.poll_events():
        if isinstance(event, tre.InputEvent.CloseRequested):
            raise SystemExit
    renderer.render(window, registry)
```

| Method | Signature | Raises |
|---|---|---|
| `__init__` | `(title: str, width: int, height: int)` | `ValueError` if dimensions are zero/unreasonable; `TreError`/`RuntimeError` on display-server or window setup failure |
| `main_window` | property, `WindowId` | |
| `open_window_count` | property, `int` | |
| `create_window` | `(title: str, width: int, height: int) -> WindowId` | Same validation as `__init__` |
| `close_window` | `(window: WindowId)` | A no-op if `window` is already unknown/closed |
| `poll_events` | `() -> list[InputEvent]` | Call once per frame; never blocks. See [Input Events](input-events.md) |
| `render` | `(window: WindowId, registry: ShapeRegistry)` | `ValueError` if `window` is unknown/closed; `TreError` otherwise |
| `flatten_into` | `(canvas: Canvas, registry: ShapeRegistry)` | `TreError` if a due text-atlas refresh fails |
| `render_canvas` | `(window: WindowId, canvas: Canvas)` | `ValueError`/`TreError`. Consumes the canvas, same as the headless renderer |
| `render_parallel` | `(window: WindowId, registries: list[ShapeRegistry])` | `ValueError` if `window` is unknown or `len(registries)` exceeds the parallelism cap; `TreError` otherwise |
| `create_texture` | `(width, height, format: TextureFormat, pixels: bytes) -> Texture` | Same contract as `HeadlessRenderer.create_texture` |

A window that reports `SwapchainOutOfDate` (e.g. after a real resize) is recreated at its last-known size and the render retried once automatically -- you don't need to handle this yourself, just call `render()` normally.

### Window chrome

| Method | Signature | Notes |
|---|---|---|
| `set_title` | `(window: WindowId, title: str)` | `ValueError` if `window` unknown |
| `set_minimized` | `(window: WindowId, minimized: bool)` | `ValueError` if unknown |
| `is_minimized` | `(window: WindowId) -> bool \| None` | |
| `set_maximized` | `(window: WindowId, maximized: bool)` | `ValueError` if unknown |
| `is_maximized` | `(window: WindowId) -> bool` | |
| `set_icon` | `(window: WindowId, rgba: bytes \| None, width: int, height: int)` | `rgba=None` clears the icon. **Real no-op on Wayland** -- the protocol has no client-side icon mechanism. `ValueError` if `window` unknown or `rgba` length ≠ `width*height*4` |
| `set_cursor` | `(window: WindowId, icon: CursorIcon)` | `ValueError` if `window` unknown |
| `set_ime_allowed` | `(window: WindowId, allowed: bool)` | Required before `poll_events()` ever returns `ImeEnabled`/etc. Enable only while a text field has focus; disable again when focus leaves. `ValueError` if `window` unknown |

### `CursorIcon`

34 variants mirroring the CSS cursor keyword set: `Default`, `ContextMenu`, `Help`, `Pointer`, `Progress`, `Wait`, `Cell`, `Crosshair`, `Text`, `VerticalText`, `Alias`, `Copy`, `Move`, `NoDrop`, `NotAllowed`, `Grab`, `Grabbing`, `EResize`, `NResize`, `NeResize`, `NwResize`, `SResize`, `SeResize`, `SwResize`, `WResize`, `EwResize`, `NsResize`, `NeswResize`, `NwseResize`, `ColResize`, `RowResize`, `AllScroll`, `ZoomIn`, `ZoomOut`.

## Custom shaders

Compile real GLSL to SPIR-V at runtime and render through it via the `CustomShaded` shape (see [Canvas & Shapes](canvas-and-shapes.md)).

```python
fragment_source = """
#version 450
layout(location = 0) in vec2 frag_uv;
layout(location = 0) out vec4 out_color;
void main() { out_color = vec4(frag_uv, 0.0, 1.0); }
"""
pipeline_id = renderer.create_custom_shader(fragment_source)
shape = tre.CustomShaded(0, 0, 100, 100, pipeline_id, tre.rgba8(255, 255, 255, 255))
```

`create_custom_shader(fragment_source: str) -> CustomShaderId` -- available on both `HeadlessRenderer` and `WindowedRenderer`. Raises `ValueError` with the shader compiler's own diagnostic if compilation fails; `TreError` if pipeline creation fails after a successful compile.

**`CustomShaderId` is scoped to the renderer that created it**, exactly like `GradientId` is scoped to its registry -- using it against a different renderer is a real, disclosed caller error (that renderer's own pipeline registry never registered this id).

### Built-in SDF soft shadows

A ready-made shader, built on the same custom-shader mechanism, for a continuously tunable soft shadow (as opposed to the fixed-radius blur-based shadow described in [Canvas & Shapes](canvas-and-shapes.md)):

```python
pipeline_id = renderer.create_custom_shader(tre.sdf_shadow_shader_source())
quad = tre.shadow_sdf_params(x, y, width, height, radius_px=8.0, blur_px=16.0)
quad_x, quad_y, quad_width, quad_height, radius_frac, sigma_x_frac, sigma_y_frac = quad
shadow = tre.CustomShaded(
    quad_x, quad_y, quad_width, quad_height, pipeline_id,
    tre.rgba8(0, 0, 0, 160),
    param_x=radius_frac, param_y=sigma_x_frac, param_z=sigma_y_frac,
)
```

- **`tre.sdf_shadow_shader_source() -> str`** -- the GLSL source, ready to pass straight to `create_custom_shader`.
- **`tre.shadow_sdf_params(x, y, width, height, radius_px, blur_px) -> tuple[float, float, float, float, float, float, float]`** -- returns `(quad_x, quad_y, quad_width, quad_height, radius_frac, sigma_x_frac, sigma_y_frac)`. The returned quad is deliberately *larger* than your logical shape (enlarged by `blur_px` on every side) -- a fragment shader can't draw outside the triangles it's rasterized on, so the soft edge needs real rasterized room to extend into. `radius_px` is clamped to the box's own smaller half-dimension; a zero-size box degrades every fraction to `0.0` rather than dividing by zero.

**Real, disclosed v1 scope:** the falloff is a real SDF soft edge (`smoothstep` around a signed-distance zero crossing), not a true Gaussian convolution of a box; a single uniform corner radius, not per-corner; non-square boxes get a per-axis-normalized approximation, not literal pixel circles at large aspect ratios.