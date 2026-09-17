#!/usr/bin/env python3
"""M7 Phase 3's real MD3 dynamic color, wired for real (§7.1): a `Window`
built with a real seed color via `Window.set_theme`, then a rect that
opts into interaction (`enable_interaction()`) -- its ripple/hover tint
is now the theme's real "on-surface" role, not the plain hardcoded black
every node painted before this phase.

M20 Phase 1 (§7.1, §7.3) extends the same real mechanism to Checkbox's
checkmark and Slider's track -- both a checkbox/slider created *before*
`set_theme` (re-tinted by the same `Tree::set_all_component_tints` push
`set_all_interaction_tints` already used) and one created *after*
(themed at construction, the real `Node.enable_interaction`-style
precedent) are demonstrated below.

What this script proves automatically (headless-CI-safe, no human
needed): `set_theme`/`enable_interaction`/`click()`, and now `add_
checkbox`/`add_slider` both before and after `set_theme`, all compile
and run through the real pipeline for real frames, exiting cleanly --
the definitive pixel-level proof that each tint is genuinely the
theme's own color (not still hardcoded black/white/gray) is `crates/
engine-render/tests/ripple_hover_dispatch.rs`/`checkbox_paint.rs`/
`slider_paint.rs`'s own new tests, not this script, the same split this
workspace has used throughout (e.g. `elevation.py` / `elevation_
shadow.rs`).
"""

from tre import App, Window

window = Window(width=240, height=160, title="tre v2 -- theme")

# Created before set_theme -- must be re-tinted by the real push below.
early_checkbox = window.add_checkbox(background=(0xFF, 0xFF, 0xFF, 0xFF), width=24, height=24)

window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

# Created after set_theme -- must start genuinely themed already.
late_slider = window.add_slider(background=(0x03, 0xDA, 0xC6, 0xFF), width=180, height=32)

card = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=120, height=80)
card.enable_interaction()
window.click(card)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("theme.py: exited cleanly after 60 frames")
