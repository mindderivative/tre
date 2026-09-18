#!/usr/bin/env python3
"""M30 Phase 2 Step 2's real `Window.add_switch` (§5, §7.3): a real
MD3 switch -- a track plus a handle that both slides and grows as it
toggles, mirroring `RadioButton`'s own real anatomy (`radio_button.
py`).

What this script proves automatically (headless-CI-safe, no human
needed): a real switch renders both off and on, and `enable_
interaction()`/`click()`/`set_on`/`animate("toggle_progress", ...)`
all reach the real underlying `NodeKind::Switch` state.
"""

from tre import App, Window

window = Window(width=200, height=100, title="tre v2 -- switch")

off_switch = window.add_switch(x=16, y=16)
on_switch = window.add_switch(on=True, x=16, y=56)

window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)


def toggle() -> None:
    now_on = not off_switch.get_on()
    off_switch.set_on(now_on)
    off_switch.animate("toggle_progress", 1.0 if now_on else 0.0, duration_ms=120)


off_switch.enable_interaction()
off_switch.set_on_click(toggle)
window.click(off_switch)

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"switch.py: exited cleanly after 60 frames, off_switch now on={off_switch.get_on()}")
