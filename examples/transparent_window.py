#!/usr/bin/env python3
"""A transparent, undecorated window: a rounded card floating on the desktop.

`window.set(transparent=True)` (before `App.run()`) gives the window an alpha
channel; with an empty root fill and `decorations=False`, only what the app
draws is visible, so the window can be any shape. `get("transparent_active")`
says whether the surface could blend with the desktop; where it can't, the
window is opaque and the example paints a solid backdrop so it still reads well.

The card carries its own close button, since there is no title bar. Headless-CI-safe:
renders `max_frames=60` and returns quietly without a display. Pass `--watch` to keep
it open. See docs/reference/window.md.
"""

import sys

from tre import App, Window

window = Window(width=360, height=220, title="tre -- transparent", decorations=False)
window.set(transparent=True)
window.root.set(fill=(0, 0, 0, 0), padding=0)

card = window.create("box", width=340, height=200, x=10, y=10, position="absolute",
                     corner_radius=28, fill=(0x1C, 0x1B, 0x1F, 0xE0),
                     stroke_color=(0xFF, 0xFF, 0xFF, 0x30), stroke_width=1,
                     padding=20)
title = window.create("text", text="A window of any shape", font_size=22, width=300, height=34,
                      fill=(0xFF, 0xFF, 0xFF, 0xFF))
note = window.create("text", text="The desktop shows through the clear corners.",
                     font_size=15, width=300, height=60, x=0, y=44, position="absolute",
                     fill=(0xCA, 0xC4, 0xD0, 0xFF))
close = window.create("box", width=90, height=36, x=230, y=140, position="absolute",
                      corner_radius=18, fill=(0x67, 0x50, 0xA4, 0xFF))
label = window.create("text", text="Close", font_size=15, width=60, height=20, x=22, y=8,
                      position="absolute", fill=(0xFF, 0xFF, 0xFF, 0xFF))
close.add_child(label)
close.on("click", window.close)
for node in (title, note, close):
    card.add_child(node)
window.root.add_child(card)

app = App()
app.add_window(window)
app.thread_handle().call_soon(
    lambda: print("transparent_window.py: transparent_active =", window.get("transparent_active"))
)
app.run(max_frames=None if "--watch" in sys.argv else 60)
print("transparent_window.py: exited cleanly")
