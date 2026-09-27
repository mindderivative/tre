# `Window`

Owns one node tree, its root, and its own size/title. Every `add_*`
method attaches a new [`Node`](node.md) as a direct child of this
window's implicit root (a flex row, 16px padding, 16px gaps).

## `Window`

**`Window(width=480, height=200, title="tre v2")`**

```python
window = Window(width=400, height=200, title="My App")
```

## Size

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

### `add_text_field`

**`add_text_field(background, width, height, content="", font_family="Roboto", font_weight=400.0, font_size=16.0, x=None, y=None, multiline=False, show_whitespace=False)`**

A text field with real keyboard editing. `multiline`/`show_whitespace`
mirror `add_code_editor`'s own two fields, both `False` by default.

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
for an unknown `name`.

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

## Overlays

Build dialogs, menus, tooltips, snackbars, and sheets from your own nodes
with `show_layer`/`hide_layer` — see [Layers](layers.md). 0.3.5 removed the
older `open_*`/`close_*` pairs and the MD3 factories they opened.

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

## Events, properties, and `simulate` (0.3.4)

`window.on`/`off` for window events (`resize`, `color_scheme`,
`scale_factor`, `close_requested`, `closed`, `dock_target`, `dock_drop`), `window.set(title=...)`,
`window.get(name)`, `window.root`, and `window.simulate(event, node=None,
**fields)` for headless tests — see [Events and Listeners](events.md).
`window.create(kind, **props)` makes detached nodes — see
[Paint, Paths, and Animation](paint.md#creating-nodes).

### `measure_text`

**`measure_text(text, font_family="Roboto", font_size=16, font_weight=400,
font_style="normal", letter_spacing=0, line_height=None, max_width=None,
wrap="word", max_lines=None, overflow="clip") -> (width, height)`** *(new in 0.3.4)*

The size `text` takes, laid out exactly as a text node with those properties
paints it: wrapped within `max_width` when given, cut to `max_lines`, ended with
an ellipsis for `overflow="ellipsis"`. The width is the widest shown line
without its trailing whitespace. A text node has no size of its own — this is
how a content-sized widget gets one. See
[Nodes and Properties](properties.md#text).

### `advance`

**`advance(ms)`** *(new in 0.3.4)*

Moves this window's time forward by exactly `ms` milliseconds, then runs
animations, their `on_complete` callbacks, and layout at the new time —
deterministic time for headless tests, where `App.run()` renders no
frames:

```python
window.advance(0)            # pin the clock before starting animations
card.animate("opacity", 1.0, 200)
window.advance(100)
assert card.get("opacity") == 0.5
```

The first call pins the window's clock at the real current time; from then
on only `advance` moves it. Each window keeps its own time, and
`App.run()` returns every window it opens to the real clock.

## Synthetic input dispatch

These work without a live rendered window — each computes layout, then
dispatches at the target node's real, current center point.

| Method | Simulates |
| --- | --- |
| `click(node)` | A primary-button press + release |
| `hover(node)` | The pointer moving over `node` (fires `HoverEnter`/`HoverExit`) |
| `scroll(node, delta_y)` | A mouse wheel scroll (bubbles to the nearest `VirtualList` ancestor) |
| `right_click(node)` | A secondary-button press + release |
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

## Docking

| Method | Purpose |
| --- | --- |
| `add_dock_zone(side, container, size)` | Registers `container` as `side`'s dock zone |
| `dock_panel(side, panel)` | Docks `panel` into `side`'s zone and shows it |
| `set_active_panel(side, index)` | Shows the zone's `index`th panel *(was `set_active_tab`)* |
| `start_panel_drag(panel)` | Starts dragging a docked panel; the drag reports through the `dock_target`/`dock_drop` window events |

`side` is one of `"left"`, `"right"`, `"top"`, `"bottom"`, `"center"`.
See [Docking](../../guide/docking-and-shell.md) for a full walkthrough.

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
