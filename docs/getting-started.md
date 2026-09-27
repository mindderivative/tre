# Getting Started

This walks through building a first window imperatively from Python. See
[Installation](installation.md) first if you haven't installed `tre` yet.

## A minimal window

```python
from tre import App, Window

window = Window(width=400, height=200, title="tre")
field = window.create("text_input", placeholder="Type here", width=300, height=48)
window.root.add_child(field)

app = App()
app.add_window(window)
app.run()
```

- `Window(width, height, title)` creates a window with its own node tree;
  `window.root` is its root, a flex row.
- `window.create(kind, **props)` makes a node — here a keyboard-editable
  `"text_input"`: click into it and type once the app is running — and
  `add_child` attaches it.
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

Every node is a [`Node`](api/python/node.md) — it takes listeners,
animation, and property reads and writes. Extending the example above with
a clickable button:

```python
from tre import App, Window

window = Window(width=400, height=240, title="tre")

label = window.create("text", text="0 clicks", width=300, height=48, font_size=20)
button = window.create("box", fill=(0x67, 0x50, 0xA4, 0xFF), width=96, height=32,
                       corner_radius=16, focusable=True, role="button", label="Count")
window.root.add_child(label)
window.root.add_child(button)

clicks = 0


def on_click():
    global clicks
    clicks += 1
    label.set(text=f"{clicks} clicks")


button.on("click", on_click)

app = App()
app.add_window(window)
app.run()
```

- [`node.on("click", ...)`](api/python/events.md) registers a Python
  callback fired on a real click — the mouse, or Enter or Space while the
  node is focused; `focusable=True` puts it in the Tab order (see
  [Accessibility & Fonts](guide/theming-and-accessibility.md)).
- Hover and press feedback is the framework's to draw: listen for
  `pointer_enter`/`pointer_leave`/`pointer_down` and change the node's
  paint.

## Animating a property

```python
card = window.create("box", fill=(0x67, 0x50, 0xA4, 0xFF), width=100, height=100)
window.root.add_child(card)
card.animate("opacity", 0.2, duration_ms=400, on_complete=lambda: print("faded"))
```

Every animatable property — `opacity`, `corner_radius`, `fill`,
`stroke_color`, `stroke_width`, `shadows`, `translate_x`/`translate_y`,
`scale`, `rotation_deg`, a path's `data` and trim — can be driven this way,
with an optional `easing`; see [`Node.animate`](api/python/node.md#animate) and
[Paint, Paths, and Animation](api/python/paint.md).

## Where to next

- [Imperative API](guide/imperative-api.md) — the full `Window`/`Node`
  tour: every node kind, events, animation, focus.
- Browse [`examples/`](https://github.com/mindderivative/tre/tree/main/examples)
  in the repository — each script is a small, self-contained proof of one
  mechanism (`examples/docking.py` for docking, `examples/text_field.py`
  for text input, `examples/canvas.py` for custom drawing, and so on).
