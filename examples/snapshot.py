#!/usr/bin/env python3
"""Snapshot: read what a window draws, with no display needed.

`window.snapshot()` renders the window's tree offscreen and returns
`(rgba, width, height)`; `tre.write_png` saves it. Here a small card is
saved at 1x and 2x to a temporary folder, and one pixel is read back to
show the colour is the one that was set. Because the same renderer draws a
window and its snapshot, the pictures are what the window shows: use it for
visual regression tests and documentation screenshots.

Headless-CI-safe: nothing opens a window. See docs/reference/window.md.
"""

import tempfile
from pathlib import Path

import tre
from tre import Window

PURPLE = (0x67, 0x50, 0xA4, 0xFF)

window = Window(width=240, height=140)
window.root.set(fill=(0x1C, 0x1B, 0x1F, 0xFF))
card = window.create(
    "box", width=160, height=80, x=40, y=30, position="absolute",
    fill=PURPLE, corner_radius=16,
)
window.root.add_child(card)

folder = Path(tempfile.mkdtemp(prefix="tre-snapshot-"))
for name, scale in (("card.png", 1), ("card@2x.png", 2)):
    rgba, width, height = window.snapshot(scale=scale)
    tre.write_png(folder / name, rgba, width, height)
    print(f"snapshot.py: {name}: {width} x {height}")

rgba, width, _ = window.snapshot()
i = (70 * width + 120) * 4
assert tuple(rgba[i : i + 4]) == PURPLE, "the card's colour is the one that was set"
print("snapshot.py: wrote", folder)
print("snapshot.py: exited cleanly")
