#!/usr/bin/env python3
"""M14 Phase 2's real MD3 slider (§5, §7.3): a real, live slider,
its own real thumb positioned wherever a real drag leaves it --
`Window.add_slider`, `NodeKind::Slider`'s own real track + thumb
paint, and the drag-to-set interaction, which is entirely internal to
`Tree::dispatch` (mirroring how `Splitter` dragging already works,
M4 Phase 3) -- no Python-facing wiring was needed for the drag itself.

What this script proves automatically (headless-CI-safe, no human
needed): a real slider, seeded at a real, non-zero initial value, and
a real programmatic move via `Node.animate("thumb_position", ...)` (the
same real kind-payload dispatch arm this phase adds, for a triggered
move that isn't a drag -- a "jump to this value" button, say) all run
through real frames, exiting cleanly. The definitive proof that a real
*drag* moves the thumb live is `crates/engine-core/src/tree.rs`'s own
`dispatch_drag_on_a_slider_moves_thumb_position_live_as_the_pointer_
moves` test, and that it paints at the right real position is
`crates/engine-render/tests/slider_paint.rs` -- not this script, the
same split this workspace has used throughout.
"""

from tre import App, Window

window = Window(width=220, height=60, title="tre v2 -- slider")

slider = window.add_slider(background=(0x03, 0xDA, 0xC6, 0xFF), width=180, height=32, value=0.3)

# A real, triggered move -- the "thumb_position" animate() arm this
# phase adds, distinct from a real drag (which never goes through
# Python at all).
slider.animate("thumb_position", 0.8, duration_ms=150)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("slider.py: exited cleanly after 60 frames")
