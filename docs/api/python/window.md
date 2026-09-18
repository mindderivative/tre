# `Window`

Owns one node tree, its root, and its own size/title. Every `add_*`
method attaches a new [`Node`](node.md) as a direct child of this
window's implicit root (a flex row, 16px padding, 16px gaps).

## `Window`

**`Window(width=480, height=200, title="tre v2")`**

```python
window = Window(width=400, height=200, title="My App")
```

## Creating nodes

### `add_rect`

**`add_rect(background, width, height, x=None, y=None)`**

A plain colored rectangle. `background` is an `(r, g, b, a)` tuple of
ints 0–255.

### `add_checkbox`

**`add_checkbox(background, width, height, checked=False, x=None, y=None)`**

An MD3 checkbox. See [MD3 Components → Checkbox](../../guide/components.md#checkbox).

### `add_slider`

**`add_slider(background, width, height, value=0.0, x=None, y=None)`**

An MD3 slider with drag-to-set built in. `value` seeds `thumb_position`,
clamped to `0.0..=1.0`. See [MD3 Components → Slider](../../guide/components.md#slider).

### `add_text_field`

**`add_text_field(background, width, height, content="", font_family="Roboto", font_weight=400.0, font_size=16.0, x=None, y=None)`**

An MD3 text field with real keyboard editing. See
[MD3 Components → TextField](../../guide/components.md#textfield).

### `add_image`

**`add_image(path, width, height, fit="fill", x=None, y=None)`**

Loads and decodes a real image file (`png`/`jpeg`) and uploads it as a
GPU texture. `fit` is `"cover"`, `"contain"`, or `"fill"`. Raises
`OSError` if the file can't be read or decoded, `ValueError` for an
unknown `fit`. See [MD3 Components → Image](../../guide/components.md#image).

### `add_icon`

**`add_icon(name, color, size, x=None, y=None)`**

A curated Material Symbols vector icon (`home`, `search`, `menu`,
`close`, `check`, `arrow_back`, `add`, `settings`). Raises `ValueError`
for an unknown `name`. See [MD3 Components → Icon](../../guide/components.md#icon).

### `add_splitter`

**`add_splitter(background, width, height, initial_position=0.5)`**

A drag-resizable pane divider. See
[MD3 Components → Splitter](../../guide/components.md#splitter).

### `add_canvas`

**`add_canvas(width, height, draw, x=None, y=None)`**

A custom-drawn surface. `draw: Callable[[CanvasContext], None]` is
stored, not invoked yet — see [`redraw_canvas`](#redraw_canvas) and
[Canvas & Virtualized Lists](../../guide/canvas-and-lists.md).

### `add_virtual_list`

**`add_virtual_list(item_count, materialize, item_extent=None, size_hint=None, width=None, height=None)`**

A virtualized list of `item_count` logical rows. Give exactly one of
`item_extent`/`size_hint`. Raises `ValueError` if neither or both are
given. See [Canvas & Virtualized Lists](../../guide/canvas-and-lists.md#virtualized-lists).

## Layout composition

### `build_shell`

**`build_shell(menu_bar=None, toolbar=None, status_bar=None) -> Node`**

Builds an `AppShell`-style column container sized to the window's full
width/height, re-parenting the given chrome nodes into it, and returns
the empty `content` container (`flex_grow: 1.0`). See
[Docking & Shell Layout → App shell](../../guide/docking-and-shell.md#app-shell).

## Theming

### `set_theme`

**`set_theme(seed, dark=False)`**

Builds a full MD3 dynamic color scheme from a `(r, g, b, a)` seed color
and makes it active. See
[Theming & Accessibility → Dynamic color theming](../../guide/theming-and-accessibility.md#dynamic-color-theming).

## Synthetic input dispatch

These work without a live rendered window — each computes layout, then
dispatches at the target node's real, current center point.

| Method | Simulates |
| --- | --- |
| `click(node)` | A primary-button press + release |
| `hover(node)` | The pointer moving over `node` (fires `HoverEnter`/`HoverExit`) |
| `scroll(node, delta_y)` | A mouse wheel scroll (bubbles to the nearest `VirtualList` ancestor) |
| `right_click(node)` | A secondary-button press + release (opens a registered context menu, if any) |
| `press_key(key, shift=False)` | A keypress — see accepted keys below |
| `type_text(text)` | A produced text-input event (affects the currently focused `TextField` only) |
| `copy()` | Ctrl+C — returns the focused field's selected text, or `None` (hermetic, no real OS clipboard) |
| `cut()` | Ctrl+X — also edits the field and fires `Change` (hermetic) |
| `paste(text)` | Ctrl+V with explicit text — same mechanism as `type_text` (hermetic) |

Accepted `key` values for `press_key`: `"tab"`, `"enter"`, `"space"`,
`"escape"`, `"backspace"`, `"delete"`, `"left"`, `"right"`, `"home"`,
`"end"`. Anything else raises `ValueError`.

## Context menus

Registered on the [`Node`](node.md) itself via `set_context_menu` — see
`right_click` above for how a registered menu opens.

## Docking

| Method | Purpose |
| --- | --- |
| `add_dock_zone(side, container, size)` | Registers `container` as `side`'s dock zone |
| `dock_panel(side, panel)` | Attaches `panel` as `side`'s active tab |
| `set_active_tab(side, index)` | Switches which docked panel is active |
| `set_dock_handle(handle, panel)` | Makes `handle` the drag grip for `panel` |
| `set_drop_zone_highlight(content)` | Registers the drag-over highlight node |
| `start_panel_drag(handle) -> bool` | Starts a drag; returns whether it did |
| `drag_panel_over(x, y)` | Updates the highlight during a drag |
| `drop_panel_at(x, y)` | Ends the drag, reparenting into the enclosing zone |

`side` is one of `"left"`, `"right"`, `"top"`, `"bottom"`, `"center"`.
See [Docking & Shell Layout](../../guide/docking-and-shell.md#docking) for
a full walkthrough.

## Container transform

### `begin_container_transform`

**`begin_container_transform(trigger, destination, duration_ms=300, content_stagger_ms=90, on_complete=None)`**

### `end_container_transform`

**`end_container_transform(trigger)`**

See [Docking & Shell Layout → Container-transform navigation](../../guide/docking-and-shell.md#container-transform-navigation).

## Canvas

### `redraw_canvas`

**`redraw_canvas(canvas)`**

Calls `canvas`'s stored `draw(ctx)` callback exactly once and replaces
its drawn content. Raises `ValueError` if `canvas` wasn't created by this
window's `add_canvas`. See
[Canvas & Virtualized Lists](../../guide/canvas-and-lists.md#custom-drawing-with-canvas).

## Virtualized lists

### `set_virtual_list_window`

**`set_virtual_list_window(list, start, end)`**

Materializes rows `start..end`, removing whatever was materialized
outside that range. Raises `ValueError` if `list` wasn't created by this
window's `add_virtual_list`. See
[Canvas & Virtualized Lists](../../guide/canvas-and-lists.md#virtualized-lists).
