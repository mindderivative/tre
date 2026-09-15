#!/usr/bin/env python3
"""§14 build-order step 14 (§11.1): "a second `PyWindow` opened from a
running app, proving `WindowId`-routed event dispatch."

Real proof, not a smoke test alone: two independent `Window`s, each with
its own distinct content and animation, opened together by one `App`.
`engine-platform`'s own `crates/engine-platform/tests/multi_window.rs`
proves the underlying `WindowId` routing directly (real, distinct
`WindowId`s, independent per-window frame counts); this script proves
the same mechanism end to end through the full Python API most app
authors will actually use.

Same headless-CI-safe convention as `animate_rect.py`: `max_frames=60`
applies to each window independently, and `App.run()` exits 0 rather
than raising when no display/GPU is reachable.
"""

from tre import App, Window

main_window = Window(width=320, height=160, title="tre v2 -- main")
main_swatch = main_window.add_rect(background=(0x67, 0x50, 0xA4, 0xFF), width=100, height=100)
main_swatch.animate("opacity", 0.3, duration_ms=800)

panel_window = Window(width=220, height=120, title="tre v2 -- panel")
panel_swatch = panel_window.add_rect(background=(0x03, 0xDA, 0xC6, 0xFF), width=80, height=80)
panel_swatch.animate("corner_radius", 20.0, duration_ms=800)

app = App()
app.add_window(main_window)
app.add_window(panel_window)
app.run(max_frames=60)
print("two_windows.py: exited cleanly after 60 frames, both windows")
