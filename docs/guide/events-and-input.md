# Events and Input

Input reaches your code through listeners. `node.on(event, handler)`
registers one — replacing any earlier listener for that event on that node —
and `node.off(event)` removes it. A handler takes no arguments, or one: the
[`Event`](../api/python/events.md#event).

```python
button.on("click", lambda: save())
button.on("pointer_down", lambda e: print(e.x, e.y, e.button))
```

An exception in a handler is logged and doesn't stop the app. Every event,
and every field it carries, is in
[Events and Listeners](../api/python/events.md).

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

`pointer_enter`, `pointer_leave`, `change`, and `dismiss` don't bubble.
`pointer_enter`/`pointer_leave` fire when the pointer enters or leaves the
node's whole subtree, not when it moves between the node and its children —
so a hover effect doesn't flicker as the pointer crosses a label.

## Pointer

A press delivers `pointer_down`, then `pointer_up`, then `click` if both
landed on the same node (`secondary_click` for the secondary button).
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
is already a keyboard button.

`focus` and `unfocus` bubble too, and carry `related_target`, the node on the
other side of the move, so a composite widget can tell focus moving between
its own parts from focus leaving it. `focus` also carries `focus_visible`:
`True` when focus came from the keyboard or an assistive technology, `False`
after a pointer press — the rule browsers use for `:focus-visible`, and the
one to show a focus ring by. `tre` draws no focus ring itself.

## Text input

A focused `"text_input"` handles its own editing: typing, the arrow keys and
Home/End (with Shift to select), Backspace and Delete, Enter in a `multiline`
input, and Ctrl+C, Ctrl+X, Ctrl+V, and Ctrl+A. Two events report it:

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
| `color_scheme` | the OS switched light or dark (`dark`) |
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
