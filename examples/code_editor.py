#!/usr/bin/env python3
"""M30 Phase 9 Step 3's real `Code Editor` component (§5, §8, §10): a
genuinely multiline `TextField` (`Window.add_code_editor`), closing
the real, stated single-line-only gap `TextField` always had. Real,
honestly-scoped v1 -- see `add_code_editor`'s own Rust doc comment for
the full list of deliberately deferred pieces (syntax highlighting,
a line-number gutter, scroll/clip past the box's own edges, a bundled
monospace font, Tab-key indentation capture); what *is* real here:
`Enter` inserts a genuine newline, `Home`/`End` operate on the current
line rather than the whole buffer, and `ArrowUp`/`ArrowDown` navigate
by line, preserving the caret's own real column.

What this script proves automatically (headless-CI-safe, no human
needed): a real click focuses the editor; a real `Enter` keypress
splits one line into two; `Home` targets the current line, not byte 0;
`ArrowUp`/`ArrowDown` genuinely move between lines (proven by typing
after navigating and checking exactly which line received the new
text); and a real render loop paints the whole multiline buffer over
actual frames without crashing. The definitive proof that a real `\\n`
produces a real, vertically-stacked second layout line (not just
accepted into `content` with no visual effect) is `crates/engine-
render/tests/text_field_paint.rs::a_multiline_fields_own_newline_
produces_a_real_second_layout_line`, not this script.
"""

from tre import App, Window

window = Window(width=420, height=280, title="tre v2 -- code editor")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

editor = window.add_code_editor(
    content="def add(a, b):\n    return a + b",
    background=(0xFF, 0xFB, 0xFE, 0xFF),
    width=380,
    height=200,
)

print(f"before click: is_focused={editor.is_focused()}")
window.click(editor)
print(f"after click: is_focused={editor.is_focused()}")
assert editor.is_focused(), "a real click on a Code Editor must focus it"

# Real Enter mid-buffer: place the cursor right after "def add(a, b):"
# (the cursor begins at content's own end, on the second line, so
# ArrowUp first reaches the first line) and split it into two real
# lines.
window.press_key("up")
window.press_key("end")
window.press_key("enter")
window.type_text("    # adds two numbers")
print(f"after Enter mid-buffer:\n{editor.get_text()}")
assert editor.get_text() == "def add(a, b):\n    # adds two numbers\n    return a + b"

# Real Home targets the current line, not the whole buffer -- the
# cursor is still on the comment line just typed.
window.press_key("home")
window.type_text(">>")
print(f"after Home + typing '>>':\n{editor.get_text()}")
assert editor.get_text() == "def add(a, b):\n>>    # adds two numbers\n    return a + b"

# Real ArrowDown: move from the comment line to the final "return" line.
window.press_key("down")
window.press_key("end")
window.type_text("  # end")
print(f"after ArrowDown + End + typing:\n{editor.get_text()}")
assert editor.get_text() == (
    "def add(a, b):\n>>    # adds two numbers\n    return a + b  # end"
)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("code_editor.py: exited cleanly after 60 frames -- a real 3-line buffer, edited live")
