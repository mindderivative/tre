# `Node`

A handle to one node in a [`Window`](window.md)'s tree. Returned by
[`window.create`](window.md#nodes), `window.root`, and the tree methods
below; never constructed directly. Two handles compare equal, and hash
equal, when they name the same node, so nodes can key a dict.

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

**`get(name)`** reads any property back exactly as set — an animating one
at its current, mid-animation value — plus the read-only `kind`, `focused`,
`layer_placement`, and `layout_*` values; see
[Nodes and Properties](properties.md#read-only).

**`focus()`** moves keyboard focus to the node, firing `unfocus` and `focus`;
the scroll views around it scroll to show it (0.4.2).

**`scroll_into_view()`** (0.4.2) scrolls every scroll view around the node,
innermost first, by the least that shows it; a node longer than a view is
aligned to its start. An assistive technology's `scroll_into_view` request
does the same, after any `a11y_action` listener.

## `animate`

**`animate(property, to, duration_ms=0, easing=None, on_complete=None)`**

Eases one property from its current value to `to` and returns at once:

```python
node.animate("opacity", 0.0, 300, easing=(0.3, 0.0, 0.8, 0.15),
             on_complete=lambda: node.remove())
```

The animatable properties are `fill`, `stroke_color`, `stroke_width`,
`opacity`, `corner_radius`, `shadows`, `translate_x`, `translate_y`,
`scale`, `rotation_deg`, a scroll view's `scroll_offset`, and a path's
`data`, `trim_start`, and `trim_end`. Any other name raises `ValueError`;
a `to` of the wrong shape raises `ValueError`, or `TypeError` for a
non-number `to` on a numeric property. `easing` is `"linear"` or a cubic bezier
`(x1, y1, x2, y2)`. `on_complete` is called with no arguments exactly once,
in the frame the value arrives — or in `window.advance(ms)` in a test — and
never for an animation replaced or stopped first.

**`get_target(name)`** is where a running animation is heading — `get(name)`
when nothing animates it — and **`stop_animation(name)`** stops it where it
is. See [Paint, Paths, and Animation](paint.md#animating) and the
[Animation](../../guide/animation.md) guide.

## `redraw`

**`redraw()`** runs a canvas's `draw` callback now; see
[`Painter`](painter.md). Raises `ValueError` on any other kind.

## Events

**`on(event, handler)`** registers a listener — `click`, `pointer_down`,
`pointer_enter`, `change`, `focus`, and the rest — and **`off(event)`**
removes it; see [Events and Listeners](events.md) for every event, bubbling,
and the `Event` fields. A handler takes no arguments or one, the `Event`. An
exception raised inside a handler is caught, logged, and non-fatal.

**`capture_pointer()`** routes every later pointer event to this node until
the button is released or **`release_pointer()`** is called — for drags.

## Tree structure

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
