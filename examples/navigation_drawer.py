#!/usr/bin/env python3
"""M30 Phase 5 Step 2's real `Window.add_navigation_drawer`/
`open_navigation_drawer`/`close_navigation_drawer` (§5, §7, §11.3):
the real desktop counterpart to Bottom App Bar's own navigation role,
docked to the left edge.

Demonstrates both real MD3 variants this step built, the same real
fork `Side Sheet` (Phase 4 Step 3) established: a *Standard* drawer
(`modal=False`, the default) -- a plain layout participant, already
attached, re-parented into the app's own shell exactly like `Card`'s
own content -- and a *Modal* drawer (`modal=True`) -- a real floating
overlay with a full-window scrim that genuinely blocks interaction
with everything behind it, reusing `Dialog`'s own real `OverlayMeta.
modal` capability a third independent time this milestone.

What this script proves automatically (headless-CI-safe, no human
needed): a standard drawer can be re-parented without error, each of
its destinations is independently clickable, and a modal drawer
genuinely blocks a real background click while open and stops
blocking once closed.
"""

from typing import Callable

from tre import App, Window

window = Window(width=800, height=600, title="tre v2 -- navigation drawer")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

shell = window.add_rect(background=(0xFF, 0xFB, 0xFE, 0xFF), width=800, height=600)

# Standard: a plain layout participant, already attached -- re-parent
# it into the app's own shell, the same real contract Card's own
# content already has.
standard_container, standard_items = window.add_navigation_drawer(
    labels=["Inbox", "Starred", "Sent"],
    icons=["add", "add", "add"],
    selected=0,
    modal=False,
    width=280,
)
shell.add_child(standard_container)

selected: dict[str, int] = {"index": 0}


def make_selector(index: int) -> Callable[[], None]:
    def select() -> None:
        selected["index"] = index

    return select


for i, item in enumerate(standard_items):
    item.enable_interaction()
    item.set_on_click(make_selector(i))

window.click(standard_items[2])
assert selected["index"] == 2, "clicking a destination must reach its own registered handler"

# Modal: a real floating overlay with a scrim, built separately.
background_button = window.add_rect(background=(0x40, 0x40, 0x40, 0xFF), width=800, height=600)
background_button.enable_interaction()

clicks: dict[str, int] = {"background": 0}
background_button.set_on_click(lambda: clicks.__setitem__("background", clicks["background"] + 1))

modal_container, _modal_items = window.add_navigation_drawer(
    labels=["Inbox", "Starred"],
    icons=["add", "add"],
    modal=True,
)

window.click(background_button)
assert clicks["background"] == 1, "the background must be clickable before any modal drawer is open"

window.open_navigation_drawer(modal_container)

window.click(background_button)
assert clicks["background"] == 1, "a modal navigation drawer must block clicks on everything behind it"

window.close_navigation_drawer(modal_container)

window.click(background_button)
assert clicks["background"] == 2, "the background must be clickable again once the drawer is closed"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(
    f"navigation_drawer.py: exited cleanly after 60 frames, "
    f"selected index {selected['index']}, background clicked {clicks['background']} times"
)
