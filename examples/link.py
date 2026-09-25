#!/usr/bin/env python3
"""M30 Phase 8 Step 2's real `Window.add_link` (§5, §7): a real,
standalone clickable label, backed by the new `NodeKind::Link` this
step added to `engine-core`.

Fulfills a real, explicit commitment this codebase already made to
itself (Phase 1's own `Tree::hit_test_at` fix): a bare `Text` node
deliberately never independently claims a hit -- always deferring to
its real interactive container -- so a real standalone clickable
label needs its own dedicated `NodeKind`, the same "each interactive
component is its own real `NodeKind`" precedent `Checkbox`/`Slider`/
`TextField` already established.

What this script proves automatically (headless-CI-safe, no human
needed): the link genuinely claims its own click, while a plain
`add_text` label at otherwise-identical anatomy never does.
"""

from tre import App, Window

window = Window(width=400, height=150, title="tre v2 -- link")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

shell = window.add_rect(background=(0xFF, 0xFB, 0xFE, 0xFF), width=400, height=150)

label = window.add_text(
    content="Reading this doesn't do anything.", foreground=(0x1D, 0x1B, 0x20, 0xFF), width=320, height=20
)
link = window.add_link(content="But clicking this does.", width=320)

shell.add_child(label)
shell.add_child(link)

label_calls: list[str] = []
label.enable_interaction()
label.set_on_click(lambda: label_calls.append("clicked"))
window.click(label)
assert label_calls == [], "a bare Text label must never independently claim its own click"

link_calls: list[str] = []
link.enable_interaction()
link.set_on_click(lambda: link_calls.append("clicked"))
window.click(link)
assert link_calls == ["clicked"], "a Link, unlike Text, must claim its own click directly"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"link.py: exited cleanly after 60 frames, label_calls={label_calls}, link_calls={link_calls}")
