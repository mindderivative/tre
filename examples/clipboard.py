#!/usr/bin/env python3
"""A text input's clipboard shortcuts (M17, M53, M100): select part of a
field with Shift+Right, copy it with Ctrl+C, cut it with Ctrl+X, paste
it back with Ctrl+V, and select everything with Ctrl+A -- driven with
`window.simulate`, the same input pipeline a live key press takes, and
checked through `window.read_clipboard()`.

Headless and immediate: no `App.run()`, no frames. Where no OS clipboard
service is reachable, the copy/cut/paste steps are reported and skipped.
"""

from tre import Window

window = Window(width=280, height=120, title="tre -- clipboard")
field = window.create("text_input", text="hello world", width=220, height=32)
window.root.add_child(field)


def key(name, **modifiers):
    window.simulate("key_down", key=name, **modifiers)


key("tab")  # focus the field
key("home")
for _ in range(5):
    key("arrow_right", shift=True)  # select "hello"

if window.write_clipboard("probe") and window.read_clipboard() == "probe":
    key("c", ctrl=True)
    print(f"Ctrl+C: clipboard {window.read_clipboard()!r}, field {field.get('text')!r}")
    assert window.read_clipboard() == "hello"
    assert field.get("text") == "hello world", "a copy never edits the field"

    key("x", ctrl=True)
    print(f"Ctrl+X: clipboard {window.read_clipboard()!r}, field {field.get('text')!r}")
    assert field.get("text") == " world"

    key("home")
    key("v", ctrl=True)
    print(f"Ctrl+V at Home: field {field.get('text')!r}")
    assert field.get("text") == "hello world"
else:
    print("no OS clipboard service reachable: skipping copy, cut, and paste")

key("a", ctrl=True)
print(f"Ctrl+A: selection {field.get('selection')}")
assert field.get("selection") == (0, 11)
window.simulate("input", text="HI")
assert field.get("text") == "HI", "typing replaces the selection"
print("clipboard.py: exited cleanly")
