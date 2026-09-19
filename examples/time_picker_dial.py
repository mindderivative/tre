#!/usr/bin/env python3
"""M39 Phase 2 Step 2's real `Window.add_time_picker_dial` (§5, §7):
MD3's real *analog* Time Picker circular drag control -- genuinely
distinct from `examples/time_picker.py`'s own digital `add_time_input_
field`/`add_period_selector` (that example's own doc comment named
this analog dial as a real, deferred gap).

There is no Python-facing pointer-move-while-pressed hook yet (only
`Window.click`, a press+release pair at a node's own center) -- the
real drag-to-angle math itself is proven directly at the Rust level
(`crates/engine-core/src/tree.rs`'s own `dispatch_drag_in_hour_mode_*`
tests). This script instead proves the real programmatic surface: two
dials, one moved via `set_time_picker_dial_time`, the other switched
to minute mode and moved too, both painted live across many real
frames.

What this script proves automatically (headless-CI-safe, no human
needed): construction doesn't raise, the real hour/minute values
round-trip through the setter/getter pair, mode-switching works, and a
real, live render loop runs cleanly with both dials on screen.
"""

from tre import App, Window

window = Window(width=400, height=300, title="tre v2 -- time picker dial")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

hour_dial = window.add_time_picker_dial(hour=9, minute=15, size=200.0, x=20.0, y=20.0)
assert hour_dial.get_time_picker_dial_time() == (9, 15)
assert hour_dial.get_time_picker_dial_mode() == "hour"

minute_dial = window.add_time_picker_dial(size=200.0, x=250.0, y=20.0)
minute_dial.set_time_picker_dial_mode("minute")
minute_dial.set_time_picker_dial_time(14, 35)
assert minute_dial.get_time_picker_dial_time() == (14, 35)
assert minute_dial.get_time_picker_dial_mode() == "minute"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(
    "time_picker_dial.py: exited cleanly after 60 frames -- "
    f"hour_dial={hour_dial.get_time_picker_dial_time()!r}, "
    f"minute_dial={minute_dial.get_time_picker_dial_time()!r}"
)
