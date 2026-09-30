# `Window`

Owns one node tree and the OS window it's painted into. Add windows to an
[`App`](app.md), then call `App.run()`.

**`Window(width=480, height=200, title="tre v2")`** — raises `ValueError`
for a zero width or height.

```python
window = Window(width=640, height=400, title="Inbox")
```

## Nodes

**`create(kind, **props) -> Node`** makes a detached node of `kind` —
`"box"`, `"text"`, `"text_input"`, `"image"`, `"path"`, `"canvas"`,
`"scroll_view"`, `"virtual_list"`, or `"terminal"` — and sets `props`
atomically, as `Node.set` does: a bad property raises `ValueError` and
nothing is created. Every kind and property is on
[Nodes and Properties](properties.md).

**`root`** is the tree's root node: a flex row with 16px padding and 16px
gaps, sized to the window. Attach nodes under it with `add_child`.

```python
card = window.create("box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=200, height=120,
                     corner_radius=12)
window.root.add_child(card)
```

## Layers

**`show_layer(node, anchor=None, placement="below", modal=False,
dismissible=True)`** shows `node` over the window's content, and
**`hide_layer(node)`** hides it — for menus, dialogs, tooltips, and sheets.
See [Layers](layers.md).

## Window events and properties

**`on(event, handler)`** and **`off(event)`** listen to the window itself —
`resize`, `color_scheme`, `scale_factor`, `close_requested`, `closed`,
`dock_target`, and `dock_drop`; see
[Events and Listeners](events.md#window-listeners).

**`set(title=...)`** changes the title, live if the window is open.
**`set(partial_redraw=False)`** makes the window redraw all of itself every
frame. By default (`True`) each frame redraws only the parts of the window
that changed, and a frame where nothing visible changed draws nothing.
The pixels are the same either way; the switch is there for measuring and
for ruling partial redraw out when chasing a rendering bug. A window whose
surface the platform won't let `tre` copy into always redraws in full, and
logs a warning saying so; `get("partial_redraw")` reports the setting, and
`get("partial_redraw_active")` whether it's in effect (`None` until
`App.run()` opens the window).
**`set(show_damage=True)`** makes each presented frame show what it
redrew: its damage rects tinted magenta, or, for a full redraw, the window's
edge outlined in orange. It's for seeing partial redraw work and for finding
redraws an app didn't mean to cause. The tint goes on the image the window
shows, never on the frame `tre` keeps, so it never changes what later frames
draw. Off by default.
**`get(name)`** reads `width`, `height`, `title`, `scale_factor` (`1.0`
until `App.run()` opens the window), `dark` — the OS's current appearance,
or `None` where it can't say ([Window properties](events.md#window-properties))
— `partial_redraw`, `partial_redraw_active`, or `show_damage`.

**`resize(width, height)`** sets the window's size from code, and the
root's layout box follows. It fires no `resize` event — that reports a
change the user made; `simulate("resize", width=, height=)` stands in for
one.

## `measure_text`

**`measure_text(text, font_family="Roboto", font_size=16, font_weight=400,
font_style="normal", letter_spacing=0, line_height=None, max_width=None,
wrap="word", max_lines=None, overflow="clip") -> (width, height)`**

The size `text` takes, laid out exactly as a text node with those properties
paints it: wrapped within `max_width` when given, cut to `max_lines`, ended
with an ellipsis for `overflow="ellipsis"`. The width is the widest shown
line without its trailing whitespace. A text node has no size of its own —
this is how a content-sized widget gets one. See
[Text](../../guide/text.md#sizing-text-to-its-content).

## Clipboard

**`read_clipboard() -> str | None`** returns the OS clipboard's text, or
`None` when it holds no text or can't be reached. **`write_clipboard(text) ->
bool`** puts `text` on it, `False` when it can't be reached. Neither raises:
a headless environment may have no clipboard service, which is logged. A
window keeps what it wrote on the clipboard for as long as it runs.

A focused text input handles Ctrl+C, Ctrl+X, Ctrl+V, and Ctrl+A itself —
copy, cut (which fires `change`), paste (typed input, so `input` fires), and
select all — and never copies from an `obscured` input.

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

Every event and field is in
[Events and Listeners](events.md#testing-without-a-display).

**`advance(ms)`** moves this window's time forward by exactly `ms`
milliseconds, then runs animations, their `on_complete` callbacks, and
layout at the new time — deterministic time for tests, where `App.run()`
renders no frames:

```python
window.advance(0)            # pin the clock before starting animations
card.animate("opacity", 0.0, 200)
window.advance(100)
assert card.get("opacity") == 0.5
```

The first call pins the window's clock at the real current time; from then
on only `advance` moves it. Each window keeps its own time, and
`App.run()` returns every window it opens to the real clock.

## Docking

| Method | Purpose |
| --- | --- |
| `add_dock_zone(side, container, size)` | Registers `container` as `side`'s dock zone |
| `dock_panel(side, panel)` | Docks `panel` into `side`'s zone and shows it — moving it, if it's docked in another zone |
| `set_active_panel(side, index)` | Shows the zone's `index`th panel |
| `start_panel_drag(panel)` | Starts dragging a docked panel; the drag reports through the `dock_target`/`dock_drop` window events |
| `undock_panel(panel)` | Takes `panel` out of its zone and off the tree; `dock_panel` can dock it again |

`side` is one of `"left"`, `"right"`, `"top"`, `"bottom"`, `"center"`.
See [Docking](../../guide/docking.md) for a walkthrough.
