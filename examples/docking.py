#!/usr/bin/env python3
"""M4 Phase 9's real docking drag-to-rearrange (§11.4, the final M4
phase): a real, live window with a Left zone (one red panel, with its
own gray drag-handle strip) and an empty Right zone. Press and drag the
gray handle from the left side to the right side to move the panel
there for real.

What this script proves automatically (headless-CI-safe, no human
needed): `Window.add_dock_zone`/`dock_panel`/`set_dock_handle` build a
real docked layout and the whole thing renders through the real
`engine-render` pipeline for real frames, exiting cleanly -- the same
"construction + rendering, not the live interaction itself" honesty
`resizable_panes.py`/`ripple_button.py`/`context_menu.py` already
state. What it does *not* prove automatically: an actual mouse drag
actually moving the panel on screen -- that needs a real pointer, which
only a human running this script interactively (or
`tests/test_docking.py`, which *does* drive the real mechanism directly
without one) can supply.

M10 Phase 3 (§11.4) closed the gap this script's own doc comment used
to name here: a translucent drop-zone highlight now covers whichever
zone is really under the pointer while dragging, via `Window.
set_drop_zone_highlight`/`drag_panel_over` (a human dragging the handle
sees this live; it needs the same real pointer this script already
states it can't supply on its own).

`max_frames=180` (3s at 60fps) gives a human a moment to actually try
dragging before this exits, matching this workspace's own
headless-CI-safe convention otherwise.
"""

from tre import App, Window

window = Window(width=300, height=140, title="tre v2 -- docking drag-to-rearrange")

left_container = window.add_rect(background=(0, 0, 0, 0), width=120, height=100)
right_container = window.add_rect(background=(0x22, 0x22, 0x22, 0xFF), width=120, height=100)
window.add_dock_zone("left", left_container, 120.0)
window.add_dock_zone("right", right_container, 120.0)

handle = window.add_rect(background=(0x80, 0x80, 0x80, 0xFF), width=120, height=20)
panel = window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=120, height=80)
window.dock_panel("left", panel)
window.set_dock_handle(handle, panel)

highlight = window.add_rect(background=(0x00, 0x80, 0xFF, 0x60), width=1, height=1)
window.set_drop_zone_highlight(highlight)

app = App()
app.add_window(window)
app.run(max_frames=180)
print("docking.py: exited cleanly after 180 frames")
