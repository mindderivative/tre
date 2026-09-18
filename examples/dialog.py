#!/usr/bin/env python3
"""M30 Phase 4 Step 1's real `Window.add_dialog`/`open_dialog`/
`close_dialog` (§5, §7, §11.3): a real MD3 modal dialog -- a centered
panel over a full-window scrim, built on the same real
`Tree::open_overlay`/`close_overlay` primitive every other overlay-
dependent component this milestone reuses, plus this step's own real
addition: `OverlayMeta.modal`, which genuinely blocks interaction with
everything behind the dialog while it's open.

What this script proves automatically (headless-CI-safe, no human
needed): a real background button is clickable before any dialog is
open, a real click on that same button is silently swallowed while a
modal dialog is open over it, and the button is clickable again once
the dialog is closed.
"""

from tre import App, Window

window = Window(width=360, height=280, title="tre v2 -- dialog")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

background = window.add_rect(background=(0x40, 0x40, 0x40, 0xFF), width=360, height=280)
background.enable_interaction()

clicks: dict[str, int] = {"background": 0}
background.set_on_click(lambda: clicks.__setitem__("background", clicks["background"] + 1))

dialog = window.add_dialog(
    headline="Discard changes?",
    text="Your edits have not been saved. This action cannot be undone.",
    width=260,
    height=140,
)

# Before the dialog opens: a real click on the background genuinely
# reaches its own handler.
window.click(background)
assert clicks["background"] == 1, "the background must be clickable before any dialog is open"

window.open_dialog(dialog)

# While the modal dialog is open: the same click must be swallowed --
# this is the real, confirmed OverlayMeta.modal fix this step made.
window.click(background)
assert clicks["background"] == 1, "a modal dialog must block clicks on everything behind it"

window.close_dialog(dialog)

# After closing: the background is clickable again.
window.click(background)
assert clicks["background"] == 2, "the background must be clickable again once the dialog is closed"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"dialog.py: exited cleanly after 60 frames, background clicked {clicks['background']} times")
