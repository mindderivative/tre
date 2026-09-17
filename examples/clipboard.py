#!/usr/bin/env python3
"""M17 Phase 1's real clipboard support for `TextField` (§8): `Window.
copy`/`cut`/`paste`, all real. The genuine Ctrl+C/Ctrl+X/Ctrl+V ->
real OS clipboard path is entirely `engine-platform`/`App::run`'s own
real winit-driven wiring (`translate_clipboard_shortcut` + the real
`arboard` I/O in `on_input`'s handling of `InputEvent::Copy`/`Cut`/
`PasteRequested`) -- there is no synthetic way to inject a real,
Ctrl-modified OS keyboard event from Python, the same real category of
gap this codebase's own "no live AT-SPI client" note already states
honestly elsewhere. This script instead proves the real, hermetic half
`Window.copy`/`cut`/`paste` expose: exactly what a real Ctrl+C/X/V
would do to a focused `TextField`'s own real selection, without
touching the actual OS clipboard (so this stays deterministic and
headless-CI-safe).

What this script proves automatically (headless-CI-safe, no human
needed): a real selection, read without mutation (copy); read and
removed (cut); and a real paste both inserting at the cursor and
replacing an active selection. The real, live arboard <-> OS clipboard
round trip itself is verified separately, by a real Rust-level test
(`crates/engine-py/src/app.rs::tests::arboard_genuinely_round_trips_
through_a_real_clipboard`), not by this script.
"""

from tre import Window

window = Window(width=280, height=120, title="tre v2 -- clipboard")

field = window.add_text_field(
    background=(0xEE, 0xEE, 0xEE, 0xFF),
    width=220,
    height=32,
    content="hello world",
)
window.press_key("tab")

# Select "hello" (the first 5 characters) via the same real keyboard
# path examples/text_field.py already establishes.
window.press_key("home")
for _ in range(5):
    window.press_key("right", shift=True)

copied = window.copy()
print(f"copy(): {copied!r}, field still reads {field.get_text()!r}")
assert copied == "hello"
assert field.get_text() == "hello world", "a real copy must never mutate the field"

cut = window.cut()
print(f"cut(): {cut!r}, field now reads {field.get_text()!r}")
assert cut == "hello"
assert field.get_text() == " world"

window.press_key("home")
window.paste("hi")
print(f"paste('hi') at Home: field now reads {field.get_text()!r}")
assert field.get_text() == "hi world"

window.press_key("home")
for _ in range(2):
    window.press_key("right", shift=True)
window.paste("HI")
print(f"paste('HI') over a real selection: field now reads {field.get_text()!r}")
assert field.get_text() == "HI world"

print("clipboard.py: exited cleanly, hermetic copy/cut/paste all proved")
