#!/usr/bin/env python3
"""M35 Phase 3's real `Window.add_button_group` (§5, §7, §11.7): MD3's
real Standard Button Group -- an invisible container holding several
real `add_button`-built buttons, spaced with a real gap. Pressing one
live-reflows its own width and its immediate neighbors' -- real MD3's
own distinctive "pressing a button also affects the width of adjacent
buttons" mechanic, driven entirely by `engine-core::Tree::
sync_button_group_layouts` reading the already-tracked live pointer-
press state. No app-side wiring needed for the reflow itself (unlike
`Split Button`'s own rotation, which the app must drive).

What this script proves automatically (headless-CI-safe, no human
needed): each button in the group reaches only its own registered
click handler, independent of its siblings.
"""

from typing import Callable

from tre import App, Window

window = Window(width=800, height=200, title="tre v2 -- button group")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

group, buttons = window.add_button_group(
    labels=["Day", "Week", "Month"],
    width=80,
    height=40,
    x=40.0,
    y=40.0,
)

events: list[str] = []


def make_handler(label: str) -> Callable[[], None]:
    def handle() -> None:
        events.append(label)

    return handle


for button, label in zip(buttons, ["Day", "Week", "Month"]):
    button.enable_interaction()
    button.set_on_click(make_handler(label))

window.click(buttons[1])
window.click(buttons[0])

assert events == ["Week", "Day"], f"each button must reach only its own handler, got {events}"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"button_group.py: exited cleanly after 60 frames, events {events}")
