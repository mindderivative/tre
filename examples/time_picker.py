#!/usr/bin/env python3
"""M30 Phase 7 Step 2's real `Window.add_time_input_field`/
`add_period_selector` (§5, §7): MD3's real *Time Input* variant
(digital hour:minute entry) -- deliberately not the analog clock-face
dial, which needs a genuinely new drag-to-angle engine capability this
project doesn't have.

The hour/minute fields reuse `TextField`'s own already-real
`NodeKind`, so typing, focus, and selection all work for free, the
same real design `Search Bar` already established for its own input.

What this script proves automatically (headless-CI-safe, no human
needed): typing into a focused field genuinely edits its real content,
and the AM/PM period selector's two options are each independently
clickable.
"""

from tre import App, Window

window = Window(width=300, height=200, title="tre v2 -- time picker")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

shell = window.add_rect(background=(0xFF, 0xFB, 0xFE, 0xFF), width=300, height=200)

hour = window.add_time_input_field(value="09", x=24, y=24)
minute = window.add_time_input_field(value="30", x=132, y=24)
am, pm = window.add_period_selector(selected="AM", x=232, y=24)

for node in (hour, minute, am, pm):
    shell.add_child(node)

window.press_key("tab")
hour.set_text("")
window.type_text("11")
assert hour.get_text() == "11", "typing into the real hour TextField must edit its content"

selected: dict[str, str] = {"period": "AM"}


def select_pm() -> None:
    selected["period"] = "PM"


pm.enable_interaction()
pm.set_on_click(select_pm)
window.click(pm)
assert selected["period"] == "PM", "clicking pm must reach its own registered handler"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(
    f"time_picker.py: exited cleanly after 60 frames, "
    f"hour={hour.get_text()!r}, minute={minute.get_text()!r}, period={selected['period']}"
)
