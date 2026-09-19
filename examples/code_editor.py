#!/usr/bin/env python3
"""M30 Phase 9 Step 3's real `Code Editor` component (§5, §8, §10): a
genuinely multiline `TextField` (`Window.add_code_editor`), closing
the real, stated single-line-only gap `TextField` always had. Real,
honestly-scoped v1 -- see `add_code_editor`'s own Rust doc comment for
the full list of deliberately deferred pieces at the time (syntax
highlighting, a line-number gutter, scroll/clip past the box's own
edges, a bundled monospace font, Tab-key indentation capture); what
*is* real here: `Enter` inserts a genuine newline, `Home`/`End`
operate on the current line rather than the whole buffer, and
`ArrowUp`/`ArrowDown` navigate by line, preserving the caret's own
real column. M31 Phase 1 (Line-Number Gutter, `examples/code_editor_
gutter.py`) and M31 Phase 2 (Tab-Key Indentation Capture, demonstrated
below) have since closed two of those real gaps. M31 Phase 3
(Tab/Space Indicators) closed a third: `add_code_editor` now shows
real space/tab characters as visible `·`/`→` glyphs at paint time --
purely visual, `get_text()` always reads back the real, unsubstituted
content, proven below. M31 Phase 4 (Syntax Highlighting) closed a
fourth: `Node.set_syntax_spans` paints real per-byte-range colors --
app-side tokenization only (Design Principle 6, this script's own
tiny keyword tokenizer below), no engine-bundled lexer. M32 Phase 1
(§5, §8, §10) closed the fifth: `add_code_editor` now always shapes
with a real bundled monospace face ("Hack Nerd Font Mono") instead of
falling back to the general-purpose `Roboto` this step originally had
to use -- a genuinely monospace editor at last.

What this script proves automatically (headless-CI-safe, no human
needed): a real click focuses the editor; a real `Enter` keypress
splits one line into two; `Home` targets the current line, not byte 0;
`ArrowUp`/`ArrowDown` genuinely move between lines (proven by typing
after navigating and checking exactly which line received the new
text); a real `Tab` keypress inserts a genuine `\\t` rather than moving
focus away (M31 Phase 2), and reads back unsubstituted despite the
real visible `→` glyph it paints as (M31 Phase 3); and a real render
loop paints the whole multiline buffer -- whitespace indicators
included -- over actual frames without crashing. The definitive proof
that a real `\\n` produces a real, vertically-stacked second layout
line (not just accepted into `content` with no visual effect) is
`crates/engine-render/tests/text_field_paint.rs::
a_multiline_fields_own_newline_produces_a_real_second_layout_line`;
the definitive proof of the real space/tab byte-offset remapping is
`crates/engine-render/src/text.rs::
display_offset_mapping_round_trips_every_real_char_boundary` -- not
this script.
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

# M31 Phase 2: a real Tab keypress indents in place -- the cursor is
# still on the "return" line just typed on; Home first so the inserted
# tab lands at the real line's own start, not mid-word.
window.press_key("home")
window.press_key("tab")
print(f"after Tab-key indentation:\n{editor.get_text()}")
assert editor.get_text() == (
    "def add(a, b):\n>>    # adds two numbers\n\t    return a + b  # end"
)
assert editor.is_focused(), "claiming Tab for indentation must never lose focus over it"

# M31 Phase 4: a real, minimal app-side tokenizer (no engine-bundled
# lexer -- Design Principle 6) colors every real "def"/"return" keyword
# occurrence; everything else keeps the editor's own default text color.
KEYWORD_COLOR = (0xC0, 0x1C, 0x28, 0xFF)


def keyword_spans(text: str) -> list[tuple[int, int, tuple[int, int, int, int]]]:
    spans = []
    for keyword in ("def", "return"):
        start = 0
        while (found := text.find(keyword, start)) != -1:
            spans.append((found, found + len(keyword), KEYWORD_COLOR))
            start = found + len(keyword)
    return spans


editor.set_syntax_spans(keyword_spans(editor.get_text()))
print(f"real syntax spans for the current buffer: {keyword_spans(editor.get_text())}")
assert editor.get_text() == (
    "def add(a, b):\n>>    # adds two numbers\n\t    return a + b  # end"
), "set_syntax_spans must never touch the real content it colors"

app = App()
app.add_window(window)
app.run(max_frames=60)
print("code_editor.py: exited cleanly after 60 frames -- a real 3-line buffer, edited live")
