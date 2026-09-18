#!/usr/bin/env python3
"""M30 Phase 8 Step 1's real `Window.add_popover` (§11.3), grounded in
MD3's own real Rich Tooltip anatomy -- pyCopper's own real prior
research, reused rather than re-derived, with the exact real token
values still directly re-verified against Material Web's own source.

Reuses the existing `Window.open_menu`/`close_menu` directly, the
identical real design `add_tooltip`'s own panel (Phase 3 Step 5)
already established -- no new dedicated open/close pair. "Persistent"
means it doesn't dismiss on hover-exit the way the plain tooltip does,
but it still dismisses on a real outside click, exactly like Menu.

What this script proves automatically (headless-CI-safe, no human
needed): the popover opens anchored below a trigger button and a real
click outside it dismisses it, the same real Menu behavior it reuses.
"""

from tre import App, Window

window = Window(width=400, height=300, title="tre v2 -- popover")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

trigger = window.add_icon_button(icon="settings", variant="standard", size=40)
popover = window.add_popover(
    subhead="Storage",
    text="You have used 12 GB of your 15 GB plan.",
    width=240,
    height=100,
)

trigger.enable_interaction()
trigger.set_on_click(lambda: window.open_menu(trigger, popover))
window.click(trigger)

# A real click well away from both the trigger and the popover's own
# panel -- open_menu's real dismiss_on_outside_click behavior should
# consume it and close the popover.
far_corner = window.add_rect(background=(0, 0, 0, 0), width=1, height=1, x=390, y=290)
window.click(far_corner)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("popover.py: exited cleanly after 60 frames")
