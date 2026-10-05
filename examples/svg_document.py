#!/usr/bin/env python3
"""SVG documents: an `svg` node paints a whole SVG file, scaled to its box.

`window.create("svg", svg=<str or bytes>)` parses the document once and draws
it as one scene: shapes, strokes with dashes, linear and radial gradients,
group opacity and clip paths. The first card is the document at its own aspect
ratio; the second is the same document in a wide box (it is fitted, centred,
never stretched). Text, raster images, masks, filters and patterns inside a
document are not drawn. See docs/reference/paint.md.

Headless-CI-safe: renders `max_frames=120` and returns quietly without a
display. Pass `--watch` to keep the window open.
"""

import sys

from tre import App, Window

BADGE = """<svg xmlns="http://www.w3.org/2000/svg" width="120" height="120" viewBox="0 0 120 120">
  <defs>
    <radialGradient id="sun" cx="50%" cy="40%" r="60%">
      <stop offset="0" stop-color="#FFE082"/>
      <stop offset="1" stop-color="#FF6F00"/>
    </radialGradient>
    <clipPath id="disc"><circle cx="60" cy="60" r="52"/></clipPath>
  </defs>
  <circle cx="60" cy="60" r="56" fill="#263238"/>
  <g clip-path="url(#disc)">
    <circle cx="60" cy="48" r="40" fill="url(#sun)"/>
    <rect x="0" y="78" width="120" height="42" fill="#37474F" opacity="0.85"/>
  </g>
  <path d="M14 96 H106" stroke="#80CBC4" stroke-width="3" stroke-dasharray="8 5"
        stroke-linecap="round" fill="none"/>
</svg>"""

window = Window(width=520, height=260, title="tre -- svg")
window.root.set(fill=(0x1C, 0x1B, 0x1F, 0xFF), flex_direction="horizontal", gap=16)

square = window.create("svg", svg=BADGE, width=200, height=200, fill=(0x2B, 0x29, 0x30, 0xFF), corner_radius=16)
wide = window.create("svg", svg=BADGE, width=260, height=120, fill=(0x2B, 0x29, 0x30, 0xFF), corner_radius=16)
for node in (square, wide):
    window.root.add_child(node)

print("document size:", square.get("svg_size"))

app = App()
app.add_window(window)
app.run(max_frames=None if "--watch" in sys.argv else 120)
print("svg_document.py: exited cleanly")
