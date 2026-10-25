#!/usr/bin/env python3
"""A menu that hangs off a thin bar, as a compositor popup (0.5.8, #223): `PopupWindow`.

A bar is 32 px tall and a `show_layer` menu draws inside its own window, so a menu for the
bar's button needs a surface of its own. A `PopupWindow` is that surface: an `xdg_popup`
parented to the bar, sized to the menu, placed by the compositor beside the button and flipped
above it when the bar is at the bottom edge. A click outside the application, or Escape,
closes it with `closed` reason `"dismissed"`. Item B opens a submenu, a popup whose parent is
the menu.

Needs a Wayland compositor with the layer shell (KDE's KWin, wlroots'); elsewhere
`LayerWindow(...)` raises `tre.UnsupportedPlatform`, which this example reports and returns
from. With no display at all (CI) it builds the bar, the menu and the submenu and checks them
headless with simulated clicks. Pass `--watch` to keep the bar up. See
docs/guide/menus-from-a-bar.md and docs/reference/layer-window.md#popupwindow.
"""

import sys

import tre
from tre import App, LayerWindow, PopupWindow, UnsupportedPlatform

BAR_HEIGHT = 32
BUTTON = (100, 0, 60, BAR_HEIGHT)  # the button's rectangle, in the bar's coordinates

try:
    bar = LayerWindow("example.popupbar", layer="top", anchor=("top", "left", "right"),
                      size=(0, BAR_HEIGHT), exclusive_zone=BAR_HEIGHT)
except UnsupportedPlatform as why:
    print(f"popup_menu.py: this session has no layer shell ({why}); nothing to show")
    sys.exit(0)

bar.root.set(fill=(0x1D, 0x1B, 0x20, 0xF0))
button = bar.create("box", width=BUTTON[2], height=22, x=BUTTON[0], y=5, position="absolute",
                    fill=(0xE8, 0xB4, 0x4C, 0xFF), corner_radius=6)
bar.root.add_child(button)

app = App()
picked = []
menus = []


def make_submenu(menu):
    """A popup whose parent is the menu, hung off item B's right edge."""
    sub = PopupWindow(menu, size=(120, 60), anchor_rect=(0, 42, 160, 30), anchor="top_right",
                      gravity="bottom_right", constraint_adjustment=("flip_x", "slide_y"))
    sub.root.set(fill=(0x1E, 0x5A, 0x8C, 0xFF))
    item = sub.create("box", width=108, height=40, x=6, y=10, position="absolute",
                      fill=(0xD2, 0xDC, 0xF0, 0xFF), corner_radius=6)
    item.on("click", lambda e: (picked.append("B1"), sub.close()))
    sub.root.add_child(item)
    return sub, item


def make_menu():
    menu = PopupWindow(bar, size=(160, 90), anchor_rect=BUTTON, anchor="bottom_left",
                       gravity="bottom_right", constraint_adjustment=("flip_y", "slide_x"))
    menu.root.set(fill=(0x1E, 0x8C, 0x5A, 0xFF))
    items = []
    for row, name in enumerate(("A", "B")):
        item = menu.create("box", width=148, height=30, x=6, y=6 + 36 * row, position="absolute",
                           fill=(0xC8, 0xE6, 0xD2, 0xFF), corner_radius=6)
        menu.root.add_child(item)
        items.append(item)
    return menu, items


def open_menu(e=None):
    if menus:
        return
    menu, (a, b) = make_menu()
    a.on("click", lambda e: (picked.append("A"), menu.close()))

    def open_sub(e):
        sub, _ = make_submenu(menu)
        sub.on("closed", lambda e: None)
        app.add_window(sub)

    b.on("click", open_sub)
    # "dismissed" on an outside click or Escape; "app" after close(): either way it is gone.
    menu.on("closed", lambda e: (menus.clear(), print(f"menu closed: {e.reason}")))
    menu.on("configure", lambda e: print(f"menu placed at ({e.x}, {e.y}), {e.width}x{e.height}"))
    menus.append(menu)
    app.add_window(menu)


button.on("click", open_menu)

# Headless: the same menu and submenu, built on surfaces that are never opened and driven
# with simulated input.
check, items = make_menu()
check.advance(0)
pixels, width, height = check.snapshot()
assert (width, height) == (160, 90), (width, height)
assert check.get("anchor_rect") == BUTTON and check.get("grab") is True
check.simulate("click", node=items[0])  # nothing listens; a click on an item must not raise
sub, sub_item = make_submenu(check)
sub.simulate("click", node=sub_item)
assert picked == ["B1"], picked

if tre.supports("layer_shell"):
    app.add_window(bar)
    app.run(max_frames=None if "--watch" in sys.argv else 90)
    print("popup_menu.py: exited cleanly")
else:
    print("popup_menu.py: no compositor with a layer shell here; checked headless")
