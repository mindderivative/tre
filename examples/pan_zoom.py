#!/usr/bin/env python3
"""M6 Phase 2's real `transform` exposure from Python (§8): a real, live
window with one node whose own `transform` animates a pan+zoom --
`Node.animate("transform", (translate_x, translate_y, scale),
duration_ms)`, reaching the real, already-composed `PaintProperties.
transform` M5 Phase 1 built. The concrete thing M5 Phase 4 found it
couldn't build (`Node.animate()` had no `transform` property at all),
now real.

A single node's own transform, not a "camera" `Container` wrapping
children -- `Window` has no Python-facing way to create a plain
`Container` yet (checked directly: `add_rect`/`add_splitter`/
`add_virtual_list`/`add_canvas` are the whole list), so this is the
real, honest, currently-buildable shape: the swatch itself pans and
grows, exactly matching §11.9's own "pan offset × zoom scale" text, and
exactly the `Affine::translate((tx, ty)) * Affine::scale(scale)`
product M5 Phase 1's own pixel test already used.

What this script proves automatically (headless-CI-safe, no human
needed): the animation registers without error and the whole layout
renders through the real pipeline for real frames, exiting cleanly. The
underlying paint mechanism itself is already pixel-proven at the Rust
level (`crates/engine-render/tests/transform_composition.rs`) -- what's
new here is only that a live Python call can now reach it at all.
"""

from tre import App, Window

window = Window(width=320, height=240, title="tre v2 -- pan/zoom")

swatch = window.add_rect(background=(0x67, 0x50, 0xA4, 0xFF), width=80, height=80)
swatch.animate("transform", (60.0, 40.0, 1.5), duration_ms=1500)

app = App()
app.add_window(window)
app.run(max_frames=180)
print("pan_zoom.py: exited cleanly after 180 frames")
