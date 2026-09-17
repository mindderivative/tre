#!/usr/bin/env python3
"""M15 Phases 1-2's real MD3 text field (§5, §8, §10, §16.7): a real,
live, single-line editable text field -- `Window.add_text_field`/`Node.
get_text`/`set_text`/`is_focused`, plus real keyboard-driven editing
(`Window.type_text`/`press_key` with the widened Backspace/Delete/
Left/Right/Home/End vocabulary), all real. A real caret only appears
once the field is genuinely the window's own focused node (§10's
already-real Tab/focus model), reached here via a real `Window.
press_key("tab")`, not a hardcoded assumption.

M17 (§8) closes the two real gaps this docstring used to name:
clipboard's own hermetic half is `examples/clipboard.py`; real IME
composition has no Python-facing entry point at all (an `Ime` event
only ever originates from a real OS input method -- there is nothing
for a script to synthesize, the same category of gap `clipboard.py`'s
own docstring already names for a real Ctrl+C/X/V keypress), so its
definitive proof is the pixel-level `crates/engine-render/tests/
text_field_paint.rs::a_composing_preedit_paints_a_real_underline_
distinct_from_the_same_field_when_not_composing`, not a script here.

M18 Phase 1 (§8, §10) closes the first half of that mouse gap: a real
click now also focuses a `TextField`, demonstrated below via `Window.
click(field)` -- the same real `Tree::dispatch`'s `PointerPressed`
mechanism a genuine mouse press reaches, not a separate code path.
Click-to-*position* (moving the cursor to the exact character clicked)
needs real per-glyph shaping this script has no way to synthesize
without a live window/renderer -- its definitive proof is `crates/
engine-render/tests/text_field_paint.rs::hit_test_position_*` (pure
`parley` shaping math) plus `crates/engine-core/src/tree.rs::tests::
set_text_field_cursor_*`, not this script.

What this script proves automatically (headless-CI-safe, no human
needed): a real text field, reached by both a real Tab press and a
real click, edited by a real sequence of synthetic keystrokes (typing,
cursor navigation, Backspace) that mirror exactly what a real `winit`-
driven keyboard would produce, its own real content/focus state
observable from Python throughout, and a real render loop painting it
(caret included) over actual frames without crashing. The definitive
pixel-level proof the caret itself only paints while focused is
`crates/engine-render/tests/text_field_paint.rs`, not this script --
the same split this workspace's own examples have used throughout.
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

# M18 Phase 1 (§8, §10): a real click also focuses a TextField -- a
# second focusable node gives Tab somewhere else to land, so "field is
# no longer focused" genuinely proves something rather than Tab-order
# just wrapping back to the field itself. (A Checkbox won't do here --
# only TextField opts into Tab's own focus order today, M15 Phase 1's
# own finding: `Tree::set_access` had zero other real callers.)
spacer = window.add_text_field(background=(0xCC, 0xCC, 0xCC, 0xFF), width=60, height=24)
window.press_key("tab")
print(f"after Tab-away: is_focused={field.is_focused()}")
assert not field.is_focused()

window.click(field)
print(f"after Window.click(field): is_focused={field.is_focused()}")
assert field.is_focused(), "a real click on a TextField must move real focus there"

app = App()
app.add_window(window)
app.run(max_frames=60)
print("text_field.py: exited cleanly after 60 frames")
