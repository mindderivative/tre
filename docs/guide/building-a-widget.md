# Building a Widget

`tre` has no buttons, switches, or sliders. It has the blocks they're built
from, and this page builds one — an on/off switch — to show how the blocks
fit: layout and paint for how it looks, listeners for what it does,
animation for how it moves, and accessibility for what a screen reader says.
The finished widget is `examples/switch.py`, which also checks every
behavior below headlessly.

## What it has to do

- Toggle on a click, and on Space or Enter while it's focused.
- Ease its thumb across and its track to the "on" color.
- Grow its thumb a little on hover.
- Show a focus ring when it was reached by keyboard, but not after a click.
- Tell assistive technology it's a switch, what it's called, and whether
  it's on — and toggle when asked to.

## The structure

Two boxes: a rounded track, and a round thumb inside it. The track is the
widget — it's the node that takes focus and carries the accessibility
role — and it lays the thumb out with its own padding:

```python
TRACK_OFF = (0xE6, 0xE0, 0xE9, 0xFF)
TRACK_ON = (0x67, 0x50, 0xA4, 0xFF)
THUMB = (0xFF, 0xFF, 0xFF, 0xFF)
TRAVEL = 20.0  # track width 52 - padding 2*4 - thumb 24


class Switch:
    def __init__(self, window, label, checked=False, on_change=None):
        self.checked = checked
        self.on_change = on_change
        self.node = window.create(
            "box", width=52, height=32, corner_radius=16, padding=4,
            align_items="center", fill=TRACK_OFF,
            shadows=[(NO_RING, 0, 0, 0, 0)],
            role="switch", label=label, checked=checked,
            focusable=True, cursor="pointer",
        )
        self.thumb = window.create(
            "box", width=24, height=24, corner_radius=12, fill=THUMB,
            shadows=[((0, 0, 0, 60), 0, 1, 2, 0)],
        )
        self.node.add_child(self.thumb)
```

`role="switch"`, `label`, and `checked` are all a screen reader needs, and
because the role is button-like, `tre` offers assistive technology an
activate action for it — see [Accessibility](accessibility.md).
`focusable=True` puts it in the Tab order.

## Toggling

Everything that should toggle the switch arrives as one event. A click on
the thumb bubbles up to the track; Space and Enter on a focused node fire
`click`; and an assistive technology's activate request does too. So one
listener covers all of them:

```python
        self.node.on("click", self.toggle)

    def toggle(self):
        self.checked = not self.checked
        self.node.set(checked=self.checked)
        self.paint(duration_ms=200)
        if self.on_change:
            self.on_change(self.checked)
```

Setting `checked` keeps the accessibility state in step with what's on
screen.

## Moving

The thumb slides with `translate_x` rather than a layout change: transforms
animate, and they move what's painted without relaying out anything around
it. The track's `fill` eases at the same time, on the same curve:

```python
EASE = (0.2, 0.0, 0.0, 1.0)

    def paint(self, duration_ms):
        self.thumb.animate("translate_x", TRAVEL if self.checked else 0.0,
                           duration_ms, easing=EASE)
        self.node.animate("fill", TRACK_ON if self.checked else TRACK_OFF,
                          duration_ms, easing=EASE)
```

The constructor calls `self.paint(duration_ms=0)`, so a switch created
`checked=True` starts in place. Because `animate` starts from the current
value, toggling twice quickly reverses the thumb mid-slide rather than
jumping.

## Hover

`pointer_enter` and `pointer_leave` cover the whole switch — they don't fire
as the pointer crosses from the track onto the thumb — so a hover effect
needs just the two:

```python
        self.node.on("pointer_enter", lambda: self.thumb.animate("scale", 1.1, 100))
        self.node.on("pointer_leave", lambda: self.thumb.animate("scale", 1.0, 100))
```

## The focus ring

A ring is a shadow with no blur and a 3px spread — it sits outside the
track, so it never shifts layout. `focus_visible` says whether focus came
from the keyboard, so a mouse user never sees the ring:

```python
RING = (0x67, 0x50, 0xA4, 0x80)
NO_RING = (0, 0, 0, 0)

        self.node.on("focus", self.show_focus)
        self.node.on("unfocus", lambda: self.node.animate(
            "shadows", [(NO_RING, 0, 0, 0, 0)], 100))

    def show_focus(self, event):
        if event.focus_visible:
            self.node.animate("shadows", [(RING, 0, 0, 0, 3)], 100)
```

The track starts with one transparent shadow of the same shape, so the ring
fades in rather than appearing.

## Using it

```python
window = Window(width=320, height=120)
window.root.set(align_items="center", gap=12)
wifi = Switch(window, "Wi-Fi", on_change=lambda on: print("Wi-Fi", on))
window.root.add_child(window.create("text", text="Wi-Fi", width=60, height=20))
window.root.add_child(wifi.node)
```

## Testing it

`window.simulate` drives the real input pipeline and `window.advance` moves
time, so the whole widget is testable without a display:

```python
window.advance(0)                          # pin the clock
window.simulate("click", node=wifi.thumb)  # bubbles to the track
window.advance(200)
assert wifi.node.get("checked") is True
assert wifi.thumb.get("translate_x") == 20.0

window.simulate("key_down", key="space")   # the track is focused now
window.advance(200)
assert wifi.node.get("checked") is False
```

## Going further

The same pieces build the rest of a widget set:

- **A slider** captures the pointer on `pointer_down` so a drag keeps
  tracking off the track, steps on `arrow_left`/`arrow_right` in `key_down`,
  and answers `increment`, `decrement`, and `set_value` in `a11y_action` —
  `examples/slider.py` is one. A `key_down` listener keeps every key for
  itself: while the slider, or anything inside a node with such a listener,
  is focused, the arrows, Page Up/Down, Home, and End no longer scroll the
  scroll view around it, even keys the listener ignores. The switch above
  takes Space and Enter as `click` instead, so those keys still scroll the
  page around it. See
  [Keyboard scrolling](nodes-and-layout.md#keyboard-scrolling).
- **A button's ripple** is a round box inside a `clip_children` button,
  scaled up and faded out from the press point, destroying itself from its
  `on_complete` — `examples/ripple.py`.
- **A menu or dialog** is a [layer](layers.md).
- **A text field** is a `text_input` inside a box that paints the field's
  background, border, and label — see [Text](text.md#text-input).
