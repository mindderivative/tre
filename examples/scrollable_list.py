#!/usr/bin/env python3
"""A scrollable, virtualized list (§11.7): 1,000 rows, of which `tre`
builds only the few its 200px viewport shows -- `materialize(index)`
returns a node for a row as it comes into view, and rows scrolled away
are released. A wheel gesture scrolls it three rows; a scrollbar thumb
paints on its right edge, since the rows overflow the viewport.

Headless-CI-safe: it renders `max_frames=60` and exits. Drag the thumb
with a real mouse to see it live; the pixel proofs are
`crates/engine-render/tests/virtual_list_scroll.rs` and the thumb math
`crates/engine-core/src/tree.rs`'s `virtual_list_thumb_*` tests.
"""

from tre import App, Window

window = Window(width=200, height=200, title="tre -- scrollable list")

ROW_COUNT = 1_000
ROW_HEIGHT = 24.0


def materialize(idx):
    shade = (idx * 37) % 200
    return window.create("box", fill=(shade, shade, 0xFF, 0xFF))


rows = window.create(
    "virtual_list", item_count=ROW_COUNT, item_extent=ROW_HEIGHT, materialize=materialize,
    width=168, height=168,
)
window.root.add_child(rows)
window.scroll(rows, ROW_HEIGHT * 3)  # scroll down three rows

app = App()
app.add_window(window)
app.run(max_frames=60)
print("scrollable_list.py: exited cleanly after 60 frames")
