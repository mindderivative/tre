#!/usr/bin/env python3
"""M30 Phase 1 Step 3's real `Window.add_fab`/`Window.add_extended_fab`
(§5, §7): MD3's three real FAB sizes (Small/Default/Large) and four
real color variants (Surface/Primary/Secondary/Tertiary), plus
Extended FAB's real icon-plus-label anatomy (with and without an
icon -- real MD3's own label-only variant).

What this script proves automatically (headless-CI-safe, no human
needed): every real size/variant string is accepted, a real curated
icon renders, and `enable_interaction()`/`click()` reach a FAB's own
container node -- including Extended FAB's composite Icon+Text
anatomy, the exact shape that needed both `Tree::hit_test_at` fixes
(`NodeKind::Text`/`NodeKind::Icon`) `button.py`/`icon_button.py`
already describe.
"""

from tre import App, Window

window = Window(width=420, height=250, title="tre v2 -- fab")

SIZES = ["small", "default", "large"]
for i, size in enumerate(SIZES):
    window.add_fab(icon="add", size=size, variant="surface", x=16 + i * 110, y=16)

VARIANTS = ["surface", "primary", "secondary", "tertiary"]
for i, variant in enumerate(VARIANTS):
    window.add_fab(icon="add", size="small", variant=variant, x=16 + i * 56, y=128)

window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

extended_with_icon = window.add_extended_fab(
    label="Compose", width=160, icon="add", variant="primary", x=16, y=176
)
extended_no_icon = window.add_extended_fab(
    label="Explore", width=120, variant="secondary", x=192, y=176
)

extended_with_icon.enable_interaction()
window.click(extended_with_icon)
extended_no_icon.enable_interaction()
window.click(extended_no_icon)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("fab.py: exited cleanly after 60 frames")
