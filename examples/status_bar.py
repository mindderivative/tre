#!/usr/bin/env python3
"""M30 Phase 8 Step 5's real `Window.add_status_bar` (§5, §7, §11.2).
MD3 has no official Status Bar page (confirmed via the same directory-
listing technique this milestone already uses).

Real, deliberate reuse: `AppShell`'s own real `build_shell` already
accepts a pre-built `status_bar` region (§14 step 13) -- this step
adds the real, styled bar *content*, not new shell-level wiring. The
returned `Node` works directly as `build_shell`'s own existing
parameter.

What this script proves automatically (headless-CI-safe, no human
needed): a real status bar attaches cleanly into a real `AppShell`
alongside a real menu bar and toolbar.
"""

from tre import App, Window

window = Window(width=800, height=500, title="tre v2 -- status bar")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

menu_bar = window.add_top_app_bar(title="tre v2")[0]
toolbar = window.add_rect(background=(0xEC, 0xE6, 0xF0, 0xFF), width=800, height=48)
status_bar = window.add_status_bar(text="Ready – 0 errors, 0 warnings")

shell = window.build_shell(menu_bar=menu_bar, toolbar=toolbar, status_bar=status_bar)
assert shell is not None, "build_shell must return a real shell node"

app = App()
app.add_window(window)
app.run(max_frames=60)
print("status_bar.py: exited cleanly after 60 frames")
