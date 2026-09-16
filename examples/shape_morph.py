#!/usr/bin/env python3
"""M7 Phase 4's real MD3 shape morphing, wired for real (§7.4): a square
rect morphs into a triangle over 800ms via `Node.animate("shape", ...)`
-- `PaintProperties.shape` (an `Animated<ShapeKey>`) existed as pure,
unit-tested math since M3 step 10, but `paint_node` never read it and no
Python API ever set it before this phase.

What this script proves automatically (headless-CI-safe, no human
needed): the morph animation registers and the whole layout renders
through the real pipeline for real frames, exiting cleanly. The
definitive pixel-level proof that a real morph actually paints the
target silhouette (not just the plain rect) is `crates/engine-render/
tests/shape_morph_paint.rs`, not this script -- the same split this
workspace has used throughout (e.g. `elevation.py` / `elevation_shadow.
rs`).
"""

from tre import App, Window

window = Window(width=160, height=160, title="tre v2 -- shape morph")

card = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=100, height=100)
triangle = [(50.0, 0.0), (100.0, 100.0), (0.0, 100.0)]
card.animate("shape", triangle, duration_ms=800)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("shape_morph.py: exited cleanly after 60 frames")
