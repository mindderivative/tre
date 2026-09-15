#!/usr/bin/env python3
"""Phase 15 Step 15.4 proof: real word/line caret jumps --
`tre.EditableText` gains `move_caret_word_left`/`move_caret_word_right`
(Ctrl+arrow, real Unicode UAX #29 word-boundary segmentation via
`tre_text::next_word_end`/`prev_word_start` -- pure string logic, no
shaping needed) and `move_caret_line_start`/`move_caret_line_end`
(Home/End, needing real layout via `compute_layout`, the same as
`move_caret_up`/`down`). All four take the same `extend: bool`
parameter Step 15.3 already established, sharing the identical
`apply_caret_move` selection-anchor rule.

No GPU renderer is needed here -- everything exercised is either pure
string logic or layout computation already proven headless in Step
15.2/15.3's own demos.
"""

import tre_python as tre


def make_editor(text: str, wrap_width=None) -> tre.EditableText:
    font = tre.Font.system_cascade()
    return tre.EditableText(0.0, 0.0, text, font, 16.0, tre.rgba8(0, 0, 0, 255), wrap_width)


def selected(editable: tre.EditableText) -> str:
    if editable.selection_anchor is None:
        return ""
    start, end = sorted((editable.selection_anchor, editable.caret))
    return editable.text[start:end]


def check_word_jumps_land_at_exact_real_word_boundaries() -> None:
    text = "the quick brown fox"
    editable = make_editor(text)
    editable.set_caret(0)

    expected_ends = [3, 9, 15, 19]  # end of "the", "quick", "brown", "fox"
    for expected in expected_ends:
        assert editable.move_caret_word_right() is True
        assert editable.caret == expected, f"expected word-end {expected}, got {editable.caret}"
    assert editable.move_caret_word_right() is False, "no real word left after 'fox'"
    print(f"Ctrl+Right x4 across '{text}': lands exactly at {expected_ends}, then a real no-op -- OK")

    expected_starts = [16, 10, 4, 0]  # start of "fox", "brown", "quick", "the"
    for expected in expected_starts:
        assert editable.move_caret_word_left() is True
        assert editable.caret == expected, f"expected word-start {expected}, got {editable.caret}"
    assert editable.move_caret_word_left() is False, "no real word left before 'the'"
    print(f"Ctrl+Left x4 back across the same text: lands exactly at {expected_starts}, then a real no-op -- OK")


def check_word_jumps_skip_punctuation_and_respect_utf8() -> None:
    # "café, don't" -- a real 2-byte 'é' inside a word, plus a real
    # mid-word apostrophe (UAX #29: neither splits its own word), plus
    # punctuation that must be skipped entirely, not stopped at.
    text = "café, don't"
    editable = make_editor(text)
    editable.set_caret(0)
    cafe_end = len("café".encode())
    assert editable.move_caret_word_right() is True
    assert editable.caret == cafe_end, f"expected end of 'café' ({cafe_end}), got {editable.caret}"
    assert editable.move_caret_word_right() is True
    assert editable.caret == len(text.encode()), (
        f"expected the end of \"don't\" (skipping ', ' entirely), got {editable.caret}"
    )
    print("Ctrl+Right across a real 2-byte 'é' and a mid-word apostrophe: exact UAX #29 boundaries -- OK")


def check_word_jump_shift_extend_shares_the_same_anchor_rule() -> None:
    editable = make_editor("hello world again")
    editable.set_caret(0)
    assert editable.move_caret_word_right(extend=True) is True
    assert editable.selection_anchor == 0
    assert selected(editable) == "hello"
    assert editable.move_caret_word_right(extend=True) is True
    assert editable.selection_anchor == 0, "the anchor must still be 0 after a second Ctrl+Shift+Right"
    assert selected(editable) == "hello world"
    print("Ctrl+Shift+Right x2: one fixed anchor at 0, selection grows word by word -- OK")


HARD_WRAP_TEXT = "abc\ndefgh\nij"


def check_home_and_end_land_on_real_content_boundaries() -> None:
    editable = make_editor(HARD_WRAP_TEXT, wrap_width=float("inf"))

    editable.set_caret(6)  # inside "defgh", on line 1
    assert editable.move_caret_line_start() is True
    assert editable.caret == 4, f"Home on line 1 must land at its own real start (byte 4), got {editable.caret}"
    assert editable.move_caret_line_start() is False, "already at the line's own start: real no-op"

    editable.set_caret(6)
    assert editable.move_caret_line_end() is True
    assert editable.caret == 9, (
        f"End on line 1 must land right before its own real \\n (byte 9), not byte 10 "
        f"(line 2's own start), got {editable.caret}"
    )
    assert editable.move_caret_line_end() is False, "already at the line's own end: real no-op"
    print("Home/End on line 1 ('defgh'): exact real content boundaries (4, 9), then real no-ops -- OK")


def check_end_avoids_the_real_tie_with_the_next_lines_start() -> None:
    """The real regression this step's own README discloses: End must
    NOT land on `line.byte_range.end` directly, since that byte value
    is ALSO the next line's own start -- a real tie `move_caret_down`
    would resolve toward the WRONG (next) line. Proves End on line 0
    followed by a real Down still moves to line 1, not line 2."""
    editable = make_editor(HARD_WRAP_TEXT, wrap_width=float("inf"))
    editable.set_caret(1)  # inside "abc", on line 0
    assert editable.move_caret_line_end() is True
    assert editable.caret == 3, f"End on line 0 must land at byte 3 (before its own \\n), got {editable.caret}"

    assert editable.move_caret_down() is True
    assert 4 <= editable.caret <= 9, (
        f"Down right after End must move to line 1 (byte range [4,9]), not skip to line 2, "
        f"got {editable.caret}"
    )
    print(f"End on line 0 (byte 3) then Down correctly moves to line 1 (byte {editable.caret}), not line 2 -- OK")


def check_line_jump_extend_uses_the_same_anchor_rule() -> None:
    editable = make_editor(HARD_WRAP_TEXT, wrap_width=float("inf"))
    editable.set_caret(6)
    assert editable.move_caret_line_start(extend=True) is True
    assert editable.selection_anchor == 6
    assert editable.caret == 4
    assert selected(editable) == "de"
    print("Shift+Home: anchor fixed at the original caret (6), selects 'de' -- OK")


def main() -> None:
    check_word_jumps_land_at_exact_real_word_boundaries()
    check_word_jumps_skip_punctuation_and_respect_utf8()
    check_word_jump_shift_extend_shares_the_same_anchor_rule()
    check_home_and_end_land_on_real_content_boundaries()
    check_end_avoids_the_real_tie_with_the_next_lines_start()
    check_line_jump_extend_uses_the_same_anchor_rule()
    print("tre_python word/line caret jumps (Phase 15 Step 15.4) demo: PASSED")


if __name__ == "__main__":
    main()
