# Canvas & Shapes

## `ShapeRegistry`

The generational shape store. Insert shapes, get back a stable `ShapeId`, then render via a renderer's `render(registry)`/`flatten_into(canvas, registry)`.

```python
registry = tre.ShapeRegistry()
rect_id = registry.insert_rectangle(tre.Rectangle(10, 10, 60, 40, tre.rgba8(200, 60, 60, 255)))
```

| Method | Signature | Raises |
|---|---|---|
| `__init__` | `()` | |
| `create_gradient` | `(gradient: Gradient) -> GradientId` | `ValueError` if the gradient's stops are invalid (too many, non-finite, out of `0.0..1.0`, out of order) or a radial radius isn't positive |
| `insert_rectangle` | `(rect: Rectangle) -> ShapeId` | `ValueError` on non-finite fields, negative width/height, or an invalid `fill_color` |
| `insert_circle` | `(circle: Circle) -> ShapeId` | `ValueError` (same style: non-finite/negative radii, bad `fill_color`) |
| `insert_polygon` | `(polygon: Polygon) -> ShapeId` | `ValueError` if `sides`/`star_points` exceed 4096, or other numeric checks fail |
| `insert_path` | `(path: Path) -> ShapeId` | `ValueError` on non-finite/negative `border_thickness`/`opacity`, or bad `fill_color` |
| `insert_svg` | `(svg: Svg) -> ShapeId` | `ValueError` if `x`/`y`/`opacity` are non-finite |
| `insert_text` | `(text: Text) -> ShapeId` | `ValueError` if `x`/`y`/`opacity` are non-finite, or `px_size` is non-finite or not `> 0` |
| `insert_custom_shaded` | `(custom: CustomShaded) -> ShapeId` | `ValueError` on non-finite fields or negative `width`/`height` |
| `remove` | `(id: ShapeId) -> bool` | |
| `__len__` | `() -> int` | |
| `is_empty` | `() -> bool` | |

Notes:

- `corner_smoothing` on `Rectangle` is **clamped** to `0.0..1.0` at insert time rather than rejected -- "a cosmetic parameter an animation can briefly overshoot" is treated as harmless.
- `insert_custom_shaded` does **not** validate that `custom.pipeline_id` was actually registered against whatever renderer eventually renders this shape -- a foreign or unregistered pipeline id is a real runtime failure surfaced at render time instead.
- `Path`'s own coordinates are validated when you call `move_to`/`line_to`/etc, not again at insert time.

## `Canvas`

The scene-assembly/compositing context shapes flatten into -- real scissor clipping, real offscreen layers with optional blur, and accessibility/focus tagging. Doubles as a context manager: entering calls `save()`, exiting calls `restore()`.

```python
with tre.Canvas() as canvas:
    with canvas.clip(0, 0, 200, 100):
        ...
```

| Method | Signature | Notes / raises |
|---|---|---|
| `__init__` | `()` | |
| `clip` | `(x: int, y: int, width: int, height: int) -> ClipGuard` | Pushes onto the clip stack immediately; the returned guard's `__exit__` pops it -- use with `with`, not bare calls |
| `layer` | `(x: int, y: int, width: int, height: int, blur: bool = False) -> LayerGuard` | Real offscreen compositing; `blur=True` applies the engine's Dual-Kawase blur to the layer's *own* content only (not a backdrop blur) |
| `tag_accessibility_node` | `(node_id: int, x: float, y: float, width: float, height: float, role: AccessibilityRole)` | Not a context manager -- no matching "untag". Coordinates are in this canvas's local space, transformed by whatever `save()`/`clip()`/`layer()` scope is active |
| `accessibility_nodes` | `() -> list[AccessibilityNode]` | Every node tagged so far this frame -- see [Accessibility & Focus](accessibility-and-focus.md) |
| `tag_focusable` | `(node_id: int, x: float, y: float, width: float, height: float, tab_index: int \| None = None)` | `tab_index` follows the HTML `tabindex` convention -- see [Accessibility & Focus](accessibility-and-focus.md) |
| `focusable_nodes` | `() -> list[FocusableNode]` | Every node tagged so far this frame via `tag_focusable` |

**Read `accessibility_nodes()`/`focusable_nodes()` before calling `render_canvas()`.** A renderer's `render_canvas(canvas)` consumes the canvas's recorded content -- including tagged nodes -- so it comes back empty afterward. Tag, read, *then* render.

## Shape primitives

All shape classes are plain, mutable data objects -- constructing one never touches the engine; only `registry.insert_*` does. Every shape has `fill_color`, `opacity: float = 1.0`, `scale_x: float = 1.0`, `scale_y: float = 1.0`, `rotation: float = 0.0` (radians) in addition to the fields listed below.

### `Rectangle`

```python
tre.Rectangle(x, y, width, height, fill_color, scale_x=1.0, scale_y=1.0, rotation=0.0)
```

Additional fields (all read/write): `border_color: int = 0`, `border_thickness: float = 0.0`, `corner_radius: float = 0.0`, `corner_smoothing: float = 0.0`, `border_enabled: bool = True` -- a real on/off switch independent of `border_thickness`, so you can toggle a border off without losing its configured width.

### `Circle`

```python
tre.Circle(x, y, radius, fill_color, scale_x=1.0, scale_y=1.0, rotation=0.0)
```

**`x`/`y` are the top-left of the shape's bounding box, not the circle's center.** For a circle centered at `(cx, cy)` with a given `radius`, pass `x=cx-radius, y=cy-radius`.

Additional fields: `radius_x`/`radius_y: float` (both set to `radius` by the constructor -- set them independently afterward for an ellipse), `border_color`, `border_thickness`, `border_enabled`, and `arc_length: float = 360.0` -- degrees, `0.0..=360.0`, a partial sweep starting at 12 o'clock, clockwise.

### `Polygon`

```python
tre.Polygon(x, y, sides, radius, fill_color, scale_x=1.0, scale_y=1.0, rotation=0.0)
```

A regular polygon, or a star if `star_points` is set. Additional fields: `sides: int`, `radius: float`, `vertex_radius: float` (only used when `star_points` is not `None`), `star_points: int | None = None` (`None`: a regular `sides`-gon; `Some(k)`: a `2*k`-vertex star), `border_color`, `border_thickness`, `border_enabled`. `sides`/`star_points` are capped at 4096 by `insert_polygon`.

### `Path`

```python
path = tre.Path(fill_color, scale_x=1.0, scale_y=1.0, rotation=0.0)
path.move_to(0, 0)
path.line_to(100, 0)
path.quad_to(100, 50, 50, 100)
path.cubic_to(0, 100, -50, 50, 0, 0)
path.close()
```

Built from `move_to`/`line_to`/`quad_to`/`cubic_to`/`close` calls -- the same shape as the HTML5 Canvas path API. `Path` has no `x`/`y` of its own (commands are already authored in absolute coordinates), so `scale`/`rotation` apply about the local origin `(0, 0)`.

| Method | Signature | Raises |
|---|---|---|
| `move_to` | `(x: float, y: float)` | `ValueError` if `x`/`y` aren't finite |
| `line_to` | `(x: float, y: float)` | same |
| `quad_to` | `(cx: float, cy: float, x: float, y: float)` | same |
| `cubic_to` | `(c1x, c1y, c2x, c2y, x, y: float)` | same |
| `close` | `()` | never raises |

Additional fields: `border_color`, `border_thickness`, `border_enabled`.

### `Text`

```python
tre.Text(x, y, text, font, px_size, fill_color, scale_x=1.0, scale_y=1.0, rotation=0.0, wrap_width=None)
```

**`x`/`y` are the top-left of the text block, not the baseline.** **Solid fill only** -- `fill_color` is a plain `int`, not the polymorphic union.

Additional fields: `text: str`, `font: Font`, `px_size: float`, `wrap_width: float | None = None` -- `None` renders a single line with no wrapping; `Some(width)` enables real multi-line rendering (`\n` always breaks a line, words greedily wrap to fit `width`; pass `float("inf")` for hard-wrap-only, breaking only on `\n`).

### `CustomShaded`

```python
tre.CustomShaded(x, y, width, height, pipeline_id, fill_color,
                 scale_x=1.0, scale_y=1.0, rotation=0.0,
                 param_x=0.0, param_y=0.0, param_z=0.0)
```

A quad rendered through a caller-registered custom shader pipeline. Get a `pipeline_id` (a `CustomShaderId`) from a renderer's `create_custom_shader(...)` first -- see [Rendering](rendering.md). `fill_color` here is a plain flat color multiplier (the vertex color your fragment shader receives as `frag_color`), not the polymorphic union. `param_x`/`param_y`/`param_z` are the real third per-vertex channel (`UiVertex.params`) -- your fragment shader reads them back as `frag_params.x`/`.y`/`.z`. They're generic, opaque floats: what they mean is entirely up to your own shader.

## `ShapeId`

A frozen, opaque handle returned by every `insert_*` call. Pass it to `registry.remove(id)`.

## Free functions

- **`tre.rgba8(r: int, g: int, b: int, a: int) -> int`** -- packs 8-bit RGBA channels into the `int` every `fill_color`/`border_color` field expects.
- **`tre.shadow_layer_bounds(x, y, width, height, offset_x, offset_y, blur_margin=24.0) -> tuple[int, int, int, int]`** -- computes the `(x, y, width, height)` a drop shadow's `canvas.layer(..., blur=True)` call should use for a shape at `(x, y, width, height)`, cast with `(offset_x, offset_y)` and `blur_margin` pixels of extra room for the blur to spread into. Pure geometry -- draw the shadow shape itself inside that layer using the *same* `canvas.layer(blur=True)` mechanism.