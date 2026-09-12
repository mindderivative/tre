# Fonts, Gradients & Textures

## `Font`

Real, caller-loaded font bytes, validated eagerly against both real consumers a `Text` shape will need. A `Font` is not tied to any one `ShapeRegistry` -- it's a plain, reusable value you can pass to shapes across multiple registries.

There is no plain constructor -- a `Font` only comes from one of three static methods:

| Method | Signature | Raises |
|---|---|---|
| `Font.load` | `(path: str) -> Font` | `RuntimeError` if the path can't be read; `ValueError` if the contents aren't a valid font |
| `Font.load_bytes` | `(data: bytes) -> Font` | `ValueError` if `data` isn't a valid font |
| `Font.system_cascade` | `() -> Font` | `RuntimeError` if fontconfig discovery is unavailable or the discovered file can't be read; `TreError` if discovery succeeds but returns zero candidate fonts |

```python
font = tre.Font.system_cascade()  # the primary system font, whatever it is
# or
font = tre.Font.load("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
```

`Font` is frozen (immutable) once constructed.

## `Gradient` / `GradientId`

`Gradient` describes a linear or radial gradient. It is **not itself usable as a fill** -- pass it to `registry.create_gradient(gradient)` to get back a `GradientId`, which *is* a valid `fill_color` value. There is no plain constructor; use one of the two static methods.

| Method | Signature |
|---|---|
| `Gradient.linear` | `(start: tuple[float, float], end: tuple[float, float], stops: list[tuple[float, int]]) -> Gradient` |
| `Gradient.radial` | `(center: tuple[float, float], radius: float, stops: list[tuple[float, int]]) -> Gradient` |

`stops` is a list of `(position, color)` pairs -- `position` must be in `0.0..=1.0` and non-decreasing across the list (this is validated for real, but only once you call `registry.create_gradient(...)`, which raises `ValueError` on an invalid list -- constructing the `Gradient` itself never raises).

```python
gradient = tre.Gradient.linear(
    (0.0, 0.0), (1.0, 0.0),
    [(0.0, tre.rgba8(255, 0, 0, 255)), (1.0, tre.rgba8(0, 0, 255, 255))],
)
gradient_id = registry.create_gradient(gradient)
rect = tre.Rectangle(10, 10, 200, 40, gradient_id)
```

**`GradientId` is scoped to the registry that created it.** Passing one to a *different* registry's `insert_*` call is a foreign/stale handle -- this **panics** (not a catchable Python exception) at that registry's own render/flatten time, with a clear message. Keep a `GradientId` paired with the `ShapeRegistry` that produced it.

## `Texture` / `TextureFormat`

A real, GPU-resident texture. Unlike `Gradient`, a `Texture` is **renderer-scoped**, not registry-scoped -- creating one needs a live GPU device, which only a renderer owns. There is no Python constructor at all; get one from a renderer:

```python
texture = renderer.create_texture(width, height, tre.TextureFormat.Rgba8Unorm, pixel_bytes)
rect = tre.Rectangle(0, 0, width, height, texture)
```

`Texture` exposes no methods or fields to Python -- it's an opaque handle. It keeps its underlying GPU resource alive via a shared reference for as long as any shape, or the `Texture` object itself, still needs it.

**`TextureFormat`** variants: `Bgra8Srgb`, `Rgba16Float`, `Rgba8Unorm`.

See [Rendering](rendering.md) for `create_texture`'s full signature and error conditions.