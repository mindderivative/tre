# Getting Started

This builds a small counter: a label, and a button that counts clicks. See
[Installation](installation.md) first if you haven't installed `tre`.

## A window

```python
from tre import App, Window

window = Window(width=360, height=160, title="Counter")

app = App()
app.add_window(window)
app.run()
```

`Window` makes a window with its own node tree. `App` collects windows, and
`app.run()` opens them and blocks until every one has closed. `window.root`
is the tree's root: a flex row with 16px of padding and 16px between
children.

## Nodes

Nodes come from `window.create(kind, **props)` and are attached with
`add_child`. Add a label and a button before `app.run()`:

```python
window.root.set(align_items="center")

label = window.create("text", text="0 clicks", font_size=20, width=120, height=28)
button = window.create("box", width=96, height=40, corner_radius=20,
                       fill=(0x67, 0x50, 0xA4, 0xFF),
                       align_items="center", justify_content="center")
button.add_child(window.create("text", text="Count", fill=(0xFF, 0xFF, 0xFF, 0xFF),
                               width=48, height=20, text_align="center"))
window.root.add_child(label)
window.root.add_child(button)
```

The button is just a box with a text inside it. A text node needs a size —
[`window.measure_text`](guide/text.md#sizing-text-to-its-content) finds one
for any string.

## Events

Listen for clicks with `on`. A click on the button's text bubbles up to the
button:

```python
clicks = 0


def count():
    global clicks
    clicks += 1
    label.set(text=f"{clicks} clicks")


button.on("click", count)
```

To make it work from the keyboard and for screen readers too, say what it
is and let it take focus:

```python
button.set(role="button", label="Count", focusable=True, cursor="pointer")
```

Now Tab reaches it, and Enter or Space clicks it.

## Feedback

`tre` draws no hover or press effect; you choose one. Fade the button while
the pointer is over it:

```python
button.on("pointer_enter", lambda: button.animate("opacity", 0.85, 120))
button.on("pointer_leave", lambda: button.animate("opacity", 1.0, 120))
```

## Where to next

- The [Guide](guide/nodes-and-layout.md) covers each building block in turn;
  [Building a Widget](guide/building-a-widget.md) makes a complete switch.
- [`examples/`](https://github.com/mindderivative/tre/tree/main/examples)
  has a small runnable script for each one.
