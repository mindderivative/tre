#!/usr/bin/env python3
"""M35 Phase 2's real `Window.add_split_button` (§5, §7, §8): MD3's
real Split Button -- a leading button plus a separate trailing menu-
icon button. The engine never opens/closes a menu or rotates the
trailing icon on its own -- the app drives both directly via `Node.
animate("rotation", ...)` on the returned `trailing_icon` node,
Design Principle 6's own "engine provides the mechanism, app decides
the real state change" split (the identical shape `Checkbox.checked`
already uses).

What this script proves automatically (headless-CI-safe, no human
needed): clicking the trailing button toggles a real menu-open state
and rotates its own icon; clicking the leading button reaches only its
own, separate handler.
"""

from tre import App, Window

window = Window(width=800, height=300, title="tre v2 -- split button")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

leading, trailing, icon = window.add_split_button(
    label="Watch later",
    width=140,
    height=32,
    x=40.0,
    y=40.0,
)

events: list[str] = []
menu_open = False


def on_leading_click() -> None:
    events.append("leading")


def on_trailing_click() -> None:
    global menu_open
    menu_open = not menu_open
    icon.animate("rotation", 180.0 if menu_open else 0.0, 0)
    events.append(f"trailing-{'open' if menu_open else 'closed'}")


leading.enable_interaction()
leading.set_on_click(on_leading_click)
trailing.enable_interaction()
trailing.set_on_click(on_trailing_click)

window.click(trailing)
window.click(leading)
window.click(trailing)

assert events == [
    "trailing-open",
    "leading",
    "trailing-closed",
], f"each button must reach only its own handler, real toggling in order, got {events}"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"split_button.py: exited cleanly after 60 frames, events {events}")
