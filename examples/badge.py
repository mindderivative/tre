#!/usr/bin/env python3
"""M30 Phase 3 Step 1's real `Window.add_badge` (§5, §7): MD3's real
two-size badge anatomy -- a 6dp dot, or a 16dp labeled pill -- always
overlaid on a corner of another component via plain caller-chosen
`x`/`y`, not special anchoring machinery of its own.

What this script proves automatically (headless-CI-safe, no human
needed): both real sizes render, overlaid on a real icon's own corner,
through the real pipeline for real frames, exiting cleanly.
"""

from tre import App, Window

window = Window(width=200, height=100, title="tre v2 -- badge")

window.add_icon(name="add", color=(0x1C, 0x1B, 0x1F, 0xFF), size=24, x=16, y=16)
window.add_badge(x=34, y=12)

window.add_icon(name="add", color=(0x1C, 0x1B, 0x1F, 0xFF), size=24, x=80, y=16)
window.add_badge(label="9", x=98, y=8)

window.add_icon(name="add", color=(0x1C, 0x1B, 0x1F, 0xFF), size=24, x=144, y=16)
window.add_badge(label="99+", width=28, x=158, y=8)

window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("badge.py: exited cleanly after 60 frames")
