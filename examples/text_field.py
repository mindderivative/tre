#!/usr/bin/env python3
"""M15 Phases 1-2's real MD3 text field (§5, §8, §10, §16.7): a real,
live, single-line editable text field -- `Window.add_text_field`/`Node.
get_text`/`set_text`/`is_focused`, plus real keyboard-driven editing
(`Window.type_text`/`press_key` with the widened Backspace/Delete/
Left/Right/Home/End vocabulary), all real. A real caret only appears
once the field is genuinely the window's own focused node (§10's
already-real Tab/focus model), reached here via a real `Window.
press_key("tab")`, not a hardcoded assumption.

What this script proves automatically (headless-CI-safe, no human
needed): a real text field, reached by a real Tab press, edited by a
real sequence of synthetic keystrokes (typing, cursor navigation,
Backspace) that mirror exactly what a real `winit`-driven keyboard
would produce, its own real content/focus state observable from
Python throughout, and a real render loop painting it (caret included)
over actual frames without crashing. The definitive pixel-level proof
the caret itself only paints while focused is `crates/engine-render/
tests/text_field_paint.rs`, not this script -- the same split this
workspace's own examples have used throughout. Mouse click-to-position
and clipboard/IME are real, separate, un-scoped gaps this milestone's
own status line already names (M17 covers clipboard/IME).
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

# Real keyboard-driven editing (M15 Phase 2): type past the end, then
# navigate back to the start and insert there too, proving both
# insertion and real cursor movement.
window.type_text(" world")
print(f"after typing ' world': text={field.get_text()!r}")
assert field.get_text() == "hello world"

window.press_key("home")
window.type_text(">> ")
print(f"after Home + typing '>> ': text={field.get_text()!r}")
assert field.get_text() == ">> hello world"

window.press_key("end")
window.press_key("backspace")
print(f"after End + Backspace: text={field.get_text()!r}")
assert field.get_text() == ">> hello worl"

app = App()
app.add_window(window)
app.run(max_frames=60)
print("text_field.py: exited cleanly after 60 frames")
