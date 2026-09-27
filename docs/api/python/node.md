# `Node`

A handle to one node in a [`Window`](window.md)'s tree. Returned by [`window.create`](window.md#nodes); never constructed directly.

*New in 0.3.4:* `on`/`off` listeners, `capture_pointer`/`release_pointer`,
and handle equality — see [Events and Listeners](events.md) — plus `set`,
the wider `get`, and `focus`, below, and the paint, path, and animation
properties, `get_target`, `stop_animation`, and `animate`'s `easing` — see
[Paint, Paths, and Animation](paint.md).

## `set`, `get`, and `focus`

**`set(**props)`** sets any number of properties at once, atomically: every
value is checked first, and a bad name or value raises `ValueError` without
changing anything. Optional properties take `None` to clear them.

```python
thumb.set(role="slider", label="Volume", value=0.4, value_min=0.0,
          value_max=1.0, value_step=0.1, focusable=True, cursor="pointer")
```

| Property | Value |
| --- | --- |
| `role` | `"button"`, `"checkbox"`, `"radio"`, `"switch"`, `"slider"`, `"progressbar"`, `"link"`, `"textbox"`, `"tab"`, `"tablist"`, `"tabpanel"`, `"menu"`, `"menuitem"`, `"dialog"`, `"alert"`, `"list"`, `"listitem"`, `"tree"`, `"treeitem"`, `"heading"`, `"img"`, `"group"`, `"none"` |
| `label` | `str` or `None` — the name assistive technology reads |
| `value` | `str`, number, or `None` |
| `value_min`, `value_max`, `value_step` | number or `None` |
| `checked`, `selected`, `expanded` | `bool` or `None` |
| `disabled` | `bool` |
| `level` | positive `int` or `None` — a heading's level |
| `live` | `"off"`, `"polite"`, `"assertive"`, or `None` — how changes are announced |
| `a11y_hidden` | `bool` — hidden from assistive technology |
| `focusable` | `bool` — focusable by Tab, click, and `focus()`; a click on a descendant focuses the nearest focusable ancestor |
| `tab_index` | `int` — positive values come first in Tab order, ascending; then `0` in tree order; negative leaves the node out of Tab order but focusable by click and `focus()` |
| `cursor` | the pointer shape over the node, inherited by descendants: `"default"`, `"pointer"`, `"text"`, `"grab"`, `"grabbing"`, `"move"`, `"not_allowed"`, `"wait"`, `"progress"`, `"crosshair"`, `"help"`, `"col_resize"`, `"row_resize"`, `"ew_resize"`, `"ns_resize"`, `"nesw_resize"`, `"nwse_resize"`, `"copy"`, `"cell"`, `"context_menu"`, `"zoom_in"`, `"zoom_out"`, `"all_scroll"`, or `None` |
| `hit_testable` | `bool` — whether the node can be the target of pointer events |

The actions assistive technology is offered follow from role and state:
focusable nodes offer focus, button-like roles offer activation, a slider
or a value range offers increment/decrement/set-value, and `expanded` offers
expand/collapse. Requests arrive as the [`a11y_action`](events.md) event.

**`get(name)`** reads any of those back exactly as set, `focused`, or an
animatable number's current value (below). On a built-in slider or progress
indicator, `value` stays that widget's numeric value.

**`focus()`** moves keyboard focus to the node, firing `unfocus` and `focus`.

## `animate`

**`animate(property, to, duration_ms=0, on_complete=None)`**

Starts (or retargets) an animation on one property. Returns immediately —
never blocks waiting for the animation to finish.

```python
node.animate("opacity", 0.0, duration_ms=300, on_complete=lambda: print("faded"))
```

| `property` | `to` type | Applies to |
| --- | --- | --- |
| `"opacity"` | `float` (0.0–1.0) | every node |
| `"corner_radius"` | `float` | every node |
| `"background"` | `(r, g, b, a)` int tuple | every node with a fill — not `Text` |
| `"foreground"` | `(r, g, b, a)` int tuple | `Text` — the glyph color |
| `"border_color"` | `(r, g, b, a)` int tuple | every node |
| `"border_width"` | `float` | every node |
| `"transform"` | `(translate_x, translate_y, scale)` float tuple | every node |

*0.3.5 removed* `elevation` (use `shadows`) and `shape`, the MD3 shape
library (use a `path` node's `data`) — see
[Paint, Paths, and Animation](paint.md).

Raises `ValueError` for an unknown property name, or `TypeError` if `to`
doesn't match the property's expected shape. `on_complete`, when given,
is called with no arguments exactly once, the real frame the animation
finishes — drained by `App.run()`'s per-frame loop, or by
`window.advance(ms)` in a test.

## Events

**`on(event, handler)`** registers a listener — `click`, `pointer_down`,
`pointer_enter`, `change`, `focus`, and the rest — and **`off(event)`**
removes it; see [Events and Listeners](events.md) for every event, bubbling,
and the `Event` fields. A handler takes no arguments or one, the `Event`. An
exception raised inside a handler is caught, logged, and non-fatal.

*0.3.5 removed the `set_on_*` methods:* `set_on_click` is `on("click")`,
`set_on_hover_enter`/`set_on_hover_exit` are `on("pointer_enter")`/
`on("pointer_leave")`, `set_on_change` is `on("change")`, and
`set_on_focus_enter`/`set_on_focus_exit` are `on("focus")`/`on("unfocus")`.
Unlike them, `click` bubbles to ancestors, and `on("click")` doesn't make a
node focusable — set `focusable=True` for that.

## Tree structure

*Changed in 0.3.4:* `remove()` detaches rather than freeing, and nodes
have a lifetime of their own — see [Lifetime](#lifetime).

| Method | Does |
| --- | --- |
| `add_child(child)` | Appends `child`, moving it if it's attached elsewhere |
| `insert_child(index, child)` | Attaches `child` so it ends up at `index` — afterwards `children()[index] == child` — moving it if it's attached anywhere. `index` counts the children once `child` has left its old place; past the end raises `IndexError` |
| `children()` | This node's children, in order |
| `parent()` | Its parent, or `None` for the root or a detached node |
| `remove()` | Detaches this node from its parent; it stays alive and can be attached again |
| `destroy()` | Frees this node and its whole subtree now, with their listeners |

Attaching raises `ValueError` if `child` is this node or one of its
ancestors, or belongs to a different `Window`. **Moving a node keeps
everything about it** — the same `Node`, its listeners, its focus, and any
running animation — which is what a keyed list reconciler needs:

```python
for index, key in enumerate(new_order):
    list_box.insert_child(index, rows[key])
```

### Lifetime

- A node **attached** to a window lives while it's attached.
- A node made by `window.create`, or detached by `remove()`, lives while
  any `Node` handle to it — or to anything in its subtree — exists. When
  the last one goes, the subtree is freed with its listeners, so a
  forgotten `destroy()` never leaks.
- `destroy()` frees now. Using a handle to a freed node raises
  `ValueError`.

Switching screens is `old.remove()` then `window.root.add_child(new)`:

- **Focus leaves with a removed or destroyed subtree.** When focus is inside
  it, the focused node gets `unfocus` first — bubbling through the tree as it
  was — and nothing is focused afterwards.
- **A detached subtree keeps everything else**: scroll offsets, a text
  input's text, caret, and selection, and running animations, which keep
  advancing on the window's clock. Attach it again and it's as you left it.

*0.3.5 removed the MD3 widget kinds* — checkbox, radio button, switch,
slider, the progress indicators, loading indicator, time picker dial,
carousel, splitter, link, and icon — with their `Node` methods
(`set_checked`/`get_checked`, `set_selected`/`get_selected`, the
carousel and time-picker-dial accessors) and animatable properties. A
framework builds them from boxes, text, and paths.

*0.3.5 also moved the per-kind methods onto `set` and `get`:* `set_layout(...)`
is `set(...)`; `set_text(t)` is `set(text=t)`, which fires no `change` (that's
for edits the user makes); `get_text()` is `get("text")` (on a terminal, its
visible grid, one line per row); `is_focused()` is `get("focused")`;
`set_syntax_spans`, `set_folded_ranges`, and `set_clip_children` are
`set(syntax_spans=...)`, `set(folded_ranges=...)`, `set(clip_children=...)`;
`set_terminal_selection(a, b, c, d)` is `set(selection=(a, b, c, d))`; and
`push_frame(rgba, w, h)` is `set(rgba=rgba, pixel_width=w, pixel_height=h)`.
Every property is on [Nodes and Properties](properties.md).
