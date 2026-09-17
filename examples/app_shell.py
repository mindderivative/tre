#!/usr/bin/env python3
"""M13 Phase 1's real `AppShell` composition (§11.2): a real, live
window with a menu bar, a toolbar, a status bar, and one stable
`content` region -- `Window.build_shell` ties three already-built
chrome regions (each an ordinary `add_rect`-built node, the app's own
content) into one real column layout, returning `content`: a real,
empty node the app can `add_child`/populate however it likes, sized to
fill whatever vertical space the given chrome regions don't take.

What this script proves automatically (headless-CI-safe, no human
needed): a real four-region shell (menu bar, toolbar, one screen's
worth of content, status bar) renders through the real `engine-render`
pipeline for real frames, exiting cleanly -- the same "construction +
rendering, not the live interaction itself" honesty `docking.py`/
`resizable_panes.py` already state.
"""

from tre import App, Window

window = Window(width=320, height=220, title="tre v2 -- app shell")

menu_bar = window.add_rect(background=(0x21, 0x21, 0x21, 0xFF), width=320, height=24)
toolbar = window.add_rect(background=(0x33, 0x33, 0x33, 0xFF), width=320, height=32)
status_bar = window.add_rect(background=(0x21, 0x21, 0x21, 0xFF), width=320, height=20)

content = window.build_shell(menu_bar=menu_bar, toolbar=toolbar, status_bar=status_bar)

# The app's own first "screen" -- an ordinary node, attached the same
# way any other add_* result is, via Node.add_child (already real).
screen = window.add_rect(background=(0x18, 0x18, 0x18, 0xFF), width=320, height=100)
content.add_child(screen)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("app_shell.py: exited cleanly after 60 frames")
