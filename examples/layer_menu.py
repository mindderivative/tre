#!/usr/bin/env python3
"""A menu that hangs off a thin bar (0.5.8, #206): the overlay-surface pattern.

A bar is 32 px tall, and a `show_layer` menu draws inside its window's bounds, so a menu
for a bar's button needs a surface of its own. The pattern: when the button is clicked, map
a second `LayerWindow` on the `overlay` layer that covers the output, transparent, with
`input_region="auto"` (clicks anywhere there is nothing drawn reach the windows below) and
`keyboard="exclusive"` (Escape works at once), and show the menu in it with `show_layer`.
`dismissible=True` makes a click outside the menu, or Escape, deliver `dismiss`; while the
menu is open the overlay takes the whole surface so that click is the menu's, not the
window below's. Closing the overlay on dismiss means it costs nothing while no menu is open.
`placement="below"` flips above the button when the bar is at the bottom edge.

Needs a Wayland compositor with the layer shell (KDE's KWin, wlroots'); elsewhere
`LayerWindow(...)` raises `tre.UnsupportedPlatform`, which this example reports and returns
from. With no display at all (CI) it builds both surfaces and checks the menu headless: it
opens, takes an item click, and dismisses on Escape. Pass `--watch` to keep the bar up.
See docs/guide/menus-from-a-bar.md.
"""

import sys

import tre
from tre import App, LayerWindow, UnsupportedPlatform

BAR_HEIGHT = 32

try:
    bar = LayerWindow("example.menubar", layer="top", anchor=("top", "left", "right"),
                      size=(0, BAR_HEIGHT), exclusive_zone=BAR_HEIGHT)
except UnsupportedPlatform as why:
    print(f"layer_menu.py: this session has no layer shell ({why}); nothing to show")
    sys.exit(0)

bar.root.set(fill=(0x1D, 0x1B, 0x20, 0xF0), padding_left=12, align_items="center")
button = bar.create("box", width=64, height=22, fill=(0xE8, 0xB4, 0x4C, 0xFF), corner_radius=6)
bar.root.add_child(button)

state = {"overlay": None, "picked": [], "dismissed": 0}
app = App()


def build_overlay(output=None):
    """The transparent overlay, its menu and the menu's items; `None` is the default output."""
    overlay = LayerWindow("example.menu", layer="overlay", anchor=("top", "bottom", "left", "right"),
                          size=(0, 0), exclusive_zone=-1, keyboard="exclusive",
                          input_region="auto", output=output)
    overlay.root.set(fill=(0, 0, 0, 0))
    # An invisible anchor where the bar's button is, in the overlay's coordinates.
    anchor = overlay.create("box", width=64, height=BAR_HEIGHT, x=12, y=0, position="absolute")
    anchor.set(hit_testable=False)
    overlay.root.add_child(anchor)
    menu = overlay.create("box", width=170, fill=(0x2B, 0x29, 0x30, 0xFF), corner_radius=10,
                          flex_direction="vertical", padding=6, gap=4)
    items = []
    for name in ("Network", "Volume", "Power"):
        item = overlay.create("box", width=158, height=30, fill=(0x4F, 0x37, 0x8B, 0xFF), corner_radius=6)
        item.on("click", lambda e, n=name: (state["picked"].append(n), hide()))
        menu.add_child(item)
        items.append(item)
    menu.on("dismiss", lambda: (state.__setitem__("dismissed", state["dismissed"] + 1), hide()))
    overlay.show_layer(menu, anchor=anchor, placement="below", dismissible=True, flip=True)
    return overlay, menu, items


def show(e=None):
    if state["overlay"] is None:
        state["overlay"] = build_overlay(bar.get("output"))[0]
        app.add_window(state["overlay"])


def hide():
    if state["overlay"] is not None:
        overlay, state["overlay"] = state["overlay"], None
        overlay.close()  # unmaps it: nothing is held while no menu is open


button.on("click", show)

# Headless: the same menu, on a surface that is never opened, driven with simulated input.
check, _menu, items = build_overlay()
check.advance(0)
assert check.get("input_region") == "auto"
check.simulate("click", node=items[0])
assert state["picked"] == ["Network"], state["picked"]
check, _menu, items = build_overlay()
check.advance(0)
check.simulate("key_down", key="escape")
assert state["dismissed"] == 1, state["dismissed"]

if tre.supports("layer_shell"):
    app.add_window(bar)
    app.run(max_frames=None if "--watch" in sys.argv else 90)
    print("layer_menu.py: exited cleanly")
else:
    print("layer_menu.py: no compositor with a layer shell here; checked headless")
