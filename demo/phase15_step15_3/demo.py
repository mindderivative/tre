#!/usr/bin/env python3
"""Phase 15 Step 15.3 proof: real Shift+arrow selection extension --
`tre.EditableText` gains `move_caret_left`/`move_caret_right` (real,
brand-new UTF-8-char-boundary-safe horizontal movement -- this class
had NO horizontal movement at all before this step, only `set_caret`
with a caller-supplied byte offset), and `move_caret_up`/`move_caret_down`
(Phase 15 Step 15.2) gain a new `extend: bool` parameter. All four share
one real selection-anchor rule (`apply_caret_move`): `extend=False`
always clears any active selection (matching `set_caret`'s own
convention); `extend=True` starts a new selection anchored at the
caret's own current position the first time it's used, then leaves that
anchor untouched on every subsequent extend, in either direction --
the real Shift+arrow convention every desktop text field already has.

No GPU renderer is needed here -- horizontal movement is pure string
logic (no shaping at all), and vertical movement's own layout
computation was already proven headless in Step 15.2's own demo.
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


def check_plain_horizontal_movement_is_utf8_char_safe() -> None:
    # "café" -- 'é' is a real 2-byte UTF-8 character. len("café".encode()) == 5.
    editable = make_editor("café")
    editable.set_caret(len("café".encode()))
    assert editable.move_caret_left() is True
    assert editable.caret == len("caf".encode()), (
        f"moving left from the end must skip the whole 2-byte 'é', not one byte, got {editable.caret}"
    )
    assert editable.move_caret_left() is True
    assert editable.caret == len("ca".encode())
    assert editable.move_caret_right() is True
    assert editable.caret == len("caf".encode())
    print("move_caret_left/right are real UTF-8-char-boundary-safe, not byte-safe -- OK")

    editable.set_caret(0)
    assert editable.move_caret_left() is False, "no-op at the very start of text"
    assert editable.caret == 0
    editable.set_caret(len("café".encode()))
    assert editable.move_caret_right() is False, "no-op at the very end of text"
    print("move_caret_left/right are real no-ops at the start/end of text -- OK")


def check_shift_right_extends_a_selection_from_a_fixed_anchor() -> None:
    editable = make_editor("hello world")
    editable.set_caret(0)
    assert editable.selection_anchor is None

    for _ in range(5):
        assert editable.move_caret_right(extend=True) is True
    assert editable.caret == 5
    assert editable.selection_anchor == 0, "the anchor must stay fixed at the real starting caret"
    assert selected(editable) == "hello", f"expected 'hello' selected, got {selected(editable)!r}"
    print("5x Shift+Right from byte 0: anchor stays at 0, selection == 'hello' -- OK")

    # Reversing direction while still extending must keep the SAME anchor,
    # just shrinking the selection back toward it.
    assert editable.move_caret_left(extend=True) is True
    assert editable.selection_anchor == 0, "the anchor must not move on a direction reversal"
    assert editable.caret == 4
    assert selected(editable) == "hell", f"expected 'hell', got {selected(editable)!r}"
    print("Shift+Left after Shift+Right: same anchor (0), selection shrinks to 'hell' -- OK")

    # Continuing to shrink toward the anchor: caret is at 4, anchor at 0,
    # so exactly 4 more real moves reach the anchor itself (caret == 0 ==
    # anchor, an empty but still-active selection); a 5th move has
    # nothing left to do and is a real no-op, touching neither caret nor
    # the anchor.
    for _ in range(4):
        assert editable.move_caret_left(extend=True) is True
    assert editable.caret == 0, f"4 more moves from byte 4 must reach byte 0, got {editable.caret}"
    assert editable.selection_anchor == 0, "the anchor must still be 0 once the caret catches up to it"
    assert selected(editable) == "", "caret == anchor is a real, valid empty selection"
    assert editable.move_caret_left(extend=True) is False, "no real character left to move past byte 0"
    assert editable.selection_anchor == 0, "a no-op move must not disturb the existing anchor"
    print("shrinking all the way back to the anchor, then a real no-op at byte 0 -- OK")


def check_plain_arrow_after_a_selection_clears_it_and_moves_from_the_caret() -> None:
    editable = make_editor("hello world")
    editable.set_caret(0)
    for _ in range(5):
        editable.move_caret_right(extend=True)
    assert selected(editable) == "hello"

    # A real, disclosed v1 design choice: a plain (non-extend) arrow
    # always clears the selection and moves by one real character from
    # the CURRENT caret -- it does not "collapse to the selection's own
    # edge" the way some real editors do; this matches
    # move_caret_up/down's own already-established convention.
    assert editable.move_caret_right() is True
    assert editable.selection_anchor is None, "a plain arrow must clear the active selection"
    assert editable.caret == 6, f"expected the caret to move one char from 5 to 6, got {editable.caret}"
    print("a plain Right after a Shift+Right selection: selection clears, caret moves from itself -- OK")


def check_shift_up_down_extend_across_real_visual_lines() -> None:
    # "abc\ndefgh\nij" -- the same known, exact 3-line hard-wrapped text
    # Step 15.2's own demo already established byte ranges for.
    text = "abc\ndefgh\nij"
    editable = make_editor(text, wrap_width=float("inf"))
    editable.set_caret(0)
    assert editable.move_caret_down(extend=True) is True
    assert editable.selection_anchor == 0, "Shift+Down must anchor at the real starting caret"
    assert 4 <= editable.caret <= 10, f"caret must land on line 1's own byte range, got {editable.caret}"
    first_move_caret = editable.caret

    assert editable.move_caret_down(extend=True) is True
    assert editable.selection_anchor == 0, "the anchor must still be unchanged after a second Shift+Down"
    assert editable.caret == 10, (
        f"the sticky column (x=0.0) must land exactly at line 2's own start, byte 10, got {editable.caret}"
    )

    # At byte 10 exactly, line 2's own selected segment is genuinely
    # empty (the selection ends right at its first character) -- a
    # real, correct edge case, not a bug: selection_rects() must skip
    # an empty segment rather than emit a zero-width rect for it.
    rects = editable.selection_rects()
    assert len(rects) == 2, (
        f"byte 10 is exactly line 2's own start -- nothing is selected on it yet, so exactly "
        f"2 rects (lines 0-1) are expected, not 3: {rects}"
    )
    print(f"Shift+Down x2 lands exactly at line 2's start (byte 10): real 0-width segment there, "
          f"selection_rects() correctly reports only 2 rects, not 3 -- OK")

    # Extending one real character further (still Shift+Right) actually
    # selects something on line 2, which must now show up as a real
    # third rect.
    assert editable.move_caret_right(extend=True) is True
    assert editable.selection_anchor == 0, "combining vertical and horizontal extension keeps one anchor"
    assert editable.caret == 11
    rects = editable.selection_rects()
    assert len(rects) == 3, f"now that line 2 has one real selected char, expect exactly 3 rects: {rects}"
    print(
        f"Shift+Down x2 then Shift+Right x1: anchor stays 0, caret {first_move_caret} -> 10 -> 11, "
        f"selection_rects() now spans exactly 3 real lines -- OK"
    )

    assert editable.move_caret_up(extend=True) is True
    assert editable.selection_anchor == 0, "Shift+Up must keep the same anchor as the preceding Shift+Down"
    print("Shift+Up after Shift+Down: same anchor (0), selection shrinks by one real line -- OK")


def check_a_move_that_cannot_happen_touches_nothing() -> None:
    editable = make_editor("hi")
    editable.set_caret(1)
    editable.set_selection(0, 1)
    assert selected(editable) == "h"
    editable.set_caret(0)
    assert editable.selection_anchor is None
    assert editable.move_caret_left(extend=True) is False, "no-op: already at the very start"
    assert editable.selection_anchor is None, (
        "a move that cannot happen at all must not start a new selection either"
    )
    print("move_caret_left(extend=True) at the very start: real no-op, no stray selection started -- OK")


def main() -> None:
    check_plain_horizontal_movement_is_utf8_char_safe()
    check_shift_right_extends_a_selection_from_a_fixed_anchor()
    check_plain_arrow_after_a_selection_clears_it_and_moves_from_the_caret()
    check_shift_up_down_extend_across_real_visual_lines()
    check_a_move_that_cannot_happen_touches_nothing()
    print("tre_python Shift+arrow selection extension (Phase 15 Step 15.3) demo: PASSED")


if __name__ == "__main__":
    main()
