#!/usr/bin/env python3
"""M30 Phase 8 Step 4's real `Window.add_pagination` (§5, §7). MD3 has
no official Pagination page (confirmed via the same directory-listing
technique this milestone already uses).

Built as a plain composition -- the same real dividing line
`Segmented Button`/`Tabs`/`Navigation Rail` already established:
"which page is current" is app-owned state, not a new engine-owned
toggle, so `add_pagination` returns one real, independently clickable
`Node` per page plus real prev/next controls, and the app drives live
re-selection itself.

What this script proves automatically (headless-CI-safe, no human
needed): clicking a page number and clicking next each reach their
own registered handler independently.
"""

from typing import Callable

from tre import App, Window

window = Window(width=500, height=100, title="tre v2 -- pagination")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

previous, pages, next_ = window.add_pagination(page_count=5, current=0, x=24, y=24)

current: dict[str, int] = {"page": 0}


def make_selector(index: int) -> Callable[[], None]:
    def select() -> None:
        current["page"] = index

    return select


for i, page in enumerate(pages):
    page.enable_interaction()
    page.set_on_click(make_selector(i))


def go_next() -> None:
    current["page"] = min(current["page"] + 1, len(pages) - 1)


def go_previous() -> None:
    current["page"] = max(current["page"] - 1, 0)


previous.enable_interaction()
previous.set_on_click(go_previous)
next_.enable_interaction()
next_.set_on_click(go_next)

window.click(pages[3])
assert current["page"] == 3, "clicking a page must reach its own registered handler"

window.click(next_)
assert current["page"] == 4, "clicking next must advance the current page"

window.click(previous)
window.click(previous)
assert current["page"] == 2, "clicking previous must retreat the current page"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"pagination.py: exited cleanly after 60 frames, current page {current['page']}")
