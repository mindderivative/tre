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

Plain write to a `TextField`'s content; resets the cursor to the new
content's end. Fires `Change`. Raises `ValueError` if this node isn't a
`TextField`.

### `get_text`

**`get_text() -> str`**

Reads a `TextField`'s current content.

## Focus

### `is_focused`

**`is_focused() -> bool`**

Whether this node currently has keyboard focus. Works for any node kind.
