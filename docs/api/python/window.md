# `Window`

Owns one node tree, its root, and its own size/title. Nodes come from
[`create`](#nodes); attach them under [`root`](#nodes), a flex row with 16px
padding and 16px gaps.

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

## Nodes

**`create(kind, **props) -> Node`** makes a detached node of `kind` —
`"box"`, `"text"`, `"text_input"`, `"image"`, `"path"`, `"canvas"`,
`"scroll_view"`, `"virtual_list"`, or `"terminal"` — and **`root`** is the
node to attach it under. Every kind and property is on
[Nodes and Properties](properties.md).

```python
card = window.create("box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=200, height=120,
                     corner_radius=12)
window.root.add_child(card)
```

*0.3.5 replaced the `add_*` factories* (`add_rect`, `add_text`,
`add_text_field`, `add_code_editor`, `add_image_from_bytes`, `add_video`,
`add_canvas`, `add_scroll_view`, `add_virtual_list`, `add_terminal`) with
`create`, and removed `redraw_canvas` (use `canvas.redraw()`),
`set_virtual_list_window` (a virtual list builds its own visible rows),
`resize_terminal` (use `terminal.set(cols=, rows=)`),
`copy_terminal_selection` (`terminal.get("selection")`), and
`get_monospace_cell_size` (use [`measure_text`](#measure_text)).

### Images

An `"image"` node takes straight-alpha RGBA8 pixels; `tre` never reads or
decodes files — any decoder works:

```python
from PIL import Image

img = Image.open("photo.png").convert("RGBA")
picture = window.create("image", rgba=img.tobytes(), pixel_width=img.width,
                        pixel_height=img.height, width=200, height=120, fit="cover")
```

Video is the same node with new pixels set each frame.

### Icons

An icon is a `"path"` node from its SVG `d` and `viewBox` — see
[Paint, Paths, and Animation](paint.md). *0.3.5 removed `add_icon`.*

## Overlays

Build dialogs, menus, tooltips, snackbars, and sheets from your own nodes
with `show_layer`/`hide_layer` — see [Layers](layers.md). 0.3.5 removed the
older `open_*`/`close_*` pairs and the MD3 factories they opened.

## Terminal

A `"terminal"` node is driven by real mouse and keyboard dispatch like any
other focusable node. While one is focused, it takes every key: text and
named keys go to its shell, and Ctrl+`letter` sends that letter's control
byte (Ctrl+C is SIGINT). Ctrl+Shift+C copies its selection to the
clipboard; a wheel over it scrolls its history.

## Events, properties, and `simulate` (0.3.4)

`window.on`/`off` for window events (`resize`, `color_scheme`,
`scale_factor`, `close_requested`, `closed`, `dock_target`, `dock_drop`), `window.set(title=...)`,
`window.get(name)`, `window.root`, and `window.simulate(event, node=None,
**fields)` for headless tests — see [Events and Listeners](events.md).
`window.create(kind, **props)` makes detached nodes — see [Nodes](#nodes).

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

## Testing without a display

**`simulate(event, node=None, **fields)`** delivers a synthetic event through
the same input pipeline as a live one — listeners, focus, text editing, a
focused terminal, the clipboard shortcuts, and docking drags all behave as
they would for a real mouse or keyboard:

```python
window.simulate("click", node=button)
window.simulate("pointer_move", node=card)          # hover
window.simulate("wheel", node=rows, delta_y=60)     # positive scrolls down
window.simulate("key_down", key="tab", shift=True)  # focus backward
window.simulate("input", text="hello")              # typing into the focused input
window.simulate("key_down", key="c", ctrl=True)     # copy its selection
```

Every event and field is in [Events and Listeners](events.md#testing-without-a-display).
*0.3.5 removed the older synthetic methods* — `click`, `hover`, `focus`,
`scroll`, `right_click`, `press_key`, `type_text`, `press_ctrl`, `copy`,
`cut`, `paste`, and `select_all` — which `simulate` covers.

### Clipboard

**`read_clipboard() -> str | None`** returns the OS clipboard's text, or
`None` when it holds no text or can't be reached. **`write_clipboard(text) ->
bool`** puts `text` on it, `False` when it can't be reached. Neither raises: a
headless environment may have no clipboard service, which is logged.

A focused text input handles Ctrl+C, Ctrl+X, Ctrl+V, and Ctrl+A itself —
copy, cut (which fires `change`), paste (typed input, so `input` fires), and
select all — and never copies from an `obscured` input. A window keeps what
it wrote on the clipboard for as long as it runs. *0.3.5 replaced
`copy_to_system_clipboard`, `cut_to_system_clipboard`, and
`paste_from_system_clipboard` with these.*

## Docking

| Method | Purpose |
| --- | --- |
| `add_dock_zone(side, container, size)` | Registers `container` as `side`'s dock zone |
| `dock_panel(side, panel)` | Docks `panel` into `side`'s zone and shows it |
| `set_active_panel(side, index)` | Shows the zone's `index`th panel *(was `set_active_tab`)* |
| `start_panel_drag(panel)` | Starts dragging a docked panel; the drag reports through the `dock_target`/`dock_drop` window events |

`side` is one of `"left"`, `"right"`, `"top"`, `"bottom"`, `"center"`.
See [Docking](../../guide/docking-and-shell.md) for a full walkthrough.
