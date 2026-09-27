#!/usr/bin/env python3
"""Nodes and layout: an app frame built from boxes -- a toolbar across
the top, a fixed-width sidebar, and a content area that takes the rest,
with a badge placed absolutely over a toolbar button. Reading
`layout_*` runs layout on demand, so the script checks the geometry
before any frame renders.

Headless-CI-safe: it checks the layout, renders `max_frames=60`, and
exits; `App.run()` returns quietly where no display or GPU is reachable.
See docs/guide/nodes-and-layout.md.
"""

from tre import App, Window

SURFACE = (0xFE, 0xF7, 0xFF, 0xFF)
BAR = (0xF3, 0xED, 0xF7, 0xFF)
ACCENT = (0x67, 0x50, 0xA4, 0xFF)

window = Window(width=480, height=300, title="tre -- layout")
window.root.set(flex_direction="vertical", padding=0, gap=0, fill=SURFACE)

toolbar = window.create("box", width="100%", height=48, padding=8, gap=8,
                        align_items="center", fill=BAR)
title = window.create("text", text="Inbox", font_size=18, width=80, height=24)
spacer = window.create("box", flex_grow=1)  # pushes the button to the end
button = window.create("box", width=32, height=32, corner_radius=16, fill=ACCENT)
badge = window.create("box", position="absolute", x=22, y=-4, width=14, height=14,
                      corner_radius=7, fill=(0xB3, 0x26, 0x1E, 0xFF))
button.add_child(badge)
for node in (title, spacer, button):
    toolbar.add_child(node)

body = window.create("box", width="100%", flex_grow=1)
sidebar = window.create("box", width=120, flex_shrink=0, fill=BAR,
                        flex_direction="vertical", padding=8, gap=4)
for name in ("Inbox", "Sent", "Drafts"):
    sidebar.add_child(window.create("text", text=name, width=100, height=20))
content = window.create("box", flex_grow=1, padding=16)
body.add_child(sidebar)
body.add_child(content)

window.root.add_child(toolbar)
window.root.add_child(body)

# -- checks ----------------------------------------------------------------
assert toolbar.get("layout_width") == 480 and toolbar.get("layout_height") == 48
assert button.get("layout_x") == 480 - 8 - 32, "the spacer pushes the button to the end"
assert badge.get("layout_x") == button.get("layout_x") + 22, "absolute: placed from its parent"
assert sidebar.get("layout_width") == 120
assert content.get("layout_x") == 120 and content.get("layout_width") == 360
assert content.get("layout_height") == 300 - 48, "the body grows into the rest"
print("layout.py: layout checks passed")

app = App()
app.add_window(window)
app.run(max_frames=60)
print("layout.py: exited cleanly")
