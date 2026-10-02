# `Painter`

Passed to a canvas node's `draw` callback, which runs when the canvas is
created, when its `draw` is set, and on each `canvas.redraw()`. Everything
the callback draws replaces what the canvas showed. See
[Painting → Canvas](../guide/painting.md#canvas). Never constructed
directly.

## `fill_rect`

**`fill_rect(x, y, width, height, color)`**

Queues a filled rectangle in node-local coordinates, which start at the
canvas's padding: `(0, 0)` is the corner of its content box (0.5.1; with no
padding, the node's own corner). `color` is an `(r, g, b, a)` byte tuple.

## `fill_circle`

**`fill_circle(cx, cy, radius, color)`**

Queues a filled circle, centered at `(cx, cy)`.

## `stroke_path`

**`stroke_path(points, color, width)`**

Queues a stroked bezier path. Raises `ValueError` if fewer than 2 points
are given, if the first isn't a plain 2-number pair, or if any point has
an unsupported number count.

`points` is a list of segments, each a list of numbers:

| Segment length | Meaning |
| --- | --- |
| `[x, y]` | The first entry must be this (a plain start point); later 2-number entries are straight lines |
| `[cx, cy, x, y]` | A quadratic curve to `(x, y)` |
| `[c1x, c1y, c2x, c2y, x, y]` | A cubic curve to `(x, y)` |

## `set_hit_test_circle`

**`set_hit_test_circle(cx, cy, radius)`**

Overrides this canvas node's default rectangular hit-test with a circle —
a point inside the node's bounding box but outside this circle counts as
a miss.

## `set_hit_test_path`

**`set_hit_test_path(points, tolerance)`**

Overrides the hit-test with "within `tolerance` pixels of this curve,"
built from `points` (same segment vocabulary as `stroke_path`, same
validation). Hit-tests against the true curve geometry, not a straight-
line approximation.
