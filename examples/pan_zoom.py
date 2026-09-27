#!/usr/bin/env python3
"""Pan and zoom: a node's `translate_x`, `translate_y`, and `scale` each
animate on their own, so easing one never disturbs another. The swatch
pans 60px right and 40px down while it grows to 1.5x about its center.

Headless-CI-safe: it renders `max_frames=180` (3s at 60fps) and exits;
`App.run()` returns quietly where no display or GPU is reachable. The
pixel proof that transforms compose is
`crates/engine-render/tests/transform_composition.rs`.
"""

from tre import App, Window

window = Window(width=320, height=240, title="tre -- pan/zoom")

swatch = window.create("box", fill=(0x67, 0x50, 0xA4, 0xFF), width=80, height=80)
window.root.add_child(swatch)
EASE = (0.2, 0.0, 0.0, 1.0)
swatch.animate("translate_x", 60.0, duration_ms=1500, easing=EASE)
swatch.animate("translate_y", 40.0, duration_ms=1500, easing=EASE)
swatch.animate("scale", 1.5, duration_ms=1500, easing=EASE)

app = App()
app.add_window(window)
app.run(max_frames=180)
print("pan_zoom.py: exited cleanly after 180 frames")
