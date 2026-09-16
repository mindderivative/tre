#!/usr/bin/env python3
"""M7 Phase 2's real elevation → shadow rendering (§7.2): five real
cards, each at one of MD3's real elevation levels (0 through 5),
animated in from 0 -- `Node.animate("elevation", ...)` already existed
(the property setter itself predates this phase), but `paint_node`
never read `elevation` at all until now, so no node has ever actually
painted a shadow before this phase.

What this script proves automatically (headless-CI-safe, no human
needed): the elevation animation registers and the whole layout renders
through the real pipeline for real frames, exiting cleanly. The
definitive pixel-level proof that a real shadow actually paints, and
that elevation 0 paints none, is `crates/engine-render/tests/
elevation_shadow.rs`, not this script -- the same split this workspace
has used throughout.
"""

from tre import App, Window

window = Window(width=420, height=160, title="tre v2 -- elevation")

for level in range(6):
    card = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=56, height=56)
    card.animate("corner_radius", 8.0, duration_ms=0)
    card.animate("elevation", float(level), duration_ms=800)

app = App()
app.add_window(window)
app.run(max_frames=180)
print("elevation.py: exited cleanly after 180 frames")
