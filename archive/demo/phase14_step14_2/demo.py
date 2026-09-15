#!/usr/bin/env python3
"""Phase 14 Step 14.2 proof: `EditableText.copy`/`.cut`/`.paste`, wiring
the real system `Clipboard` (Step 14.1) into the real single-line text
editor (Phase 13 Step 13.5) -- the natural pairing both READMEs already
named as the obvious next step once each shipped independently.

`Clipboard` is passed in explicitly to each call rather than owned by
`EditableText` -- a real design choice, not an oversight: a real app
has ONE system clipboard connection shared by every text field in it,
not one per field, and `EditableText` never pays the cost of opening a
clipboard connection unless a caller actually invokes cut/copy/paste.
"""

import tre_python as tre


def make_editable(font: "tre.Font", text: str) -> "tre.EditableText":
    return tre.EditableText(0.0, 0.0, text, font, 16.0, tre.rgba8(0, 0, 0, 255))


def check_copy_leaves_text_unchanged(font: "tre.Font", clipboard: "tre.Clipboard") -> None:
    editable = make_editable(font, "Hello World")
    editable.set_selection(6, 11)  # "World"
    editable.copy(clipboard)
    assert editable.text == "Hello World", "copy() must never modify the source text"
    assert clipboard.get_text() == "World"
    print("copy() with 'World' selected: text unchanged, clipboard='World' -- OK")


def check_cut_removes_the_selection_and_copies_it() -> None:
    font = tre.Font.system_cascade()
    clipboard = tre.Clipboard()
    editable = make_editable(font, "Hello World")
    editable.set_selection(6, 11)  # "World"
    editable.cut(clipboard)
    assert editable.text == "Hello ", f"expected 'Hello ' after cutting 'World', got {editable.text!r}"
    assert editable.caret == 6, f"the caret must land at the cut's own start, got {editable.caret}"
    assert clipboard.get_text() == "World"
    print("cut() with 'World' selected: text='Hello ', caret=6, clipboard='World' -- OK")


def check_paste_inserts_at_the_caret_and_replaces_a_selection() -> None:
    font = tre.Font.system_cascade()
    clipboard = tre.Clipboard()
    clipboard.set_text("World")

    # Paste at a plain caret (no selection): inserts, doesn't replace.
    editable = make_editable(font, "Hi ")
    editable.set_caret(len("Hi ".encode()))
    editable.paste(clipboard)
    assert editable.text == "Hi World", f"expected 'Hi World', got {editable.text!r}"
    print(f"paste() at a plain caret: text={editable.text!r} -- OK")

    # Paste with an active selection: replaces the selection, the same
    # real semantics insert() already has everywhere else in the class.
    editable2 = make_editable(font, "Hi Mars")
    editable2.set_selection(3, 7)  # "Mars"
    editable2.paste(clipboard)
    assert editable2.text == "Hi World", f"expected 'Hi World' (Mars replaced), got {editable2.text!r}"
    print(f"paste() replacing an active selection: text={editable2.text!r} -- OK")


def check_copy_and_cut_are_real_no_ops_without_a_selection() -> None:
    font = tre.Font.system_cascade()
    clipboard = tre.Clipboard()
    clipboard.set_text("untouched sentinel")

    editable = make_editable(font, "no selection here")
    assert editable.selection_anchor is None
    editable.copy(clipboard)
    assert clipboard.get_text() == "untouched sentinel", (
        "copy() with no selection must not touch the clipboard"
    )
    editable.cut(clipboard)
    assert editable.text == "no selection here", "cut() with no selection must not touch the text"
    assert clipboard.get_text() == "untouched sentinel", (
        "cut() with no selection must not touch the clipboard"
    )
    print("copy()/cut() with no active selection are real no-ops -- OK")


def check_real_utf8_selection_round_trips_through_the_clipboard() -> None:
    font = tre.Font.system_cascade()
    clipboard = tre.Clipboard()
    # "café" -- "é" is a real 2-byte UTF-8 character. A byte-unsafe
    # selection/cut would corrupt it. Selecting exactly "café" (bytes
    # [3:8]) leaves "un " (the space before it) joined to " chaud" (the
    # space still attached to "chaud"), i.e. "un  chaud" -- two spaces.
    editable = make_editable(font, "un café chaud")
    start = "un ".encode().__len__()
    end = "un café".encode().__len__()
    editable.set_selection(start, end)
    editable.cut(clipboard)
    assert editable.text == "un  chaud", f"expected 'un  chaud', got {editable.text!r}"
    assert clipboard.get_text() == "café", (
        f"expected 'café' on the clipboard, got {clipboard.get_text()!r}"
    )
    print(
        f"real UTF-8 cut/clipboard round-trip: text={editable.text!r}, "
        f"clipboard={clipboard.get_text()!r} -- OK"
    )


def main() -> None:
    font = tre.Font.system_cascade()
    clipboard = tre.Clipboard()
    check_copy_leaves_text_unchanged(font, clipboard)
    check_cut_removes_the_selection_and_copies_it()
    check_paste_inserts_at_the_caret_and_replaces_a_selection()
    check_copy_and_cut_are_real_no_ops_without_a_selection()
    check_real_utf8_selection_round_trips_through_the_clipboard()
    print("tre_python EditableText cut/copy/paste (Phase 14 Step 14.2) demo: PASSED")


if __name__ == "__main__":
    main()
