#!/usr/bin/env python3
"""Building a tree from one description: `window.build(spec)` and `window.apply(changes)`.

`build` makes a whole detached subtree from nested dicts and returns the root and the nodes that
have an `id`; `apply` changes many nodes in one checked call. Both check everything first, so a
typo makes nothing and changes nothing. See docs/reference/properties.md.

Headless-CI-safe: renders `max_frames=120` and returns quietly without a display. Pass `--watch`
to keep the window open.
"""

import sys

from tess_re import App, Window

window = Window(width=360, height=120, title="tess_re -- build")
window.root.set(fill=(0x1C, 0x1B, 0x1F, 0xFF), align_items="center", justify_content="center")

root, nodes = window.build({
    "kind": "box",
    "props": {"flex_direction": "horizontal", "gap": 16, "padding": 12,
              "fill": (0x2B, 0x29, 0x30, 0xFF), "corner_radius": 12},
    "children": [
        {"kind": "text", "id": "battery", "props": {"text": "Battery 87%", "fill": (0xE6, 0xE1, 0xE5, 0xFF)}},
        {"kind": "text", "id": "clock", "props": {"text": "14:05", "fill": (0xE6, 0xE1, 0xE5, 0xFF)}},
    ],
})
window.root.add_child(root)

try:
    window.build({"kind": "box", "children": [{"kind": "text", "props": {"text": "ok"}}, {"kind": "text", "props": {"text": 3}}]})
except ValueError as err:
    print("a typo makes nothing:", err)

window.apply([(nodes["battery"], {"text": "Battery 86%"}), (nodes["clock"], {"text": "14:06"})])
print("battery:", nodes["battery"].get("text"), "clock:", nodes["clock"].get("text"))

app = App()
app.add_window(window)
app.run(max_frames=None if "--watch" in sys.argv else 120)
print("build_tree.py: exited cleanly")
