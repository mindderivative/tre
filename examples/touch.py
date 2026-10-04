#!/usr/bin/env python3
"""Touch: taps, long presses, pans and pinches, with no touch screen needed.

A card listens for `tap`, `long_press`, `pan` and `pinch`. A touch screen
delivers them from fingers; here `window.simulate("touch_start", ...)` stands
in for the fingers (and `window.advance` for time passing), so it runs
anywhere. Run with a real display and a touch screen to feel it: pass
`--watch` to keep the window open.

See docs/guide/events-and-input.md.
"""

import sys

from tre import App, Window

window = Window(width=360, height=240, title="tre -- touch")
card = window.create("box", width=200, height=140, x=80, y=50, position="absolute",
                     fill=(0x67, 0x50, 0xA4, 0xFF), corner_radius=16)
window.root.add_child(card)

state = {"x": 0.0, "y": 0.0, "scale": 1.0}


def apply() -> None:
    card.set(translate_x=state["x"], translate_y=state["y"], scale=state["scale"])


card.on("tap", lambda e: print(f"touch.py: tap x{e.count}"))
card.on("long_press", lambda: card.animate("fill", (0xB3, 0x26, 0x1E, 0xFF), 200))


def pan(e) -> None:
    state["x"] += e.delta_x
    state["y"] += e.delta_y
    apply()


def pinch(e) -> None:
    state["scale"] = max(0.3, min(3.0, state["scale"] * e.scale_delta))
    apply()


card.on("pan", pan)
card.on("pinch", pinch)

if "--watch" in sys.argv:
    app = App()
    app.add_window(window)
    app.run()
else:
    # Drive it with simulated fingers.
    window.simulate("touch_start", x=180, y=120)
    window.simulate("touch_end", x=180, y=120)                      # a tap
    window.simulate("touch_start", x=180, y=120)
    window.advance(600)                                             # a long press
    window.simulate("touch_end", x=180, y=120)
    window.simulate("touch_start", x=150, y=100)                    # a pan
    window.simulate("touch_move", x=190, y=110)
    window.simulate("touch_move", x=230, y=130)
    window.simulate("touch_end", x=230, y=130)
    window.simulate("touch_start", x=150, y=120, id=1)              # a pinch
    window.simulate("touch_start", x=250, y=120, id=2)
    window.simulate("touch_move", x=330, y=120, id=2)
    window.simulate("touch_end", x=330, y=120, id=2)
    window.simulate("touch_end", x=150, y=120, id=1)
    print(f"touch.py: moved to ({state['x']:.0f}, {state['y']:.0f}), scale {state['scale']:.2f}")
    assert state["x"] > 40 and state["scale"] > 1.5
    print("touch.py: exited cleanly")
