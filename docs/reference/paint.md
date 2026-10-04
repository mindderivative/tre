# Paint, Paths, and Animation

The paint and animation building blocks: vector paths, the paint names every
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

`get("data")` reads the path back **normalized**, not as you wrote it: relative
commands become absolute, `H` and `V` become `L`, an arc `A` becomes cubic
curves `C`, and coordinates are comma-separated. `"m1 1 l2 2 h3 v4 z"` reads
back as `"M1,1 L3,3 L6,3 L6,7 Z"`. The shape is the same. In a test, compare a
read-back with a read-back (`path.get("data")` before and after), not with the
string you set.

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
| `fill` | `(r, g, b, a)` — a box's background, which also shows behind an image's transparent pixels and under a canvas, scroll view, virtual list, or terminal's content; a text's or a text input's glyph color; a path's fill |
| `stroke_color`, `stroke_width` | the border, drawn inside the box; it never changes layout. On a path they are the path's outline instead |
| `corner_radius` | one radius, or `(top_left, top_right, bottom_right, bottom_left)`: rounds the background and the border, and clips an image to the rounded box (a scroll view and a virtual list clip their content to it). A path has no box, so it ignores it |
| `opacity` | `0.0`–`1.0` — **group opacity**: the node and its whole subtree fade together, as one layer |
| `shadows` | a list of `(color, offset_x, offset_y, blur, spread)` — CSS `box-shadow`'s model, the first listed on top |
| `blur` | (0.5.4) a number ≥ 0: a Gaussian blur of the node and its whole subtree, as a standard deviation in pixels |
| `backdrop_blur` | (0.5.4) a number ≥ 0: frosted glass — what is behind the node, blurred, inside its box |
| `blend_mode` | (0.5.4) how the node and its subtree mix with what is behind: `"normal"` (the default) or a CSS `mix-blend-mode` |

Every kind paints these for its own box (0.5.1). Before, a text, text input,
image, canvas, scroll view, virtual list, or terminal accepted `stroke_color`,
`stroke_width`, or `corner_radius` (and the last four a `fill`) and drew nothing.
Only a box showed them all.

Colors are `(r, g, b, a)` tuples of ints, and a color's own alpha renders: a
fill of `(255, 0, 0, 128)` is half-transparent red. `get` returns exactly the
tuple that was set.

`corner_radius` reads back as a number while all four corners share one radius,
and as a 4-tuple once they're set (or animated) separately; setting a number
makes them uniform again. A shadow under a node with differing corner radii uses
their mean.

A design system's elevation levels are shadow lists — Material Design 3's
are a key and an ambient shadow per level:

```python
card.set(shadows=[((0, 0, 0, 77), 0, 1, 2, 0), ((0, 0, 0, 38), 0, 1, 3, 1)])
```

## Gradients

A box's `fill` can be a `tre.Gradient` (0.5.4) instead of a colour: a ramp
across the box, linear, radial or sweeping around a point. Only a box takes one
(`window.create("box")`, a window's root); on a text, a text input or a path,
`fill` is a colour.

```python
from tre import Gradient

card.set(fill=Gradient.linear([(0x67, 0x50, 0xA4, 0xFF), (0x21, 0x00, 0x5D, 0xFF)], angle=135))
glow.set(fill=Gradient.radial([(255, 255, 255, 200), (255, 255, 255, 0)], radius=0.8))
dial.set(fill=Gradient.sweep([red, yellow, green, red], start=0))
```

| Constructor | Runs |
| --- | --- |
| `Gradient.linear(stops, angle=180)` | along a line through the box's centre at `angle` degrees: 0 points up, 90 right, 180 down (the default). The line spans the box, so the first and last stops land on its corners |
| `Gradient.radial(stops, center=(0.5, 0.5), radius=1.0)` | outward from `center`, a fraction of the box; `radius` is a fraction of the half-diagonal, so `1.0` reaches the far corner of a centred gradient |
| `Gradient.sweep(stops, center=(0.5, 0.5), start=0)` | around `center`, clockwise from `start` degrees past up |

`stops` are colours, spaced evenly from 0 to 1, or `(offset, color)` pairs with
offsets from 0 to 1 that don't decrease. There must be at least two. A stop's
alpha is honoured, so a gradient can fade to transparent. A gradient follows its
box as layout resizes it, and follows its rounded corners.

A `Gradient` is immutable. `node.get("fill")` returns it while one is set, and
setting `fill` to a colour replaces it. Animating `fill` to a gradient of the
same kind and number of stops interpolates the colours, offsets, angle, centre
and radius; animating from a flat colour fades the gradient in from that colour.
Animating between unlike gradients, or from a gradient to a colour, raises
`ValueError`: set `fill` instead.

## Blur, frosted glass, and blend modes

Three effects (0.5.4), all acting on a node as a group, like `opacity`:

```python
card.set(blur=3)                        # the card and everything in it, soft
panel.set(fill=(255, 255, 255, 60), backdrop_blur=12, corner_radius=16)  # frosted glass
highlight.set(blend_mode="screen")      # lighten what is behind
```

- **`blur`** is the standard deviation of a Gaussian blur, in the node's own
  pixels, so it scales with the display. The blur spreads the node's pixels
  about three deviations past its box, and that margin is damaged and redrawn
  with it. It animates (`node.animate("blur", 0, 300)`).
- **`backdrop_blur`** blurs everything painted *behind* the node and shows it
  inside the node's box, clipped to its rounded corners. The node's own `fill`
  draws over it, so a translucent white `fill` makes frosted glass. Cost follows
  the box, not the window: the content behind is drawn again inside the box and
  the blur's reach, then blurred. It animates. A node behind another frosted
  node is drawn blurred in that one's backdrop, to a depth of two.
- **`blend_mode`** is one of `normal`, `multiply`, `screen`, `overlay`, `darken`,
  `lighten`, `color_dodge`, `color_burn`, `hard_light`, `soft_light`,
  `difference`, `exclusion`, `hue`, `saturation`, `color`, `luminosity`. The
  node and its subtree are drawn as one layer and mixed with what is behind it.

Not available: masks (the renderer does not support them yet; a rounded
`corner_radius` with `clip_children` covers most shapes) and colour filters such as
saturate or brightness (use a WGSL [effect shader](shader.md), which can do both).

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

Animatable: `fill`, `stroke_color`, `stroke_width`, `opacity`, `blur`, `backdrop_blur`, `corner_radius`
(a number or a 4-tuple), `shadows` (lists of different lengths fade the extra
shadows in or out), the transform parts `translate_x`, `translate_y`, `scale`,
and `rotation_deg`, a scroll view's `scroll_offset`, and a path's `data`,
`trim_start`, and `trim_end`. Colors
interpolate component-wise in sRGB.

**`get(name)`** returns the value on screen, mid-animation included;
**`get_target(name)`** returns where a running animation is heading, the same as
`get` when nothing animates it. **`stop_animation(name)`** stops where it is.

## Text inputs, scroll views, and terminals

Tesserae Engine paints no color you can't set:

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
new palette recolors what's already on screen. Tesserae Engine draws no focus ring on a
node you build: show focus yourself from the `focus` and `unfocus` events.
