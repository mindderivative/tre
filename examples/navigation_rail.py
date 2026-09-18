#!/usr/bin/env python3
"""M30 Phase 5 Step 1's real `Window.add_navigation_rail` (§5, §7):
the real desktop counterpart to Navigation Bar (excluded as a mobile
pattern, this milestone's own scope).

Built as a plain composition -- the same real dividing line
`Segmented Button`/`Filter Chip` already established: a rail's own
"active item" is app-owned group-select state, not a new engine-owned
toggle, so `add_navigation_rail` returns one real, independently
clickable `Node` per item and the app drives live re-selection itself,
exactly like `Segmented Button`'s own example does.

This step also found and fixed a real, confirmed engine-core gap: the
active-indicator pill (a decorative interior `Rect` behind the icon)
sat squarely over each item's own geometric center and silently ate
every click meant for the item's own registered handler, since a
plain `Rect` always independently claims a hit and nothing bubbles
back out to an ancestor. The fix -- a new, purely additive `Node.
hit_testable` opt-out -- is exercised for real by every click below.

What this script proves automatically (headless-CI-safe, no human
needed): each rail item is independently clickable and reaches only
its own registered handler.
"""

from typing import Callable

from tre import App, Window

window = Window(width=400, height=500, title="tre v2 -- navigation rail")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

items = window.add_navigation_rail(
    labels=["Home", "Search", "Profile"],
    icons=["add", "add", "add"],
    selected=0,
)

selected: dict[str, int] = {"index": 0}


def make_selector(index: int) -> Callable[[], None]:
    def select() -> None:
        selected["index"] = index

    return select


for i, item in enumerate(items):
    item.enable_interaction()
    item.set_on_click(make_selector(i))

window.click(items[2])
assert selected["index"] == 2, "clicking an item must reach its own registered handler"

window.click(items[0])
assert selected["index"] == 0, "each item must remain independently clickable"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"navigation_rail.py: exited cleanly after 60 frames, selected index {selected['index']}")
