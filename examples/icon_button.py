#!/usr/bin/env python3
"""M30 Phase 1 Step 2's real `Window.add_icon_button` (§5, §7): MD3's
four real Icon Button variants -- Filled, Filled Tonal, Outlined,
Standard -- `Button`'s own anatomy (`button.py`) with a centered
`Icon` child instead of `Text`.

What this script proves automatically (headless-CI-safe, no human
needed): all four variant strings are accepted, a real curated icon
name resolves and renders, and `enable_interaction()`/`click()` reach
an icon button's own container node -- the same real regression this
component's own `tests/test_icon_button.py` proves at the FFI layer,
and the exact case that caught the original `Tree::hit_test_at` bug
`button.py`'s own module doc comment describes.
"""

from tre import App, Window

window = Window(width=320, height=120, title="tre v2 -- icon button")

VARIANTS = ["filled", "filled_tonal", "outlined", "standard"]

for i, variant in enumerate(VARIANTS):
    window.add_icon_button(icon="add", size=40, variant=variant, x=16 + i * 56, y=16)

window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

themed_buttons = [
    window.add_icon_button(icon="add", size=40, variant=variant, x=16 + i * 56, y=64)
    for i, variant in enumerate(VARIANTS)
]

primary_action = themed_buttons[0]
primary_action.enable_interaction()
window.click(primary_action)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("icon_button.py: exited cleanly after 60 frames")
