#!/usr/bin/env python3
"""A text field: a `text_input` inside a box that paints its border, the
border turning the accent color while the input has focus. Tab or a click
focuses it; typing, Home/End, and Backspace edit it; `input` reports what
was typed and `change` each edit's before and after.

The script types into it with `window.simulate`, then opens the window
so you can type for real. Headless-CI-safe: `App.run()` renders
`max_frames=60` and returns quietly without a display or GPU. See
docs/guide/text.md.
"""

from tre import App, Window

OUTLINE, ACCENT = (0x79, 0x74, 0x7E, 0xFF), (0x67, 0x50, 0xA4, 0xFF)

window = Window(width=300, height=140, title="tre -- text field")
window.root.set(flex_direction="vertical", gap=12)

box = window.create("box", width=240, height=40, padding=8, corner_radius=4,
                    stroke_color=OUTLINE, stroke_width=1, align_items="center")
field = window.create("text_input", text="hello", placeholder="Say something",
                      width=224, height=22)
box.add_child(field)
other = window.create("text_input", placeholder="Another field", width=240, height=22)
window.root.add_child(box)
window.root.add_child(other)

field.on("focus", lambda: box.set(stroke_color=ACCENT, stroke_width=2))
field.on("unfocus", lambda: box.set(stroke_color=OUTLINE, stroke_width=1))
typed, changes = [], []
field.on("input", lambda e: typed.append(e.text))
field.on("change", lambda e: changes.append((e.old_value, e.new_value)))

# -- checks ------------------------------------------------------------------
window.simulate("key_down", key="tab")
assert field.get("focused") and box.get("stroke_color") == ACCENT

window.simulate("input", text=" world")  # the caret starts at the end
window.simulate("key_down", key="home")
window.simulate("input", text=">> ")
window.simulate("key_down", key="end")
window.simulate("key_down", key="backspace")
assert field.get("text") == ">> hello worl"
assert typed == [" world", ">> "]
assert changes[-1] == (">> hello world", ">> hello worl")

field.set(text="reset")  # from code: no `change`
assert len(changes) == 3

window.simulate("key_down", key="tab")
assert other.get("focused") and box.get("stroke_color") == OUTLINE
window.simulate("click", node=field)
assert field.get("focused")
print("text_field.py: checks passed")

app = App()
app.add_window(window)
app.run(max_frames=60)
print("text_field.py: exited cleanly")
