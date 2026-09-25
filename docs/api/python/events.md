# Events and Listeners

*New in 0.3.4.* The event model of the [target API](../../design/target-api.md):
listeners registered with `on(event, handler)` on a [`Node`](node.md) or a
[`Window`](window.md). It sits beside the older `set_on_*` handlers, which keep
working exactly as before (they don't bubble) until they're removed in 0.3.5.

## Node listeners

```python
button.on("click", lambda e: print("clicked", e.target == button))
button.off("click")
```

`node.on(event, handler)` registers `handler`, replacing any earlier listener
for the same event on that node. `handler` receives an [`Event`](#event), or
nothing if it takes no parameters. An unknown event name raises `ValueError`
listing the valid ones.

| Event | Fires when | Bubbles |
| --- | --- | --- |
| `pointer_enter`, `pointer_leave` | The pointer enters or leaves the node's **subtree** — not when it moves between the node and its own descendants, and `pointer_leave` also fires when the pointer leaves the window | no |
| `pointer_down`, `pointer_move`, `pointer_up` | A button is pressed, the pointer moves, a button is released | yes |
| `click` | Primary press and release on the same node, or keyboard activation | yes |
| `secondary_click` | Secondary press and release on the same node | yes |
| `wheel` | A wheel or trackpad scroll | yes |
| `key_down`, `key_up` | A key is pressed or released while the node, or a descendant, has focus (the root gets keys when nothing is focused) | yes |
| `input` | Committed text arrives for the focused text field | yes |
| `focus`, `blur` | A node gains or loses keyboard focus | yes |
| `change` | A text field's text was changed by the user | no |
| `a11y_action` | An assistive technology requested an action | yes |

### Propagation

A bubbling event runs the target's listener, then its parent's, and so on to
the root. Any listener can call `event.stop()` to end it. `event.target` is
where the event happened and `event.current` is the node whose listener is
running. Because `focus` and `blur` bubble, a container knows focus moved
somewhere inside it, and `event.target` says where.

Raw input is delivered before what it caused, so a click delivers
`pointer_down`, then `pointer_up`, then `click`.

`Text` nodes and icons are never the target of pointer events: a press on a
button's label lands on the button.

### Pointer capture

```python
def on_down(e):
    thumb.capture_pointer()

thumb.on("pointer_down", on_down)
thumb.on("pointer_move", lambda e: print(e.x))  # keeps firing off the thumb
```

`node.capture_pointer()` routes every later pointer event to `node`, wherever
the pointer goes, until a button is released or `node.release_pointer()` is
called. While captured, the capturing node is the target and events still
bubble from it.

## Window listeners

```python
window.on("resize", lambda e: print(e.width, e.height))
window.on("close_requested", lambda e: e.cancel())  # keep the window open
```

| Event | Fires when | Payload |
| --- | --- | --- |
| `resize` | The window's client area changed size | `width`, `height` |
| `color_scheme` | The OS switched between light and dark | `dark` |
| `scale_factor` | The window moved to a display with a different scale factor | `scale_factor` |
| `close_requested` | The user asked to close the window; `event.cancel()` keeps it open | — |
| `closed` | The window closed — by the user or by reaching `max_frames` | — |

A window event has no node: `event.target` is `None`.

## Window properties

```python
window.set(title="Editor — draft.md")
window.get("scale_factor")  # 1.0 until App.run() opens the window
```

`window.set(title=...)` changes the title, live if the window is open.
`window.get(name)` reads `width`, `height`, `title`, or `scale_factor`. `root`
is the window's root node.

## Event

| Field | Present for |
| --- | --- |
| `type` | every event |
| `target` | every node event — where it happened |
| `current` | node listener events — the node whose listener is running |
| `x`, `y` | pointer and wheel events — local to `current` |
| `window_x`, `window_y` | pointer and wheel events — in the window |
| `button` | pointer events and pointer clicks — `"primary"`, `"secondary"`, `"middle"` |
| `delta_x`, `delta_y` | `wheel` — pixels, positive scrolling right and down |
| `key`, `repeat` | `key_down`, `key_up` |
| `shift`, `ctrl`, `alt`, `meta` | pointer, wheel, key, and click events |
| `text` | `input` |
| `old_value`, `new_value` | `change` |
| `action`, `value` | `a11y_action` |
| `width`, `height` / `dark` / `scale_factor` | `resize` / `color_scheme` / `scale_factor` |

Methods: `stop()` ends propagation; `cancel()` prevents `close_requested`'s
default (any other event raises `ValueError`).

Key names are snake_case (`"enter"`, `"escape"`, `"arrow_left"`, `"page_up"`,
`"f5"`) for named keys, and the produced character (`"a"`, or `"A"` with Shift)
for character keys.

`Node` handles compare equal, and hash equal, when they name the same node —
`event.target == button` works, and nodes can key a dict.

## Testing without a display

`window.simulate(event, node=None, **fields)` delivers a synthetic event through
the same pipeline as real input, so listeners and the older handlers both fire.

```python
window.simulate("click", node=button)
window.simulate("pointer_down", node=thumb, x=4, y=10, shift=True)
window.simulate("key_down", key="tab")
window.simulate("resize", width=800, height=600)
```

| Event | Fields |
| --- | --- |
| `pointer_down`, `pointer_up` | `node` and/or `x`, `y`; `button` |
| `pointer_move`, `pointer_enter`, `click`, `secondary_click` | `node` and/or `x`, `y` |
| `wheel` | `node` and/or `x`, `y`; `delta_x`, `delta_y` |
| `pointer_leave` | — (the pointer leaves the window) |
| `key_down`, `key_up` | `key`; `repeat` |
| `input` | `text` |
| `focus`, `blur` | `node` |
| `resize` | `width`, `height` |
| `color_scheme` | `dark` |
| `scale_factor` | `scale_factor` |
| `close_requested`, `closed` | — |

Pointer events aim at `node`'s center, at `x`/`y` local to `node`, or at
window-space `x`/`y` without a node. Any event also takes `shift`, `ctrl`,
`alt`, and `meta`. A simulated key press edits text and moves focus exactly
as a real one does. An unknown event or field raises `ValueError`.
