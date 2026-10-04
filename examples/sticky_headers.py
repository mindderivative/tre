#!/usr/bin/env python3
"""Sticky section headers in a scroll view.

Each section has a header with `sticky=0`: it scrolls with its section until it
reaches the top of the view, holds there while the section scrolls past, and is
pushed out by the next section. A header needs a `z_index` to stay above the rows
that scroll under it.

Headless-CI-safe: renders `max_frames=60` and returns quietly without a display.
Pass `--watch` to scroll it with the wheel. See docs/guide/nodes-and-layout.md.
"""

import sys

from tre import App, Window

window = Window(width=320, height=300, title="tre -- sticky headers")
window.root.set(padding=0)
view = window.create("scroll_view", width=320, height=300)
window.root.add_child(view)

SECTIONS = ["Fruit", "Vegetables", "Grains", "Dairy", "Pantry"]
ROW, HEADER = 36, 32
height = len(SECTIONS) * (HEADER + 6 * ROW)
content = window.create("box", width=320, height=height, fill=(0xFF, 0xFB, 0xFE, 0xFF))
view.add_child(content)

for s, title in enumerate(SECTIONS):
    top = s * (HEADER + 6 * ROW)
    section = window.create("box", width=320, height=HEADER + 6 * ROW, x=0, y=top, position="absolute")
    content.add_child(section)
    header = window.create("box", width=320, height=HEADER, x=0, y=0, position="absolute",
                           fill=(0x67, 0x50, 0xA4, 0xFF), z_index=1, sticky=0)
    header.add_child(window.create("text", text=title, font_size=16, width=280, height=24, x=16, y=6,
                                   position="absolute", fill=(0xFF, 0xFF, 0xFF, 0xFF)))
    section.add_child(header)
    for r in range(6):
        row = window.create("box", width=320, height=ROW, x=0, y=HEADER + r * ROW, position="absolute",
                            fill=(0xF3, 0xED, 0xF7, 0xFF) if r % 2 else (0xFF, 0xFB, 0xFE, 0xFF))
        row.add_child(window.create("text", text=f"{title} item {r + 1}", font_size=15, width=280,
                                    height=22, x=16, y=7, position="absolute",
                                    fill=(0x1C, 0x1B, 0x1F, 0xFF)))
        section.add_child(row)

view.set(scroll_offset=HEADER + 6 * ROW + 80)  # into the second section
app = App()
app.add_window(window)
app.run(max_frames=None if "--watch" in sys.argv else 60)
print("sticky_headers.py: exited cleanly")
