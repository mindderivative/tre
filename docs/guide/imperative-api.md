# Imperative API

The imperative path builds a UI directly from Python method calls —
`Window` creates nodes, and each returned [`Node`](../api/python/node.md)
is a handle for events, animation, and property reads/writes. A
declarative, data-driven layer on top belongs to a framework built on
`tre`, such as Tesserae.

## Windows and the app loop

```python
from tre import App, Window

win1 = Window(width=400, height=200, title="Main")
win2 = Window(width=300, height=150, title="Panel")

app = App()
app.add_window(win1)
app.add_window(win2)
app.run()
```

`App` collects one or more `Window`s and drives them all together in one
blocking `run()` call; each `Window` owns its own node tree and size.
`run()` returns once every window has closed. Pass `max_frames=N` to
`run()` to cap each window at `N` frames — useful for headless/CI runs
that need a real exit condition with no interactive close.

Every `Window` starts with one implicit root node: a flex row with
16px padding and 16px gaps between children. Every `add_*` method below
attaches its new node as a direct child of that root, in call order.

### Updating from another thread

`App`, `Window`, and `Node` may only be used on the thread that
created them, and once `run()` starts it owns that thread. Work that
finishes on a background thread (a download, a subprocess, a file
watcher) hands its UI update to the event loop through
[`App.thread_handle()`](../api/python/app.md#thread_handle):

```python
handle = app.thread_handle()

def worker():                            # a background thread
    result = slow_computation()
    handle.call_soon(lambda: label.set(text=result))

threading.Thread(target=worker, daemon=True).start()
app.run()
```

`call_soon` wakes the loop even when it's idle, and the callable runs on
the event-loop thread at the top of the next frame.

## Creating nodes

`window.create(kind, **props)` makes a detached node; attach it with
`add_child`, to the window's `root` or any other node. The kinds are
`"box"`, `"text"`, `"text_input"`, `"image"`, `"path"`, `"canvas"`,
`"scroll_view"`, `"virtual_list"`, and `"terminal"`; every property is on
[Nodes and Properties](../api/python/properties.md). See
[Docking](docking-and-shell.md) for the docking methods.

```python
panel = window.create("box", width=240, flex_direction="vertical", gap=8, padding=12)
title = window.create("text", text="Inbox", font_size=22, width=200, height=28)
search = window.create("text_input", placeholder="Search", width=216, height=32)
window.root.add_child(panel)
panel.add_child(title)
panel.add_child(search)
```

A node flows in its parent's flex layout; give it `position="absolute"`
with `x`/`y` to place it relative to its parent's padding box instead.
*0.3.5 replaced the `add_*` factories with `create`.*

## Events

Every `Node` takes listeners with `on(event, handler)`:

```python
node.on("click", lambda: ...)
node.on("pointer_enter", lambda: ...)
node.on("pointer_leave", lambda: ...)
node.on("change", lambda event: print(event.old_value, "->", event.new_value))
node.on("focus", lambda: ...)
node.on("unfocus", lambda: ...)
```

A handler takes no arguments or one, the `Event` — its `type`, its
`target` (the live node it's about), `current` (whose listener is running,
as it bubbles), and the fields the event has something to say about, such
as `window_x`/`window_y` and `button` for pointer events and
`old_value`/`new_value` for `change`. See
[Events and Listeners](../api/python/events.md) for every event and field.
An exception raised inside a handler is caught, logged, and non-fatal.

A node is in the Tab order once it's focusable — set `focusable=True`.
Enter and Space then activate it: a `click`.
*0.3.5 removed `set_on_click` and its siblings;* unlike them, `on("click")`
doesn't make a node focusable.

Hover and press feedback is the framework's to draw, from the
`pointer_enter`/`pointer_leave`/`pointer_down` listeners — `tre` 0.3.5
removed the built-in MD3 ripple and state layer (`enable_interaction`).

### Driving events without a live window

`window.simulate(event, node=None, **fields)` drives the same input pipeline
a live window does, so a test or headless script needs no display:

```python
window.simulate("click", node=node)
window.simulate("pointer_move", node=node)        # hover
window.simulate("wheel", node=node, delta_y=10.0)
window.simulate("secondary_click", node=node)
window.simulate("key_down", key="tab")            # "enter", "escape", "arrow_left", ...
window.simulate("input", text="hello")            # typing into the focused text input
window.simulate("key_down", key="c", ctrl=True)   # copy its selection
```

Pointer events aim at the node's center, or at `x`/`y` local to it — see
[Events and Listeners](../api/python/events.md#testing-without-a-display).

## Animation

```python
node.animate(property, to, duration_ms=0, on_complete=None)
```

Starts (or retargets) an animation on one property. Returns immediately —
it never blocks waiting for the animation to finish. `on_complete`, when
given, is called with no arguments exactly once, the real frame the
animation genuinely finishes.

Universal properties (every node kind):

| Property | `to` type | Meaning |
| --- | --- | --- |
| `"opacity"` | `float` | 0.0–1.0 |
| `"corner_radius"` | `float` | pixels |
| `"background"` | `(r, g, b, a)` tuple of ints 0–255 | fill color |
| `"transform"` | `(translate_x, translate_y, scale)` tuple of floats | pan + zoom |

Read a property's current (possibly still-animating) value back with
`node.get(property)`.

```python
def on_faded():
    print("done fading")

rect.animate("opacity", 0.0, duration_ms=300, on_complete=on_faded)
```

!!! note
    `on_complete` callbacks are drained and invoked by `App.run()`'s own
    per-frame loop, or by `window.advance(ms)` in a test.

## Tree structure

```python
child_node.add_child(other_node)  # attach other_node under child_node
node.remove()                     # remove this node and its whole subtree
```

`add_child` rejects (with `ValueError`) attaching a node as a child of its
own descendant, and rejects attaching a node that belongs to a different
`Window`'s tree.
