"""0.6.3 (#266, TRE-REQ-007): a popup is placed by its body, not by its transparent margin.

A popup with a shadow draws the shadow in a transparent margin inside its own surface. With
`geometry=(m, m, w, h)` the compositor places that body rectangle and ignores the margin, so the body's
corner lands on the anchor point; without it the whole surface is placed and the body sits a margin off.
Read from a screenshot (grim, headless sway), because a position the compositor reports cannot tell which
rectangle it placed.

Needs a layer-shell compositor with `grim` and a private sway (`tools/sway_live.sh`); skips without them.
"""

import json
import os
import shutil
import subprocess
import sys
import textwrap

import pytest

import tess_re

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux")
    or not tess_re.supports("layer_shell")
    or not shutil.which("grim")
    or not os.environ.get("SWAYSOCK"),
    reason="needs a private sway (tools/sway_live.sh) with grim",
)

MARGIN = 20
CHILD = """
import json
import tess_re

M = %(margin)d
GEOMETRY = %(geometry)s
app = tess_re.App()
bar = tess_re.LayerWindow("tre-test-bar", layer="overlay", anchor=("top", "left"), size=(400, 32),
                          margin=(%(top)d, 0, 0, 40), keyboard="none", exclusive_zone=0)
bar.root.set(fill=(30, 30, 40, 255))
settings = dict(size=(120 + 2 * M, 60 + 2 * M), anchor_rect=(100, 0, 60, 32),
                anchor="bottom_left", gravity="bottom_right", grab=False, constraint_adjustment=%(adjust)s)
if GEOMETRY:
    settings["geometry"] = (M, M, 120, 60)
popup = tess_re.PopupWindow(bar, **settings)
body = popup.create("box", width=120, height=60, position="absolute", x=M, y=M, fill=(255, 0, 0, 255))
popup.root.add_child(body)
placed = []
popup.on("configure", lambda e: placed.append((e.x, e.y)))
app.add_window(bar)
bar.after(400, lambda: app.add_window(popup))
handle = app.thread_handle()


def report():
    print(json.dumps({"placed": placed[-1] if placed else None, "geometry": popup.get("geometry")}),
          flush=True)


bar.after(2500, report)
bar.after(5500, handle.stop)
app.run(keep_alive=True)
"""


WIDTH = 400


def output_height():
    outputs = json.loads(subprocess.run(["swaymsg", "-t", "get_outputs"], capture_output=True, check=True).stdout)
    return outputs[0]["rect"]["height"]


def run(geometry, top=50, adjust="()"):
    height = output_height()
    code = textwrap.dedent(CHILD) % {
        "margin": MARGIN,
        "geometry": "True" if geometry else "False",
        "top": top if top >= 0 else height + top,
        "adjust": adjust,
    }
    child = subprocess.Popen(
        [sys.executable, "-c", code], stdout=subprocess.PIPE, text=True, stderr=subprocess.DEVNULL
    )
    assert child.stdout is not None
    try:
        line = child.stdout.readline()
        assert line, "the child printed nothing"
        info = json.loads(line)
        region = f"0,0 {WIDTH}x{height}"
        shot = subprocess.run(
            ["grim", "-g", region, "-t", "ppm", "-"], capture_output=True, timeout=20, check=False
        ).stdout
    finally:
        child.wait(timeout=20)
    header, rest = shot.split(b"\n255\n", 1)
    width, height = (int(v) for v in header.split()[1:3])
    xs, ys = [], []
    for y in range(height):
        row = rest[y * width * 3 : (y + 1) * width * 3]
        for x in range(width):
            if row[x * 3 : x * 3 + 3] == b"\xff\x00\x00":
                xs.append(x)
                ys.append(y)
    assert xs, "the popup's red body is not on screen"
    return info, (min(xs), min(ys), max(xs) + 1, max(ys) + 1)


# The bar is at (40, 50) on the output; the anchor point is the bottom-left of the rectangle (100, 0, 60, 32)
# in the bar's own coordinates: (140, 82) on screen.
ANCHOR = (140, 82)


def test_with_a_geometry_the_body_lands_on_the_anchor_point():
    info, (left, top, right, bottom) = run(geometry=True)
    assert (left, top) == ANCHOR, f"the body's corner is at {(left, top)}"
    assert (right - left, bottom - top) == (120, 60)
    assert tuple(info["geometry"]) == (MARGIN, MARGIN, 120, 60)
    assert tuple(info["placed"]) == (100, 32), "configure reports the body's position"


def test_without_a_geometry_the_whole_surface_is_placed_as_before():
    info, (left, top, _, _) = run(geometry=False)
    assert (left, top) == (ANCHOR[0] + MARGIN, ANCHOR[1] + MARGIN), "the margin pushes the body off the anchor"
    assert tuple(info["geometry"]) == (0, 0, 120 + 2 * MARGIN, 60 + 2 * MARGIN)


def test_the_compositor_flips_by_the_body_not_by_the_margin():
    # The anchor point is 94 px above the bottom edge: the 60 px body fits below it with 2 px to spare,
    # the whole 100 px surface does not. With a geometry the popup stays below the anchor; without one
    # the compositor flips it above the bar, because it judged the margin too.
    anchor_y = output_height() - 62
    _, (_, top, _, _) = run(geometry=True, top=-94, adjust='("flip_y",)')
    assert top == anchor_y, f"the body flipped: its top is at {top}, the anchor at {anchor_y}"
    _, (_, top, _, _) = run(geometry=False, top=-94, adjust='("flip_y",)')
    assert top < anchor_y - 32, f"the surface was not flipped: the body's top is at {top}"
