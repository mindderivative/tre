#!/usr/bin/env python3
"""M4 Phase 7's real right-click context menu (§11.3): a real, live
window with one white "anchor" rect. Right-click it to open a small
green menu item below it; click the menu item to see it print.

What this script proves automatically (headless-CI-safe, no human
needed): `Node.set_context_menu`/`Window.right_click` build without
error and the whole layout renders through the real `engine-render`
pipeline for real frames, exiting cleanly -- the same "construction +
rendering, not the live interaction itself" honesty `resizable_panes.
py`/`ripple_button.py` already state. What it does *not* prove
automatically: an actual right-click actually opening the menu on
screen -- that needs a real pointer, which only a human running this
script interactively (or `tests/test_context_menu.py`, which *does*
drive real synthetic `InputEvent`s through the real pipeline without
one) can supply.

`max_frames=180` (3s at 60fps) gives a human a moment to actually try
right-clicking before this exits, matching this workspace's own
headless-CI-safe convention otherwise.
"""

from tre import App, Window

window = Window(width=220, height=120, title="tre v2 -- context menu")

anchor = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=120, height=48)
menu_item = window.add_rect(background=(0x00, 0x80, 0x00, 0xFF), width=100, height=28)
menu_item.set_on_click(lambda: print("menu item selected"))
anchor.set_context_menu(menu_item)

app = App()
app.add_window(window)
app.run(max_frames=180)
print("context_menu.py: exited cleanly after 180 frames")
