#!/usr/bin/env python3
"""A canvas: a small line chart drawn with a `Painter` -- a background,
a curve through the samples, and a dot on each. The canvas narrows its
hit test to the newest sample's dot with `set_hit_test_circle`, so only a
click on that point reaches its `click` listener. Changing the data and
calling `redraw()` repaints it; nothing redraws a canvas on its own.

The script checks the hit test and a redraw with `window.simulate`, then
opens the window. Headless-CI-safe: `App.run()` renders `max_frames=60`
and returns quietly without a display or GPU. See
docs/guide/painting.md.
"""

from tre import App, Window

W, H, R = 240, 120, 5
BACKGROUND = (0xF7, 0xF2, 0xFA, 0xFF)
LINE = (0x67, 0x50, 0xA4, 0xFF)

window = Window(width=280, height=160, title="tre -- canvas")
samples = [30, 55, 40, 80, 65, 90]
draws = []


def points():
    step = (W - 2 * R) / (len(samples) - 1)
    return [(R + i * step, H - R - v) for i, v in enumerate(samples)]


def draw(painter):
    draws.append(len(samples))
    painter.fill_rect(0, 0, W, H, BACKGROUND)
    pts = points()
    path = [list(pts[0])]
    for (x0, y0), (x1, y1) in zip(pts, pts[1:]):  # a smooth curve: cubic segments
        mid = (x0 + x1) / 2
        path.append([mid, y0, mid, y1, x1, y1])
    painter.stroke_path(path, LINE, 2.0)
    for x, y in pts:
        painter.fill_circle(x, y, R, LINE)
    x, y = pts[-1]
    painter.set_hit_test_circle(x, y, R + 3)  # only the newest point is clickable


chart = window.create("canvas", width=W, height=H, draw=draw)
window.root.add_child(chart)
clicked = []
chart.on("click", lambda: clicked.append(samples[-1]))

# -- checks ------------------------------------------------------------------
assert draws == [6], "a canvas draws once when it's created"
x, y = points()[-1]
window.simulate("click", node=chart, x=x, y=y)
window.simulate("click", node=chart, x=10, y=10)  # inside the box, off the point
assert clicked == [90]

samples.append(70)
chart.redraw()
assert draws == [6, 7]
x, y = points()[-1]
window.simulate("click", node=chart, x=x, y=y)
assert clicked == [90, 70], "the hit test moved with the redraw"
print("canvas.py: checks passed")

app = App()
app.add_window(window)
app.run(max_frames=60)
print("canvas.py: exited cleanly")
