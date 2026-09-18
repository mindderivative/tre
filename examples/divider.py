#!/usr/bin/env python3
"""M30 Phase 3 Step 4's real `Window.add_divider` (§5, §7): MD3's real
1dp separator line, horizontal or vertical.

What this script proves automatically (headless-CI-safe, no human
needed): both orientations render through the real pipeline for real
frames, exiting cleanly.
"""

from tre import App, Window

window = Window(width=200, height=100, title="tre v2 -- divider")

window.add_divider(length=168, x=16, y=40)
window.add_divider(length=68, vertical=True, x=100, y=16)

window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("divider.py: exited cleanly after 60 frames")
