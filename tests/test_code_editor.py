"""M30 Phase 9 Step 3 (§5, §8, §10): real, repeatable coverage of
`Window.add_code_editor` -- a real, genuinely multiline `TextField`
(`TextFieldState.multiline = true`), closing the real, stated
single-line-only gap `TextField` always had (`Tree::dispatch_text_
field_key`'s own original "Enter is consumed but never inserts a
newline... real, stated, single-line scope" comment, predating this
step).

The definitive proof that a real `\\n` genuinely produces a second,
vertically-stacked `parley` layout line (not just accepted into
`content` without visual effect) is `crates/engine-render/tests/
text_field_paint.rs`'s own `a_multiline_fields_own_newline_produces_a_
real_second_layout_line`, not this file -- the same "FFI wiring only"
split `test_text_field.py`'s own module doc comment already
established. This file proves, through real keyboard dispatch (the
same `press_key`/`type_text`/`get_text` round-trip `test_text_field.py`
already uses): `add_code_editor` returns a real, usable, click-
focusable `Node`; `Enter` inserts a real newline; `Home` jumps to the
*current line's* own start, not the whole buffer's; `ArrowUp`/
`ArrowDown` navigate by line, landing typed text on the real, expected
line.
"""

import pytest

from tre import Node, Window


def test_add_code_editor_returns_a_node():
    window = Window(width=400, height=300)
    node = window.add_code_editor(content="", background=(255, 255, 255, 255), width=300, height=150)
    assert isinstance(node, Node)


def test_add_code_editor_seeds_the_real_initial_content():
    window = Window(width=400, height=300)
    node = window.add_code_editor(
        content="def f():\n    pass", background=(255, 255, 255, 255), width=300, height=150
    )
    assert node.get_text() == "def f():\n    pass"


def test_a_themed_code_editor_does_not_raise():
    window = Window(width=400, height=300)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    node = window.add_code_editor(content="", background=(255, 255, 255, 255), width=300, height=150)
    assert isinstance(node, Node)


def test_a_click_focuses_the_code_editor():
    window = Window(width=400, height=300)
    editor = window.add_code_editor(content="x", background=(255, 255, 255, 255), width=300, height=150)
    assert editor.is_focused() is False
    window.click(editor)
    assert editor.is_focused() is True


def test_enter_inserts_a_real_newline_not_consumed_like_a_plain_text_field():
    window = Window(width=400, height=300)
    editor = window.add_code_editor(content="ab", background=(255, 255, 255, 255), width=300, height=150)
    window.click(editor)
    window.press_key("home")
    window.press_key("right")
    window.press_key("enter")
    assert editor.get_text() == "a\nb", (
        "Enter on a real Code Editor must insert a genuine newline, unlike a plain "
        f"single-line TextField, got {editor.get_text()!r}"
    )


def test_home_jumps_to_the_current_line_not_the_whole_buffer():
    window = Window(width=400, height=300)
    editor = window.add_code_editor(
        content="one\ntwo\nthree", background=(255, 255, 255, 255), width=300, height=150
    )
    window.click(editor)
    # A fresh field's own real cursor starts at content's own end,
    # inside "three" -- Home here must only ever reach "three"'s own
    # real start, never byte 0.
    window.press_key("home")
    window.type_text("X")
    assert editor.get_text() == "one\ntwo\nXthree"


def test_end_jumps_to_the_current_lines_own_end():
    window = Window(width=400, height=300)
    editor = window.add_code_editor(
        content="one\ntwo\nthree", background=(255, 255, 255, 255), width=300, height=150
    )
    window.click(editor)
    for _ in range(len("one\ntwo\nthree")):
        window.press_key("left")
    window.press_key("right")
    window.press_key("right")
    window.press_key("right")  # cursor now right after "one"
    window.press_key("end")
    window.type_text("X")
    assert editor.get_text() == "oneX\ntwo\nthree"


def test_arrow_up_navigates_to_the_previous_line():
    window = Window(width=400, height=300)
    editor = window.add_code_editor(
        content="line1\nline2\nline3", background=(255, 255, 255, 255), width=300, height=150
    )
    window.click(editor)
    # Cursor starts at content's own end, inside "line3" -- two real
    # ArrowUp presses must land somewhere on "line1".
    window.press_key("up")
    window.press_key("up")
    window.press_key("home")
    window.type_text("X")
    assert editor.get_text() == "Xline1\nline2\nline3"


def test_arrow_down_navigates_to_the_next_line():
    window = Window(width=400, height=300)
    editor = window.add_code_editor(
        content="line1\nline2\nline3", background=(255, 255, 255, 255), width=300, height=150
    )
    window.click(editor)
    for _ in range(len("line1\nline2\nline3")):
        window.press_key("left")
    window.press_key("down")
    window.press_key("home")
    window.type_text("X")
    assert editor.get_text() == "line1\nXline2\nline3"


def test_tab_inserts_a_real_tab_character_instead_of_moving_focus():
    """M31 Phase 2 (§5, §8, §10): a focused Code Editor claims Tab for
    real indentation now, rather than falling through to ordinary
    focus traversal the way every other node (and a single-line
    TextField, proven below) still does.
    """
    window = Window(width=400, height=300)
    editor = window.add_code_editor(
        content="ab", background=(255, 255, 255, 255), width=300, height=150
    )
    window.click(editor)
    assert editor.is_focused()
    window.press_key("home")
    window.press_key("right")
    window.press_key("tab")
    assert editor.get_text() == "a\tb"
    assert editor.is_focused(), "claiming Tab for indentation must never lose focus over it"


def test_tab_still_moves_focus_away_from_a_single_line_text_field():
    """The real, deliberate scope boundary: Tab-as-indentation is
    Code-Editor-specific (multiline only) -- an ordinary single-line
    `TextField` must keep its own prior real behavior, byte-for-byte.
    """
    window = Window(width=400, height=300)
    field = window.add_text_field(
        content="ab", background=(255, 255, 255, 255), width=200, height=30
    )
    # A second focusable node, so Tab genuinely has somewhere else to
    # land -- with only one focusable node in the tree, focus
    # traversal would trivially wrap back onto itself either way.
    window.add_text_field(content="", background=(255, 255, 255, 255), width=200, height=30)
    window.click(field)
    assert field.is_focused()
    window.press_key("tab")
    assert not field.is_focused(), "Tab on a single-line field must still move focus away"
    assert field.get_text() == "ab", "Tab must not insert anything into a single-line field"


def test_whitespace_indicators_never_touch_the_real_content():
    """M31 Phase 3 (§5, §8): `show_whitespace` is paint-only -- a real
    space/tab in a Code Editor's own content must read back exactly as
    typed via `get_text()`, never as the substituted `·`/`→` glyphs
    `engine-render`'s own `draw_field` paints instead. The real
    substitution/byte-offset-remapping claim itself is proven at the
    Rust level (`crates/engine-render/src/text.rs`'s own
    `display_offset_mapping_round_trips_every_real_char_boundary`, and
    `crates/engine-render/tests/text_field_paint.rs`'s own
    `hit_test_position_on_a_field_with_visible_whitespace_returns_
    real_content_offsets`); this test proves the real FFI-level
    guarantee pytest actually can prove without a live render loop.
    """
    window = Window(width=400, height=300)
    editor = window.add_code_editor(
        content="a b\tc", background=(255, 255, 255, 255), width=300, height=150
    )
    assert editor.get_text() == "a b\tc"

    window.click(editor)
    window.type_text(" x\ty")
    assert editor.get_text() == "a b\tc x\ty", (
        "typing more spaces/tabs into a whitespace-indicator-showing editor "
        "must still land in get_text() completely unsubstituted"
    )


def test_set_syntax_spans_does_not_raise_and_never_touches_real_content():
    """M31 Phase 4 (§5, §8): `set_syntax_spans` is paint-only -- the
    real per-run-color proof itself is at the Rust level
    (`crates/engine-render/tests/text_field_paint.rs`'s own
    `syntax_spans_color_only_their_own_real_byte_range`, a real pixel-
    diff proof no spans leak past their own byte range); this test
    proves the real FFI surface: real spans can be set without
    raising, and `get_text()` still reads back exactly what was typed.
    """
    window = Window(width=400, height=300)
    editor = window.add_code_editor(
        content="if x:\n    y", background=(255, 255, 255, 255), width=300, height=150
    )
    editor.set_syntax_spans(
        [
            (0, 2, (0xC0, 0x1C, 0x28, 0xFF)),  # "if" -- keyword-red
            (6, 10, (0x21, 0x6D, 0xFF, 0xFF)),  # the indent -- irrelevant-blue
        ]
    )
    assert editor.get_text() == "if x:\n    y"

    # Real spans replace the whole list every call, matching a real
    # re-tokenize-on-every-edit app pattern -- an empty list is a real,
    # valid way to clear all coloring.
    editor.set_syntax_spans([])
    assert editor.get_text() == "if x:\n    y"


def test_set_syntax_spans_rejects_a_non_text_field_node():
    window = Window(width=400, height=300)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'syntax_spans'"):
        rect.set_syntax_spans([(0, 1, (255, 0, 0, 255))])
