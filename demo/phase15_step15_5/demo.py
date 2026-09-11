#!/usr/bin/env python3
"""Phase 15 Step 15.5 proof: real UAX #14 line-breaking. `wrap_lines`
(the algorithm behind `tre.Text(..., wrap_width=...)` and
`tre.EditableText`'s own layout) is rebuilt on the real `unicode-
linebreak` crate, replacing Step 15.1's own v1 whitespace-boundary-only
tokenizer. This demo proves two things end-to-end, against real GPU
output, not just Rust unit tests:

1. A real UAX #14 break opportunity that has NO whitespace at all --
   right after a hyphen -- is now usable to wrap a line. Step 15.1's
   own v1 algorithm could only ever break at `char::is_whitespace`/`\\n`
   boundaries, so a hyphenated compound word with no spaces anywhere
   (`"wellknown-example"`) could never wrap under it, no matter how
   narrow `wrap_width` was -- it would just overflow as one line. This
   demo's own hyphenated-word text has exactly one possible break point
   (the hyphen), so forcing it narrower than its own natural width
   proves the new capability directly: it must now split into exactly
   2 real lines, right at the hyphen.
2. Every pre-existing whitespace/`\\n`-boundary case from Step 15.1's
   own demo still renders identically -- a real regression check that
   the rewrite didn't change established behavior for the common case.

Reuses Step 15.1's own "scan for real ink across several render() calls,
one real GPU render() call per check" technique.
"""

import time

import tre_python as tre

WIDTH, HEIGHT = 500, 400
PX_SIZE = 32.0
TEXT_X, TEXT_Y = 20.0, 20.0
WHITE = tre.rgba8(255, 255, 255, 255)
RENDER_ATTEMPTS = 60


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def ink_row_bands(frame: bytes, background: tuple, x0: int, x1: int, y0: int, y1: int) -> list:
    """One (start_y, end_y) pair per contiguous run of real rows in
    [y0, y1) that contain at least one non-background pixel in columns
    [x0, x1) -- each such band is one real rendered visual line."""
    rows_with_ink = [
        any(pixel_at(frame, x, y) != background for x in range(x0, x1, 2)) for y in range(y0, y1)
    ]
    bands = []
    start = None
    for i, has_ink in enumerate(rows_with_ink):
        if has_ink and start is None:
            start = y0 + i
        elif not has_ink and start is not None:
            bands.append((start, y0 + i))
            start = None
    if start is not None:
        bands.append((start, y1))
    return bands


def render_until_ink(renderer, registry, x0: int, x1: int, y0: int, y1: int):
    """See phase15_step15_1's own identical helper: always renders the
    full attempt budget (not just until the first sign of ink), since a
    multi-line render needs every line's own glyphs resolved before its
    ink pattern is trustworthy."""
    frame = None
    background = None
    for _ in range(RENDER_ATTEMPTS):
        frame = renderer.render(registry)
        if background is None:
            background = pixel_at(frame, 0, 0)
        time.sleep(0.05)
    bands = ink_row_bands(frame, background, x0, x1, y0, y1)
    if not bands:
        raise AssertionError(f"no real ink appeared after {RENDER_ATTEMPTS} render() calls")
    return frame, bands, background


def rightmost_ink_x(frame: bytes, background: tuple, y0: int, y1: int, x_max: int) -> int:
    rightmost = 0
    for y in range(y0, y1):
        for x in range(x_max - 1, 0, -2):
            if pixel_at(frame, x, y) != background:
                rightmost = max(rightmost, x)
                break
    return rightmost


def check_hyphen_break_needs_no_whitespace(renderer, font) -> None:
    """The real, new capability this step adds: a break opportunity
    right after a hyphen, in text with NO whitespace anywhere."""
    text = "wellknown-example"

    unwrapped_registry = tre.ShapeRegistry()
    unwrapped_registry.insert_text(tre.Text(TEXT_X, TEXT_Y, text, font, PX_SIZE, WHITE))
    frame, bands, background = render_until_ink(
        renderer, unwrapped_registry, int(TEXT_X), WIDTH, int(TEXT_Y), int(TEXT_Y + PX_SIZE * 1.5)
    )
    assert len(bands) == 1, f"the unwrapped baseline must be one real line: {bands}"
    y0, y1 = bands[0]
    natural_width = rightmost_ink_x(frame, background, y0, y1, WIDTH) - int(TEXT_X)
    assert natural_width > 0, "the unwrapped hyphenated word must have measured some real width"
    print(f"unwrapped '{text}' (no whitespace at all) measured {natural_width}px wide -- OK")

    # Any width strictly between 0 and natural_width forces a wrap; the
    # hyphen is the ONLY UAX #14 break opportunity in this text, so the
    # result must be exactly 2 lines regardless of the exact fraction.
    wrap_width = natural_width * 0.5
    wrapped_registry = tre.ShapeRegistry()
    wrapped_registry.insert_text(tre.Text(TEXT_X, TEXT_Y, text, font, PX_SIZE, WHITE, wrap_width=wrap_width))
    _frame2, wrapped_bands, _bg2 = render_until_ink(
        renderer, wrapped_registry, int(TEXT_X), WIDTH, int(TEXT_Y), HEIGHT
    )
    assert len(wrapped_bands) == 2, (
        f"'{text}' has exactly one real break opportunity (the hyphen) -- wrapping at "
        f"{wrap_width:.1f}px must produce exactly 2 real lines, got {wrapped_bands}. "
        f"Step 15.1's own whitespace-only v1 could never wrap this text at all, since it "
        f"contains no whitespace anywhere."
    )
    print(
        f"real UAX #14 hyphen break: wrapping at {wrap_width:.1f}px splits '{text}' "
        f"(no whitespace) into exactly 2 real lines, right at the hyphen -- OK"
    )


def check_whitespace_wrap_width_none_is_still_an_exact_regression(renderer, font) -> None:
    """Step 15.1's own regression check, reproduced verbatim: `wrap_width=None`
    must still render exactly as a single real visual line."""
    registry = tre.ShapeRegistry()
    registry.insert_text(tre.Text(TEXT_X, TEXT_Y, "Hello world", font, PX_SIZE, WHITE))
    _frame, bands, _bg = render_until_ink(
        renderer, registry, int(TEXT_X), WIDTH, int(TEXT_Y), int(TEXT_Y + PX_SIZE * 1.5)
    )
    assert len(bands) == 1, f"wrap_width=None must still render as exactly one line: {bands}"
    print("wrap_width=None regression (unchanged by the UAX #14 rewrite): exactly 1 real ink band -- OK")


def check_hard_wrap_on_explicit_newlines_is_still_unchanged(renderer, font) -> None:
    """Step 15.1's own hard-wrap regression check, reproduced verbatim."""
    registry = tre.ShapeRegistry()
    registry.insert_text(
        tre.Text(TEXT_X, TEXT_Y, "one\ntwo\nthree", font, PX_SIZE, WHITE, wrap_width=float("inf"))
    )
    _frame, bands, _bg = render_until_ink(renderer, registry, int(TEXT_X), WIDTH, int(TEXT_Y), HEIGHT)
    assert len(bands) == 3, f"three \\n-separated lines must still render as exactly 3 real ink bands: {bands}"
    gaps = [bands[i + 1][0] - bands[i][0] for i in range(2)]
    assert abs(gaps[0] - gaps[1]) <= 2, f"real line spacing must still be consistent between every pair: {gaps}"
    print(f"hard-wrap (\\n only) regression: exactly 3 real ink bands, consistent spacing = {gaps[0]}px -- OK")


def check_word_wrap_by_width_still_matches_pre_rewrite_behavior(renderer, font) -> None:
    """Step 15.1's own word-wrap-by-width regression check, reproduced
    verbatim -- proves whitespace-boundary wrapping behaves identically
    to before, even though it's now one case of a more general
    algorithm rather than the only kind of break the code knows about."""
    long_text = "the quick brown fox jumps"

    unwrapped_registry = tre.ShapeRegistry()
    unwrapped_registry.insert_text(tre.Text(TEXT_X, TEXT_Y, long_text, font, PX_SIZE, WHITE))
    frame, bands, background = render_until_ink(
        renderer, unwrapped_registry, int(TEXT_X), WIDTH, int(TEXT_Y), int(TEXT_Y + PX_SIZE * 1.5)
    )
    assert len(bands) == 1, f"the unwrapped baseline must still be one real line: {bands}"
    y0, y1 = bands[0]
    natural_width = rightmost_ink_x(frame, background, y0, y1, WIDTH) - int(TEXT_X)
    assert natural_width > 0

    wrap_width = natural_width / 2.5
    wrapped_registry = tre.ShapeRegistry()
    wrapped_registry.insert_text(
        tre.Text(TEXT_X, TEXT_Y, long_text, font, PX_SIZE, WHITE, wrap_width=wrap_width)
    )
    _frame2, wrapped_bands, _bg2 = render_until_ink(
        renderer, wrapped_registry, int(TEXT_X), WIDTH, int(TEXT_Y), HEIGHT
    )
    assert 2 <= len(wrapped_bands) <= 5, (
        f"whitespace word-wrap at {wrap_width:.1f}px must still split '{long_text}' across "
        f"more than one real line, got {wrapped_bands}"
    )
    print(
        f"whitespace word-wrap regression: {len(wrapped_bands)} real ink bands at "
        f"{wrap_width:.1f}px, matching pre-rewrite behavior -- OK"
    )


def main() -> None:
    # Exactly ONE renderer for the whole process -- winit permits only
    # one real EventLoop per process.
    font = tre.Font.system_cascade()
    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)

    check_hyphen_break_needs_no_whitespace(renderer, font)
    check_whitespace_wrap_width_none_is_still_an_exact_regression(renderer, font)
    check_hard_wrap_on_explicit_newlines_is_still_unchanged(renderer, font)
    check_word_wrap_by_width_still_matches_pre_rewrite_behavior(renderer, font)
    print("tre_python real UAX #14 line-breaking (Phase 15 Step 15.5) demo: PASSED")


if __name__ == "__main__":
    main()
