#!/usr/bin/env python3
"""M8 Phase 3's real scroll input, wired to `VirtualList` (§11.7),
closing Milestone 8: `Window.add_virtual_list` now gets a real,
meaningful `height` (a real viewport, not the old `auto()` that taffy
resolved against absolutely-positioned children's own zero contribution
to intrinsic size -- a real, in-scope fix this phase made). `Window.
scroll` dispatches a real `InputEvent::Scroll`, which now (M8 Phase 3)
walks up to the nearest `VirtualList` ancestor and moves its own real,
clamped `scroll_offset` -- composed into materialized children's own
paint-time position and clipped for real (M8 Phase 2).

What this script proves automatically (headless-CI-safe, no human
needed): a real 1,000-row list, its own real viewport, a real scroll
gesture, and the whole pipeline render through real frames, exiting
cleanly. The definitive pixel-level proof that scrolling actually
shifts/clips content is `crates/engine-render/tests/
virtual_list_scroll.rs`, not this script -- the same split this
workspace has used throughout.
"""

from tre import App, Window

window = Window(width=200, height=200, title="tre v2 -- scrollable list")

ROW_COUNT = 1_000
ROW_HEIGHT = 24.0


def materialize(idx):
    shade = (idx * 37) % 200
    return (shade, shade, 0xFF, 0xFF)


vl = window.add_virtual_list(
    item_count=ROW_COUNT, item_extent=ROW_HEIGHT, materialize=materialize, height=200
)
window.set_virtual_list_window(vl, 0, 10)
window.scroll(vl, ROW_HEIGHT * 3)  # scroll down three real rows

app = App()
app.add_window(window)
app.run(max_frames=60)
print("scrollable_list.py: exited cleanly after 60 frames")
