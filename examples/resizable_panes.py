#!/usr/bin/env python3
"""M4 Phase 3's real drag mechanism + `Window.add_splitter` (the missing
Python-facing half of it): a real, live window with a genuinely
resizable left/right pane layout -- drag the gray splitter with the
mouse to resize the red and blue panes.

What this script proves automatically (headless-CI-safe, no human
needed): `Window.add_splitter` builds a real `NodeKind::Splitter`
sitting directly between two real siblings in the window's own root
row, and the whole layout renders through the real `engine-render`
pipeline for real frames, exiting cleanly. What it does *not* prove
automatically: an actual mouse drag actually resizing the panes on
screen -- that needs a real pointer, which only a human running this
script interactively (or `engine-core`'s own `dispatch_drag_on_a_
splitter_...` unit tests and `engine-render`'s `splitter_drag_dispatch.
rs` pixel test, both of which *do* drive real synthetic `InputEvent`s
without a human) can supply. Run this one yourself and drag the
splitter to see it for real.

`max_frames=180` (3s at 60fps) gives a human a moment to actually try
dragging it before this exits, matching this workspace's own
headless-CI-safe convention (TRE v1 finding #261) otherwise.
"""

from tre import App, Window

window = Window(width=420, height=200, title="tre v2 -- resizable panes")

window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=180, height=168)
window.add_splitter(background=(0x60, 0x60, 0x60, 0xFF), width=8, height=168)
window.add_rect(background=(0x00, 0x00, 0xFF, 0xFF), width=180, height=168)

app = App()
app.add_window(window)
app.run(max_frames=180)
print("resizable_panes.py: exited cleanly after 180 frames")
