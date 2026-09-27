#!/usr/bin/env python3
"""Docking's bare bones (M99, D10): a left zone holding a red panel, and
an empty right zone. Press the gray handle and drag to the right zone to
move the panel there.

`tre` docks, drags, and reports; the look is the app's own. The handle
is an ordinary rect whose `pointer_down` starts the drag, and the
translucent highlight follows the `dock_target` window event --
`dock_drop` hides it again.

Headless-CI-safe: it renders `max_frames=180` (3s at 60fps) and exits.
A real drag needs a person at the mouse; `tests/test_docking.py` drives
the same path with `window.simulate`.
"""

from tre import App, Window

window = Window(width=320, height=160, title="tre -- docking")

handle = window.add_rect(background=(0x80, 0x80, 0x80, 0xFF), width=24, height=100)
left = window.add_rect(background=(0x22, 0x22, 0x22, 0xFF), width=120, height=100)
right = window.add_rect(background=(0x22, 0x22, 0x22, 0xFF), width=120, height=100)
window.add_dock_zone("left", left, 120.0)
window.add_dock_zone("right", right, 120.0)

panel = window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=120, height=100)
window.dock_panel("left", panel)
handle.on("pointer_down", lambda: window.start_panel_drag(panel))

# The app's own highlight: a translucent overlay laid over the target zone.
highlight = window.create(
    "box",
    fill=(0x00, 0x80, 0xFF, 0x60),
    position="absolute",
    x=0,
    y=0,
    width="100%",
    height="100%",
)
zones = {"left": left, "right": right}


def show_target(event):
    """Moves the highlight into the zone under the pointer, or hides it."""
    highlight.remove()
    if event.side is not None:
        zones[event.side].add_child(highlight)


window.on("dock_target", show_target)
window.on("dock_drop", lambda: highlight.remove())

app = App()
app.add_window(window)
app.run(max_frames=180)
print("docking.py: exited cleanly after 180 frames")
