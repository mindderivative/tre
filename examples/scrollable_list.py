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

M47 (§5, §7, §11.7): this same 1,000-row list now also paints a real
scrollbar thumb (the one real gap M38 Phase 6 left open for `VirtualList`
-- named in M37's own trailer note) -- no script change needed to show
it, since it paints automatically whenever `total_extent() > viewport`,
which this list's own real 1,000 rows already are. What this script
proves automatically (headless-CI-safe, no human needed): the real
scroll gesture above, and the whole pipeline (thumb included) rendering
through real frames without error. The definitive pixel-level proof
that the thumb genuinely paints (and that scrolling shifts/clips
content) is `crates/engine-render/tests/virtual_list_scroll.rs`, and the
real grab/drag math is `crates/engine-core/src/tree.rs`'s own `virtual_
list_thumb_*`/`update_virtual_list_thumb_drag` unit tests -- not this
script, the same split this workspace has used throughout. **What this
script does *not* prove automatically, matching `docking.py`/
`resizable_panes.py`'s own established honesty about the identical real
gap:** an actual mouse drag on the real scrollbar thumb itself -- this
engine has no synthetic Python-level "press at an arbitrary point, then
move" primitive for *any* drag gesture (confirmed via grep before
writing this: `Window.click`/`scroll` are the only synthetic pointer
entry points, neither takes an arbitrary point or a move step), so
every drag-based feature in this catalog (Splitter, Slider, Carousel,
`ScrollView`'s own thumb, and now this one) is proven at the Rust level
and left for a real human to try interactively, not faked here. Run
this script with a real window open and drag the thin bar on the list's
own right edge to see it live.
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
