#!/usr/bin/env python3
"""M30 Phase 6 Step 1's real `Window.add_list_item`/`add_list` (§5,
§7): a plain, non-virtualized list for small real collections --
`VirtualList` (already real since M4/M8) stays the real choice for
large ones, not two unrelated mechanisms.

Demonstrates both real anatomy variants this step built: one-line
items (headline only) and a real two-line item (headline + supporting
text), the latter proving this step's own proactive fix -- the
headline/supporting-text wrapper is opted out of hit-testing via
`Tree::set_hit_testable`, the same real capability `Navigation Rail`
added and `Tabs` already confirmed generalizes.

What this script proves automatically (headless-CI-safe, no human
needed): every item, one-line or two-line, remains independently
clickable once grouped into a real list frame.
"""

from typing import Callable

from tre import App, Window

window = Window(width=400, height=400, title="tre v2 -- list")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

items = [
    window.add_list_item(headline="Inbox", leading_icon="add", supporting_text="12 unread"),
    window.add_list_item(headline="Starred", leading_icon="add"),
    window.add_list_item(headline="Sent", leading_icon="add", trailing_icon="add"),
]

window.add_list(items, width=360)

clicked: list[int] = []


def make_selector(index: int) -> Callable[[], None]:
    def select() -> None:
        clicked.append(index)

    return select


for i, item in enumerate(items):
    item.enable_interaction()
    item.set_on_click(make_selector(i))

window.click(items[0])
assert clicked == [0], "clicking the two-line item must reach its own registered handler"

window.click(items[2])
assert clicked == [0, 2], "each item must remain independently clickable once grouped into a list"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"list.py: exited cleanly after 60 frames, clicked {clicked}")
