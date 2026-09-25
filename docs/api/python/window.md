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

### `add_text`

**`add_text(content, background, width, height, font_family="Roboto", font_weight=400.0, font_size=16.0, x=None, y=None)`**

A plain, non-editable text label — `background` is repurposed as the
glyph color (a label has no visible box of its own), the same real
convention a declarative `kind: Text` widget already uses. `width`/
`height` are required (there's no intrinsic-sizing/measure-function
support to size a label from its own content). For editable text, see
[`add_text_field`](#add_text_field).

### `add_checkbox`

**`add_checkbox(background, width, height, checked=False, x=None, y=None)`**

An MD3 checkbox. See [MD3 Components → Selection & Input](../../guide/components.md#selection-input).

### `add_slider`

**`add_slider(background, width, height, value=0.0, x=None, y=None)`**

An MD3 slider with drag-to-set built in. `value` seeds `thumb_position`,
clamped to `0.0..=1.0`. See [MD3 Components → Selection & Input](../../guide/components.md#selection-input).

### `add_text_field`

**`add_text_field(background, width, height, content="", font_family="Roboto", font_weight=400.0, font_size=16.0, x=None, y=None, multiline=False, show_whitespace=False)`**

An MD3 text field with real keyboard editing. `multiline`/
`show_whitespace` mirror `add_code_editor`'s own two fields, both
`False` by default (the pre-existing behavior for every caller that
doesn't pass them). See
[MD3 Components → Text Fields, Code Editor & Terminal](../../guide/components.md#text-fields-code-editor-terminal).

### `add_image`

**`add_image(path, width, height, fit="fill", x=None, y=None)`**

Loads and decodes a real image file (`png`/`jpeg`) and uploads it as a
GPU texture. `fit` is `"cover"`, `"contain"`, or `"fill"`. Raises
`OSError` if the file can't be read or decoded, `ValueError` for an
unknown `fit`. See [MD3 Components → Media & Graphics](../../guide/components.md#media-graphics).

### `add_image_from_bytes`

**`add_image_from_bytes(rgba, pixel_width, pixel_height, width, height, fit="fill", x=None, y=None)`**

`add_image`'s decode-free sibling: `rgba` is already-decoded, straight-
alpha RGBA8 pixels (`pixel_width * pixel_height * 4` bytes exactly, or
a clear `ValueError`) — no file, no image-decoding crate involved, the
caller owns decoding entirely (from a network fetch, a different image
library, a generated texture, anything). `width`/`height` are the
node's own fixed display box — `add_image`'s identical contract;
`pixel_width`/`pixel_height` describe `rgba` itself, and `fit` resolves
any mismatch between the two. The node this returns is a real,
ordinary `Image` node — [`Node.push_frame`](node.md#video-specific)
keeps working on it afterward, identically to one built via
`add_video`.

```python
pixels = bytes([255, 0, 0, 255]) * (64 * 64)  # a solid red 64x64 image
image = window.add_image_from_bytes(pixels, 64, 64, width=200, height=200)
```

### `add_icon`

**`add_icon(name, color, size, x=None, y=None)`**

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
terminal counterpart to `copy()` below.

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
`default_theme`/`custom_theme` (paths to theme YAML files) layer
shape/elevation/color overrides for the imperative catalog on top —
see [Theming & Accessibility → Dynamic color theming](../../guide/theming-and-accessibility.md#dynamic-color-theming).
`default_theme_spec`/`custom_theme_spec` are the `dict` forms of those two
paths (the same schema the theme YAML file holds), each mutually
exclusive with its path twin — see
[Theming & Accessibility → Themes as data](../../guide/theming-and-accessibility.md#themes-as-data).

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
