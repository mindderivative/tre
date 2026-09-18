# Canvas & Virtualized Lists

## Custom drawing with Canvas

```python
def draw(ctx):
    ctx.fill_rect(0, 0, 100, 60, color=(0x67, 0x50, 0xA4, 0xFF))
    ctx.fill_circle(50, 30, 20, color=(0xFF, 0xFF, 0xFF, 0xFF))
    ctx.stroke_path([[0, 0], [50, 60], [100, 0]], color=(0x00, 0x00, 0x00, 0xFF), width=2.0)

canvas = window.add_canvas(width=100, height=60, draw=draw)
window.redraw_canvas(canvas)
```

`add_canvas(width, height, draw, x=None, y=None)` creates a custom-drawn
node — `draw` is *stored*, not called yet. `redraw_canvas(canvas)` is
the real entry point: it calls `draw(ctx)` exactly once, then replaces
the canvas's entire drawn content with whatever `ctx` collected. Nothing
redraws a canvas automatically every frame — call `redraw_canvas` again
whenever the content actually needs to change.

`CanvasContext` methods (see the
[CanvasContext reference](../api/python/canvas-context.md) for full
details):

| Method | Draws |
| --- | --- |
| `fill_rect(x, y, width, height, color)` | A filled rectangle, node-local coordinates |
| `fill_circle(cx, cy, radius, color)` | A filled circle |
| `stroke_path(points, color, width)` | A stroked bezier path |
| `set_hit_test_circle(cx, cy, radius)` | Overrides the default rectangular hit-test with a circle |
| `set_hit_test_path(points, tolerance)` | Overrides the hit-test with "within `tolerance` px of this curve" |

`stroke_path`/`set_hit_test_path` share one path-segment vocabulary —
each inner list is one segment:

- `[x, y]` — the first entry must be this (a plain start point);
  subsequent 2-number entries are straight lines
- `[cx, cy, x, y]` — a quadratic curve segment
- `[c1x, c1y, c2x, c2y, x, y]` — a cubic curve segment

By default, a canvas hits-tests against its full rectangular bounds —
`set_hit_test_circle`/`set_hit_test_path` narrow that to a more precise
shape, useful for things like a specific plotted data point on a chart.

## Virtualized lists

```python
def materialize(index):
    return (0xEE, 0xEE, 0xEE, 0xFF) if index % 2 == 0 else (0xFF, 0xFF, 0xFF, 0xFF)

my_list = window.add_virtual_list(item_count=10_000, materialize=materialize, item_extent=32.0)
window.set_virtual_list_window(my_list, start=0, end=20)
```

`add_virtual_list` creates `item_count` logical rows without building
`item_count` real nodes — only the visible window is ever materialized.
Give exactly one of:

- `item_extent` — every row has this fixed height, or
- `size_hint` — a `Callable[[int], float]` returning one row's height,
  called **eagerly, once per item, up front** (not lazily) to build a
  cumulative-offset table before `add_virtual_list` returns; this is a
  real `O(item_count)` cost at creation time the fixed-extent path
  doesn't pay — see `examples/variable_height_list.py`.

`materialize` is a `Callable[[int], (r, g, b, a)]`, stored but not
called yet. `set_virtual_list_window(list, start, end)` is the real
materialization entry point: it calls `materialize(index)` for every new
index in `start..end` and removes whatever was previously materialized
outside that range — call it whenever the visible scroll range changes
(e.g. from a real `scroll` handler). A materializer that raises
propagates as a real exception; indices already materialized earlier in
the same call are not rolled back.

Real mouse-wheel scrolling over a virtual list (or any of its children)
already bubbles up through the engine's own dispatch — `window.scroll(node, delta_y)`
exercises the same path headlessly.
