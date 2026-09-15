#!/usr/bin/env python3
"""Phase 15 Step 15.2 proof: real multi-line text *editing* --
`tre.EditableText(..., wrap_width=...)` gains `line_count()`,
`hit_test_2d()`, `move_caret_up()`/`move_caret_down()` (real "sticky
column" vertical movement), and `selection_rects()`, built on Step
15.1's real `Text.wrap_width` rendering and `tre_text::wrap_lines`.

No GPU renderer is needed here -- unlike Step 15.1's rendering proof,
every method exercised in this demo is pure computation (shaping +
line-breaking), not an actual render, so this demo runs headless with
no `HeadlessRenderer` at all.

`insert`/`delete_selection`/`delete_backward`/`set_caret`/
`set_selection`/`copy`/`cut`/`paste`/`handle_ime` are all deliberately
UNCHANGED by this step (they only ever touch the flat byte string, never
line layout) -- the cut/copy/paste check below proves that real reuse
holds under multi-line text, not just single-line.
"""

import tre_python as tre


def make_editor(text: str, wrap_width=None) -> tre.EditableText:
    font = tre.Font.system_cascade()
    return tre.EditableText(0.0, 0.0, text, font, 16.0, tre.rgba8(0, 0, 0, 255), wrap_width)


# "abc\ndefgh\nij" -- 3 real hard-wrapped lines with known, exact byte
# ranges: line0 "abc\n" [0,4), line1 "defgh\n" [4,10), line2 "ij" [10,12).
HARD_WRAP_TEXT = "abc\ndefgh\nij"


def check_line_count_and_boundary_no_ops() -> None:
    editable = make_editor(HARD_WRAP_TEXT, wrap_width=float("inf"))
    assert editable.line_count() == 3, f"expected 3 real hard-wrapped lines, got {editable.line_count()}"
    print("line_count() on a 3-line hard-wrapped text: 3 -- OK")

    editable.set_caret(0)
    moved = editable.move_caret_up()
    assert moved is False, "move_caret_up() on the first line must be a real no-op"
    assert editable.caret == 0, "a no-op move must leave the caret exactly where it was"
    print("move_caret_up() on line 0 is a real no-op -- OK")

    editable.set_caret(len(HARD_WRAP_TEXT))
    moved = editable.move_caret_down()
    assert moved is False, "move_caret_down() on the last line must be a real no-op"
    assert editable.caret == len(HARD_WRAP_TEXT)
    print("move_caret_down() on the last line is a real no-op -- OK")


def check_sticky_column_vertical_movement() -> None:
    editable = make_editor(HARD_WRAP_TEXT, wrap_width=float("inf"))

    # Column 0 (x=0.0) is font-metric-independent: the nearest stop to
    # x=0.0 on any real line is always that line's own first byte, so
    # moving straight down through column 0 has an exactly predictable
    # byte-offset path regardless of the real system font's own glyph
    # widths.
    editable.set_caret(0)
    assert editable.move_caret_down() is True
    assert editable.caret == 4, f"column 0, line 0 -> line 1 must land at byte 4, got {editable.caret}"
    assert editable.move_caret_down() is True
    assert editable.caret == 10, f"column 0, line 1 -> line 2 must land at byte 10, got {editable.caret}"
    print("straight-down column-0 traversal: byte 0 -> 4 -> 10, exactly as predicted -- OK")
    assert editable.move_caret_up() is True
    assert editable.caret == 4, f"moving back up must retrace to byte 4, got {editable.caret}"
    assert editable.move_caret_up() is True
    assert editable.caret == 0, f"moving back up again must retrace to byte 0, got {editable.caret}"
    print("straight-up retrace: byte 10 -> 4 -> 0 -- OK")

    # A real "sticky column" case: start at the END of line 2 ("ij", the
    # shortest real line), then move up -- the real measured x from
    # "ij"'s own end must land somewhere on line 1 ("defgh", a longer
    # real line), never on line 0 or line 2 -- a real, falsifiable claim
    # that doesn't require knowing the system font's own exact glyph
    # widths ahead of time.
    editable.set_caret(len(HARD_WRAP_TEXT))
    assert editable.move_caret_up() is True
    assert 4 <= editable.caret <= 10, (
        f"moving up from the end of line 2 must land within line 1's own byte range [4,10), "
        f"got {editable.caret}"
    )
    print(f"sticky-column move up from line 2's end lands within line 1 (byte {editable.caret}) -- OK")


def check_selection_rects_spans_exactly_the_lines_it_touches() -> None:
    editable = make_editor(HARD_WRAP_TEXT, wrap_width=float("inf"))
    assert editable.selection_rects() == [], "no active selection must produce an empty rect list"
    print("selection_rects() with no selection: empty list -- OK")

    # Selects "bc\nde" (bytes [1,6)) -- real content on line 0 ("bc")
    # and line 1 ("de"), so exactly 2 rects are expected.
    editable.set_selection(1, 6)
    rects = editable.selection_rects()
    assert len(rects) == 2, f"a selection spanning 2 real lines must produce exactly 2 rects: {rects}"
    (x0, y0, w0, h0), (x1, y1, w1, h1) = rects
    assert y1 > y0, f"line 1's rect must sit below line 0's: {rects}"
    assert h0 == h1 > 0, f"every rect must share the same real, positive line_height: {rects}"
    assert w0 > 0 and w1 > 0, f"both selected segments have real, non-empty width: {rects}"
    print(f"selection spanning lines 0-1: exactly 2 rects, consistent line_height -- OK ({rects})")

    # A single-line selection must produce exactly 1 rect.
    editable.set_selection(0, 2)
    rects = editable.selection_rects()
    assert len(rects) == 1, f"a selection within one real line must produce exactly 1 rect: {rects}"
    print("selection within a single line: exactly 1 rect -- OK")


def check_cut_copy_paste_still_work_across_a_multiline_selection() -> None:
    editable = make_editor(HARD_WRAP_TEXT, wrap_width=float("inf"))
    clipboard = tre.Clipboard()

    # Selects "bc\nde" (bytes [1,6)), spanning a real line break.
    editable.set_selection(1, 6)
    editable.cut(clipboard)
    assert editable.text == "afgh\nij", (
        f"expected 'afgh\\nij' after cutting across a line break, got {editable.text!r}"
    )
    assert editable.caret == 1, f"the caret must land at the cut's own start, got {editable.caret}"
    assert clipboard.get_text() == "bc\nde", (
        f"the clipboard must hold the exact cut text, got {clipboard.get_text()!r}"
    )
    print("cut() across a real line break: text/caret/clipboard all exactly as expected -- OK")

    editable.set_caret(1)
    editable.paste(clipboard)
    assert editable.text == HARD_WRAP_TEXT, (
        f"pasting the cut text back must exactly restore the original: {editable.text!r}"
    )
    print("paste() restores the original multi-line text exactly -- OK")


def check_real_word_wrap_editing_integration() -> None:
    """A real, finite `wrap_width` (not just hard-wrap) must also drive
    `line_count()`/vertical movement correctly -- proves Step 15.1's
    rendering primitive and Step 15.2's editing layer share one real,
    consistent layout, not two diverging implementations."""
    long_text = "the quick brown fox jumps over the lazy dog"
    unwrapped = make_editor(long_text)
    assert unwrapped.line_count() == 1, "wrap_width=None must still be exactly one line"

    wrapped = make_editor(long_text, wrap_width=80.0)
    line_count = wrapped.line_count()
    assert line_count > 1, f"wrapping a long sentence at 80px must produce more than one line, got {line_count}"
    print(f"real word-wrap at 80px: {long_text!r} became {line_count} real lines -- OK")

    wrapped.set_caret(0)
    moved_down = 0
    while wrapped.move_caret_down():
        moved_down += 1
    assert moved_down == line_count - 1, (
        f"moving down from line 0 must reach the last real line in exactly "
        f"{line_count - 1} steps, took {moved_down}"
    )
    print(
        f"move_caret_down() reaches the last of {line_count} real wrapped lines in "
        f"exactly {moved_down} steps -- OK"
    )


def main() -> None:
    check_line_count_and_boundary_no_ops()
    check_sticky_column_vertical_movement()
    check_selection_rects_spans_exactly_the_lines_it_touches()
    check_cut_copy_paste_still_work_across_a_multiline_selection()
    check_real_word_wrap_editing_integration()
    print("tre_python multi-line text editing (Phase 15 Step 15.2) demo: PASSED")


if __name__ == "__main__":
    main()
