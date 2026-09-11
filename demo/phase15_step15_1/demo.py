#!/usr/bin/env python3
"""Phase 15 Step 15.1 proof: real multi-line/word-wrap text rendering,
`tre.Text(..., wrap_width=...)` -- the first time this engine has ever
rendered more than one visual line of text from a single shape. Built
on a new `tre_text::wrap_lines` line-breaking algorithm (Rust unit
tests already cover its own logic in isolation); this demo proves the
real, end-to-end GPU rendering result: distinct glyph rows really do
appear at distinct, evenly-spaced real pixel rows.

Reuses `phase12_step12_3`'s own "scan for real ink, not background,
across several render() calls" technique (the atlas resolves glyphs on
a real background thread; the first several calls are legitimate cache
misses), extended here into a real vertical ink-band scanner: each
contiguous run of rows containing real (non-background) ink is one
real visual line.
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
    """Renders `RENDER_ATTEMPTS` real frames with real wall-clock sleeps
    between them, matching phase12_step12_3's own atlas warm-up
    discipline -- unlike a single-line demo, a multi-line render needs
    *every* line's own glyphs resolved before its ink pattern is
    trustworthy, so this always renders the full attempt budget rather
    than stopping at the first sign of any ink (which can be a real but
    incomplete partial render while later lines' glyphs are still being
    rasterized on the atlas's background thread)."""
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


def check_wrap_width_none_is_an_exact_regression(renderer, font) -> None:
    """`wrap_width=None` must render exactly as before Phase 15 --
    a single real visual line, matching phase12_step12_3's own check."""
    registry = tre.ShapeRegistry()
    registry.insert_text(tre.Text(TEXT_X, TEXT_Y, "Hello world", font, PX_SIZE, WHITE))
    _frame, bands, _bg = render_until_ink(
        renderer, registry, int(TEXT_X), WIDTH, int(TEXT_Y), int(TEXT_Y + PX_SIZE * 1.5)
    )
    assert len(bands) == 1, f"wrap_width=None must still render as exactly one line: {bands}"
    print("wrap_width=None regression: exactly 1 real ink band -- OK")


def check_hard_wrap_on_explicit_newlines(renderer, font) -> float:
    """Three `\\n`-separated segments must render as exactly 3 real,
    evenly-spaced ink bands -- returns the real measured line_height in
    pixels for the next check to reuse."""
    registry = tre.ShapeRegistry()
    registry.insert_text(
        tre.Text(TEXT_X, TEXT_Y, "one\ntwo\nthree", font, PX_SIZE, WHITE, wrap_width=float("inf"))
    )
    _frame, bands, _bg = render_until_ink(renderer, registry, int(TEXT_X), WIDTH, int(TEXT_Y), HEIGHT)
    assert len(bands) == 3, f"three \\n-separated lines must render as exactly 3 real ink bands: {bands}"
    gaps = [bands[i + 1][0] - bands[i][0] for i in range(2)]
    assert abs(gaps[0] - gaps[1]) <= 2, f"real line spacing must be consistent between every pair: {gaps}"
    assert PX_SIZE * 0.5 <= gaps[0] <= PX_SIZE * 2.0, (
        f"line_height must be a plausible multiple of px_size, got {gaps[0]}"
    )
    print(f"hard-wrap (\\n only): exactly 3 real ink bands, consistent spacing = {gaps[0]}px -- OK")
    return float(gaps[0])


def check_real_word_wrap_by_width(renderer, font, measured_line_height: float) -> None:
    """Measures a long sentence's own real single-line pixel width,
    then forces it to wrap at a fraction of that width and confirms it
    now spans multiple real lines, spaced by the same real line_height
    the hard-wrap case measured -- proving one consistent layout, not
    two diverging code paths."""
    long_text = "the quick brown fox jumps"

    unwrapped_registry = tre.ShapeRegistry()
    unwrapped_registry.insert_text(tre.Text(TEXT_X, TEXT_Y, long_text, font, PX_SIZE, WHITE))
    frame, bands, background = render_until_ink(
        renderer, unwrapped_registry, int(TEXT_X), WIDTH, int(TEXT_Y), int(TEXT_Y + PX_SIZE * 1.5)
    )
    assert len(bands) == 1, f"the unwrapped baseline must be one real line: {bands}"
    y0, y1 = bands[0]
    natural_width = rightmost_ink_x(frame, background, y0, y1, WIDTH) - int(TEXT_X)
    assert natural_width > 0, "the unwrapped sentence must have measured some real rendered width"
    print(f"unwrapped '{long_text}' measured {natural_width}px wide at px_size={PX_SIZE} -- OK")

    wrap_width = natural_width / 2.5
    wrapped_registry = tre.ShapeRegistry()
    wrapped_registry.insert_text(
        tre.Text(TEXT_X, TEXT_Y, long_text, font, PX_SIZE, WHITE, wrap_width=wrap_width)
    )
    _frame2, wrapped_bands, _bg2 = render_until_ink(
        renderer, wrapped_registry, int(TEXT_X), WIDTH, int(TEXT_Y), HEIGHT
    )
    assert 2 <= len(wrapped_bands) <= 5, (
        f"wrapping at {wrap_width:.1f}px (~40% of the natural width) must split "
        f"'{long_text}' (5 words) across more than one real line, got {wrapped_bands}"
    )
    gaps = [wrapped_bands[i + 1][0] - wrapped_bands[i][0] for i in range(len(wrapped_bands) - 1)]
    for gap in gaps:
        assert abs(gap - measured_line_height) <= 2, (
            f"word-wrapped line spacing ({gap}px) must match the same real line_height "
            f"the hard-wrap case measured ({measured_line_height}px) -- one consistent layout"
        )
    print(
        f"real word-wrap at {wrap_width:.1f}px: {len(wrapped_bands)} real ink bands, "
        f"spacing matches the hard-wrap case's own {measured_line_height}px -- OK"
    )


def main() -> None:
    # Exactly ONE renderer for the whole process -- winit permits only
    # one real EventLoop per process, an established, hard-won lesson
    # from every earlier demo in this project.
    font = tre.Font.system_cascade()
    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)

    check_wrap_width_none_is_an_exact_regression(renderer, font)
    line_height = check_hard_wrap_on_explicit_newlines(renderer, font)
    check_real_word_wrap_by_width(renderer, font, line_height)
    print("tre_python multi-line/word-wrap text rendering (Phase 15 Step 15.1) demo: PASSED")


if __name__ == "__main__":
    main()
