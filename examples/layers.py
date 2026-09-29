#!/usr/bin/env python3
"""Layers: a menu anchored to a button, and a modal dialog over a scrim --
both ordinary nodes shown with `window.show_layer`.

- The menu opens below its button, closes when an item is chosen, and is
  dismissed by a press outside it or by Escape.
- The dialog blocks input to everything beneath it, takes focus, keeps
  Tab inside itself, and returns focus to the button when it closes.

The script drives both with `window.simulate` before opening the window.
Headless-CI-safe: `App.run()` renders `max_frames=60` and returns quietly
without a display. See docs/guide/layers.md.
"""

from tre import App, Window

SURFACE = (0xF3, 0xED, 0xF7, 0xFF)
ACCENT = (0x67, 0x50, 0xA4, 0xFF)
WHITE = (0xFF, 0xFF, 0xFF, 0xFF)
RAISED = [((0, 0, 0, 77), 0, 1, 2, 0), ((0, 0, 0, 38), 0, 2, 6, 2)]

window = Window(width=420, height=300, title="tre -- layers")
window.root.set(align_items="start")
log = []


def button(label, width=96):
    node = window.create("box", width=width, height=36, corner_radius=18, fill=ACCENT,
                         align_items="center", justify_content="center",
                         role="button", label=label, focusable=True)
    node.add_child(window.create("text", text=label, fill=WHITE, width=width - 16,
                                 height=18, text_align="center"))
    return node


# -- a menu ------------------------------------------------------------------
more = button("More")
menu = window.create("box", width=160, padding_top=8, padding_bottom=8,
                     flex_direction="vertical", corner_radius=4, fill=SURFACE,
                     shadows=RAISED, role="menu")
for name in ("Rename", "Duplicate", "Delete"):
    item = window.create("box", width=160, height=40, padding_left=12, align_items="center",
                         role="menuitem", label=name, focusable=True, cursor="pointer")
    item.add_child(window.create("text", text=name, width=130, height=18))
    item.on("click", lambda name=name: choose(name))
    menu.add_child(item)
items = menu.children()


def choose(name):
    log.append(name)
    window.hide_layer(menu)


menu.on("dismiss", lambda: window.hide_layer(menu))
more.on("click", lambda: window.show_layer(menu, anchor=more, placement="below"))

# -- a modal dialog ------------------------------------------------------------
delete = button("Delete…", width=112)
scrim = window.create("box", width="100%", height="100%", fill=(0, 0, 0, 82),
                      align_items="center", justify_content="center")
dialog = window.create("box", width=280, padding=24, gap=16, flex_direction="vertical",
                       corner_radius=28, fill=SURFACE, role="dialog", label="Delete file?")
dialog.add_child(window.create("text", text="Delete file?", font_size=20, width=232, height=26))
actions = window.create("box", width=232, gap=8, justify_content="end")
cancel, confirm = button("Cancel"), button("Delete")
actions.add_child(cancel)
actions.add_child(confirm)
dialog.add_child(actions)
scrim.add_child(dialog)
scrim.on("dismiss", lambda: window.hide_layer(scrim))  # Escape
# The scrim covers the window, so no press is "outside" it: a click on the
# scrim itself (not the dialog, which bubbles here too) closes it.
scrim.on("click", lambda e: window.hide_layer(scrim) if e.target == scrim else None)
cancel.on("click", lambda: window.hide_layer(scrim))
confirm.on("click", lambda: (log.append("deleted"), window.hide_layer(scrim)))
delete.on("click", lambda: window.show_layer(scrim, modal=True))

window.root.add_child(more)
window.root.add_child(delete)

# -- checks ------------------------------------------------------------------
window.simulate("click", node=more)
assert menu.parent() is None and menu.get("layer_placement") == "below"
assert menu.get("layout_y") >= more.get("layout_y") + more.get("layout_height")
window.simulate("click", node=items[1])
assert log == ["Duplicate"]

window.simulate("click", node=more)
window.simulate("pointer_down", x=400, y=280)  # outside: dismissed, press consumed
window.simulate("pointer_up", x=400, y=280)
window.show_layer(menu, anchor=more)
window.simulate("key_down", key="escape")
assert log == ["Duplicate"], "dismissal closes without choosing"

window.simulate("key_down", key="tab")  # focus a button by keyboard...
while not delete.get("focused"):
    window.simulate("key_down", key="tab")
window.simulate("key_down", key="enter")  # ...and open the dialog with Enter
assert cancel.get("focused"), "a modal layer takes focus"
window.simulate("key_down", key="tab")
window.simulate("key_down", key="tab")
assert cancel.get("focused"), "Tab stays inside the dialog"
window.simulate("key_down", key="escape")  # dismiss: focus goes back
assert delete.get("focused"), "focus returns to where it was"
window.simulate("click", node=delete)
window.simulate("click", node=more)  # lands on the scrim, not the button beneath
assert menu.get("layer_placement") is None, "the modal blocked the button"
assert delete.get("focused"), "the scrim click closed the dialog"
window.simulate("click", node=delete)
window.simulate("click", node=confirm)
assert log == ["Duplicate", "deleted"]
print("layers.py: checks passed")

app = App()
app.add_window(window)
app.run(max_frames=60)
print("layers.py: exited cleanly")
