#!/usr/bin/env python3
"""M4 Phase 5's real ripple/hover state layer (§7.3): a real, live
window with one white button that opts into interaction. Move the
mouse over it to see a hover tint; click it to see a ripple.

What this script proves automatically (headless-CI-safe, no human
needed): `Node.enable_interaction` builds without error and the whole
layout renders through the real `engine-render` pipeline for real
frames, exiting cleanly -- the same "construction + rendering, not the
live interaction itself" honesty `resizable_panes.py` already states.
What it does *not* prove automatically: an actual mouse hover/click
actually showing a tint/ripple on screen -- that needs a real pointer,
which only a human running this script interactively (or
`engine-render`'s own `ripple_hover_dispatch.rs`, which *does* drive
real synthetic `InputEvent`s through the real pipeline and reads the
resulting pixels back, without a human) can supply.

`max_frames=180` (3s at 60fps) gives a human a moment to actually try
hovering/clicking before this exits, matching this workspace's own
headless-CI-safe convention otherwise.
"""

from tre import App, Window

window = Window(width=200, height=100, title="tre v2 -- ripple/hover")

button = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=120, height=48)
button.enable_interaction()

app = App()
app.add_window(window)
app.run(max_frames=180)
print("ripple_button.py: exited cleanly after 180 frames")
