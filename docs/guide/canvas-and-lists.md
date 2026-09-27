# Canvas & Virtualized Lists

## Custom drawing with a canvas

```python
def draw(painter):
    painter.fill_rect(0, 0, 100, 60, color=(0x67, 0x50, 0xA4, 0xFF))
    painter.fill_circle(50, 30, 20, color=(0xFF, 0xFF, 0xFF, 0xFF))
    painter.stroke_path([[0, 0], [50, 60], [100, 0]], color=(0x00, 0x00, 0x00, 0xFF), width=2.0)

canvas = window.create("canvas", width=100, height=60, draw=draw)
window.root.add_child(canvas)
```

A canvas calls `draw(painter)` when it's created, when its `draw` is set,
and on each `canvas.redraw()`, and replaces its drawn content with
whatever the painter collected. Nothing redraws it every frame — call
`redraw()` whenever the content needs to change.

[`Painter`](../api/python/painter.md) methods:

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

By default, a canvas hit-tests against its full rectangular bounds —
`set_hit_test_circle`/`set_hit_test_path` narrow that to a more precise
shape, useful for things like a specific plotted data point on a chart.

## Virtualized lists

```python
def row(index):
    shade = 0xEE if index % 2 == 0 else 0xFF
    return window.create("box", fill=(shade, shade, shade, 0xFF))

rows = window.create("virtual_list", item_count=10_000, item_extent=32.0,
                     materialize=row, width=320, height=400)
window.root.add_child(rows)
```

A virtual list has `item_count` logical rows but builds only the ones its
viewport shows: `materialize(index)` returns a node for a row as it comes
into view, and rows scrolled away are released. Give exactly one of:

- `item_extent` — every row has this fixed height, or
- `size_hint` — `size_hint(index)` returns one row's height, for rows of
  different heights; it's called once per row, the first time the list is
  laid out — see `examples/variable_height_list.py`.

A `materialize` or `size_hint` that raises is logged and non-fatal; its
row stays empty. Mouse-wheel scrolling over the list, or any of its rows,
scrolls it; `window.simulate("wheel", node=rows, delta_y=...)` drives the
same path headlessly. See
[Nodes and Properties](../api/python/properties.md#virtual-list) for every
property.

*0.3.5 removed `add_canvas`/`redraw_canvas` and
`add_virtual_list`/`set_virtual_list_window`.*
