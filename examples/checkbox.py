#!/usr/bin/env python3
"""M14 Phase 1's real MD3 checkbox (§5, §7.3): a real, live checkbox
that toggles on click, its own real checkmark animating in/out via
`check_progress` -- `Window.add_checkbox`/`Node.set_checked`/`Node.
animate("check_progress", ...)`, all real this phase.

The engine deliberately never toggles `checked` itself (Design
Principle 6: checked-state is app-owned data, not anything the engine
determines geometrically) -- `on_click` below is exactly what a real
app does: flip `checked`, then animate the real visual consequence.
The already-generic `Click` dispatch and `Node.enable_interaction()`'s
own ripple/state-layer, real since M4, needed zero new wiring to work
on this new `NodeKind`.

What this script proves automatically (headless-CI-safe, no human
needed): a real checkbox, its own real click-to-toggle handler, and a
real triggered `check_progress` animation all run through real frames,
exiting cleanly. The definitive pixel-level proof the checkmark itself
paints correctly is `crates/engine-render/tests/checkbox_paint.rs`,
not this script -- the same split this workspace has used throughout.
"""

from tre import App, Window

window = Window(width=200, height=120, title="tre v2 -- checkbox")

checkbox = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=32, height=32)
checkbox.enable_interaction()


def on_click():
    now_checked = checkbox.get("check_progress") < 0.5
    checkbox.set_checked(now_checked)
    checkbox.animate("check_progress", 1.0 if now_checked else 0.0, duration_ms=150)


checkbox.set_on_click(on_click)
window.click(checkbox)  # a real toggle, right away -- proves the whole chain end to end

app = App()
app.add_window(window)
app.run(max_frames=60)
print("checkbox.py: exited cleanly after 60 frames")
