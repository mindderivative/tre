# Painting

`tre` paints what you set and nothing else: no default borders, no focus
rings, no theme. Every node shares one set of paint properties; paths,
images, and canvases add their own. Everything here can be set with
`create` or `set`, and most of it [animates](animation.md).

## Color

A color is an `(r, g, b, a)` tuple of 0–255 ints, straight (not
premultiplied) alpha: `(255, 0, 0, 128)` is half-transparent red. Colors
default to transparent, except a text's and a text input's `fill`, which
default to opaque black. `get` returns exactly the tuple you set.

## Boxes: fill, stroke, corners

```python
card = window.create("box", width=240, height=120,
                     fill=(0xFF, 0xFF, 0xFF, 0xFF),
                     stroke_color=(0xCA, 0xC4, 0xD0, 0xFF), stroke_width=1,
                     corner_radius=12)
```

- `fill` — the box's color. On a text it's the glyph color, and on a path
  the shape's fill.
- `stroke_color`, `stroke_width` — a border drawn inside the box, so it never
  changes layout.
- `corner_radius` — one radius, or `(top_left, top_right, bottom_right,
  bottom_left)`:

```python
sheet.set(corner_radius=(28, 28, 0, 0))  # rounded top, square bottom
```

## Shadows

`shadows` is a list of `(color, offset_x, offset_y, blur, spread)` —
CSS `box-shadow`'s model, first listed on top. Two soft shadows give a
raised card:

```python
card.set(shadows=[((0, 0, 0, 77), 0, 1, 2, 0), ((0, 0, 0, 38), 0, 2, 6, 2)])
```

With no blur and a positive spread, a shadow is a ring outside the box —
a focus indicator that never shifts layout:

```python
button.set(shadows=[((0x67, 0x50, 0xA4, 0xFF), 0, 0, 0, 3)])
```

## Opacity and transforms

`opacity` fades a node **and its whole subtree together**, as one layer, so
overlapping children don't show through each other. `translate_x`,
`translate_y`, `scale` (about the center), and `rotation_deg` (clockwise
about the center) move what's painted without touching layout — the pointer
still finds the node where it's drawn. Each animates on its own.

## Paths

A `"path"` node draws SVG path data — the `d` attribute. With a `view_box`,
the data is fitted into the node's box the way an SVG `viewBox` is, so an
icon from any icon set drops straight in:

```python
check = window.create("path", data="M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4z",
                      view_box=(0, 0, 24, 24), width=24, height=24,
                      fill=(0x1D, 0x1B, 0x20, 0xFF))
```

A path has a `fill`, a `stroke_color` and `stroke_width` (in pixels however
the view box scales), and `trim_start`/`trim_end`, which draw only part of
the stroke. Animating `trim_end` from 0 to 1 draws a line on; moving both
ends together makes a spinner:

```python
ring = window.create("path", data="M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20",
                     view_box=(0, 0, 24, 24), width=48, height=48,
                     stroke_color=(0x67, 0x50, 0xA4, 0xFF), stroke_width=4,
                     trim_end=0.0)
ring.animate("trim_end", 1.0, 1000)
```

Animating `data` morphs one shape into another — see
[Animation](animation.md#morphing-paths).

## Images

An `"image"` node shows straight-alpha RGBA8 pixels you've already decoded.
`tre` never reads image files, so any decoder works:

```python
from PIL import Image

img = Image.open("photo.jpg").convert("RGBA")
photo = window.create("image", rgba=img.tobytes(), pixel_width=img.width,
                      pixel_height=img.height, width=200, height=150, fit="cover")
```

`fit` is `"fill"` (stretch), `"contain"` (letterbox), or `"cover"` (crop).
Video is the same node with each frame set as it arrives:
`photo.set(rgba=frame, pixel_width=w, pixel_height=h)`.

## Canvas

A `"canvas"` node draws with code. Its `draw` callback gets a
[`Painter`](../api/python/painter.md) and runs when the canvas is created,
when `draw` is set, and whenever you call `canvas.redraw()` — never on its
own, so redraw when your data changes:

```python
def draw(painter):
    painter.fill_rect(0, 0, 200, 100, (0xF7, 0xF2, 0xFA, 0xFF))
    for i, value in enumerate(samples):
        painter.fill_circle(10 + i * 20, 100 - value, 4, (0x67, 0x50, 0xA4, 0xFF))
    painter.stroke_path([[10, 90], [100, 20, 190, 90]], (0x1D, 0x1B, 0x20, 0xFF), 2.0)

chart = window.create("canvas", width=200, height=100, draw=draw)
```

`stroke_path` takes a start point, then lines (`[x, y]`), quadratic curves
(`[cx, cy, x, y]`), and cubic curves (`[c1x, c1y, c2x, c2y, x, y]`). A canvas
is hit as its whole box unless its painter narrows that with
`set_hit_test_circle` or `set_hit_test_path` — so a click on a chart's point
can land on that point alone.

## What `tre` leaves to you

The colors `tre` draws for you all have properties: a text input's
`placeholder_fill`, `caret_color`, and `selection_fill`; a scroll view's
`scrollbar_fill` and `scrollbar_width`; a terminal's `palette`. Hover and
press feedback, focus rings, ripples, and disabled styling are yours to draw
from [events](events-and-input.md) — [Building a Widget](building-a-widget.md)
shows how.
