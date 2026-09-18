#!/usr/bin/env python3
"""M30 Phase 1's real `Window.add_button` (§5, §7): MD3's five real
button variants -- Elevated, Filled, Filled Tonal, Outlined, Text --
each built once through `add_button` instead of hand-composed from
`Rect`+`Text`+ripple the way `ripple_button.py` still shows the old way
to do it. One row created before `set_theme` (must paint `add_button`'s
own real historical baseline default -- see `window_factory.rs`'s
`ButtonBaseline`), a second row created after (must start genuinely
themed already, the same real construction-time precedent `theme.py`
already established for `Checkbox`/`Slider`/`TextField`).

What this script proves automatically (headless-CI-safe, no human
needed): all five variant strings are accepted, `enable_interaction()`
+ `click()` work on a button's returned node exactly like any other
node, and the whole tree renders through the real pipeline for real
frames, exiting cleanly. The definitive pixel-level proof that each
variant's container/label/border color is genuinely resolved (not
still the plain historical default after a real theme is set, and that
`Outlined`'s border and `Elevated`'s shadow are real) is `engine-py`'s
own future `Button`-specific Rust test, not this script -- the same
split this workspace applies throughout (e.g. `theme.py` /
`checkbox_paint.rs`).
"""

from tre import App, Window

window = Window(width=760, height=160, title="tre v2 -- button")

VARIANTS = ["elevated", "filled", "filled_tonal", "outlined", "text"]
BUTTON_WIDTH = 140
BUTTON_HEIGHT = 40

# Un-themed row -- must paint add_button's own real historical default,
# the same "real color, not black" contract Checkbox/Slider establish.
for variant in VARIANTS:
    window.add_button(
        label=variant.replace("_", " ").title(),
        width=BUTTON_WIDTH,
        height=BUTTON_HEIGHT,
        variant=variant,
    )

window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

# Themed row -- created after set_theme, must start genuinely themed.
themed_buttons = [
    window.add_button(
        label=variant.replace("_", " ").title(),
        width=BUTTON_WIDTH,
        height=BUTTON_HEIGHT,
        variant=variant,
    )
    for variant in VARIANTS
]

# A button is a real node like any other -- opts into ripple/hover and
# click dispatch exactly the same way `ripple_button.py`'s hand-composed
# rect already did.
primary_action = themed_buttons[1]
primary_action.enable_interaction()
window.click(primary_action)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("button.py: exited cleanly after 60 frames")
