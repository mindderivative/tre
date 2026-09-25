# Legacy widget behavior

What the [legacy widget reference](legacy-widgets.md) can't show: how the
widgets 0.3.5 removes move, respond, and draw. The reference is each factory's
static tree; this page is everything dynamic, read from the source, with the
exact numbers and a way to rebuild each piece from the 0.3.4 building blocks.
`tests/test_legacy_behavior.py` runs this page's rebuild recipes against its
numbers.

One fact frames the rest: **the factories attach no listeners and start no
animations.** Every legacy `add_*` builds a static tree and returns it. What
moves is engine behavior — the state layer and ripple every node can get, the
Rust-painted kinds' own drawing and drag handling — plus whatever an app did
itself through `set_checked`, `animate`, and the `open_*`/`close_*` overlay
methods.

Times are milliseconds; "linear" and "standard" are the easings `"linear"` and
`(0.2, 0.0, 0.0, 1.0)`.

## State layer and ripple

Removed in 0.3.5 (D7). A node carries an interaction state once
`enable_interaction()` is called on it — tinted the theme's `on_surface` — or,
without that call, from the first time it's pressed, tinted black. **No factory
calls `enable_interaction()`**, so a legacy widget has no hover layer until the
app opts in or the widget is first pressed.

| Part | Behavior |
| --- | --- |
| Hover | The topmost hit node's layer animates to `0.08` over `100`, linear; back to `0` over `100` when the pointer leaves. Only the hit node — never its ancestors. |
| Ripple | **Every** press of any button, on whatever node it hits, spawns a ripple at the press point: radius `0 → 100` px and opacity `0.12 → 0`, both over `300`, linear, together — no separate press and release phases. Ripples overlap freely. A fixed `100` px, not sized to the node. |
| Painting | Hover and ripples paint in the tint, after the node's own fill and before its children, clipped to the node's rounded box (its single `corner_radius` — a per-corner radius or a shape morph is ignored), multiplied by the node's opacity. |
| Focus ring | Animates `0 → 1` over `100` on focus — and is **never painted**. There is no legacy focus indicator to reproduce. |
| Theme | `set_theme` re-tints every node that has a state, to `on_surface`. |

Rebuilt from building blocks — a layer child first, so it paints under the
content, and a circle that grows from the press point:

```python
def add_state_layer(window, node, tint):
    layer = window.create(
        "box", position="absolute", x=0, y=0, width="100%", height="100%",
        fill=tint, opacity=0.0, hit_testable=False,
        corner_radius=node.get("corner_radius"),
    )
    node.insert_child(0, layer)
    node.set(clip_children=True)
    node.on("pointer_enter", lambda e: layer.animate("opacity", 0.08, 100))
    node.on("pointer_leave", lambda e: layer.animate("opacity", 0.0, 100))

    def ripple(e):
        # A 200 px circle centered on the press, scaled up from nothing, so
        # its radius runs 0 -> 100 px while it fades.
        circle = window.create(
            "box", position="absolute", x=e.x - 100, y=e.y - 100,
            width=200, height=200, corner_radius=100, fill=tint,
            opacity=0.12, scale=0.0, hit_testable=False,
        )
        node.insert_child(1, circle)
        circle.animate("scale", 1.0, 300)
        circle.animate("opacity", 0.0, 300, on_complete=circle.destroy)

    node.on("pointer_down", ripple)
    return layer
```

`e.x`/`e.y` are local to the node whose listener runs, and `clip_children`
clips to the node's own rounded box, as the legacy layer was.

## Shapes that change on hover or press

Two factories morph corners; both animate over `100`, linear.

- **`add_split_button`** — while hovered, the leading button's two right corners
  and the trailing button's two left corners (the corners facing each other)
  tighten from the resting radius — the theme's `button` shape, else
  `height / 2` — to `8`, the theme's `split_button.tightened` shape. They relax
  when the pointer leaves. The outer corners never change.
- **`add_button_group`** — while a child is pressed, all four of its corners
  tighten to `8` (height ≤ 38), `12` (≤ 44), or `16`, the theme's
  `button_group.tightened` shape; and the row reflows, unanimated. The intent:
  the pressed child grows `12` px and its immediate neighbours shrink by `12`
  shared between them (`12` for one neighbour, `6` each for two, never below
  `0`), so the row keeps its width; children sit side by side, `8` px apart.
  **The legacy reflow is broken:** it reads each child's resting width from
  the same layout it writes, so every layout pass while a child is held
  applies it again — the pressed child keeps growing, its neighbours keep
  shrinking — and releasing never restores the resting widths. Rebuild the
  intent, not the bug.

Rebuilt: `animate("corner_radius", (tl, tr, br, bl), 100)` on
`pointer_enter`/`pointer_leave` (split button) or `pointer_down`/`pointer_up`
(group), and `set(width=...)` on the pressed child and its neighbours.

## Checkbox, radio button, switch

**The engine never toggles them.** A click delivers `click` and nothing else;
the app called `set_checked`/`set_selected` and then animated
`check_progress`/`select_progress`/`toggle_progress` itself, at whatever
duration it chose. The kinds contribute only their drawing, driven by that
progress `t` (`0` off, `1` on), in the node's `w × h` box:

| Kind | Drawing |
| --- | --- |
| Checkbox | A rounded box in `fill` (the `background` argument) with the node's `corner_radius`. At `t > 0`, a check stroked through `(0.2w, 0.55h) → (0.42w, 0.75h) → (0.8w, 0.25h)`, `max(min(w, h) × 0.12, 1)` wide, in the mark color at alpha `t` — white, or `on_surface` once a theme is set. |
| Radio button | A ring of width `s = max(min(w, h) × 0.1, 2)` and radius `min(w, h)/2 − s/2`, its color blended from `outline` to `primary` by `t`. At `t > 0`, a centered dot of radius `min(w, h)/2 × 0.5 × t`, `primary` at alpha `t`. |
| Switch | A track with radius `h/2`, blended from `surface_container_highest` to `primary` by `t`. While `t < 1`, an outline `max(h × 0.06, 1.5)` wide, inside the track, in `outline` at alpha `1 − t`. A handle of radius `h × (0.25 + 0.125t)` centered at `(h/2 + t(w − h), h/2)`, blended from `outline` to `on_primary`. |

A checkbox and a switch are a `box` and children; a radio's ring is a `box`
with a stroke and a `corner_radius` of half its size. A check mark is a `path`
whose `trim_end` runs `0 → 1` to draw it in.

## Slider

Drawn as a track `max(h × 0.15, 2)` tall across the full width, vertically
centered — `#79747A`, or `on_surface` once a theme is set — and a thumb, a
circle of radius `max(h × 0.4, 4)` at `x = value × w` in the node's `fill`.

- **Drag** — pressing starts a drag on the slider; the value follows the pointer
  exactly, `(x − node_x) / w` clamped to `0..1`, with no easing. `change`
  fires once, on release, with the value from before the drag.
- **Keys** — focused, Left and Right move the value `0.05`, firing `change`
  each time. The factory makes the slider focusable (role `slider`).

Rebuilt: a `box` track and a thumb `box`; `pointer_down` calls
`capture_pointer()` and `pointer_move` sets the value; `key_down` handles the
arrows.

## Progress and loading

| Factory | Behavior |
| --- | --- |
| `add_linear_progress` | Two square-cornered rectangles: the track across the full box in `surface_container_highest`, and the indicator from the left, `value × w` wide, in `primary`. No indeterminate mode. |
| `add_circular_progress` | An arc stroked `max(min(w, h) × 4/48, 1)` wide, of radius `min(w, h)/2 − width/2`, starting at 12 o'clock and sweeping clockwise `value × 360°`, in `primary`. No track ring. |
| `add_loading_indicator` | A filled shape in `primary` (or `foreground`) that morphs forever through four shapes — pentagon, pill, cookie, oval, then back — `650` per step, linear, from its first tick. The shapes fill the whole box and are built once, at construction size. Their outlines come with the engine-md3 handover. |

Rebuilt: `box`es for the linear bar; a `path` circle with `trim_end = value`,
its start rotated to 12 o'clock, for the arc; and a `path` whose `data`
animates from outline to outline for the loading indicator — a `path` morphs
between any two outlines.

## Time picker dial

Drawn in a square of side `d` with `r = d / 2`: a face circle in
`surface_container_highest`; twelve tick dots of radius `max(0.04r, 1)` on a
circle of radius `0.84r`, in `primary` at alpha `0.4`; an hour hand to `0.5r`
and a minute hand to `0.78r`, both `max(0.05r, 1.5)` wide, in `primary`; a
selector dot of radius `0.14r` on the tip of whichever hand is active; and a
hub of radius `0.03r`. Angles start at 12 o'clock and run clockwise. No digits.

Dragging anywhere in the box sets the active hand from the angle to the
pointer, with no easing: the hour snaps to twelve positions and keeps its
AM/PM half; the minute snaps to the nearest five. The app switches hands.

## Carousel

A clip, rounded to the node's own radius, holding absolutely placed items.
Items are laid out from `16` px in, `8` px apart, `16` px shorter than the
carousel (`8` above and below).

| Layout | Items |
| --- | --- |
| `uncontained` | Each keeps its own width (`200` if unset). Scrolls freely by pixel, clamped to the content. |
| `hero`, `multi_browse` | Slots `large, small` and `large, medium, small`. `small` is `56`, `medium` `112`, and `large` whatever the rest leaves (`width − 32 − fixed slots − gaps`, at least `56`). Slots past the pattern repeat its last; items before the current one are `small`. |

The snapping layouts track a fractional position: each item's width blends
between the slots on either side of it, and the strip shifts so the current
item starts at the left padding — widths and offsets change continuously as
the position moves.

- **Moving to an index** — the position animates to it over `300`, standard.
- **Wheel** — either axis; one index per notch (snapping), or half the wheel
  delta in pixels (uncontained).
- **Drag** — from anywhere in the carousel, even over an item: snapping layouts
  move one index per `60` px dragged, so a long drag steps several;
  uncontained scrolls with the pointer, 1:1.

Rebuilt: a `box` with `clip_children`, its items positioned absolutely from the
same formulas whenever the position changes, the position driven by the
framework's own animation (or a `scroll_view` for `uncontained`).

## Splitter

A plain rectangle that must sit between two siblings. Dragging it sets the
split to `(pointer − first sibling's start) / (both siblings' combined
extent)`, clamped to `0..1`, and gives the siblings those shares of their
combined width (in a horizontal parent) or height (vertical) — at once, no
easing.

Rebuilt: a `box` with `cursor="col_resize"` (or `"row_resize"`); `pointer_down`
captures the pointer and `pointer_move` sets both siblings' `width`/`height`.

## Icons, links, containers

- **Icon** — a filled path in a `960 × 960` view box offset to `(0, −960)`,
  scaled to the node's box, in its color, rotatable about its center. Rebuilt:
  a `path` with `view_box=(0, -960, 960, 960)`, `fill`, and `rotation_deg`.
- **Link** — drawn exactly as `text`, with the `link` role. No hover underline.
- **Container** — draws no fill or border of its own, but still draws its
  elevation shadow and state layer. A `box` with no `fill`.

## Elevation

Every legacy `elevation` level paints two shadows — an ambient one, then a key
one on top — in black at alpha `0.15` and `0.3`. As a `shadows` list (key
first, since the first listed paints on top), level by level:

| Level | `shadows` |
| --- | --- |
| 1 | `[((0, 0, 0, 77), 0, 1, 2, 0), ((0, 0, 0, 38), 0, 1, 3, 1)]` |
| 2 | `[((0, 0, 0, 77), 0, 1, 2, 0), ((0, 0, 0, 38), 0, 2, 6, 2)]` |
| 3 | `[((0, 0, 0, 77), 0, 1, 3, 0), ((0, 0, 0, 38), 0, 4, 8, 3)]` |
| 4 | `[((0, 0, 0, 77), 0, 2, 3, 0), ((0, 0, 0, 38), 0, 6, 10, 4)]` |
| 5 | `[((0, 0, 0, 77), 0, 4, 4, 0), ((0, 0, 0, 38), 0, 8, 12, 6)]` |

A fractional level blends linearly between these, and animating `shadows`
between two lists does the same. One small difference: the legacy ambient
shadow kept the node's corner radius as it spread, while a `shadows` entry
grows its radius by its spread, so its corners are slightly rounder.

## Overlays

The legacy `open_*`/`close_*` methods placed a prebuilt node over the content.
Unlike `show_layer`, a legacy overlay **closed itself** on the dismissals it
allowed, and never flipped or shifted to fit the window.

| Method | Placement | Outside press closes | Escape closes | Modal |
| --- | --- | --- | --- | --- |
| `open_menu(anchor, menu)`, also used for tooltips | Below the anchor, at its left edge | yes | yes | no |
| A context menu (right-click) | Below the pressed node, at its left edge | yes | yes | no |
| `open_dialog` | The window's top-left; the factory's scrim fills the window and centers the panel | no | yes | yes |
| `open_side_sheet`, `open_navigation_drawer` (modal builds only) | The window's top-left, inside the factory's scrim | no | yes | yes |
| `open_snackbar` | At `(24, window height − 72)` | no | no | no |

A snackbar has no timer; the app closed it. Rebuilt: `show_layer` with
`anchor` and `placement="below"`, `modal`, and `dismissible` to match, and a
`dismiss` listener that calls `hide_layer` wherever the legacy overlay closed
itself.

## Colors

Every color in the legacy reference is the no-theme baseline. With a theme set,
each factory resolved its colors from theme roles when built, and again on every
`set_theme`. The role names on this page are those roles; the baseline scheme
that maps them to the reference's values comes with the engine-md3 handover.
