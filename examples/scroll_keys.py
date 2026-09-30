#!/usr/bin/env python3
"""Scrolling by keyboard: a list of 40 focusable rows in a 200-pixel scroll
view, with a status line the `scroll` event keeps up to date.

Tab moves focus down the rows, and the view follows it -- focus scrolls a
node into view. With a row focused, the arrow keys scroll 40 pixels, Page
Up and Page Down a viewport, and Home and End to either end. The status
line hears every change of offset, whatever made it. The script drives
each of these through `window.simulate` and checks the offsets before any
frame renders.

Headless-CI-safe: it checks the scrolling, renders `max_frames=60`, and
exits; `App.run()` returns quietly where no display is reachable.
See docs/guide/nodes-and-layout.md.
"""

from tre import App, Window

SURFACE = (0xFE, 0xF7, 0xFF, 0xFF)
ROW = (0xF3, 0xED, 0xF7, 0xFF)
ROW_HEIGHT = 32
GAP = 4

window = Window(width=320, height=280, title="tre -- scroll keys")
window.root.set(flex_direction="vertical", padding=16, gap=12, fill=SURFACE)

status = window.create("text", text="0 px down", font_size=14, height=20)
view = window.create("scroll_view", width=288, height=200)
content = window.create("box", flex_direction="vertical", width=288, gap=GAP)
rows = []
for i in range(40):
    row = window.create("box", width=288, height=ROW_HEIGHT, corner_radius=6,
                        fill=ROW, focusable=True, padding=6)
    row.add_child(window.create("text", text=f"Row {i + 1}", font_size=14, height=20))
    content.add_child(row)
    rows.append(row)
view.add_child(content)
window.root.add_child(status)
window.root.add_child(view)

seen = []


def on_scroll(e):
    seen.append(e.new_value)
    status.set(text=f"{e.new_value:.0f} px down")


view.on("scroll", on_scroll)

# Focus reveals: the seventh row sits below the fold until Tab reaches it.
rows[0].focus()
for _ in range(6):
    window.simulate("key_down", key="tab")
assert view.get("scroll_offset") > 0, "focusing row 7 scrolled it into view"

# Keys scroll the view around the focused row.
start = view.get("scroll_offset")
window.simulate("key_down", key="arrow_down")
assert view.get("scroll_offset") == start + 40
window.simulate("key_down", key="page_down")
assert view.get("scroll_offset") == start + 40 + 200
window.simulate("key_down", key="end")
bottom = view.get("scroll_offset")
assert bottom == content.get("layout_height") - 200
window.simulate("key_down", key="home")
assert view.get("scroll_offset") == 0

# Revealing a node from code, and the event heard every change.
rows[30].scroll_into_view()
assert seen[-1] == view.get("scroll_offset") > 0
assert status.get("text") == f"{seen[-1]:.0f} px down"
print(f"scroll_keys.py: checks passed, {len(seen)} scroll events")

app = App()
app.add_window(window)
app.run(max_frames=60)
print("scroll_keys.py: exited cleanly")
