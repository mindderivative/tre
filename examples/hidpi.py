#!/usr/bin/env python3
"""HiDPI: lay out in logical pixels, draw at the display's scale.

`window.set(dpi_scaling=True)` makes a window HiDPI-aware. This card is
written once, in logical pixels, and is the same size on a 1x, 2x or
fractional-scale screen while its text, border and rounded corners are drawn
at the display's full resolution. Off (the default), sizes are physical
pixels, as before.

Prints the scale factor and the logical window size once the window is open.
Headless-CI-safe: renders `max_frames=60` and returns quietly without a
display. Pass `--watch` to keep the window open -- drag it between monitors
of different scales to see it follow. See docs/reference/window.md.
"""

import sys

from tre import App, Window

PURPLE = (0x67, 0x50, 0xA4, 0xFF)
WHITE = (0xFF, 0xFF, 0xFF, 0xFF)

window = Window(width=360, height=220, title="tre -- hidpi")
window.set(dpi_scaling=True)
assert window.get("dpi_scaling") is True

card = window.create(
    "box", width=280, height=120, x=40, y=40, position="absolute",
    fill=PURPLE, corner_radius=16, stroke_color=WHITE, stroke_width=2,
)
window.root.add_child(card)
label = window.create(
    "text", text="Sharp at any scale", font_size=20, fill=WHITE,
    width=240, height=32, x=20, y=44, position="absolute",
)
card.add_child(label)

app = App()
app.add_window(window)
window.on("scale_factor", lambda e: print("hidpi.py: scale factor is now", e.scale_factor))


app.thread_handle().call_soon(
    lambda: print(
        f"hidpi.py: scale_factor={window.get('scale_factor')} "
        f"logical size={window.get('width')}x{window.get('height')}"
    )
)
app.run(max_frames=None if "--watch" in sys.argv else 60)
print("hidpi.py: exited cleanly")
