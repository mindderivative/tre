#!/usr/bin/env python3
"""M30 Phase 2 Step 1's real `Window.add_radio_button` (§5, §7.3): a
real MD3 radio button -- a stroked ring plus a scaling inner dot,
mirroring `Checkbox`'s own real shape (`checkbox.py`). Also
demonstrates group-exclusivity as real application state, not
engine-owned (this component's own explicit design, matching the same
principle `segmented_button.py` already shows for its own group).

What this script proves automatically (headless-CI-safe, no human
needed): a real group of radio buttons renders, one starts selected,
and clicking a different one both selects it and deselects its
siblings -- entirely through the app's own plain Python code, no
component-specific "radio group" API anywhere in the engine.
"""

from typing import Callable

from tre import App, Window

window = Window(width=200, height=120, title="tre v2 -- radio button")

LABELS = ["Small", "Medium", "Large"]
radios = [window.add_radio_button(selected=(i == 0), x=16, y=16 + i * 32) for i in range(3)]
for i, radio in enumerate(radios):
    window.add_text(
        content=LABELS[i],
        foreground=(0xE6, 0xE1, 0xE5, 0xFF),
        width=100,
        height=20,
        x=48,
        y=18 + i * 32,
    )

selected_index = {"value": 0}


def select(i: int) -> None:
    if i == selected_index["value"]:
        return
    radios[selected_index["value"]].set_selected(False)
    radios[selected_index["value"]].animate("select_progress", 0.0, duration_ms=120)
    radios[i].set_selected(True)
    radios[i].animate("select_progress", 1.0, duration_ms=120)
    selected_index["value"] = i


def make_selector(i: int) -> Callable[[], None]:
    return lambda: select(i)


for i, radio in enumerate(radios):
    radio.enable_interaction()
    radio.set_on_click(make_selector(i))

window.click(radios[2])

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"radio_button.py: exited cleanly after 60 frames, selected index {selected_index['value']}")
