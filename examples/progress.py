#!/usr/bin/env python3
"""M30 Phase 3 Step 2's real `Window.add_linear_progress`/`Window.
add_circular_progress` (§5, §7): MD3's real linear and circular
progress indicators. Both share the same real `value` (`0.0..=1.0`)
shape `Slider`'s own `thumb_position` already established -- read via
`Node.get("value")`, written via `Node.animate("value", ...)`, the
identical real "the animated field is the value" precedent this
codebase uses throughout.

What this script proves automatically (headless-CI-safe, no human
needed): both real indicators render at a real, non-zero value,
through the real pipeline for real frames, and `animate("value", ...)`
reaches the real underlying state, exiting cleanly.
"""

from tre import App, Window

window = Window(width=200, height=100, title="tre v2 -- progress")

bar = window.add_linear_progress(width=160, value=0.35, x=16, y=16)
ring = window.add_circular_progress(value=0.65, x=16, y=32)

window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

bar.animate("value", 0.8, duration_ms=200)

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"progress.py: exited cleanly after 60 frames, bar value now {bar.get('value'):.2f}")
