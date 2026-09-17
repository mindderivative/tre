#!/usr/bin/env python3
"""M23 Phase 1's real MD3 icon pipeline (§1, §3): `Window.add_icon`
paints a real, curated Material Symbols icon (`engine_md3::icons`) as
a real vector `BezPath` fill -- no raster image, no GPU texture, just
the ordinary `Scene::fill_path` mechanism every solid-color fill
already uses, transformed from the icon's own real fixed SVG source
space (`viewBox="0 -960 960 960"`) into the node's own box.

What this script proves automatically (headless-CI-safe, no human
needed): several real curated icons, at different real sizes/tints,
all load and paint through a real, full `App.run` render loop,
exiting cleanly. The definitive pixel-level proof that the real
transform lands correctly (not flipped, not off-box) is
`crates/engine-render/tests/icon_paint.rs`, not this script.
"""

from tre import App, Window

window = Window(width=280, height=120, title="tre v2 -- icon")

home = window.add_icon(name="home", color=(0x1C, 0x1B, 0x1F, 0xFF), size=48, x=20, y=36)
search = window.add_icon(name="search", color=(0x67, 0x50, 0xA4, 0xFF), size=48, x=90, y=36)
check = window.add_icon(name="check", color=(0x38, 0x8E, 0x3C, 0xFF), size=48, x=160, y=36)
close = window.add_icon(name="close", color=(0xB3, 0x26, 0x1E, 0xFF), size=48, x=230, y=36)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("icon.py: exited cleanly after 60 frames")
