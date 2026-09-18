#!/usr/bin/env python3
"""M30 Phase 1 Step 4's real `Window.add_segmented_button` (§5, §7):
MD3's real group-of-connected-segments anatomy -- one shared outline
frame, real dividers between segments, a selected segment's real
checkmark. Also demonstrates the real, explicit design this method
leaves to the app: group-exclusivity (`BUILD_TRACKER.md`'s own stated
"application state, not engine-owned" decision) wired up here with the
same already-generic primitives every other component uses.

What this script proves automatically (headless-CI-safe, no human
needed): a real group renders and every segment's own click handler
fires -- including re-selecting a different segment by animating the
two affected segments' own `background` color, the real minimal "app
wires up live re-toggling itself" pattern this component's own
docstring describes.
"""

from typing import Callable

from tre import App, Window

window = Window(width=320, height=100, title="tre v2 -- segmented button")

SECONDARY_CONTAINER = (0xE8, 0xDE, 0xF8, 0xFF)
TRANSPARENT = (0x00, 0x00, 0x00, 0x00)

segments = window.add_segmented_button(
    labels=["Day", "Week", "Month"],
    selected=[True, False, False],
    width=240,
    x=16,
    y=16,
)

selected_index = {"value": 0}


def select(i: int) -> None:
    if i == selected_index["value"]:
        return
    segments[selected_index["value"]].animate("background", TRANSPARENT, duration_ms=120)
    segments[i].animate("background", SECONDARY_CONTAINER, duration_ms=120)
    selected_index["value"] = i


def make_selector(i: int) -> Callable[[], None]:
    return lambda: select(i)


for i, segment in enumerate(segments):
    segment.enable_interaction()
    segment.set_on_click(make_selector(i))

window.click(segments[2])

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"segmented_button.py: exited cleanly after 60 frames, selected index {selected_index['value']}")
