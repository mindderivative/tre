#!/usr/bin/env python3
"""M12 Phase 2's real, Python-facing variable-height `VirtualList`
(§11.7), closing Milestone 12: `Window.add_virtual_list`'s `size_hint`
-- a real `Callable[[int], float]`, one row's own real height -- in
place of `item_extent`'s single uniform value. Every row here has a
genuinely different height (a repeating five-step pattern), not a
disguised uniform list.

`size_hint` resolves eagerly, once, for every one of `ROW_COUNT` rows,
right when `add_virtual_list` is called -- a real, stated `O(item_
count)` cost this phase's own `PLAN.md`/`LOG.md` document, the
tradeoff a real cumulative offset for variable-height items requires.
Once resolved, scrolling is exactly as cheap as `scrollable_list.py`'s
own fixed-height list -- only the small visible window is ever a real
`Node`, regardless of `ROW_COUNT`.

What this script proves automatically (headless-CI-safe, no human
needed): a real 500-row list with real, non-uniform row heights, its
own real viewport, a real scroll gesture, and the whole pipeline
render through real frames, exiting cleanly. The definitive proof that
each row's own real on-screen position/height is genuinely non-uniform
(not just "this doesn't crash") is `crates/engine-core/src/tree.rs`'s
own M12 Phase 1 tests -- the same split `scrollable_list.py`'s own doc
comment already states for its sibling pixel test.
"""

from tre import App, Window

window = Window(width=200, height=200, title="tre v2 -- variable-height list")

ROW_COUNT = 500
ROW_HEIGHTS = [16.0, 32.0, 48.0, 24.0, 40.0]  # a real, repeating, non-uniform pattern


def size_hint(idx):
    return ROW_HEIGHTS[idx % len(ROW_HEIGHTS)]


def materialize(idx):
    shade = (idx * 37) % 200
    return (shade, shade, 0xFF, 0xFF)


vl = window.add_virtual_list(
    item_count=ROW_COUNT, materialize=materialize, size_hint=size_hint, height=200
)
window.set_virtual_list_window(vl, 0, 10)
window.scroll(vl, 150.0)  # scroll down into real, non-uniform-height rows

app = App()
app.add_window(window)
app.run(max_frames=60)
print("variable_height_list.py: exited cleanly after 60 frames")
