#!/usr/bin/env python3
"""Two windows from one `App`: each `Window` has its own tree, size, and
title, and `App.run()` opens and drives them together, returning when
both have closed.

Headless-CI-safe: `max_frames=60` applies to each window, and `App.run()`
returns quietly without a display or GPU.
"""

from tre import App, Window

main_window = Window(width=320, height=160, title="tre -- main")
main_swatch = main_window.create("box", fill=(0x67, 0x50, 0xA4, 0xFF), width=100, height=100)
main_window.root.add_child(main_swatch)
main_swatch.animate("opacity", 0.3, duration_ms=800)

panel_window = Window(width=220, height=120, title="tre -- panel")
panel_swatch = panel_window.create("box", fill=(0x03, 0xDA, 0xC6, 0xFF), width=80, height=80)
panel_window.root.add_child(panel_swatch)
panel_swatch.animate("corner_radius", 20.0, duration_ms=800)

app = App()
app.add_window(main_window)
app.add_window(panel_window)
app.run(max_frames=60)
print("two_windows.py: exited cleanly after 60 frames, both windows")
