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
from helpers import CODE_EDITOR, add


def test_add_code_editor_returns_a_node():
    window = Window(width=400, height=300)
    node = add(window, "text_input", **CODE_EDITOR, text="", width=300, height=150, font_size=14)
    assert isinstance(node, Node)


def test_add_code_editor_seeds_the_real_initial_content():
    window = Window(width=400, height=300)
    node = add(window, "text_input", **CODE_EDITOR, text="def f():\n    pass", width=300, height=150, font_size=14)
    assert node.get("text") == "def f():\n    pass"


def test_a_click_focuses_the_code_editor():
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="x", width=300, height=150, font_size=14)
    assert editor.get("focused") is False
    window.simulate("click", node=editor)
    assert editor.get("focused") is True


def test_enter_inserts_a_real_newline_not_consumed_like_a_plain_text_field():
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="ab", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    window.simulate("key_down", key="home")
    window.simulate("key_down", key="arrow_right")
    window.simulate("key_down", key="enter")
    assert editor.get("text") == "a\nb", (
        "Enter on a real Code Editor must insert a genuine newline, unlike a plain "
        f"single-line TextField, got {editor.get("text")!r}"
    )


def test_home_jumps_to_the_current_line_not_the_whole_buffer():
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="one\ntwo\nthree", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    # A fresh field's own real cursor starts at content's own end,
    # inside "three" -- Home here must only ever reach "three"'s own
    # real start, never byte 0.
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
    # Cursor starts at content's own end, inside "line3" -- two real
    # ArrowUp presses must land somewhere on "line1".
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


def test_arrow_up_and_down_remember_a_real_goal_column_through_a_shorter_line():
    """M38 Phase 2 (§5, §8): real goal-column memory -- a consecutive
    run of ArrowUp presses must keep landing at the *original* column
    even after an intermediate shorter line clamps the real cursor to
    something smaller, not silently adopt that clamped column as a new
    goal. Uses the same real `type_text`-then-`get_text()` positional
    probe `test_arrow_up_navigates_to_the_previous_line` already
    establishes -- there is no Python-level getter for a TextField's
    own raw cursor offset, so where a typed marker character lands is
    the real, observable proof.
    """
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="alphabet\nhi\nbanana", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    # Cursor starts at content's own end, inside "banana" -- real
    # column 6. One ArrowUp clamps onto "hi" (only 2 real columns);
    # a second, consecutive ArrowUp must recall the real *original*
    # column 6, landing right before "alphabet"'s own 'e' (index 6),
    # not "hi"'s own clamped column 2 (which would land before 'p').
    window.simulate("key_down", key="arrow_up")
    window.simulate("key_down", key="arrow_up")
    window.simulate("input", text="X")
    assert editor.get("text") == "alphabXet\nhi\nbanana"


def test_a_non_vertical_move_resets_the_remembered_goal_column():
    """M38 Phase 2 (§5, §8): only a genuinely *consecutive* run of
    ArrowUp/ArrowDown remembers a goal column -- an ArrowLeft in
    between must reset it, so the next ArrowUp derives a fresh goal
    from wherever the cursor now really sits, not a stale one from
    before the interrupt.
    """
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="alphabet\nhi\nbanana", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    window.simulate("key_down", key="arrow_up")  # lands on "hi"'s own end (clamped from column 6 to 2)
    window.simulate("key_down", key="arrow_left")  # ordinary horizontal move -- real column now 1
    window.simulate("key_down", key="arrow_up")
    window.simulate("input", text="X")
    assert editor.get("text") == "aXlphabet\nhi\nbanana"


def test_tab_inserts_a_real_tab_character_instead_of_moving_focus():
    """M31 Phase 2 (§5, §8, §10): a focused Code Editor claims Tab for
    real indentation now, rather than falling through to ordinary
    focus traversal the way every other node (and a single-line
    TextField, proven below) still does.
    """
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="ab", width=300, height=150, font_size=14)
    window.simulate("click", node=editor)
    assert editor.get("focused")
    window.simulate("key_down", key="home")
    window.simulate("key_down", key="arrow_right")
    window.simulate("key_down", key="tab")
    assert editor.get("text") == "a\tb"
    assert editor.get("focused"), "claiming Tab for indentation must never lose focus over it"


def test_tab_still_moves_focus_away_from_a_single_line_text_field():
    """The real, deliberate scope boundary: Tab-as-indentation is
    Code-Editor-specific (multiline only) -- an ordinary single-line
    `TextField` must keep its own prior real behavior, byte-for-byte.
    """
    window = Window(width=400, height=300)
    field = add(window, "text_input", text="ab", width=200, height=30)
    # A second focusable node, so Tab genuinely has somewhere else to
    # land -- with only one focusable node in the tree, focus
    # traversal would trivially wrap back onto itself either way.
    add(window, "text_input", text="", width=200, height=30)
    window.simulate("click", node=field)
    assert field.get("focused")
    window.simulate("key_down", key="tab")
    assert not field.get("focused"), "Tab on a single-line field must still move focus away"
    assert field.get("text") == "ab", "Tab must not insert anything into a single-line field"


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
    editor = add(window, "text_input", **CODE_EDITOR, text="a b\tc", width=300, height=150, font_size=14)
    assert editor.get("text") == "a b\tc"

    window.simulate("click", node=editor)
    window.simulate("input", text=" x\ty")
    assert editor.get("text") == "a b\tc x\ty", (
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
    editor = add(window, "text_input", **CODE_EDITOR, text="if x:\n    y", width=300, height=150, font_size=14)
    editor.set(syntax_spans=[
            (0, 2, (0xC0, 0x1C, 0x28, 0xFF)),  # "if" -- keyword-red
            (6, 10, (0x21, 0x6D, 0xFF, 0xFF)),  # the indent -- irrelevant-blue
        ])
    assert editor.get("text") == "if x:\n    y"

    # Real spans replace the whole list every call, matching a real
    # re-tokenize-on-every-edit app pattern -- an empty list is a real,
    # valid way to clear all coloring.
    editor.set(syntax_spans=[])
    assert editor.get("text") == "if x:\n    y"


def test_set_syntax_spans_rejects_a_non_text_field_node():
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="`syntax_spans` applies only to a text_input node"):
        rect.set(syntax_spans=[(0, 1, (255, 0, 0, 255))])


def test_set_folded_ranges_does_not_raise_and_never_touches_real_content():
    """M31 Phase 5 (§5, §8): `set_folded_ranges` is paint-only -- the
    real proof that a folded range genuinely paints differently (and
    that a click past it resolves to a real content offset) is at the
    Rust level (`crates/engine-render/tests/text_field_paint.rs`'s own
    `a_folded_range_paints_genuinely_different_pixels_than_unfolded`/
    `hit_test_position_on_a_folded_field_returns_real_content_
    offsets`); this test proves the real FFI surface.
    """
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="def add(a, b):\n    return a + b", width=300, height=150, font_size=14)
    editor.set(folded_ranges=[(15, 33)])  # collapses the whole function body
    assert editor.get("text") == "def add(a, b):\n    return a + b"

    # Real ranges replace the whole list every call -- an empty list
    # is a real, valid way to clear all folding.
    editor.set(folded_ranges=[])
    assert editor.get("text") == "def add(a, b):\n    return a + b"


def test_set_folded_ranges_rejects_a_non_text_field_node():
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="`folded_ranges` applies only to a text_input node"):
        rect.set(folded_ranges=[(0, 1)])


def test_arrow_down_snaps_the_cursor_out_of_a_folded_range_it_would_otherwise_land_inside():
    """M38 Phase 3 (§5, §8): real Python-level proof that `Home`/`End`/
    `ArrowUp`/`ArrowDown` are fold-aware -- the same real
    `type_text`-after-navigation-then-`get_text()` positional probe
    the goal-column tests above already establish (there is no Python
    getter for the raw cursor offset, so where a typed marker lands is
    the real, observable proof).
    """
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="one\ntwo\nthree\nfour", width=300, height=150, font_size=14)
    editor.set(folded_ranges=[(4, 15)])  # "two\nthree\nfo" folded away
    window.simulate("click", node=editor)
    for _ in range(len("one\ntwo\nthree\nfour")):
        window.simulate("key_down", key="arrow_left")
    window.simulate("key_down", key="arrow_right")
    window.simulate("key_down", key="arrow_right")  # cursor now at real column 2, inside "one"
    # ArrowDown's own natural landing (real column 2 into "two", byte 6)
    # sits strictly inside the fold 4..15 -- must snap forward to byte
    # 15, right after the fold's own real marker, into "four".
    window.simulate("key_down", key="arrow_down")
    window.simulate("input", text="X")
    assert editor.get("text") == "one\ntwo\nthree\nfXour"


def test_folding_and_syntax_highlighting_compose_without_raising():
    """M31 Phase 5 (§5, §8): real proof that folding and syntax
    coloring -- both real, independent paint transforms in `draw_
    field`, each chaining its own real byte-offset remap through the
    other -- can be active simultaneously without raising, and a real
    edit afterward still reads back exactly. A real render loop
    exercising this exact combination was already run manually before
    writing this suite (per this project's own established discipline
    of a real empirical check before trusting pytest); a *second* real
    `App.run()` call is deliberately not added here -- a real, already
    -found hazard in this same test session (a second real event-loop
    invocation within one pytest process can break an unrelated,
    already-passing real render-loop test elsewhere in the suite).
    """
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text="def add(a, b):\n    return a + b", width=300, height=150, font_size=14)
    editor.set(folded_ranges=[(15, 33)])
    editor.set(syntax_spans=[(0, 3, (0xC0, 0x1C, 0x28, 0xFF))])
    window.simulate("click", node=editor)
    window.simulate("input", text="X")
    assert editor.get("text") == "def add(a, b):\n    return a + bX"


def test_navigating_and_editing_still_works_correctly_in_a_genuinely_overflowing_editor():
    """M38 Phase 7 (§5, §8): real scroll+clip+caret-follow for a Code
    Editor whose real content is much taller than its own box -- there
    is no Python getter for the raw `scroll_offset` itself, the same
    real verification-surface limit already established for other
    internal-only state (M37/M38 Phase 2/3/4/5/6); the exact real
    scroll math is proven directly at the Rust level (`crates/engine-
    core/src/tree.rs`'s own `scroll_text_field_caret_into_view_*`
    tests) and the real clip/paint effect at the pixel level
    (`crates/engine-render/tests/text_field_paint.rs`'s own `a_
    genuinely_overflowing_multiline_field_clips_its_own_content_to_
    its_own_box`/`a_nonzero_scroll_offset_paints_genuinely_different_
    pixels_than_unscrolled`). This proves the real, full FFI surface
    (construct, navigate far past the visible viewport, edit, click)
    still resolves correctly, not just that it doesn't crash.
    """
    content = "\n".join(f"line{i}" for i in range(30))
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text=content, width=300, height=80, font_size=14)
    window.simulate("click", node=editor)
    # Cursor starts at content's own real end (line29) -- 40 ArrowUps
    # walks well past the real ~4-line visible viewport, all the way
    # back up to line0's own start.
    for _ in range(40):
        window.simulate("key_down", key="arrow_up")
    window.simulate("key_down", key="home")
    window.simulate("input", text="X")
    assert editor.get("text").startswith("Xline0\n"), (
        "typing at the real content start after scrolling far past the visible viewport must "
        "still land on the real, correct line"
    )

    # A real click still resolves to a real, valid position after all
    # that real scrolling, rather than panicking or silently no-oping
    # -- exercises the same `text_field_hit_offset` (`engine-py/src/
    # app.rs`) this phase's own removal of `TextFieldState`'s `Clone`
    # derive required fixing. Not asserting *which* line it lands on
    # (a real click resolves to wherever the editor's own current
    # center point is, which line that is depends on exact real font
    # metrics this test can't predict) -- a real, exactly-one-
    # character-longer content is the decisive, position-independent
    # proof the click found a real, valid insertion point.
    before = editor.get("text")
    window.simulate("click", node=editor)
    window.simulate("input", text="Y")
    assert len(editor.get("text")) == len(before) + 1, (
        "a real click after scrolling must still focus and insert at a real, valid position"
    )


def test_navigating_horizontally_still_works_correctly_in_a_genuinely_overflowing_line():
    """M39 Phase 1 (§5, §8): the identical real horizontal sibling of
    M38 Phase 7's own vertical overflow test above -- there is no
    Python getter for `horizontal_scroll_offset` itself, the same
    real verification-surface limit; the exact real scroll math is
    proven directly at the Rust level (`crates/engine-core/src/
    tree.rs`'s own `scroll_text_field_caret_into_view_scrolls_right_*`/
    `..._scrolls_back_left_*` tests) and the real clip/paint effect at
    the pixel level (`crates/engine-render/tests/text_field_paint.rs`'s
    own `a_nonzero_horizontal_scroll_offset_paints_genuinely_different_
    pixels_than_unscrolled`). This proves the real, full FFI surface
    still resolves correctly for a single real line much wider than
    the box.
    """
    content = "a" * 60
    window = Window(width=400, height=300)
    editor = add(window, "text_input", **CODE_EDITOR, text=content, width=100, height=80, font_size=14)
    window.simulate("click", node=editor)
    # Cursor starts at content's own real end (column 60) -- far past
    # the real ~10-character visible viewport at font_size 14. 60
    # ArrowLefts walks all the way back to column 0.
    for _ in range(60):
        window.simulate("key_down", key="arrow_left")
    window.simulate("input", text="X")
    assert editor.get("text").startswith("X"), (
        "typing at the real line start after scrolling far past the visible horizontal "
        "viewport must still land at the real, correct column"
    )
