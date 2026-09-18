#!/usr/bin/env python3
"""M30 Phase 8 Step 3's real `Window.add_spin_box` (§5, §7): a real
numeric increment control. Deliberately named *SpinBox*, not
*Stepper* -- MD3's own vocabulary already uses "Stepper" for a
completely different real component (a multi-step flow indicator),
pyCopper's own real prior naming-risk finding, reused directly.

The numeric field reuses `TextField`'s own already-real `NodeKind`
(typing, focus, selection all work for free), the same real design
`Search Bar`/`Time Input` already established. Decrement/increment
reuse `Icon Button`'s own exact real anatomy. The app owns the
value's own real numeric semantics entirely -- the engine has no
notion of bounds, step size, or what "+1" even means.

What this script proves automatically (headless-CI-safe, no human
needed): clicking increment/decrement genuinely updates the field's
own real content.
"""

from tre import App, Window

window = Window(width=300, height=100, title="tre v2 -- spin box")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

shell = window.add_rect(background=(0xFF, 0xFB, 0xFE, 0xFF), width=300, height=100)

field, decrement, increment = window.add_spin_box(value="1", x=24, y=24)
shell.add_child(field)
shell.add_child(decrement)
shell.add_child(increment)

count = {"value": 1}


def apply_delta(delta: int) -> None:
    count["value"] = max(0, count["value"] + delta)
    field.set_text(str(count["value"]))


decrement.enable_interaction()
decrement.set_on_click(lambda: apply_delta(-1))
increment.enable_interaction()
increment.set_on_click(lambda: apply_delta(1))

window.click(increment)
window.click(increment)
assert field.get_text() == "3", "incrementing twice must reach the real field's own content"

window.click(decrement)
assert field.get_text() == "2", "decrementing must reach the real field's own content"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"spin_box.py: exited cleanly after 60 frames, value={field.get_text()!r}")
