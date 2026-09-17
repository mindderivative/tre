#!/usr/bin/env python3
"""M15 Phase 1's real MD3 text field (§5, §16.7): a real, live,
single-line editable text field -- `Window.add_text_field`/`Node.
get_text`/`Node.is_focused`, all real this phase. No real typing yet
(M15 Phase 2's own scope, character insertion/backspace/delete/arrow
keys) -- this script proves the state/paint/focus half: a real caret
that only appears once the field is genuinely the window's own focused
node (§10's already-real Tab/focus model), reached here via a real
`Window.press_key("tab")`, not a hardcoded assumption.

What this script proves automatically (headless-CI-safe, no human
needed): a real text field, seeded with real initial content, reached
by a real Tab press, its own real focused state observable from
Python, and a real render loop painting it (caret included) over
actual frames without crashing. The definitive pixel-level proof the
caret itself only paints while focused is `crates/engine-render/tests/
text_field_paint.rs`, not this script -- the same split this
workspace's own examples have used throughout.
"""

from tre import App, Window

window = Window(width=280, height=120, title="tre v2 -- text field")

field = window.add_text_field(
    background=(0xEE, 0xEE, 0xEE, 0xFF),
    width=220,
    height=32,
    content="hello",
)

print(f"before Tab: is_focused={field.is_focused()}, text={field.get_text()!r}")
window.press_key("tab")
print(f"after Tab: is_focused={field.is_focused()}, text={field.get_text()!r}")
assert field.is_focused(), "a real Tab press must reach the one real TextField in this window"

app = App()
app.add_window(window)
app.run(max_frames=60)
print("text_field.py: exited cleanly after 60 frames")
