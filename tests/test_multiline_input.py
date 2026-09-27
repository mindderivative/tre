"""A multiline text input set up as a code editor (`helpers.CODE_EDITOR`):
Enter and Tab insert, Home/End/arrows move by line with a remembered goal
column, `show_whitespace`/`syntax_spans`/`folded_ranges` are paint-only, and
editing still works when the text overflows the box. Where a typed marker
lands is the probe for the caret. The pixels are `text_field_paint.rs`'s job.
"""

import pytest

from tre import Node, Window
from helpers import CODE_EDITOR, add


def test_create_multiline_input_returns_a_node():
    window = Window(width=400, height=300)
    node = add(window, "text_input", **CODE_EDITOR, text="", width=300, height=150, font_size=14)
    assert isinstance(node, Node)


def test_create_multiline_input_seeds_its_text():
    window = Window(width=400, height=300)
    node = add(window, "text_input", **CODE_EDITOR, text="def f():\n    pass", width=300, height=150, font_size=14)
    assert node.get("text") == "def f():\n    pass"


def test_a_click_focuses_the_multiline_input():
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="x", width=300, height=150, font_size=14)
    assert editor.get("focused") is False
    window.simulate("click", node=editor)
    assert editor.get("focused") is True


def test_enter_inserts_a_newline_unlike_a_single_line_input():
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="ab", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    window.simulate("key_down", key="home")
    window.simulate("key_down", key="arrow_right")
    window.simulate("key_down", key="enter")
    assert editor.get("text") == "a\nb", (
        "Enter in a multiline input must insert a newline, unlike a single-line "
        f"one, got {editor.get("text")!r}"
    )


def test_home_jumps_to_the_current_line_not_the_whole_buffer():
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="one\ntwo\nthree", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    # The caret starts at the end, inside "three" -- Home must reach
    # "three"'s start, never byte 0.
    window.simulate("key_down", key="home")
    window.simulate("input", text="X")
    assert editor.get("text") == "one\ntwo\nXthree"


def test_end_jumps_to_the_current_lines_own_end():
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="one\ntwo\nthree", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    for _ in range(len("one\ntwo\nthree")):
        window.simulate("key_down", key="arrow_left")
    window.simulate("key_down", key="arrow_right")
    window.simulate("key_down", key="arrow_right")
    window.simulate("key_down", key="arrow_right")  # cursor now right after "one"
    window.simulate("key_down", key="end")
    window.simulate("input", text="X")
    assert editor.get("text") == "oneX\ntwo\nthree"


def test_arrow_up_navigates_to_the_previous_line():
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="line1\nline2\nline3", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    # The caret starts at the end, inside "line3" -- two ArrowUp presses
    # must land on "line1".
    window.simulate("key_down", key="arrow_up")
    window.simulate("key_down", key="arrow_up")
    window.simulate("key_down", key="home")
    window.simulate("input", text="X")
    assert editor.get("text") == "Xline1\nline2\nline3"


def test_arrow_down_navigates_to_the_next_line():
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="line1\nline2\nline3", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    for _ in range(len("line1\nline2\nline3")):
        window.simulate("key_down", key="arrow_left")
    window.simulate("key_down", key="arrow_down")
    window.simulate("key_down", key="home")
    window.simulate("input", text="X")
    assert editor.get("text") == "line1\nXline2\nline3"


def test_arrow_up_and_down_remember_a_goal_column_through_a_shorter_line():
    """Consecutive ArrowUp presses keep the original column even after a
    shorter line in between clamps the caret."""
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="alphabet\nhi\nbanana", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    # The caret starts at the end of "banana", column 6. One ArrowUp
    # clamps onto "hi" (2 columns); a second must recall column 6, landing
    # before "alphabet"'s 'e', not the clamped column 2 (before 'p').
    window.simulate("key_down", key="arrow_up")
    window.simulate("key_down", key="arrow_up")
    window.simulate("input", text="X")
    assert editor.get("text") == "alphabXet\nhi\nbanana"


def test_a_non_vertical_move_resets_the_remembered_goal_column():
    """An ArrowLeft between vertical moves resets the goal column to
    wherever the caret now sits."""
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="alphabet\nhi\nbanana", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    window.simulate("key_down", key="arrow_up")  # lands on "hi"'s own end (clamped from column 6 to 2)
    window.simulate("key_down", key="arrow_left")  # ordinary horizontal move -- column now 1
    window.simulate("key_down", key="arrow_up")
    window.simulate("input", text="X")
    assert editor.get("text") == "aXlphabet\nhi\nbanana"


def test_tab_inserts_a_tab_character_instead_of_moving_focus():
    """A focused multiline input takes Tab as indentation instead of
    moving focus."""
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="ab", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    assert editor.get("focused")
    window.simulate("key_down", key="home")
    window.simulate("key_down", key="arrow_right")
    window.simulate("key_down", key="tab")
    assert editor.get("text") == "a\tb"
    assert editor.get("focused"), "claiming Tab for indentation must never lose focus over it"


def test_tab_still_moves_focus_away_from_a_single_line_input():
    """Tab-as-indentation is multiline-only: a single-line input still
    moves focus on Tab."""
    window = Window(width=400, height=300)
    field = add(window, "text_input", text="ab", width=200, height=30)
    # A second focusable node, so Tab has somewhere else to land.
    add(window, "text_input", text="", width=200, height=30)
    window.simulate("click", node=field)
    assert field.get("focused")
    window.simulate("key_down", key="tab")
    assert not field.get("focused"), "Tab on a single-line field must still move focus away"
    assert field.get("text") == "ab", "Tab must not insert anything into a single-line field"


def test_whitespace_indicators_never_touch_the_text():
    """`show_whitespace` is paint-only: spaces and tabs read back from
    `get("text")` as typed, never as the `·`/`→` glyphs painted for them.
    """
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="a b\tc", width=300, height=150, font_size=14)
    assert editor.get("text") == "a b\tc"

    window.simulate("click", node=editor)
    window.simulate("input", text=" x\ty")
    assert editor.get("text") == "a b\tc x\ty", (
        "typing more spaces/tabs into a whitespace-indicator-showing editor "
        "must still read back from get(\"text\") unsubstituted"
    )


def test_syntax_spans_are_settable_and_never_touch_the_text():
    """`syntax_spans` is paint-only: setting it leaves the text as it
    was."""
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="if x:\n    y", width=300, height=150, font_size=14)
    editor.set(syntax_spans=[
            (0, 2, (0xC0, 0x1C, 0x28, 0xFF)),  # "if" -- keyword-red
            (6, 10, (0x21, 0x6D, 0xFF, 0xFF)),  # the indent -- irrelevant-blue
        ])
    assert editor.get("text") == "if x:\n    y"

    # Each set replaces the whole list; an empty list clears all coloring.
    editor.set(syntax_spans=[])
    assert editor.get("text") == "if x:\n    y"


def test_syntax_spans_rejects_a_non_text_input_node():
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="`syntax_spans` applies only to a text_input node"):
        rect.set(syntax_spans=[(0, 1, (255, 0, 0, 255))])


def test_folded_ranges_are_settable_and_never_touch_the_text():
    """`folded_ranges` is paint-only: setting it leaves the text as it
    was."""
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="def add(a, b):\n    return a + b", width=300, height=150, font_size=14)
    editor.set(folded_ranges=[(15, 33)])  # collapses the whole function body
    assert editor.get("text") == "def add(a, b):\n    return a + b"

    # Each set replaces the whole list; an empty list clears all folding.
    editor.set(folded_ranges=[])
    assert editor.get("text") == "def add(a, b):\n    return a + b"


def test_folded_ranges_rejects_a_non_text_input_node():
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="`folded_ranges` applies only to a text_input node"):
        rect.set(folded_ranges=[(0, 1)])


def test_arrow_down_snaps_the_cursor_out_of_a_folded_range_it_would_otherwise_land_inside():
    """Vertical caret movement is fold-aware: it never lands inside a
    folded range."""
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="one\ntwo\nthree\nfour", width=300, height=150, font_size=14)
    editor.set(folded_ranges=[(4, 15)])  # "two\nthree\nfo" folded away
    window.simulate("click", node=editor)
    for _ in range(len("one\ntwo\nthree\nfour")):
        window.simulate("key_down", key="arrow_left")
    window.simulate("key_down", key="arrow_right")
    window.simulate("key_down", key="arrow_right")  # caret now at column 2, inside "one"
    # ArrowDown's natural landing (column 2 of "two", byte 6) is inside
    # the fold 4..15 -- it must snap forward to byte 15, into "four".
    window.simulate("key_down", key="arrow_down")
    window.simulate("input", text="X")
    assert editor.get("text") == "one\ntwo\nthree\nfXour"


def test_folding_and_syntax_highlighting_compose_without_raising():
    """Folding and syntax coloring can be active together, and an edit
    afterward reads back exactly. (No `App.run()` here: a second real
    event loop in one pytest process can break other render-loop tests.)
    """
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="def add(a, b):\n    return a + b", width=300, height=150, font_size=14)
    editor.set(folded_ranges=[(15, 33)])
    editor.set(syntax_spans=[(0, 3, (0xC0, 0x1C, 0x28, 0xFF))])
    window.simulate("click", node=editor)
    window.simulate("input", text="X")
    assert editor.get("text") == "def add(a, b):\n    return a + bX"


def test_navigating_and_editing_work_when_the_text_overflows_vertically():
    """Text much taller than the box: navigating far past the visible
    lines, editing, and clicking still land on the right text. The scroll
    math and clipping are covered by `engine-core`'s tree tests and
    `text_field_paint.rs`.
    """
    content = "\n".join(f"line{i}" for i in range(30))
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text=content, width=300, height=80, font_size=14)
    window.simulate("click", node=editor)
    # The caret starts at the end (line29); 40 ArrowUps walk well past the
    # ~4 visible lines back to line0.
    for _ in range(40):
        window.simulate("key_down", key="arrow_up")
    window.simulate("key_down", key="home")
    window.simulate("input", text="X")
    assert editor.get("text").startswith("Xline0\n"), (
        "typing at the start after scrolling far past the visible lines must "
        "still land on the right line"
    )

    # A click after scrolling still resolves to a valid position. Which
    # line it lands on depends on font metrics, so the check is only that
    # typing afterward adds exactly one character.
    before = editor.get("text")
    window.simulate("click", node=editor)
    window.simulate("input", text="Y")
    assert len(editor.get("text")) == len(before) + 1, (
        "a click after scrolling must still focus and insert at a valid position"
    )


def test_navigating_works_when_a_line_overflows_horizontally():
    """The horizontal counterpart of the test above: one line much wider
    than the box."""
    content = "a" * 60
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text=content, width=100, height=80, font_size=14)
    window.simulate("click", node=editor)
    # The caret starts at column 60, far past the ~10 visible characters;
    # 60 ArrowLefts walk back to column 0.
    for _ in range(60):
        window.simulate("key_down", key="arrow_left")
    window.simulate("input", text="X")
    assert editor.get("text").startswith("X"), (
        "typing at the line start after scrolling far past the visible "
        "characters must still land at the right column"
    )
