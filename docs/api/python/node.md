# `Node`

A handle to one node in a [`Window`](window.md)'s tree. Returned by every `add_*` method; never constructed directly.

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
| `"elevation"` | `float` | every node |
| `"background"` | `(r, g, b, a)` int tuple | every node with a fill — not `Text` |
| `"foreground"` | `(r, g, b, a)` int tuple | `Text` — the glyph color |
| `"border_color"` | `(r, g, b, a)` int tuple | every node |
| `"border_width"` | `float` | every node |
| `"transform"` | `(translate_x, translate_y, scale)` float tuple | every node |
| `"shape"` | `list[(x, y)]` float tuples | every node — morphs to the closed polygon these vertices describe |

Raises `ValueError` for an unknown property name, or `TypeError` if `to`
doesn't match the property's expected shape. `on_complete`, when given,
is called with no arguments exactly once, the real frame the animation
finishes — drained by `App.run()`'s per-frame loop, or by
`window.advance(ms)` in a test.

## `get`

**`get(property) -> float`**

Reads a numeric property's current (possibly still-animating) value.
Supports `"opacity"`, `"corner_radius"`, `"elevation"`, and
`"border_width"`. Raises `ValueError`
for an unknown/inapplicable property. Colors (`"background"`,
`"foreground"`, `"border_color"`) aren't readable this way — they aren't
a single `float`.

## `set_layout`

**`set_layout(width=None, height=None, padding=None, padding_top=None, padding_right=None, padding_bottom=None, padding_left=None, margin=None, margin_top=None, margin_right=None, margin_bottom=None, margin_left=None, gap=None, flex_grow=None, flex_shrink=None, flex_basis=None, align_items=None, justify_content=None, flex_direction=None)`**

General live layout mutation, the imperative counterpart to editing a
declarative widget's `style:` block. Only the fields actually passed
are changed — every omitted field keeps its current value. Applies
immediately, not eased — layout fields aren't animatable the way
paint properties are.

```python
node.set_layout(width=200, flex_direction="vertical", gap=8)
```

`align_items`/`justify_content`/`flex_direction` take the same string
vocabulary as `set`'s layout properties (see
[Nodes and Properties](properties.md)) — an unrecognized value raises
`ValueError` naming the ones it does accept. `padding`/`margin` set all four sides at once;
the `_top`/`_right`/`_bottom`/`_left` variants override just one side
on top of that, applied in the order given.

## Events

### `set_on_click`

**`set_on_click(callback)`**

Registers `callback` (called with no arguments) for a real click — a
primary-button release over the node, or Enter/Space while it's
keyboard-focused. Also makes the node Tab-reachable, adding a `Click`
accessibility action if it doesn't already have one.

### `set_on_hover_enter` / `set_on_hover_exit`

**`set_on_hover_enter(callback)`** / **`set_on_hover_exit(callback)`**

Fire when the node becomes/stops being the hovered node — independent of
whether `enable_interaction()` was ever called.

### `set_on_change`

**`set_on_change(callback)`**

Fires on a real `Change` — a `TextField`'s text changing, by typing or
by `set_text`.

### `set_on_focus_enter` / `set_on_focus_exit`

**`set_on_focus_enter(callback)`** / **`set_on_focus_exit(callback)`**

Fire when the node becomes/stops being the keyboard-focused node.

### The `Event` payload

A `callback` may take zero arguments (as above) or exactly one — a real
`Event` object, detected once at registration time by inspecting the
callback's own arity:

| Field | Type | Set for |
| --- | --- | --- |
| `kind` | `str` | always — `"click"`, `"hover_enter"`, `"hover_exit"`, `"change"`, `"focus_enter"`, `"focus_exit"` |
| `node` | `Node` | always — the live node this event fired on |
| `source` | `int` | always — a stable, opaque id for that same node |
| `position` | `(float, float)` or `None` | a pointer-driven `click`/`hover_*` |
| `button` | `str` or `None` | a `click` — `"primary"`/`"secondary"`/`"middle"` |
| `old_value`, `new_value` | varies or `None` | a `change` — type matches the changed property (`bool` for `checked`, `str` for `text`) |

```python
def on_any_click(event):
    print(f"{event.kind} on {event.node} at {event.position}")

node.set_on_click(on_any_click)
```

An exception raised inside any handler is caught, logged, and non-fatal.

## Interaction visuals

### `enable_interaction`

**`enable_interaction()`**

Opts this node into the default ripple/hover state-layer animation, in
black. Independent of whether the node has any event handlers — a purely-
hoverable, non-clickable node is a supported case.

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
- Nodes made by the older `add_*` methods start attached, and are never
  freed this way unless you `remove()` them.

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

## TextField-specific

### `set_text`

**`set_text(content)`**

Works on both `TextField` and plain `Text` labels. On a `TextField`:
resets the cursor to the new content's end and fires `Change`. On a
plain `Text` label: just replaces the content (no cursor, no `Change`
— a label isn't interactive). Raises `ValueError` for any other node
kind.

### `get_text`

**`get_text() -> str`**

Reads the current content of a `TextField`, a plain `Text` label, or a
`Terminal` (its visible cell grid, one line per row).
Raises `ValueError` for any other node kind.

## Focus

### `is_focused`

**`is_focused() -> bool`**

Whether this node currently has keyboard focus. Works for any node kind.

## Code editor-specific

### `set_syntax_spans`

**`set_syntax_spans(spans)`**

`spans` is a list of `(start, end, (r, g, b, a))` tuples, each a byte
range into `get_text()`'s own content and the color to paint it.
Replaces the whole list on every call — the app re-tokenizes and calls
this again on every real edit; `tre` never interprets or validates the
ranges itself. `CodeEditor`-only (raises `ValueError` otherwise).

### `set_folded_ranges`

**`set_folded_ranges(ranges)`**

`ranges` is a list of `(start, end)` byte-offset tuples, each collapsed
to one visible "⋯" marker line. Replaces the whole list on every call.
Cursor movement (`Home`/`End`/arrow keys) is fold-aware — a move that
would land inside a folded range snaps forward past its marker instead.
`CodeEditor`-only.

## Terminal-specific

### `set_terminal_selection`

**`set_terminal_selection(start_row, start_col, end_row, end_col)`**

Sets a cell-range selection directly, without a mouse drag — a linear,
reading-order selection like every terminal emulator's. Equal start and
end mean no selection. Read it back with
[`Window.copy_terminal_selection`](window.md#copy_terminal_selection).
`Terminal` only (raises `ValueError` otherwise).

## Image-specific

### `push_frame`

**`push_frame(rgba, width, height)`**

Replaces an `Image` node's pixels — `rgba` a flat `bytes`/`bytearray`
of straight-alpha RGBA8, exactly `width * height * 4` bytes (a clear
`ValueError` otherwise). Works on any `Image` node: one from
`add_image`, `add_image_from_bytes`, or `add_video`, or a declarative
`kind: Image` (a blank one built with no `src:` is the usual target).
Call it once for a static image or once per frame for video; the app
owns decoding and pacing — `tre` bundles no video decoder. Raises
`ValueError` on a non-`Image` node.

## Clipping

### `set_clip_children`

**`set_clip_children(clip)`**

When `True`, children are visually clipped to this node's own bounds
instead of painting past them. Works on any node kind — most useful on
a plain `Container` used purely as a clipping mask.
