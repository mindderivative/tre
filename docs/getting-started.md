# Getting Started

This walks through building a first window imperatively from Python. See
[Installation](installation.md) first if you haven't installed `tre` yet.

## A minimal window

```python
from tre import App, Window

window = Window(width=400, height=200, title="tre")
window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=300, height=48)

app = App()
app.add_window(window)
app.run()
```

- `Window(width, height, title)` creates a window with its own node tree —
  every `add_*` method on it attaches a new node as a direct child of the
  window's implicit root row.
- `add_text_field(background, width, height, ...)` creates a real,
  keyboard-editable `TextField` — click into it and type once the app is
  running.
- `App()` collects one or more windows; `add_window` registers this one to
  be opened.
- `app.run()` is the single blocking call that opens every registered
  window and drives them together — it returns once every window has
  closed.

Run it:

```bash
python getting_started.py
```

## Adding interaction

Every node returned by an `add_*` method is a [`Node`](api/python/node.md)
— it supports event handlers, animation, and property reads/writes.
Extending the example above with a clickable button:

```python
from tre import App, Window

window = Window(width=400, height=240, title="tre")

label = window.add_text_field(
    background=(0xEE, 0xEE, 0xEE, 0xFF), width=300, height=48, content="0 clicks"
)

clicks = 0


def on_click():
    global clicks
    clicks += 1
    label.set_text(f"{clicks} clicks")


button = window.add_rect(background=(0x67, 0x50, 0xA4, 0xFF), width=96, height=32)
button.set_on_click(on_click)

app = App()
app.add_window(window)
app.run()
```

- [`Node.set_on_click`](api/python/node.md#set_on_click) registers a
  Python callback fired on a real click (mouse or keyboard activation —
  see [Accessibility & Fonts](guide/theming-and-accessibility.md)).
- Hover and press feedback is the framework's to draw: listen for
  `pointer_enter`/`pointer_leave`/`pointer_down` and change the node's
  paint (`tre` 0.3.5 removed the built-in MD3 ripple).

## Animating a property

```python
rect = window.add_rect(background=(0x67, 0x50, 0xA4, 0xFF), width=100, height=100)
rect.animate("opacity", 0.2, duration_ms=400, on_complete=lambda: print("faded"))
```

Every `Animated<T>` property on a node (`opacity`, `corner_radius`,
`elevation`, `background`, `transform`, `shape`, plus a few component-
specific ones) can be driven this way — see
[`Node.animate`](api/python/node.md#animate) for the full list and
[Imperative API](guide/imperative-api.md) for more on the animation model.

## Where to next

- [Imperative API](guide/imperative-api.md) — the full `Window`/`Node`
  tour: every node kind, events, animation, focus.
- Browse [`examples/`](https://github.com/mindderivative/tre/tree/main/examples)
  in the repository — each script is a small, self-contained proof of one
  mechanism (`examples/docking.py` for docking, `examples/text_field.py`
  for text input, `examples/canvas.py` for custom drawing, and so on).
