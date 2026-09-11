#!/usr/bin/env python3
"""Phase 13 Step 13.5 proof: `tre.EditableText`, the still-pending
"plan out editable text" item from the project owner's own earlier
directive, now implemented -- real single-line text editing built on
`tre_text::caret_positions`/`hit_test` (new this step, using the exact
same pen-advance formula `tre_engine::text::flatten_text` itself renders
with) and the real IME events already forwarded end to end since Step
12.7.

The pure caret-math (`caret_positions`/`hit_test`) already has exact,
hand-computed unit tests in Rust (`crates/tre-text/src/caret.rs`) using
synthetic glyph data. This demo proves the other half: real integration
against a real font, real edit operations, real IME events, and a real
render.
"""

import time

import tre_python as tre

WIDTH, HEIGHT = 300, 100
# HeadlessRenderer's own TextAtlas rasterizes glyphs on a real background
# thread and only re-uploads its GPU texture every 15 frames (Step 12.3's
# own disclosed throttle) -- the first several render() calls are real
# cache misses, not a bug. Comfortably more than that, with real sleeps
# between calls, matching demo/phase12_step12_3/demo.py's own precedent.
RENDER_ATTEMPTS = 60


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def check_insert_and_delete() -> None:
    font = tre.Font.system_cascade()
    editable = tre.EditableText(10.0, 10.0, "Hello", font, 24.0, tre.rgba8(0, 0, 0, 255))
    assert editable.caret == len("Hello")
    assert editable.selection_anchor is None

    editable.insert(" World")
    assert editable.text == "Hello World"
    assert editable.caret == len("Hello World")
    print(f"insert(' World'): text={editable.text!r}, caret={editable.caret} -- OK")

    editable.set_selection(5, 11)
    assert editable.selection_anchor == 5
    assert editable.caret == 11
    deleted = editable.delete_selection()
    assert deleted
    assert editable.text == "Hello"
    assert editable.caret == 5
    assert editable.selection_anchor is None
    print("set_selection(5, 11) + delete_selection() -> text='Hello', caret=5 -- OK")

    # "café" -- "é" is a real 2-byte UTF-8 character. A byte-wise (not
    # char-wise) backspace would corrupt the string into invalid UTF-8.
    editable2 = tre.EditableText(0.0, 0.0, "café", font, 24.0, tre.rgba8(0, 0, 0, 255))
    editable2.set_caret(len("café".encode()))
    editable2.delete_backward()
    assert editable2.text == "caf", f"expected 'caf' after deleting one real char, got {editable2.text!r}"
    print("delete_backward() on 'café' removes one real char ('é'), not one byte -- OK")


def check_set_caret_rejects_invalid_boundary() -> None:
    font = tre.Font.system_cascade()
    editable = tre.EditableText(0.0, 0.0, "café", font, 24.0, tre.rgba8(0, 0, 0, 255))
    # byte 3 is where "é" (2 bytes) starts; byte 4 would split it.
    editable.set_caret(3)  # valid: right before "é"
    try:
        editable.set_caret(4)  # invalid: inside "é"'s own 2-byte encoding
        raise AssertionError("set_caret(4) should have raised ValueError (splits a UTF-8 char)")
    except ValueError:
        print("set_caret(4) correctly rejected (would split 'é's own UTF-8 encoding) -- OK")


def check_hit_test_against_a_real_shaped_font() -> None:
    font = tre.Font.system_cascade()
    editable = tre.EditableText(0.0, 0.0, "Hello World", font, 24.0, tre.rgba8(0, 0, 0, 255))
    start = editable.hit_test(0.0)
    end = editable.hit_test(100000.0)
    middle = editable.hit_test(40.0)
    assert start == 0, f"clicking at x=0 must resolve to the very start, got {start}"
    assert end == len("Hello World"), f"clicking far past the end must resolve to the end, got {end}"
    assert 0 < middle < len("Hello World"), f"clicking in the middle must resolve strictly between, got {middle}"
    # Monotonicity: a real LTR font must place later text at increasing x.
    offsets = [editable.hit_test(x) for x in (0.0, 10.0, 20.0, 30.0, 50.0, 80.0, 100000.0)]
    assert offsets == sorted(offsets), f"hit_test results must be monotonically non-decreasing, got {offsets}"
    print(f"hit_test against a real shaped font: start={start}, middle={middle}, end={end}, monotonic -- OK")


def check_real_ime_round_trip() -> None:
    font = tre.Font.system_cascade()
    editable = tre.EditableText(0.0, 0.0, "Hello ", font, 24.0, tre.rgba8(0, 0, 0, 255))
    window = tre.WindowId(0)

    editable.handle_ime(tre.InputEvent.ImeEnabled(window=window))
    assert editable.ime_active
    print("ImeEnabled -> ime_active=True -- OK")

    editable.handle_ime(tre.InputEvent.ImePreedit(window=window, text="wor", cursor=None))
    assert editable.preedit == "wor"
    assert editable.text == "Hello ", "preedit must not touch the real committed text"
    print(f"ImePreedit('wor') -> preedit={editable.preedit!r}, text unchanged={editable.text!r} -- OK")

    editable.handle_ime(tre.InputEvent.ImeCommit(window=window, text="world"))
    assert editable.text == "Hello world"
    assert editable.preedit == ""
    print(f"ImeCommit('world') -> text={editable.text!r}, preedit cleared -- OK")

    editable.handle_ime(tre.InputEvent.ImeDisabled(window=window))
    assert not editable.ime_active
    print("ImeDisabled -> ime_active=False -- OK")


def check_real_render_integration() -> None:
    font = tre.Font.system_cascade()
    editable = tre.EditableText(10.0, 10.0, "Hi", font, 32.0, tre.rgba8(0, 0, 0, 255))
    window = tre.WindowId(0)
    editable.handle_ime(tre.InputEvent.ImePreedit(window=window, text="!", cursor=None))

    text_shape = editable.to_text()
    registry = tre.ShapeRegistry()
    registry.insert_text(text_shape)
    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)

    has_ink = False
    background = None
    for attempt in range(RENDER_ATTEMPTS):
        frame = renderer.render(registry)
        if background is None:
            background = pixel_at(frame, WIDTH - 1, HEIGHT - 1)
        for y in range(0, HEIGHT, 2):
            for x in range(0, WIDTH, 2):
                if pixel_at(frame, x, y) != background:
                    has_ink = True
                    break
            if has_ink:
                break
        if has_ink:
            print(f"real, non-background text pixels found after {attempt + 1} render() calls")
            break
        time.sleep(0.05)

    assert has_ink, "to_text() (with a live preedit spliced in) must render real, visible glyph ink"
    print("to_text() with an active preedit renders real, visible text -- OK")


def main() -> None:
    check_insert_and_delete()
    check_set_caret_rejects_invalid_boundary()
    check_hit_test_against_a_real_shaped_font()
    check_real_ime_round_trip()
    check_real_render_integration()
    print("tre_python EditableText (Phase 13 Step 13.5) demo: PASSED")


if __name__ == "__main__":
    main()
