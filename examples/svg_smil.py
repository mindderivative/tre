#!/usr/bin/env python3
"""SMIL animation in an `svg` node: `svg_time` is the seconds into the document.

A spinner that loops for ever and an icon that draws itself on. tre draws the
document as it looks at `svg_time`; `animate("svg_time", ...)` runs that on
tre's own clock, with no Python per frame. See docs/reference/paint.md.

Headless-CI-safe: renders `max_frames=120` and returns quietly without a
display. Pass `--watch` to keep the window open.
"""

import sys

from tre import App, Window

SPINNER = """<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
  <g fill="none" stroke="#80CBC4" stroke-width="2" stroke-linecap="round">
    <path d="M12 3a9 9 0 1 0 9 9" stroke-dasharray="42" stroke-dashoffset="42">
      <animate attributeName="stroke-dashoffset" values="42;14;42" dur="1.6s" repeatCount="indefinite"/>
    </path>
    <animateTransform attributeName="transform" type="rotate" to="360 12 12" dur="2s" repeatCount="indefinite"/>
  </g>
</svg>"""

CHECK = """<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
  <path d="M5 13l4 4L19 7" fill="none" stroke="#A5D6A7" stroke-width="2.4" stroke-linecap="round"
        stroke-dasharray="24" stroke-dashoffset="24">
    <animate attributeName="stroke-dashoffset" to="0" dur="0.6s" fill="freeze"/>
  </path>
</svg>"""

window = Window(width=220, height=120, title="tre -- svg smil")
window.root.set(fill=(0x1C, 0x1B, 0x1F, 0xFF), gap=24, align_items="center", justify_content="center")
window.root.set(flex_direction="horizontal")

spinner = window.create("svg", svg=SPINNER, width=64, height=64)
check = window.create("svg", svg=CHECK, width=64, height=64)
window.root.add_child(spinner)
window.root.add_child(check)

# The spinner has no end (`svg_duration` is None), so it is run through many periods.
spinner.animate("svg_time", 3600.0, 3_600_000, easing="linear")
# The check is drawn on in 0.6 s.
check.animate("svg_time", check.get("svg_duration"), 600, easing="linear")

app = App()
app.add_window(window)
app.run(max_frames=None if "--watch" in sys.argv else 120)
print("svg_smil.py: exited cleanly")
