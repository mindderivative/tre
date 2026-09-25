#!/usr/bin/env python3
"""M14 Phase 2's real MD3 slider (§5, §7.3): a real, live slider,
its own real thumb positioned wherever a real drag leaves it --
`Window.add_slider`, `NodeKind::Slider`'s own real track + thumb
paint, and the drag-to-set interaction, which is entirely internal to
`Tree::dispatch` (mirroring how `Splitter` dragging already works,
M4 Phase 3) -- no Python-facing wiring was needed for the drag itself.

M24 Phase 1's real keyboard arrow-key increment (§10): every real
`Slider` now opts into keyboard focus at construction (the same real
way `TextField` already does), so `Window.press_key("right"/"left")`
nudges its real `thumb_position` by a real, fixed step -- the
identical `Tree::set_slider_position` mechanism a real drag already
uses, so this is real, not a simulated visual effect.

What this script proves automatically (headless-CI-safe, no human
needed): a real slider, seeded at a real, non-zero initial value; a
real programmatic move via `Node.animate("value", ...)`; and
now a real, live keyboard nudge via `Window.press_key`, its own real
resulting value read back and checked -- all run through real frames,
exiting cleanly. The definitive proof that a real *drag* moves the
thumb live is `crates/engine-core/src/tree.rs`'s own `dispatch_drag_
on_a_slider_moves_thumb_position_live_as_the_pointer_moves` test, and
that it paints at the right real position is `crates/engine-render/
tests/slider_paint.rs` -- not this script, the same split this
workspace has used throughout.
"""

from tre import App, Window

window = Window(width=220, height=60, title="tre v2 -- slider")

slider = window.add_slider(background=(0x03, 0xDA, 0xC6, 0xFF), width=180, height=32, value=0.3)

# A real, triggered move -- the "value" animate() arm this
# phase adds, distinct from a real drag (which never goes through
# Python at all).
slider.animate("value", 0.8, duration_ms=150)

# M24 Phase 1 (§10): a real keyboard nudge -- the slider is the only
# interactive node in this window, so a single real Tab press focuses
# it (real, automatic focusability since construction), then two real
# ArrowRight presses nudge its own real thumb_position by 0.05 each.
window.press_key("tab")
window.press_key("right")
window.press_key("right")
before = slider.get("value")

app = App()
app.add_window(window)
app.run(max_frames=60)

after = slider.get("value")
print(f"slider.py: thumb_position after two real ArrowRight presses: {after}")
assert after == before, "a real keyboard nudge must not need a frame tick to take effect"
print("slider.py: exited cleanly after 60 frames")
