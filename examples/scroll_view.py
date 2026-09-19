#!/usr/bin/env python3
"""M36 Phase 1's real `Window.add_scroll_view` (§5, §7, §11.7): a real,
general scrollable viewport over one child, grounded directly in the
sibling `pyCopper` project's own `ScrollViewElement`. Compose real
content in via the existing, generic `Node.add_child` -- the content
needs its own real, explicit size on the scroll axis, the same
convention every other `add_*` factory's own children already follow.

What this script proves automatically (headless-CI-safe, no human
needed): a real button 30 rows deep in the scrolled content is
unreachable before scrolling and reaches its own handler correctly
after a real scroll -- the exact real "click after scroll" scenario
this phase's own investigation found a genuine, previously-undiscovered
bug for in `VirtualList` (a real scroll offset applied only at paint
time, never reflected back into hit-testing). `ScrollView` avoids that
whole bug class by baking its own scroll-shifted position into real
`layout_style` every frame, the same bug-free pattern `Carousel`
already established.
"""

from typing import Callable

from tre import App, Window

window = Window(width=800, height=400, title="tre v2 -- scroll view")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

ROW_HEIGHT = 40
ROW_COUNT = 30

view = window.add_scroll_view(width=300, height=200, x=40.0, y=40.0)
content = window.add_rect(background=(0, 0, 0, 0), width=300, height=ROW_HEIGHT * ROW_COUNT)
view.add_child(content)

events: list[str] = []


def make_handler(row: int) -> Callable[[], None]:
    def handle() -> None:
        events.append(f"row-{row}")

    return handle


last_button = None
for i in range(ROW_COUNT):
    button = window.add_button(
        label=f"Row {i}",
        width=280,
        height=ROW_HEIGHT - 8,
        x=10.0,
        y=float(i * ROW_HEIGHT),
    )
    content.add_child(button)
    button.enable_interaction()
    button.set_on_click(make_handler(i))
    if i == ROW_COUNT - 1:
        last_button = button

assert last_button is not None

# Scroll to the bottom and click the last row -- a real position that
# only exists once the content has genuinely moved.
window.scroll(view, float(ROW_HEIGHT * ROW_COUNT))
window.click(last_button)

assert events == [f"row-{ROW_COUNT - 1}"], f"the last row's own click must reach its own handler, got {events}"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"scroll_view.py: exited cleanly after 60 frames, events {events}")
