# Animation

`node.animate(property, to, duration_ms, easing=None, on_complete=None)`
eases one property from wherever it is now to `to`. It returns at once; the
window advances every running animation each frame.

```python
card.animate("opacity", 0.0, 300)
card.animate("translate_y", -24, 300, easing=(0.3, 0.0, 0.8, 0.15),
             on_complete=lambda: card.remove())
```

## What animates

| Property | Value |
| --- | --- |
| `fill`, `stroke_color` | a color — interpolated per component in sRGB |
| `stroke_width`, `opacity` | a number |
| `corner_radius` | a number, or a 4-tuple of corners |
| `shadows` | a list of shadows; lists of different lengths fade the extra shadows in or out |
| `translate_x`, `translate_y`, `scale`, `rotation_deg` | a number — each part animates on its own |
| `scroll_offset` | a scroll view's offset, in pixels |
| `data`, `trim_start`, `trim_end` | a path's shape and stroke trim |

Layout properties don't animate: move and resize with the transform parts,
or set the layout and let it snap. Any other name raises `ValueError`.

## Easing

`easing` is `"linear"` (the default), a cubic bezier `(x1, y1, x2, y2)`,
exactly as CSS `cubic-bezier()` takes it, or a [spring](#springs). Design systems publish their
motion curves as bezier values, so they drop straight in:

```python
STANDARD = (0.2, 0.0, 0.0, 1.0)
EMPHASIZED_DECELERATE = (0.05, 0.7, 0.1, 1.0)
drawer.animate("translate_x", 0, 400, easing=EMPHASIZED_DECELERATE)
```

## Springs

`easing="spring"` or `easing=("spring", bounce)` (0.5.4) animates with a damped
spring instead of a fixed curve. `duration_ms` is the spring's period, about one
cycle of its main motion, and `bounce` is how far it overshoots, from `-1` to `1`
(exclusive): `0.0` is the quickest settle with no overshoot, towards `1.0` rings more
and for longer, and a negative value is slower and softer. The plain string is a
spring with `bounce=0.2`.

```python
card.animate("translate_y", -24, 350, easing=("spring", 0.35))   # lifts and settles
panel.animate("scale", 1.0, 300, easing="spring")
```

A spring lasts until it has settled (within a tenth of a percent of its target), so
it runs longer than `duration_ms`, and `on_complete` fires then. A `duration_ms` of
`0` still snaps. Springs work on every animatable property, colours included.

Springs also fix what a retargeted animation looked like. Animating a number again
while it is moving starts the new spring at the speed the old motion had, so a card
you drag away and let go, or a hover that reverses mid-way, carries on smoothly
instead of stalling and restarting. That carry applies to numbers (opacity, the
transform parts, a scroll offset, a stroke width and so on); a colour or a shadow
list springs without it.

## Retargeting and reading back

Calling `animate` again on a property that's still moving starts from where
it is now, so a hover that ends halfway through its fade reverses smoothly
rather than jumping. Three calls inspect a property in flight:

- `get(name)` — the value on screen right now.
- `get_target(name)` — where it's heading; the same as `get` when idle.
- `stop_animation(name)` — stop where it is.

`duration_ms=0` jumps to the target on the next frame.

## Completion

`on_complete` runs once, with no arguments, in the frame the animation
arrives. An animation replaced by another, or stopped, never calls it —
so a completion never fires for a value you've since changed your mind
about. Chain steps by starting the next from the last one's completion:

```python
def settle():
    chip.animate("scale", 1.0, 120)

chip.animate("scale", 1.2, 120, on_complete=settle)
```

## Morphing paths

Animating a path's `data` to another path morphs between them. Two closed
shapes, or two open lines, morph smoothly — both are resampled by length and
aligned at the best starting point. Paths with different numbers of subpaths,
or an open path against a closed one, switch halfway instead. The last frame
is exactly the target data, so a morphing play button ends as a crisp pause
icon. See `examples/path_morph.py`.

## Testing animations

Without a live window no frames run, so tests drive time themselves with
`window.advance(ms)`. It moves the window's clock forward by exactly `ms`,
then runs animations, their completions, and layout at the new time:

```python
window.advance(0)              # pin the clock at "now"
card.animate("opacity", 0.0, 200)
window.advance(100)
assert card.get("opacity") == 0.5
window.advance(100)
assert card.get("opacity") == 0.0
```

The first call pins the window's clock; after that only `advance` moves it,
until `App.run()` returns the window to real time.
