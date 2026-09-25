# Paint, Paths, and Animation

*New in 0.3.4.* The paint and animation building blocks of the
[target API](../../design/target-api.md): vector paths, the paint names every
node shares, shadows, per-corner radii, group opacity, easing, and the colors
text inputs, scroll views, and terminals paint. Everything here goes through
[`Node.set`, `get`, and `animate`](node.md#set-get-and-focus).

## Creating nodes

```python
icon = window.create("path", data="M4,12 L10,18 L20,6", view_box=(0, 0, 24, 24),
                     width=24, height=24, stroke_color=(0x1C, 0x1B, 0x1F, 0xFF),
                     stroke_width=2)
panel = window.create("box", width=200, height="50%", fill=(0xFF, 0xFB, 0xFE, 0xFF),
                      corner_radius=12)
window.root.add_child(panel)
panel.add_child(icon)
```

`window.create(kind, **props)` makes a **detached** node — attach it with
`add_child` — and applies `props` atomically, as `set` does: a bad property
raises `ValueError` and nothing is created. Every kind, and every property, is
listed in [Nodes and Properties](properties.md).

`width` and `height` take a number of pixels, `"auto"`, or a percentage such as
`"50%"`, and read back as set.

## Paths

| Property | Value |
| --- | --- |
| `data` | SVG path data — the `d` attribute: `M`, `L`, `H`, `V`, `C`, `S`, `Q`, `T`, `A`, `Z`, absolute and relative |
| `view_box` | `(min_x, min_y, width, height)` — the region `data` is drawn in, fitted uniformly and centered into the node's box (SVG's default `xMidYMid meet`); `None` draws `data` in node-local pixels |
| `trim_start`, `trim_end` | `0.0`–`1.0` — the part of the stroke drawn, by length across every subpath in order |
| `fill` | the path's fill |
| `stroke_color`, `stroke_width` | the stroke, centered on the path with round caps and joins; its width stays in pixels however the view box scales |

A Material Symbols SVG drops straight in: pass its `d` and its `viewBox`.

**Trim** is how progress indicators draw: animate `trim_end` from `0.0` to
`1.0`, or move both ends together for an indeterminate spinner.

**Morphing:** animating `data` to another path morphs between them. Any two
closed paths or any two open paths morph — both are resampled by length, subpath
by subpath, and closed contours are aligned by the best starting point, so a
shape drawn from a different corner still morphs cleanly. Paths with different
numbers of subpaths, or an open path against a closed one, switch at the
halfway point instead. The last frame is exactly the target path.

## Paint on every node

| Property | Value |
| --- | --- |
| `fill` | `(r, g, b, a)` — a box's fill, a text's glyph color, an icon's tint |
| `stroke_color`, `stroke_width` | the border, drawn inside the box; it never changes layout |
| `corner_radius` | one radius, or `(top_left, top_right, bottom_right, bottom_left)` |
| `opacity` | `0.0`–`1.0` — **group opacity**: the node and its whole subtree fade together, as one layer |
| `shadows` | a list of `(color, offset_x, offset_y, blur, spread)` — CSS `box-shadow`'s model, the first listed on top |

Colors are `(r, g, b, a)` tuples of ints, and a color's own alpha renders: a
fill of `(255, 0, 0, 128)` is half-transparent red. `get` returns exactly the
tuple that was set.

`corner_radius` reads back as a number while all four corners share one radius,
and as a 4-tuple once they're set (or animated) separately; setting a number
makes them uniform again. A shadow under a node with differing corner radii uses
their mean.

MD3's elevation is two shadows — its key and ambient shadow per level — which
the framework supplies:

```python
card.set(shadows=[((0, 0, 0, 77), 0, 1, 2, 0), ((0, 0, 0, 38), 0, 1, 3, 1)])
```

## Animating

```python
button.animate("fill", (0x67, 0x50, 0xA4, 0xFF), 200, easing=(0.2, 0.0, 0.0, 1.0))
button.get("fill")         # the color on screen right now
button.get_target("fill")  # where it's heading
button.stop_animation("fill")
```

**`animate(name, to, duration_ms=0, easing=None, on_complete=None)`** eases a
property from its current value, so animating again mid-flight never jumps.
`easing` is `"linear"` (the default) or a cubic bezier `(x1, y1, x2, y2)` exactly
as CSS `cubic-bezier()` takes it — MD3's named curves are bezier values, e.g.
emphasized decelerate is `(0.05, 0.7, 0.1, 1.0)`. `on_complete` runs once when
the value arrives; an animation replaced by another, or stopped, never calls it.

Animatable: `fill`, `stroke_color`, `stroke_width`, `opacity`, `corner_radius`
(a number or a 4-tuple), `shadows` (lists of different lengths fade the extra
shadows in or out), and a path's `data`, `trim_start`, and `trim_end`. Colors
interpolate component-wise in sRGB.

**`get(name)`** returns the value on screen, mid-animation included;
**`get_target(name)`** returns where a running animation is heading, the same as
`get` when nothing animates it. **`stop_animation(name)`** stops where it is.

## Text inputs, scroll views, and terminals

`tre` paints no color you can't set:

| Kind | Property | Value |
| --- | --- | --- |
| text input | `placeholder` | the hint shown while the text is empty |
| text input | `placeholder_fill` | its color; `None` is the text color at 60% |
| text input | `caret_color` | `None` is the text color |
| text input | `selection_fill` | `None` is the text color at 30% |
| text input | `obscured` | a password field: every character shows as a bullet, and its text never leaves through copy or cut |
| scroll view | `scrollbar_fill` | the thumb's color; `None` is the default |
| scroll view | `scrollbar_width` | the thumb's thickness, painted and grabbed |
| terminal | `palette` | a dict with any of `ansi` (16 colors), `foreground`, `background`, `cursor`, `selection` — keys you leave out keep their colors |

A terminal resolves each cell's color against its palette when it paints, so a
new palette recolors what's already on screen. `tre` draws no focus ring on a
node you build: show focus yourself from the `focus` and `unfocus` events.
