#!/usr/bin/env python3
"""A bar (0.5.8): a `LayerWindow` is a window the compositor places, not the user.

It anchors to the top edge, stretches across it, and reserves 32 pixels, so maximised
windows stop below it. A swatch, a label and a clock: the clock is `bar.every(1000, ...)`
on the window's own clock, like any window's timer.

Needs a Wayland compositor with the layer shell (KDE's KWin, wlroots'); on X11, GNOME's
Mutter, Windows and macOS `LayerWindow(...)` raises `tre.UnsupportedPlatform`, which this
example reports and returns from. With no display at all (CI) it builds the bar and checks
it headless, like every example here. Pass `--watch` to keep the bar up until you close it
(`bar.close()`, or kill the script). See docs/guide/bars-and-panels.md.
"""

import sys

import tre
from tre import App, LayerWindow, UnsupportedPlatform

try:
    bar = LayerWindow(
        "example.bar",
        layer="top",
        anchor=("top", "left", "right"),
        size=(0, 32),       # 0 stretches between the two anchored sides
        exclusive_zone=32,  # maximised windows stop below it
    )
except UnsupportedPlatform as why:
    print(f"layer_bar.py: this session has no layer shell ({why}); nothing to show")
    sys.exit(0)

bar.root.set(
    fill=(0x1D, 0x1B, 0x20, 0xF0),
    flex_direction="horizontal",
    align_items="center",
    padding_left=12,
    padding_right=12,
    gap=10,
)
swatch = bar.create("box", width=16, height=16, fill=(0xE8, 0xB4, 0x4C, 0xFF), corner_radius=8)
title = bar.create("text", text="tre", font_size=14, font_weight=600, fill=(255, 255, 255, 255),
                   width="fit-content", height="fit-content")
spacer = bar.create("box", flex_grow=1)
clock = bar.create("text", text="00:00:00", font_size=14, fill=(0xCA, 0xC4, 0xD0, 0xFF),
                   width="fit-content", height="fit-content")
for node in (swatch, title, spacer, clock):
    bar.root.add_child(node)

seconds = {"n": 0}


def tick():
    seconds["n"] += 1
    n = seconds["n"]
    clock.set(text=f"{n // 3600:02d}:{n // 60 % 60:02d}:{n % 60:02d}")


bar.every(1000, tick)

# Headless: the bar is a window with a clock and a snapshot, with or without a compositor.
bar.advance(3000)
assert clock.get("text") == "00:00:03", clock.get("text")
pixels, width, height = bar.snapshot()
assert height == 32 and width > 0 and len(pixels) == width * height * 4

if tre.supports("layer_shell"):
    app = App()
    app.add_window(bar)
    app.run(max_frames=None if "--watch" in sys.argv else 90)
    print("layer_bar.py: exited cleanly")
else:
    print("layer_bar.py: no compositor with a layer shell here; checked headless")
