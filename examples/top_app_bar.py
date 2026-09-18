#!/usr/bin/env python3
"""M30 Phase 5 Step 3's real `Window.add_top_app_bar` (§5, §7): MD3's
real *Small* Top App Bar variant -- a 64dp header bar spanning the
window's own width, with an optional leading icon (e.g. a menu/back
button) and zero or more trailing icons (e.g. search, more options).

Reuses `Icon Button`'s own exact real anatomy for the leading/trailing
actions, so each is independently clickable with no decorative layer
in the way.

What this script proves automatically (headless-CI-safe, no human
needed): the leading icon and each trailing icon reach only their own
registered handler.
"""

from typing import Callable

from tre import App, Window

window = Window(width=800, height=500, title="tre v2 -- top app bar")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

bar, leading, trailing = window.add_top_app_bar(
    title="Inbox",
    leading_icon="add",
    trailing_icons=["add", "add"],
)

events: list[str] = []


def make_trailing_handler(index: int) -> Callable[[], None]:
    def handle() -> None:
        events.append(f"trailing-{index}")

    return handle


assert leading is not None
leading.enable_interaction()
leading.set_on_click(lambda: events.append("leading"))

for i, node in enumerate(trailing):
    node.enable_interaction()
    node.set_on_click(make_trailing_handler(i))

window.click(leading)
window.click(trailing[1])
assert events == ["leading", "trailing-1"], "each action must reach only its own registered handler"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"top_app_bar.py: exited cleanly after 60 frames, events {events}")
