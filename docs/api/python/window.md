# `Window`

Owns one node tree, its root, and its own size/title. Every `add_*`
method attaches a new [`Node`](node.md) as a direct child of this
window's implicit root (a flex row, 16px padding, 16px gaps).

## `Window`

**`Window(width=480, height=200, title="tre v2")`**

```python
window = Window(width=400, height=200, title="My App")
```

## Showing a `View`

### `from_view`

**`Window.from_view(view, width=480, height=200, title="tre v2") -> Window`** *(static)*

A window that shows a declarative [`View`](view.md), sharing the view's
node tree directly: bindings, handlers, and `reconcile()` updates on the
`View` appear in the live window.

```python
window = Window.from_view(view, width=400, height=300, title="Settings")
app.add_window(window)
```

### `show_view`

**`show_view(view)`**

Switches which `View` an already-open window shows, without closing it.
Each `View` an app keeps around stays fully alive (its bindings and
`Signal` subscriptions intact); only what this window renders and
dispatches to changes, from the next frame. The view is sized to the
window when switched to — a later resize while a *different* view is
showing won't resize this one until it's shown again.

### `resize`

**`resize(width, height)`**

Resizes the window's root layout box programmatically, without a live
window — the same synthetic pattern as `click`/`hover`. Every later
`add_*` call and synthetic dispatch uses the new size. A real OS resize
updates the same shared size.

## Creating nodes

### `add_rect`

**`add_rect(background, width, height, x=None, y=None)`**

A plain colored rectangle. `background` is an `(r, g, b, a)` tuple of
ints 0–255.

### `add_text`

**`add_text(content, foreground, width, height, typography_role=None, font_family=None, font_weight=None, font_size=None, line_height=None, x=None, y=None)`**

A plain, non-editable text label. `foreground` is its text color; a
label has no fill of its own. `typography_role` picks an MD3 type-scale
role (e.g. `"body_large"`) supplying the font defaults, each
overridable by the explicit font arguments. `width`/
`height` are required (there's no intrinsic-sizing/measure-function
support to size a label from its own content). For editable text, see
[`add_text_field`](#add_text_field).

### `add_checkbox`

**`add_checkbox(background, width, height, checked=False, x=None, y=None)`**

An MD3 checkbox. See [MD3 Components → Selection & Input](../../guide/components.md#selection-input).

### `add_slider`

**`add_slider(background, width, height, value=0.0, x=None, y=None)`**

An MD3 slider with drag-to-set built in. `value` seeds its position,
clamped to `0.0..=1.0`; read and animate it as the `"value"` property.
`background` is the thumb's fill. See [MD3 Components → Selection & Input](../../guide/components.md#selection-input).

### `add_text_field`

**`add_text_field(background, width, height, content="", font_family="Roboto", font_weight=400.0, font_size=16.0, x=None, y=None, multiline=False, show_whitespace=False)`**

An MD3 text field with real keyboard editing. `multiline`/
`show_whitespace` mirror `add_code_editor`'s own two fields, both
`False` by default (the pre-existing behavior for every caller that
doesn't pass them). See
[MD3 Components → Text Fields, Code Editor & Terminal](../../guide/components.md#text-fields-code-editor-terminal).

### `add_image_from_bytes`

**`add_image_from_bytes(rgba, pixel_width, pixel_height, width, height, fit="fill", x=None, y=None)`**

An `Image` node from already-decoded, straight-alpha RGBA8 pixels
(`pixel_width * pixel_height * 4` bytes exactly, or a clear
`ValueError`). No file and no decoding inside `tre` — the caller owns
decoding (a network fetch, any image library, a generated texture).
`width`/`height` are the node's display box; `pixel_width`/
`pixel_height` describe `rgba`, and `fit` (`"cover"`, `"contain"`, or
`"fill"`) resolves any mismatch. The returned node is an ordinary
`Image` node — [`Node.push_frame`](node.md#push_frame) replaces its
pixels afterward.

```python
pixels = bytes([255, 0, 0, 255]) * (64 * 64)  # a solid red 64x64 image
image = window.add_image_from_bytes(pixels, 64, 64, width=200, height=200)
```

### `add_image`

**`add_image(path, width, height, fit="fill", x=None, y=None)`** *(file convenience)*

Reads and decodes an image file (PNG or JPEG only), then builds the same
node `add_image_from_bytes` does. Raises `OSError` if the file can't be
read or decoded, `ValueError` for an unknown `fit`. See
[Working with Files → Images from files](../../guide/working-with-files.md#images-from-files).

### `add_icon`

**`add_icon(name, foreground, size, x=None, y=None)`**

A curated Material Symbols vector icon (`home`, `search`, `menu`,
`close`, `check`, `arrow_back`, `add`, `settings`). Raises `ValueError`
for an unknown `name`. See [MD3 Components → Media & Graphics](../../guide/components.md#media-graphics).

### `add_splitter`

**`add_splitter(background, width, height, initial_position=0.5)`**

A drag-resizable pane divider. See
[MD3 Components → Layout & Structure](../../guide/components.md#layout-structure).

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

## The full MD3 catalog

The 6 factories above are the general-purpose primitives. `Window` also
exposes 50 more `add_*` factories for real MD3 components (buttons,
cards, dialogs, navigation, a terminal, a code editor, and more) — see
[MD3 Components](../../guide/components.md) for the complete, organized
catalog rather than duplicating all 56 signatures here.

## Overlays

Dialogs, menus, snackbars, the side sheet, and a modal navigation
drawer each open/close via a matched pair of `Window` methods, all real
thin wrappers over the same `Tree::open_overlay`/`close_overlay`
primitive:

| Open | Close |
| --- | --- |
| `open_dialog(dialog)` | `close_dialog(dialog)` |
| `open_menu(anchor, menu)` | `close_menu(menu)` |
| `open_snackbar(snackbar)` | `close_snackbar(snackbar)` |
| `open_side_sheet(sheet)` | `close_side_sheet(sheet)` |
| `open_navigation_drawer(drawer)` | `close_navigation_drawer(drawer)` |

See [MD3 Components → Overlays](../../guide/components.md#overlays).

## Terminal

`add_terminal`'s returned `Node` is driven by real mouse/keyboard
dispatch like any other focusable node; these three `Window`-level
methods cover what isn't reachable through the node itself:

### `resize_terminal`

**`resize_terminal(node, cols, rows)`**

Resizes a live terminal session's PTY and VT100 grid in place.

### `get_monospace_cell_size`

**`get_monospace_cell_size(font_size) -> (float, float)`**

Returns `(width, height)` of one character cell in the engine's bundled
monospace face at `font_size` — the same real measurement
`add_terminal` itself uses to size a new terminal from `cols`/`rows`.

### `copy_terminal_selection`

**`copy_terminal_selection() -> str | None`**

Returns the focused terminal's current text selection, or `None` — the
terminal counterpart to `copy()` below. Seed a selection without a
mouse drag with [`Node.set_terminal_selection`](node.md#set_terminal_selection).

### `press_ctrl`

**`press_ctrl(letter) -> bool`**

Sends a Ctrl+`letter` control byte to the focused terminal —
`press_ctrl("c")` sends SIGINT (`0x03`), like Ctrl+C in any terminal.
`letter` must be one ASCII letter (case-insensitive), or `ValueError`.
Returns whether a terminal was focused to receive it; it never touches a
`TextField` (use `copy`/`cut`/`paste` for those).

## Layout composition

### `build_shell`

**`build_shell(menu_bar=None, toolbar=None, status_bar=None) -> Node`**

Builds an `AppShell`-style column container sized to the window's full
width/height, re-parenting the given chrome nodes into it, and returns
the empty `content` container (`flex_grow: 1.0`). See
[Docking & Shell Layout → App shell](../../guide/docking-and-shell.md#app-shell).

## Theming

### `set_theme`

**`set_theme(seed, dark=False, default_theme=None, custom_theme=None, default_theme_spec=None, custom_theme_spec=None)`**

Builds a full MD3 dynamic color scheme from a `(r, g, b, a)` seed color
and makes it active — every already-built themed node (created via a
composition-only `add_*` factory that reads the theme, e.g.
`add_button`/`add_checkbox`) is retroactively re-themed in place.
`default_theme_spec`/`custom_theme_spec` are [theme documents](../../guide/theming-and-accessibility.md#theme-documents)
as `dict`s, layering color/shape/elevation/typography overrides for the
imperative catalog on top. `default_theme`/`custom_theme` *(file
conveniences)* read the same documents from YAML files; each is mutually
exclusive with its `*_spec` twin.

```python
window.set_theme(
    seed=(0x67, 0x50, 0xA4, 0xFF),
    custom_theme_spec={
        "colors": {"primary": "#00FF00"},
        "components": {"button": {"corner_radius": "small"}},
    },
)
```

### `theme`

**`theme -> Theme`** *(read-only property)*

Read-only access to this window's own live theme resolution — the
exact same lookups every composition-only factory (`add_button`,
`add_fab`, etc.) already makes internally, reachable from Python for
building your own MD3-consistent compositions. A fresh `Theme` wrapper
each access (cheap) — reads always see the window's current live
state, including after a real `set_theme()` call.

```python
if window.theme.is_set():
    primary = window.theme.role("primary")  # (r, g, b, a) or None
```

`Theme`'s own methods:

| Method | Returns |
| --- | --- |
| `role(name) -> (r, g, b, a) \| None` | The resolved MD3 color for a role name (e.g. `"primary"`); `None` when no theme is set, or `name` isn't a real role |
| `is_set() -> bool` | Whether a real theme has been resolved at all |
| `shape(component, variant=None) -> float \| None` | The resolved corner-radius override for `component` (and `variant`, if given); `None` if there's no override — fall back to your own formula default |
| `elevation(component, variant=None) -> float \| None` | `shape`'s own sibling for elevation — identical contract |
| `typography(role) -> (family, weight, size, line_height) \| None` | A real, shipped MD3 default for a recognized typography role, regardless of whether a theme is set; `None` only for an unrecognized role name |

## Events, properties, and `simulate` (0.3.4)

`window.on`/`off` for window events (`resize`, `color_scheme`,
`scale_factor`, `close_requested`, `closed`), `window.set(title=...)`,
`window.get(name)`, `window.root`, and `window.simulate(event, node=None,
**fields)` for headless tests — see [Events and Listeners](events.md).

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
| `select_all()` | Ctrl+A — selects the focused `TextField`'s whole content (cursor lands at the end); returns whether a field was focused |

Accepted `key` values for `press_key`: `"tab"`, `"enter"`, `"space"`,
`"escape"`, `"backspace"`, `"delete"`, `"left"`, `"right"`, `"home"`,
`"end"`. Anything else raises `ValueError`.

### The real OS clipboard

`copy`/`cut`/`paste` above are deliberately hermetic — they never touch
the OS clipboard, which keeps tests deterministic. These three are
their real counterparts, the same path live Ctrl+C/X/V takes — what a
context-menu "Copy"/"Cut"/"Paste" item's `on_click` should call:

| Method | Returns |
| --- | --- |
| `copy_to_system_clipboard() -> bool` | `True` only on a complete write; `False` when nothing is focused/selected or the OS clipboard is unreachable (logged, never raised) |
| `cut_to_system_clipboard() -> bool` | Like copy, then removes the selection and fires `Change` — only once the write succeeded, so a failed write never loses the selection |
| `paste_from_system_clipboard() -> bool` | Whether the OS clipboard *read* succeeded (inserting into the focused field, if any) |

On some sandboxed Linux setups with no clipboard manager, clipboard
content may only be served while the process that wrote it is running.

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
