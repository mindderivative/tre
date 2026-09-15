#!/usr/bin/env python3
"""Phase 12 Step 12.3 proof: a real, retained-mode `tre.Text` shape,
end to end through `tre_python` -- `tre.Font.system_cascade()` (real
fontconfig discovery), `registry.insert_text(...)`, and a real headless
render that actually shows non-background pixels where the text is.

Mirrors `canvas_draw_text_demo.rs`'s own two-phase contract at the
Python level: the first few `render()` calls are real cache misses
(nothing visible yet, matching `draw_text`'s documented "report, don't
block" contract) while the real background atlas thread rasterizes each
glyph; `HeadlessRenderer`'s own `TextAtlas` (Phase 12 Step 12.3) only
re-uploads its live GPU texture from the atlas's current pixel buffer
every 15 frames (a real, disclosed throttle -- see
`crates/tre-python/src/text_atlas.rs`'s own doc comment), so this demo
renders enough real frames, with real wall-clock gaps for the
background thread to actually work, before checking pixels.
"""

import time

import tre_python as tre

WIDTH, HEIGHT = 400, 150
PX_SIZE = 40.0
TEXT_X, TEXT_Y = 20.0, 20.0
WHITE = tre.rgba8(255, 255, 255, 255)

# Comfortably more than TextAtlas's own REFRESH_INTERVAL_FRAMES (15),
# with real sleeps between calls so the atlas's background thread has
# real wall-clock time to rasterize and pack every glyph before this
# demo's final pixel check.
RENDER_ATTEMPTS = 60


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def main() -> None:
    font = tre.Font.system_cascade()
    print("loaded the real system cascade font -- OK")

    registry = tre.ShapeRegistry()
    text = tre.Text(TEXT_X, TEXT_Y, "Hi tre!", font, PX_SIZE, WHITE)
    registry.insert_text(text)
    assert len(registry) == 1

    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)

    background = None
    found_real_ink = False
    for attempt in range(RENDER_ATTEMPTS):
        frame = renderer.render(registry)
        if background is None:
            background = pixel_at(frame, 0, 0)
        # Scan the text's own real bounding region (its px_size square
        # extends below TEXT_Y by roughly px_size, and to the right by
        # roughly len(text) * px_size) for any pixel that differs from
        # the background -- the same "scan the whole region, not just
        # one point" precedent `canvas_draw_text_demo.rs` and
        # `atlas_concurrency_demo.rs` both already establish, since an
        # individual glyph's own bounding-box center can genuinely be
        # background (an open counter, whitespace, ...).
        x0, y0 = int(TEXT_X), int(TEXT_Y)
        x1 = min(WIDTH, int(TEXT_X + len("Hi tre!") * PX_SIZE))
        y1 = min(HEIGHT, int(TEXT_Y + PX_SIZE * 1.5))
        for y in range(y0, y1, 2):
            for x in range(x0, x1, 2):
                if pixel_at(frame, x, y) != background:
                    found_real_ink = True
                    break
            if found_real_ink:
                break
        if found_real_ink:
            print(f"real, non-background text pixels found after {attempt + 1} render() calls")
            break
        time.sleep(0.05)

    assert found_real_ink, (
        f"no real text pixels appeared within {RENDER_ATTEMPTS} render() calls -- either the "
        "atlas never resolved the glyphs, or the periodic texture refresh never ran"
    )

    print("tre_python Font/Text demo: PASSED")


if __name__ == "__main__":
    main()
