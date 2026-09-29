#!/usr/bin/env python3
"""The showcase: a settings window built only from `tre`'s building blocks.

- Switches and a volume slider -- the widgets from `switch.py` and
  `slider.py`, imported from them -- with the slider's value shown live.
- A name field: a `text_input` in a box that outlines on focus.
- A "Sort" button with a press ripple that reorders a keyed list in place.
- An overflow menu, anchored to its button, and an About dialog -- both
  layers, the dialog modal over a scrim.

Everything is keyboard-reachable and labeled for assistive technology.
The script drives each part with `window.simulate` and `window.advance`,
then opens the window. Headless-CI-safe: `App.run()` renders
`max_frames=120` and returns quietly without a display. Run it
from anywhere: `python examples/showcase.py`.
"""

from slider import Slider
from switch import Switch
from tre import App, Event, Window

SURFACE = (0xFE, 0xF7, 0xFF, 0xFF)
CONTAINER = (0xF3, 0xED, 0xF7, 0xFF)
ACCENT = (0x67, 0x50, 0xA4, 0xFF)
OUTLINE = (0x79, 0x74, 0x7E, 0xFF)
WHITE = (0xFF, 0xFF, 0xFF, 0xFF)
TEXT = (0x1D, 0x1B, 0x20, 0xFF)
RAISED = [((0, 0, 0, 77), 0, 1, 2, 0), ((0, 0, 0, 38), 0, 2, 6, 2)]
EASE = (0.2, 0.0, 0.0, 1.0)

window = Window(width=520, height=380, title="tre -- showcase")
window.root.set(flex_direction="vertical", padding=20, gap=14, fill=SURFACE)


def text(value, width, size=14, **props):
    _, height = window.measure_text(value, font_size=size)
    return window.create("text", text=value, font_size=size, width=width, height=height,
                         fill=props.pop("fill", TEXT), **props)


def row(*children, gap=12):
    box = window.create("box", width="100%", gap=gap, align_items="center")
    for child in children:
        box.add_child(child)
    return box


def button(label, width=96, on_click=None):
    node = window.create("box", width=width, height=36, corner_radius=18, fill=ACCENT,
                         clip_children=True, align_items="center", justify_content="center",
                         role="button", label=label, focusable=True, cursor="pointer")
    node.add_child(text(label, width - 16, fill=WHITE, text_align="center"))

    def ripple(event: Event):
        r = 2 * width
        wave = window.create("box", position="absolute", x=event.x - r, y=event.y - r,
                             width=2 * r, height=2 * r, corner_radius=r, scale=0.0,
                             fill=(0xFF, 0xFF, 0xFF, 0x50), hit_testable=False)
        node.insert_child(0, wave)
        wave.animate("scale", 1.0, 400, easing=EASE)
        wave.animate("opacity", 0.0, 400, on_complete=wave.destroy)

    node.on("pointer_down", ripple)
    if on_click:
        node.on("click", on_click)
    return node


# -- header, with an overflow menu -------------------------------------------
more = button("More", width=80)
menu = window.create("box", width=160, padding_top=8, padding_bottom=8, corner_radius=4,
                     flex_direction="vertical", fill=CONTAINER, shadows=RAISED, role="menu")
header = row(text("Settings", 360, size=22), more)
window.root.add_child(header)

# -- switches and a slider ------------------------------------------------------
changes = []
wifi = Switch(window, "Wi-Fi", checked=True, on_change=lambda on: changes.append(("wifi", on)))
bluetooth = Switch(window, "Bluetooth", on_change=lambda on: changes.append(("bt", on)))
volume_label = text("Volume 50%", 100)
volume = Slider(window, "Volume", value=0.5,
                on_change=lambda v: volume_label.set(text=f"Volume {round(v * 100)}%"))
for label, control in (("Wi-Fi", wifi.node), ("Bluetooth", bluetooth.node)):
    window.root.add_child(row(text(label, 100), control))
window.root.add_child(row(volume_label, volume.node))

# -- a name field -----------------------------------------------------------------
field_box = window.create("box", width=240, height=40, padding=8, corner_radius=4,
                          stroke_color=OUTLINE, stroke_width=1, align_items="center")
name = window.create("text_input", text="Workstation", placeholder="Device name",
                     width=224, height=22, label="Device name")
field_box.add_child(name)
name.on("focus", lambda: field_box.set(stroke_color=ACCENT, stroke_width=2))
name.on("unfocus", lambda: field_box.set(stroke_color=OUTLINE, stroke_width=1))
window.root.add_child(row(text("Name", 100), field_box))

# -- a keyed list, sorted in place --------------------------------------------------
PEERS = {"atlas": 3, "boreal": 1, "cirrus": 2}
peers = window.create("box", flex_direction="vertical", gap=4, width=240, role="list")
peer_rows = {}
for key, rank in PEERS.items():
    node = window.create("box", width=240, height=30, padding_left=10, corner_radius=6,
                         align_items="center", fill=CONTAINER, role="listitem", label=key)
    node.add_child(text(f"{key}  #{rank}", 200))
    peers.add_child(node)
    peer_rows[key] = node
by_rank = False


def sort_peers():
    global by_rank
    by_rank = not by_rank
    order = sorted(PEERS, key=PEERS.get) if by_rank else sorted(PEERS)
    for index, key in enumerate(order):
        peers.insert_child(index, peer_rows[key])


window.root.add_child(row(peers, button("Sort", width=80, on_click=sort_peers)))

# -- the About dialog ----------------------------------------------------------------
scrim = window.create("box", width="100%", height="100%", fill=(0, 0, 0, 82),
                      align_items="center", justify_content="center")
dialog = window.create("box", width=300, padding=24, gap=16, flex_direction="vertical",
                       corner_radius=28, fill=CONTAINER, role="dialog", label="About")
close = button("Close", width=88, on_click=lambda: window.hide_layer(scrim))
dialog.add_child(text("About", 252, size=20))
dialog.add_child(text("Built from boxes, text, and layers.", 252))
dialog.add_child(row(window.create("box", flex_grow=1), close))
scrim.add_child(dialog)
scrim.on("dismiss", lambda: window.hide_layer(scrim))
scrim.on("click", lambda e: window.hide_layer(scrim) if e.target == scrim else None)


def menu_item(label, action):
    item = window.create("box", width=160, height=40, padding_left=12, align_items="center",
                         role="menuitem", label=label, focusable=True, cursor="pointer")
    item.add_child(text(label, 130))

    def choose():
        window.hide_layer(menu)
        action()

    item.on("click", choose)
    menu.add_child(item)
    return item


reset = menu_item("Reset volume", lambda: volume.set_value(0.5))
about = menu_item("About", lambda: window.show_layer(scrim, modal=True))
menu.on("dismiss", lambda: window.hide_layer(menu))
more.on("click", lambda: window.show_layer(menu, anchor=more, placement="below"))

# -- checks ------------------------------------------------------------------------
window.advance(0)
window.simulate("click", node=bluetooth.thumb)
window.simulate("key_down", key="space")
assert changes == [("bt", True), ("bt", False)]

window.simulate("a11y_action", node=volume.node, action="set_value", value=0.8)
assert volume_label.get("text") == "Volume 80%"

window.simulate("click", node=name)
window.simulate("key_down", key="end")
window.simulate("input", text=" 2")
assert name.get("text") == "Workstation 2" and field_box.get("stroke_width") == 2

window.simulate("click", node=peers.parent().children()[1])  # Sort, by rank
assert [n.get("label") for n in peers.children()] == ["boreal", "cirrus", "atlas"]

window.simulate("click", node=more)
window.simulate("click", node=reset)
assert volume.value == 0.5 and menu.parent() is None
window.simulate("click", node=more)
window.simulate("click", node=about)
assert close.get("focused"), "the modal dialog took focus"
window.simulate("key_down", key="escape")
window.advance(500)  # let the ripples finish and remove themselves
print("showcase.py: checks passed")

app = App()
app.add_window(window)
app.run(max_frames=120)
print("showcase.py: exited cleanly")
