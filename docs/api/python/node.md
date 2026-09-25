# `Node`

A handle to one node in a [`Window`](window.md)'s or [`View`](view.md)'s
tree. Returned by every `add_*` method; never constructed directly.

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
| `"background"` | `(r, g, b, a)` int tuple | every node |
| `"transform"` | `(translate_x, translate_y, scale)` float tuple | every node |
| `"shape"` | `list[(x, y)]` float tuples | every node — morphs to the closed polygon these vertices describe |
| `"check_progress"` | `float` (0.0–1.0) | `Checkbox` only |
| `"thumb_position"` | `float` (0.0–1.0) | `Slider` only |

Raises `ValueError` for an unknown property name, or `TypeError` if `to`
doesn't match the property's expected shape. `on_complete`, when given,
is called with no arguments exactly once, the real frame the animation
finishes — drained by `App.run()`'s per-frame loop, so it never fires for
a node created via `View` (no render loop to drain it through).

## `get`

**`get(property) -> float`**

Reads a numeric property's current (possibly still-animating) value.
Supports `"opacity"`, `"corner_radius"`, `"elevation"`,
`"check_progress"` (`Checkbox` only), `"thumb_position"` (`Slider`
only). Raises `ValueError` for an unknown/inapplicable property.
`"background"` isn't readable this way (it isn't a single `float`).

## `set_layout`

**`set_layout(width=None, height=None, padding=None, padding_top=None, padding_right=None, padding_bottom=None, padding_left=None, margin=None, margin_top=None, margin_right=None, margin_bottom=None, margin_left=None, gap=None, flex_grow=None, flex_shrink=None, flex_basis=None, align_items=None, justify_content=None, flex_direction=None)`**

General live layout mutation, the imperative counterpart to editing a
declarative widget's `style:` block. Only the fields actually passed
are changed — every omitted field keeps its current value. Applies
immediately, not eased — layout fields aren't animatable the way
paint properties are.

```python
node.set_layout(width=200, flex_direction="Vertical", gap=8)
```

`align_items`/`justify_content`/`flex_direction` take the same string
vocabulary as their declarative `style:` equivalents (see
[Declarative Views → The view schema](../../guide/declarative-views.md#the-view-schema)
for the full list) — an unrecognized value raises `ValueError` naming
the ones it does accept. `padding`/`margin` set all four sides at once;
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

Fires on a real `Change` — a `Slider` drag ending, or
`set_checked`/`set_text` being called.

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

Opts this node into the default MD3 ripple/hover state-layer animation.
Independent of whether the node has any event handlers — a purely-
hoverable, non-clickable node is a supported case. If the window's
`set_theme` was already called, immediately applies the current theme's
on-surface tint.

## Tree structure

### `add_child`

**`add_child(child)`**

Attaches `child` under this node. Raises `ValueError` if that would
create a cycle (`child` is an ancestor of this node), or if `child`
belongs to a different `Window`'s tree.

### `remove`

**`remove()`**

Recursively removes this node and its whole subtree, unlinking it from
its parent.

### `set_context_menu`

**`set_context_menu(content)`**

Registers `content` as this node's right-click context menu, opened via
`Window.right_click`/a real right-click. `content` must belong to the
same `Window`; it's detached from its current parent first if needed.

## Checkbox-specific

### `set_checked`

**`set_checked(checked)`**

Plain, non-animated write to a `Checkbox`'s `checked` state. Fires
`Change`. Raises `ValueError` if this node isn't a `Checkbox`.

### `get_checked`

**`get_checked() -> bool`**

Reads a `Checkbox`'s current `checked` value.

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

Reads the current content of a `TextField`, a plain `Text` label, a
`Link`, or a `Terminal` (its visible cell grid, one line per row).
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

## RadioButton & Switch-specific

| Method | Notes |
| --- | --- |
| `set_selected(selected)` / `get_selected() -> bool` | `RadioButton` only |
| `set_on(on)` / `get_on() -> bool` | `Switch` only |

Each raises `ValueError` on any other kind. (`Checkbox` uses
[`set_checked`/`get_checked`](#checkbox-specific).)

## Carousel-specific

| Method | Notes |
| --- | --- |
| `set_carousel_index(index)` | Moves to `index` (clamped to the child count) with an eased snap; a no-op if already there |
| `get_carousel_index() -> int` | The item the carousel is settling on — its destination, not necessarily where it's drawn mid-snap |
| `get_carousel_position() -> float` | The currently-animating strip position: an integer at rest, fractional while a snap is travelling |
| `set_carousel_scroll(value)` / `get_carousel_scroll() -> float` | The free pixel scroll offset, clamped to `[0, max_scroll]` — meaningful for `layout="uncontained"` |

`Carousel` only; each raises `ValueError` on any other kind.

## TimePickerDial-specific

| Method | Notes |
| --- | --- |
| `set_time_picker_dial_time(hour, minute)` | Moves both hands instantly; `hour` clamps to `0`–`23`, `minute` to `0`–`59` |
| `get_time_picker_dial_time() -> (hour, minute)` | |
| `set_time_picker_dial_mode(mode)` / `get_time_picker_dial_mode() -> str` | `"hour"` or `"minute"` — which hand a drag on the dial moves next. The dial has no built-in toggle; wire a button or tab to this |

`TimePickerDial` only; each raises `ValueError` on any other kind.

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
