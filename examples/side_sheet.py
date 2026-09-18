#!/usr/bin/env python3
"""M30 Phase 4 Step 3's real `Window.add_side_sheet`/`open_side_sheet`/
`close_side_sheet` (§5, §7, §11.3): the real desktop counterpart to
Bottom Sheet (excluded as a mobile pattern, this milestone's own
scope).

Demonstrates both real MD3 variants this step built: a *Standard* side
sheet (`modal=False`, the default) -- a plain layout participant,
already attached, that the app re-parents into its own layout exactly
like `Card`'s own content -- and a *Modal* side sheet (`modal=True`) --
a real floating overlay with a full-window scrim that genuinely blocks
interaction with everything behind it, reusing `Dialog`'s own real
`OverlayMeta.modal` capability (Phase 4 Step 1).

What this script proves automatically (headless-CI-safe, no human
needed): a standard side sheet can be re-parented into an app's own
shell without error, and a modal side sheet genuinely blocks a real
background click while open and stops blocking once closed.
"""

from tre import App, Window

window = Window(width=800, height=600, title="tre v2 -- side sheet")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

shell = window.add_rect(background=(0xFF, 0xFB, 0xFE, 0xFF), width=800, height=600)

# Standard: a plain layout participant, already attached -- re-parent
# it into the app's own shell, the same real contract Card's own
# content already has.
standard_sheet = window.add_side_sheet(modal=False, width=280)
shell.add_child(standard_sheet)

# Modal: a real floating overlay with a scrim, built separately.
background_button = window.add_rect(background=(0x40, 0x40, 0x40, 0xFF), width=800, height=600)
background_button.enable_interaction()

clicks: dict[str, int] = {"background": 0}
background_button.set_on_click(lambda: clicks.__setitem__("background", clicks["background"] + 1))

modal_sheet = window.add_side_sheet(modal=True, width=320)

window.click(background_button)
assert clicks["background"] == 1, "the background must be clickable before any modal side sheet is open"

window.open_side_sheet(modal_sheet)

window.click(background_button)
assert clicks["background"] == 1, "a modal side sheet must block clicks on everything behind it"

window.close_side_sheet(modal_sheet)

window.click(background_button)
assert clicks["background"] == 2, "the background must be clickable again once the side sheet is closed"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"side_sheet.py: exited cleanly after 60 frames, background clicked {clicks['background']} times")
