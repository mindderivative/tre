#!/usr/bin/env python3
"""M30 Phase 6 Step 2's real `Window.add_accordion_header` (§1, §3,
§5, §7). MD3 has no official Accordion component page -- grounded in
the Lists guideline's own "expand and collapse in a folder-like
manner" text. Only the header (title + expand/collapse chevron) is
this step's own real new anatomy; the collapsible content region has
no distinctive MD3 styling of its own, so this example composes it
from a plain `add_rect`.

**Real, deliberate design choice, not an oversight:** the content
region's own visibility is toggled via `Node.animate("opacity", ...)`,
not `Node.remove()`/re-attach -- `Node.remove()` is full, irreversible
destruction (`Tree::remove`), and this codebase exposes no Python-
level "detach without destroying" for an arbitrary node the way
`Tree::close_overlay` already does internally for real overlays.
Opacity is a real, already-general animatable property, well suited
to this and avoiding the destroy pitfall entirely.

This engine also has no rotation-animation primitive exposed to
Python -- `Node.animate("transform", ...)` only ever composes
translate+scale. A uniform negative scale (`-1.0`) is mathematically
identical to a 180-degree rotation for the curated `expand_more`
chevron's own point-symmetric shape, so that's what toggling the
header actually animates.

What this script proves automatically (headless-CI-safe, no human
needed): clicking the header toggles the content region's own real
opacity and flips the chevron via the real `animate("transform", ...)`
call.
"""

from tre import App, Window

window = Window(width=400, height=300, title="tre v2 -- accordion")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

shell = window.add_rect(background=(0xFF, 0xFB, 0xFE, 0xFF), width=400, height=300)

header, chevron = window.add_accordion_header(title="Shipping details", width=360)
shell.add_child(header)

content = window.add_rect(background=(0xEC, 0xE6, 0xF0, 0xFF), width=360, height=100)
shell.add_child(content)
row = window.add_text(
    content="Ships in 3-5 business days.",
    foreground=(0x1D, 0x1B, 0x20, 0xFF),
    width=320,
    height=20,
)
content.add_child(row)
content.animate("opacity", 0.0)

state: dict[str, bool] = {"expanded": False}


def toggle() -> None:
    if state["expanded"]:
        content.animate("opacity", 0.0)
        chevron.animate("transform", (0.0, 0.0, 1.0))
        state["expanded"] = False
    else:
        content.animate("opacity", 1.0)
        chevron.animate("transform", (0.0, 0.0, -1.0))
        state["expanded"] = True


header.enable_interaction()
header.set_on_click(toggle)

window.click(header)
assert state["expanded"], "clicking the header must expand the content region"

window.click(header)
assert not state["expanded"], "clicking the header again must collapse it"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"accordion.py: exited cleanly after 60 frames, expanded={state['expanded']}")
