#!/usr/bin/env python3
"""Phase 12 Step 12.1 proof: a real, on-screen window driven end to end
through the `tre_python` PyO3 binding -- `WindowedRenderer` (real
`PlatformConnection` + `VulkanSwapchain`, following `multi_window.rs`'s
own proven setup sequence), real polled `InputEvent`s, and real GPU
frames submitted directly to the window's own swapchain (no byte
readback -- that stays `HeadlessRenderer`'s own job). This is the first
real, end-to-end proof that `tre-python` is a GUI framework's backend,
not just a pretty offscreen renderer.

Run against a real Wayland/X11 session (`DISPLAY`/`WAYLAND_DISPLAY` must
be set) with real GPU hardware, via `maturin develop --release` first.
"""

import time

import tre_python as tre

WIDTH, HEIGHT = 640, 480
FRAME_TIME = 1.0 / 60.0


def build_registry() -> "tre.ShapeRegistry":
    registry = tre.ShapeRegistry()
    red = tre.rgba8(255, 0, 0, 255)
    green = tre.rgba8(0, 255, 0, 255)
    blue = tre.rgba8(0, 0, 255, 255)
    registry.insert_rectangle(tre.Rectangle(x=40, y=40, width=200, height=140, fill_color=red))
    registry.insert_circle(tre.Circle(x=320 - 60, y=200 - 60, radius=60, fill_color=green))
    registry.insert_polygon(tre.Polygon(x=460, y=340, sides=6, radius=70, fill_color=blue))
    return registry


def main() -> None:
    renderer = tre.WindowedRenderer("tre-python windowed renderer demo", WIDTH, HEIGHT)
    main_window = renderer.main_window
    print(f"created main window: {main_window!r}")
    assert renderer.open_window_count == 1, "constructor must register exactly one window"

    registry = build_registry()
    assert len(registry) == 3

    seen_event_types: set[str] = set()
    frames = 120
    for i in range(frames):
        for event in renderer.poll_events():
            seen_event_types.add(type(event).__name__)
            if isinstance(event, tre.InputEvent.CloseRequested):
                print("real CloseRequested event received -- exiting early")
                return
        renderer.render(main_window, registry)
        if i == 0:
            print("first real on-screen frame submitted -- OK")
        time.sleep(FRAME_TIME)

    print(f"rendered {frames} real frames to the window's own swapchain without error")
    print(f"real InputEvent variants observed via poll_events(): {sorted(seen_event_types) or '(none this run)'}")

    # Real window-lifecycle methods must not crash and must return sane
    # values -- these are the same real `PlatformConnection` methods
    # Phase 11 Step 11.1 already proved against a real compositor.
    renderer.set_title(main_window, "tre-python -- retitled")
    print(f"is_minimized: {renderer.is_minimized(main_window)}, is_maximized: {renderer.is_maximized(main_window)}")

    # Real multi-window support, sharing this renderer's one VulkanDevice
    # (multi_window.rs's own proven pattern) -- not artificially limited
    # to a single window.
    second = renderer.create_window("tre-python second window", 320, 240)
    assert renderer.open_window_count == 2, "create_window must add a second real window"
    for _ in range(30):
        renderer.poll_events()
        renderer.render(second, registry)
        time.sleep(FRAME_TIME)
    renderer.close_window(second)
    assert renderer.open_window_count == 1, "close_window must remove the closed window"
    print("multi-window: created a second real window, rendered into it, and closed it -- OK")

    print("tre_python windowed renderer demo: PASSED")


if __name__ == "__main__":
    main()
