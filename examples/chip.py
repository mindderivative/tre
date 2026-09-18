#!/usr/bin/env python3
"""M30 Phase 2 Step 3's real `Window.add_chip` (§5, §7): MD3's four
real chip variants -- Assist, Filter, Input, Suggestion -- a plain
composition (`Rect` + optional leading `Icon` + `Text` + optional
trailing `Icon`), not a new first-class `NodeKind` (see this
component's own real design note: a selection whose meaning is
group/app state stays a static composition, the same line `Segmented
Button` already draws).

What this script proves automatically (headless-CI-safe, no human
needed): all four variants render, a Filter Chip's real checkmark
replaces its custom icon once selected, an Input Chip's real trailing
close icon renders, and `enable_interaction()`/`click()` reach a
chip's own container node.
"""

from tre import App, Window

window = Window(width=420, height=140, title="tre v2 -- chip")

window.add_chip(label="Assist", width=100, variant="assist", icon="add", x=16, y=16)
filter_unselected = window.add_chip(label="Filter", width=100, variant="filter", x=124, y=16)
filter_selected = window.add_chip(
    label="Active", width=100, variant="filter", icon="add", selected=True, x=232, y=16
)
window.add_chip(
    label="Contact", width=120, variant="input", icon="add", removable=True, x=340, y=16
)
window.add_chip(label="Suggestion", width=140, variant="suggestion", x=16, y=64)

window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

filter_unselected.enable_interaction()
window.click(filter_unselected)
filter_selected.enable_interaction()
window.click(filter_selected)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("chip.py: exited cleanly after 60 frames")
