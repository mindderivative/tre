#!/usr/bin/env python3
"""M35 Phase 1's real `Window.add_toolbar` (§5, §7): MD3's real
Toolbar -- "docked" (spans the window's own full width, square
corners) or "floating" (hugs its own content, fully rounded, real
elevation, horizontal or vertical).

A real "container with configurable slots" per MD3's own anatomy: the
caller populates it with any already-built node via the existing,
generic `Node.add_child` -- no specialized children-list parameter.

What this script proves automatically (headless-CI-safe, no human
needed): a real click on a button composed into a docked toolbar
reaches only its own registered handler, exercised alongside a
floating, vertical, vibrant toolbar to prove that configuration
doesn't raise either.
"""

from tre import App, Window

window = Window(width=800, height=600, title="tre v2 -- toolbar")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

# A docked toolbar at the bottom of the window, holding a save button
# and an icon button -- real MD3 anatomy: "a container with several
# slots... the most common elements are icon buttons, buttons, and
# text fields."
docked = window.add_toolbar(x=0.0, y=536.0)
save_button = window.add_button(label="Save", width=88, height=40)
docked.add_child(save_button)
share_icon = window.add_icon_button(icon="add", size=40.0)
docked.add_child(share_icon)

# A floating, vertical, vibrant toolbar along the trailing edge --
# real MD3 anatomy: floating toolbars can be horizontal or vertical,
# and vibrant is the real high-emphasis color configuration.
floating = window.add_toolbar(
    variant="floating",
    orientation="vertical",
    vibrant=True,
    x=740.0,
    y=200.0,
)
undo_icon = window.add_icon_button(icon="add", size=40.0)
floating.add_child(undo_icon)

events: list[str] = []
save_button.enable_interaction()
save_button.set_on_click(lambda: events.append("save"))
share_icon.enable_interaction()
share_icon.set_on_click(lambda: events.append("share"))

window.click(save_button)
assert events == ["save"], "a real click on a button composed into a toolbar must reach only its own handler"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"toolbar.py: exited cleanly after 60 frames, events {events}")
