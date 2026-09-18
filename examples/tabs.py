#!/usr/bin/env python3
"""M30 Phase 5 Step 4's real `Window.add_tabs` (§5, §7): MD3's real
*Primary Navigation Tab* variant -- a row of tabs each with a real
3dp active-indicator bar flush against its own bottom edge.

Built as a plain composition, the same real dividing line `Segmented
Button`/`Filter Chip`/`Navigation Rail`/`Navigation Drawer` already
established: a tab's own "active" state is app-owned group-select
state, not a new engine-owned toggle, so `add_tabs` returns one real,
independently clickable `Node` per tab and the app drives live
re-selection itself.

This step caught a real, confirmed repeat of `Navigation Rail`'s own
hit-test bug: the decorative layout wrapper around each tab's own
icon/label (not the thin indicator itself, which never overlaps a
tab's own geometric center) intercepted clicks meant for the tab --
fixed by reusing the same `Node.hit_testable` opt-out `Navigation
Rail` added.

What this script proves automatically (headless-CI-safe, no human
needed): each tab is independently clickable and reaches only its own
registered handler.
"""

from typing import Callable

from tre import App, Window

window = Window(width=600, height=200, title="tre v2 -- tabs")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

tabs = window.add_tabs(labels=["Recents", "Favorites", "Nearby"], selected=0)

selected: dict[str, int] = {"index": 0}


def make_selector(index: int) -> Callable[[], None]:
    def select() -> None:
        selected["index"] = index

    return select


for i, tab in enumerate(tabs):
    tab.enable_interaction()
    tab.set_on_click(make_selector(i))

window.click(tabs[2])
assert selected["index"] == 2, "clicking a tab must reach its own registered handler"

window.click(tabs[0])
assert selected["index"] == 0, "each tab must remain independently clickable"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"tabs.py: exited cleanly after 60 frames, selected index {selected['index']}")
