#!/usr/bin/env python3
"""M7 Phase 3's real MD3 dynamic color, wired for real (§7.1): a `Window`
built with a real seed color via `Window.set_theme`, then a rect that
opts into interaction (`enable_interaction()`) -- its ripple/hover tint
is now the theme's real "on-surface" role, not the plain hardcoded black
every node painted before this phase.

What this script proves automatically (headless-CI-safe, no human
needed): `set_theme`/`enable_interaction`/`click()` all compile and run
through the real pipeline for real frames, exiting cleanly -- the
definitive pixel-level proof that the tint is genuinely the theme's own
color (not still hardcoded black) is `crates/engine-render/tests/
ripple_hover_dispatch.rs`'s own new test, not this script, the same
split this workspace has used throughout (e.g. `elevation.py` /
`elevation_shadow.rs`).
"""

from tre import App, Window

window = Window(width=240, height=160, title="tre v2 -- theme")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

card = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=120, height=80)
card.enable_interaction()
window.click(card)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("theme.py: exited cleanly after 60 frames")
