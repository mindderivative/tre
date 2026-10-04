# Events and Input

Input reaches your code through listeners. `node.on(event, handler)`
registers one — replacing any earlier listener for that event on that node —
and `node.off(event)` removes it. A handler takes no arguments, or one: the
[`Event`](../reference/events.md#event).

```python
button.on("click", lambda: save())
button.on("pointer_down", lambda e: print(e.x, e.y, e.button))
```

An exception in a handler is logged and doesn't stop the app. Every event,
and every field it carries, is in
[Events and Listeners](../reference/events.md).

## Bubbling

Most events bubble: the target's listener runs, then its parent's, and so on
up to the root, until one calls `event.stop()`. `event.target` is where it
happened; `event.current` is the node whose listener is running. So one
listener on a list handles a click on any row:

```python
def pick(event):
    if event.target in row_ids:
        select(row_ids[event.target])

list_box.on("click", pick)
```

`pointer_enter`, `pointer_leave`, `change`, `dismiss`, and `scroll` don't bubble.
`pointer_enter`/`pointer_leave` fire when the pointer enters or leaves the
node's whole subtree, not when it moves between the node and its children —
so a hover effect doesn't flicker as the pointer crosses a label.

## Pointer

A press delivers `pointer_down`, then `pointer_up`, then `click` if both
landed on the same node (`secondary_click` for the secondary button). The
middle button and the mouse's side buttons (`event.button` is `"back"` or
`"forward"`, 0.4.1) make no click; watch `pointer_down` for them. A wheel or
trackpad scroll arrives as `wheel`, with `delta_x`/`delta_y` in pixels,
positive right and down; it bubbles. With Shift held, a wheel with no
horizontal part arrives as `delta_x`, so it scrolls sideways (0.4.3).
`event.x`/`event.y` are local to `event.current`; `window_x`/`window_y` are
in the window. Text nodes are never the target, so a press on a button's
label lands on the button. `hit_testable=False` makes any node transparent to
the pointer; `cursor` sets the pointer's shape over it.

A drag captures the pointer, so it keeps receiving events after the pointer
leaves it:

```python
def start(e):
    handle.capture_pointer()

handle.on("pointer_down", start)
handle.on("pointer_move", lambda e: resize_to(e.window_x))
```

Capture ends when the button is released, or on `release_pointer()`.

## Touch and gestures

A touch screen (0.5.4) delivers each finger as `touch_start`, `touch_move`,
`touch_end` and `touch_cancel`, with `event.pointer_id` telling fingers apart.
A finger's events all go to the node it landed on, wherever it moves, so a drag
that leaves a node keeps reporting to it. The engine also recognizes four
gestures from the fingers and delivers them as events:

| Event | When | Fields |
| --- | --- | --- |
| `tap` | a quick touch and release in place | `count` (2 for a double tap) |
| `long_press` | a touch held in place half a second | position |
| `pan` | one finger dragging past a 10 pixel slop | `phase`, `delta_x`/`delta_y` (since the last event), `total_x`/`total_y`, and on `"ended"` `velocity_x`/`velocity_y` in pixels a second |
| `pinch` | two fingers moving together or apart; also a trackpad pinch | `phase`, `scale` (against the start), `scale_delta` (against the last event), `delta_x`/`delta_y` and `total_x`/`total_y` of the midpoint |

```python
canvas.on("pinch", lambda e: view.zoom_by(e.scale_delta, around=(e.window_x, e.window_y)))
canvas.on("pan", lambda e: view.pan_by(e.delta_x, e.delta_y))
photo.on("long_press", lambda: show_menu(photo))
```

The first finger on the screen is also the pointer, so an app written for a mouse
works under a finger: a tap is a `click`, and a touch hovers what it is over while
it is down. When that finger starts to pan, the press is cancelled instead
(`pointer_cancel`, no `click`), and the pan scrolls the scroll view or virtual
list under it, content following the finger. If a `pan` listener sits on the node
under the finger or an ancestor, the app has taken the pan and nothing scrolls.
A second finger is a pinch's, not a second pointer. Each touch event comes first,
then the pointer events it stands for, then the gesture it completes.

Limits: a pan's velocity is reported but nothing keeps scrolling after the
finger lifts (no momentum); there is no rotation gesture; and the engine's touch
handling was written and tested with simulated touches (`window.simulate`),
not on a touch screen.

## Files dragged from the OS

Dragging files from a file manager over the window and dropping them (0.5.4)
fires `file_hover` (with `paths`), then `file_drop` (with `paths`) or
`file_hover_cancel` if the drag leaves. Each goes to the node under the pointer,
bubbling, and to the window's own listener:

```python
drop_zone.on("file_hover", lambda e: drop_zone.set(stroke_color=ACCENT, stroke_width=2))
drop_zone.on("file_hover_cancel", lambda: drop_zone.set(stroke_width=0))
drop_zone.on("file_drop", lambda e: open_files(e.paths))

window.on("file_drop", lambda e: print("dropped anywhere:", e.paths))
```

`e.paths` is a list of path strings and `e.path` its first. A drop of several files
is one event, not one per file. The engine only reports the paths: reading the files
is yours.

Limits: the OS gives a file drag no position of its own while it is over the window,
so the node is the one under the pointer's last known position, which on some
platforms is where it was when the drag entered; for a full-window drop target,
listen on the window. File drag and drop works on Windows, macOS and X11. It does not
on Wayland (`winit` 0.30 doesn't implement it there), so a Wayland session never
fires these events; `simulate` does.

## Focus and the keyboard

A node takes keyboard focus once it's `focusable=True`; text inputs and
terminals are focusable already. Tab and Shift+Tab move through focusable
nodes in tree order, and `tab_index` changes the order: positive values come
first, and a negative one leaves the node out of the Tab order while still
focusable by click and `focus()`. A click focuses the nearest focusable node
under the pointer.

The focused node gets `key_down`/`key_up`, which bubble; when nothing is
focused, the root gets them. Enter and Space on a focused node fire `click` —
except in a text input or terminal, which take every key — so a focusable box
is already a keyboard button. Keys the focused node doesn't use scroll the
nearest scroll view around it (0.4.2), unless Ctrl, Alt, or Meta is held (0.4.3) — see
[Keyboard scrolling](nodes-and-layout.md#keyboard-scrolling).

`focus` and `unfocus` bubble too, and carry `related_target`, the node on the
other side of the move, so a composite widget can tell focus moving between
its own parts from focus leaving it. `focus` also carries `focus_visible`:
`True` when focus came from the keyboard or an assistive technology, `False`
after a pointer press — the rule browsers use for `:focus-visible`, and the
one to show a focus ring by. Tesserae Engine draws no focus ring itself.

## Text input

A focused `"text_input"` handles its own editing: typing, the arrow keys and
Home/End (with Shift to select), Backspace and Delete, Enter in a `multiline`
input, and Ctrl+C, Ctrl+X, Ctrl+V, and Ctrl+A. Page Up and Page Down it
leaves alone, to scroll the view around it. Two events report it:

- `input` — committed text arrived (`event.text`), from typing, an input
  method, or a paste.
- `change` — the user changed the text (`event.old_value`,
  `event.new_value`). Setting `text` from code fires no `change`.

## Clipboard

`window.read_clipboard()` returns the OS clipboard's text and
`window.write_clipboard(text)` replaces it. Neither raises: where no clipboard
service is reachable they return `None` and `False`. What a window writes
stays on the clipboard while it runs. An `obscured` text input never copies
or cuts.

## Window events

`window.on(event, handler)` listens to the window itself:

| Event | When |
| --- | --- |
| `resize` | its size changed (`width`, `height`) |
| `color_scheme` | the OS switched light or dark (`dark`); `window.get("dark")` reads it now |
| `scale_factor` | it moved to a display with another scale (`scale_factor`) |
| `close_requested` | the user asked to close it — `event.cancel()` keeps it open |
| `closed` | it closed |
| `dock_target`, `dock_drop` | a [docking](docking.md) drag moved or ended |

```python
window.on("close_requested", lambda e: e.cancel() if unsaved() else None)
```

## Testing without a display

`window.simulate(event, node=None, **fields)` sends a synthetic event
through the same pipeline real input takes — hit testing, bubbling, focus,
text editing, the clipboard shortcuts — so tests need no display:

```python
window.simulate("click", node=button)
window.simulate("pointer_down", node=thumb, x=4, y=10)
window.simulate("key_down", key="tab", shift=True)
window.simulate("input", text="hello")
window.simulate("key_down", key="a", ctrl=True)
window.simulate("resize", width=800, height=600)
```

Pointer events aim at `node`'s center, at `x`/`y` local to it, or at window
coordinates when there's no node. Pair it with
[`window.advance`](animation.md#testing-animations) for anything animated.
