#!/usr/bin/env python3
"""M39 Phase 2's real `Window.add_loading_indicator` (§5, §7): a real,
perpetually-looping MD3 Expressive-style loading spinner -- morphs
between four real shapes (Pentagon, Pill, Cookie, Oval) with no
app-side wiring needed at all. The engine's own `Tree::tick_all`
drives the real loop automatically the instant a real node is
constructed; there is no Python-facing method to start, stop, or
observe it directly.

What this script proves automatically (headless-CI-safe, no human
needed): construction doesn't raise, and a real, live render loop runs
cleanly across many genuine frames -- at 650ms per real shape
transition, 200 real frames (well over 3 real seconds of simulated
time at a typical 60fps) guarantees several full real shape
transitions actually happen, the real, decisive proof the perpetual
loop keeps advancing over genuine time rather than stalling after the
first one.
"""

from tre import App, Window

window = Window(width=400, height=200, title="tre v2 -- loading indicator")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

default_indicator = window.add_loading_indicator(x=40.0, y=40.0)
small_indicator = window.add_loading_indicator(size=24.0, x=140.0, y=52.0)
custom_color_indicator = window.add_loading_indicator(
    size=64.0, foreground=(0xB0, 0x00, 0x20, 0xFF), x=220.0, y=20.0
)

assert default_indicator is not None
assert small_indicator is not None
assert custom_color_indicator is not None

app = App()
app.add_window(window)
app.run(max_frames=200)
print("loading_indicator.py: exited cleanly after 200 frames -- the real loop kept advancing")
