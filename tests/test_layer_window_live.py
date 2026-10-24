"""0.5.7 (#198): a `LayerWindow` on a real compositor.

Needs a Wayland session whose compositor has the layer shell (KDE's KWin, wlroots');
skips without one. A bar is mapped on the desktop for a few seconds per test and
reserves screen space while it is up. The tests that need real pointer and key events
(`TRE_LIVE_INPUT=1`) make a virtual input device with `/dev/uinput`, which moves the
desktop's pointer, so they never run unasked.

Each case runs the app in a child process and reads the events it prints, the way
`test_os_preferences_live.py` does.
"""

import json
import os
import re
import select
import subprocess
import sys
import time

import pytest

import tre

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux") or not tre.supports("layer_shell"),
    reason="needs a Wayland compositor with the layer-shell protocol",
)

LIVE_INPUT = os.environ.get("TRE_LIVE_INPUT") == "1" and os.access("/dev/uinput", os.W_OK)

BAR = """
import json, os, sys, threading, time
from tre import App, LayerWindow, Window

args = json.loads(os.environ["BAR_ARGS"])
app = App()
bars = []
for spec in args["bars"]:
    bar = LayerWindow(
        "tre-test-bar",
        layer=spec.get("layer", "top"),
        anchor=tuple(spec.get("anchor", ("top", "left", "right"))),
        size=tuple(spec.get("size", (0, 32))),
        exclusive_zone=spec.get("zone", 32),
        margin=tuple(spec.get("margin", (0, 0, 0, 0))),
        keyboard=spec.get("keyboard", "none"),
        output=spec.get("output"),
    )
    bar.root.set(fill=(0x67, 0x50, 0xA4, 0xFF))
    button = bar.create("box", width=60, height=24, fill=(0xE8, 0xB4, 0x4C, 0xFF),
                        x=100, y=4, position="absolute", focusable=True)
    bar.root.add_child(button)
    def say(kind, **kw):
        print(json.dumps({"kind": kind, **kw}), flush=True)
    button.on("pointer_enter", lambda e: say("enter"))
    button.on("pointer_leave", lambda e: say("leave"))
    button.on("click", lambda e: say("click"))
    button.on("wheel", lambda e: say("wheel"))
    button.on("key_down", lambda e: say("key", key=e.key))
    bar.on("configure", lambda e: say("configure", width=e.width, height=e.height))
    bar.on("output_changed", lambda e: say("output", name=e.output))
    bar.on("closed", lambda e: say("closed", reason=e.reason))
    app.add_window(bar)
    bars.append(bar)
if args.get("window"):
    app.add_window(Window(width=200, height=100, title="tre-test-window"))

handle = app.thread_handle()

def control():
    time.sleep(args.get("seconds", 8))
    for bar in bars:
        handle.call_soon(bar.close)
    time.sleep(1.0)
    os._exit(0)

def stats():
    # frames drawn by the first bar over `idle` seconds, after it has settled
    if not args.get("idle"):
        return
    time.sleep(2.0)
    box = []
    done = threading.Event()
    def read():
        box.append(bars[0].frame_stats()["frames"])
        done.set()
    handle.call_soon(read)
    done.wait(3)
    first = box[0]
    time.sleep(args["idle"])
    box.clear(); done.clear()
    handle.call_soon(read); done.wait(3)
    print(json.dumps({"kind": "idle_frames", "frames": box[0] - first}), flush=True)

threading.Thread(target=control, daemon=True).start()
threading.Thread(target=stats, daemon=True).start()
print(json.dumps({"kind": "start"}), flush=True)
app.run()
print(json.dumps({"kind": "done"}), flush=True)
"""

MAXIMISED = """
import os, threading, time
from tre import App, Window
w = Window(width=400, height=300, title="tre-test-maximised")
app = App(); app.add_window(w); handle = app.thread_handle()
def drive():
    time.sleep(1.0)
    handle.call_soon(w.maximize)
    time.sleep(2.0)
    box = []; done = threading.Event()
    def read():
        box.append(w.get("height")); done.set()
    handle.call_soon(read); done.wait(5)
    print(box[0], flush=True)
    os._exit(0)
threading.Thread(target=drive, daemon=True).start()
app.run()
"""


class Child:
    def __init__(self, code, **env):
        self.process = subprocess.Popen(
            [sys.executable, "-I", "-c", code],
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            env=dict(os.environ, **env),
        )
        self.buffer = b""

    def line(self, timeout=15):
        end = time.time() + timeout
        while b"\n" not in self.buffer:
            assert time.time() < end, "the child printed nothing in time"
            ready, _, _ = select.select([self.process.stdout], [], [], 0.2)
            if ready:
                chunk = os.read(self.process.stdout.fileno(), 65536)
                if not chunk:
                    assert self.process.poll() is not None
                    break
                self.buffer += chunk
        line, _, self.buffer = self.buffer.partition(b"\n")
        return line.decode().strip()

    def events(self, until=None, timeout=15):
        """Events printed so far (JSON lines), up to and including the first `until` kind."""
        got = []
        end = time.time() + timeout
        while time.time() < end:
            try:
                text = self.line(timeout=max(0.2, end - time.time()))
            except AssertionError:
                break
            if not text.startswith("{"):
                if not text:
                    break
                continue
            event = json.loads(text)
            got.append(event)
            if until and event["kind"] == until:
                break
        return got

    def close(self):
        self.process.kill()
        self.process.wait()


@pytest.fixture
def children():
    made = []

    def start(code, **env):
        child = Child(code, **env)
        made.append(child)
        return child

    yield start
    for child in made:
        child.close()


def bar_child(children, *bars, **options):
    return children(BAR, BAR_ARGS=json.dumps({"bars": list(bars), **options}))


def outputs():
    """`{connector name: (x, y, width, height)}` from `wayland-info`, or skips."""
    try:
        text = subprocess.run(["wayland-info", "-i", "wl_output"], capture_output=True, text=True, timeout=10).stdout
    except (OSError, subprocess.TimeoutExpired):
        pytest.skip("wayland-info is needed to learn the outputs")
    found = {}
    for block in text.split("interface: 'wl_output'")[1:]:
        name = re.search(r"\n\s*name: (\S+)", block)
        position = re.search(r"x: (-?\d+), y: (-?\d+)", block)
        size = re.search(r"width: (\d+) px, height: (\d+) px", block)
        if name and position and size:
            found[name.group(1)] = (int(position[1]), int(position[2]), int(size[1]), int(size[2]))
    # The logical placement (xdg-output) is what a layout uses; `wl_output` x and y are
    # the physical location, which sway reports as 0, 0 for every output.
    try:
        full = subprocess.run(["wayland-info"], capture_output=True, text=True, timeout=10).stdout
    except (OSError, subprocess.TimeoutExpired):
        full = ""
    for logical in re.finditer(
        r"name: '(\S+)'.*?logical_x: (-?\d+), logical_y: (-?\d+)\s+logical_width: (\d+), logical_height: (\d+)",
        full,
        re.S,
    ):
        if logical[1] in found:
            found[logical[1]] = tuple(int(logical[i]) for i in range(2, 6))
    if not found:
        pytest.skip("no outputs found")
    return found


def maximised_height(children):
    child = children(MAXIMISED)
    text = child.line(timeout=20)
    return int(float(text))


def test_a_bar_is_sized_by_the_compositor_reports_its_output_and_closes_cleanly(children):
    name = next(iter(outputs()))
    width = outputs()[name][2]
    child = bar_child(children, dict(output=name), seconds=3)
    events = child.events(until="done")
    kinds = [e["kind"] for e in events]
    assert kinds[0] == "start" and kinds[-1] == "done", events
    configure = next(e for e in events if e["kind"] == "configure")
    assert (configure["width"], configure["height"]) == (width, 32)
    assert {"kind": "output", "name": name} in events
    assert {"kind": "closed", "reason": "app"} in events


def test_an_output_that_does_not_exist_raises_from_run(children):
    child = children(
        "from tre import App, LayerWindow\n"
        "w = LayerWindow('x', anchor=('top','left','right'), size=(0, 32), output='NOPE-9')\n"
        "a = App(); a.add_window(w)\n"
        "try:\n    a.run(max_frames=5)\nexcept RuntimeError as e:\n    print('RAISED', e)\n"
    )
    text = child.line(timeout=20)
    assert text.startswith("RAISED") and "NOPE-9" in text and "DP-1" in text or "HDMI" in text or "output" in text


def two_bars():
    return [dict(output=name, zone=40) for name in outputs()]


def test_the_exclusive_zone_shortens_a_maximised_window_by_the_zone(children):
    bars = [dict(output=name, zone=0) for name in outputs()]
    child = bar_child(children, *bars, seconds=12)
    child.events(until="configure")
    time.sleep(1.0)
    without = maximised_height(children)
    child.close()
    bars = [dict(output=name, zone=40) for name in outputs()]
    child = bar_child(children, *bars, seconds=12)
    child.events(until="configure")
    time.sleep(1.0)
    with_zone = maximised_height(children)
    assert without - with_zone == 40, (without, with_zone)


def test_an_idle_bar_draws_no_frames(children):
    name = next(iter(outputs()))
    child = bar_child(children, dict(output=name), seconds=9, idle=4)
    events = child.events(until="idle_frames", timeout=20)
    idle = next(e for e in events if e["kind"] == "idle_frames")
    assert idle["frames"] == 0


def test_output_none_lands_on_the_output_at_the_origin_every_run(children):
    found = outputs()
    expected = next((n for n, (x, y, _, _) in found.items() if (x, y) == (0, 0)), min(found))
    landed = set()
    for _ in range(3):
        child = bar_child(children, dict(), seconds=3)
        events = child.events(until="output", timeout=20)
        landed.add(next(str(e["name"]) for e in events if e["kind"] == "output"))
    assert landed == {expected}


def sway_outputs():
    import shutil
    if not (shutil.which("swaymsg") and os.environ.get("SWAYSOCK")):
        pytest.skip("needs sway (SWAYSOCK) to add and unplug an output")
    text = subprocess.run(["swaymsg", "-t", "get_outputs", "-r"], capture_output=True, text=True, timeout=10).stdout
    return {o["name"] for o in json.loads(text)}


def test_a_bar_on_an_unplugged_output_closes_with_output_removed(children):
    before = sway_outputs()
    subprocess.run(["swaymsg", "create_output"], capture_output=True, timeout=10)
    time.sleep(0.5)
    (name,) = sway_outputs() - before
    if name not in outputs():
        subprocess.run(["swaymsg", "output", name, "unplug"], capture_output=True, timeout=10)
        pytest.skip("the session under test is not the sway that SWAYSOCK names")
    code = BAR.replace(
        "handle = app.thread_handle()",
        'app.on("output_removed", lambda o: print(json.dumps({"kind": "removed", "name": o.name}), flush=True))\n'
        "handle = app.thread_handle()",
    )
    child = children(code, BAR_ARGS=json.dumps({"bars": [dict(output=name)], "seconds": 12}))
    child.events(until="output", timeout=20)
    subprocess.run(["swaymsg", "output", name, "unplug"], capture_output=True, timeout=10)
    events = []
    while {"closed", "removed"} - {e["kind"] for e in events}:
        batch = child.events(until="closed", timeout=20)
        assert batch, events
        events += batch
    kinds = [e["kind"] for e in events]
    assert "closed" in kinds and "removed" in kinds, events
    assert next(e for e in events if e["kind"] == "closed")["reason"] == "output_removed"
    assert next(e for e in events if e["kind"] == "removed")["name"] == name


def test_a_bar_follows_a_fractional_output_scale(children):
    sway_outputs()  # skips unless sway is reachable
    name = next(iter(outputs()))
    if name not in sway_outputs():
        pytest.skip("the session under test is not the sway that SWAYSOCK names")
    code = BAR.replace(
        "handle = app.thread_handle()",
        """handle = app.thread_handle()
def read_scale():
    time.sleep(2)
    box = []; done = threading.Event()
    def read():
        box.append(bars[0].get("scale_factor")); done.set()
    handle.call_soon(read); done.wait(3)
    print(json.dumps({"kind": "scale", "value": box[0]}), flush=True)
threading.Thread(target=read_scale, daemon=True).start()""",
    )
    subprocess.run(["swaymsg", "output", name, "scale", "1.5"], capture_output=True, timeout=10)
    try:
        child = children(code, BAR_ARGS=json.dumps({"bars": [dict(output=name)], "seconds": 4}))
        events = child.events(until="scale", timeout=20)
    finally:
        subprocess.run(["swaymsg", "output", name, "scale", "1"], capture_output=True, timeout=10)
    assert next(e for e in events if e["kind"] == "scale")["value"] == pytest.approx(1.5, abs=0.01)
    # A compositor that re-sends the same size is not a change.
    sizes = [(e["width"], e["height"]) for e in events if e["kind"] == "configure"]
    assert len(sizes) == len(set(sizes)), sizes


def _on_sway(name):
    sway_outputs()  # skips unless sway is reachable
    if name not in sway_outputs():
        pytest.skip("the session under test is not the sway that SWAYSOCK names")


def test_a_transparent_bar_shows_the_desktop_behind_it(children):
    import shutil
    if not shutil.which("grim"):
        pytest.skip("needs grim to read pixels")
    name = next(iter(outputs()))
    _on_sway(name)
    # Something opaque and red behind the bar: sway tiles a new window over the output, and
    # the bar sits 100px down so it is over the window's body, not sway's title bar.
    code = BAR.replace("bar.root.set(fill=(0x67, 0x50, 0xA4, 0xFF))", "bar.root.set(fill=(0, 0, 0, 0))").replace(
        'app.add_window(Window(width=200, height=100, title="tre-test-window"))',
        'behind = Window(width=200, height=100, title="tre-test-window")\n'
        "    behind.root.set(fill=(255, 0, 0, 255))\n"
        "    app.add_window(behind)",
    )
    child = children(code, BAR_ARGS=json.dumps({"bars": [dict(output=name, zone=0, margin=(100, 0, 0, 0))], "window": True, "seconds": 6}))
    child.events(until="configure", timeout=20)
    time.sleep(2.0)
    shot = subprocess.run(["grim", "-t", "ppm", "-o", name, "-"], capture_output=True, timeout=20).stdout
    header, rest = shot.split(b"\n255\n", 1)
    width = int(header.split()[1])
    def pixel(x, y):
        i = (y * width + x) * 3
        return tuple(rest[i : i + 3])
    assert pixel(10, 116) == (255, 0, 0), "the bar is see-through where it has no fill"
    assert pixel(130, 116) == (0xE8, 0xB4, 0x4C), "the opaque box still draws"


def test_paste_reaches_a_focused_layer_window_without_x(children):
    import shutil
    if not (shutil.which("wtype") and shutil.which("wl-copy")):
        pytest.skip("needs wtype and wl-copy")
    name = next(iter(outputs()))
    _on_sway(name)
    code = """
import json, os, threading, time
from tre import App, LayerWindow
app = App()
bar = LayerWindow("paste-test", anchor=("top", "left", "right"), size=(0, 40), exclusive_zone=40,
                  keyboard="exclusive", output=%r)
bar.root.set(fill=(30, 30, 30, 255))
field = bar.create("text_input", width=400, height=30, focusable=True)
bar.root.add_child(field)
field.focus()
app.add_window(bar)
handle = app.thread_handle()
def report():
    print(json.dumps({"kind": "ready"}), flush=True)
    time.sleep(6)
    box = []; done = threading.Event()
    def read():
        box.append(field.get("text")); done.set()
    handle.call_soon(read); done.wait(3)
    print(json.dumps({"kind": "text", "value": box[0]}), flush=True)
    os._exit(0)
threading.Thread(target=lambda: (time.sleep(1.5), report()), daemon=True).start()
app.run()
""" % name
    env = {k: v for k, v in os.environ.items() if k != "DISPLAY"}  # no X server to fall back on
    child = Child(code, **{"DISPLAY": ""})
    try:
        child.events(until="ready", timeout=20)
        copier = subprocess.Popen(["wl-copy", "-f", "pasted-text"], env=env)
        try:
            # `-s 300`: a fresh virtual keyboard sends its keymap first, and a key sent at
            # once is lost.
            subprocess.run(["wtype", "-s", "300", "xhello"], env=env, timeout=20)
            subprocess.run(["wtype", "-s", "300", "-M", "ctrl", "a", "-m", "ctrl"], env=env, timeout=20)
            subprocess.run(["wtype", "-s", "300", "-M", "ctrl", "v", "-m", "ctrl"], env=env, timeout=20)
            events = child.events(until="text", timeout=20)
        finally:
            copier.kill()
    finally:
        child.close()
    assert next(e for e in events if e["kind"] == "text")["value"] == "pasted-text"


REGION = """
import json, os, sys, threading, time
from tre import App, LayerWindow, Window

args = json.loads(os.environ["REGION_ARGS"])
app = App()
def say(kind, **kw):
    print(json.dumps({"kind": kind, **kw}), flush=True)
behind = Window(width=200, height=100, title="tre-test-behind")
behind.root.set(fill=(255, 0, 0, 255))
behind.root.on("click", lambda e: say("window_click"))
app.add_window(behind)
overlay = LayerWindow("tre-test-overlay", layer="overlay", anchor=("top", "bottom", "left", "right"),
                      size=(0, 0), exclusive_zone=0, output=args["output"], input_region=args["region"])
overlay.root.set(fill=(0, 0, 0, 0))
if not args.get("nodes"):
    overlay.root.on("click", lambda e: say("layer_click"))
for x, y, w, h, radius in args.get("nodes", []):
    box = overlay.create("box", width=w, height=h, x=x, y=y, position="absolute", fill=(40, 90, 200, 255),
                         corner_radius=radius)
    box.on("click", lambda e: say("layer_click"))
    overlay.root.add_child(box)
app.add_window(overlay)
threading.Thread(target=lambda: (time.sleep(args["seconds"]), os._exit(0)), daemon=True).start()
threading.Thread(target=lambda: (time.sleep(2.5), say("ready")), daemon=True).start()
app.run()
"""


@pytest.mark.parametrize(
    "region, inside, outside",
    [
        (None, "layer_click", "layer_click"),
        ([], "window_click", "window_click"),
        ([(0, 0, 200, 200)], "layer_click", "window_click"),
    ],
)
def test_input_region_decides_who_gets_the_click(children, region, inside, outside):
    _routed(children, region, inside, outside, [(100, 100), (700, 400)])


def _routed(children, region, inside, outside, points, nodes=()):
    sys.path.insert(0, os.path.dirname(__file__))
    import wlr_pointer
    if not wlr_pointer.available():
        pytest.skip("needs pywayland (pip install pywayland) for a virtual pointer")
    name = next(iter(outputs()))
    _on_sway(name)
    pointer = wlr_pointer.Pointer(os.environ)
    try:
        _region_clicks(children, pointer, name, region, [inside, outside], points, nodes)
    finally:
        pointer.close()


def _region_clicks(children, pointer, name, region, expected, points, nodes):
    child = children(
        REGION, REGION_ARGS=json.dumps({"output": name, "region": region, "nodes": list(nodes), "seconds": 9})
    )
    child.events(until="ready", timeout=20)
    x0, y0, _, _ = outputs()[name]
    layout = outputs().values()
    layout_width = max(ox + ow for (ox, _, ow, _) in layout)
    layout_height = max(oy + oh for (_, oy, _, oh) in layout)
    for (x, y) in points:
        pointer.move(x0 + x, y0 + y, layout_width, layout_height)
        pointer.click()
        time.sleep(0.5)
    events = child.events(timeout=4)
    clicks = [e["kind"] for e in events if e["kind"].endswith("_click")]
    assert clicks == expected, clicks


def test_auto_region_follows_a_circle_not_its_box(children):
    # A 200x200 box with corner_radius 100 is a circle at (400, 200). Its middle is the
    # layer's; the box's corner, outside the circle, is the window below's.
    _routed(children, "auto", "layer_click", "window_click", [(500, 300), (408, 208)],
            nodes=[(400, 200, 200, 200, 100)])


def test_a_screen_frame_takes_clicks_on_its_ring_and_lets_the_interior_through(children):
    # Four bars round the edge of the output: the ring is the layer's, the interior is not.
    ring = [(0, 0, 1280, 30, 0), (0, 690, 1280, 30, 0), (0, 0, 30, 720, 0), (1250, 0, 30, 720, 0)]
    _routed(children, "auto", "layer_click", "window_click", [(640, 12), (640, 360)], nodes=ring)


def test_a_margin_animation_moves_the_surface_smoothly_and_ends_on_time(children):
    import shutil
    if not shutil.which("grim"):
        pytest.skip("needs grim to read pixels")
    name = next(iter(outputs()))
    _on_sway(name)
    code = """
import json, os, threading, time
from tre import App, LayerWindow
app = App()
bar = LayerWindow("anim-test", anchor=("top", "left"), size=(200, 40), exclusive_zone=0, output=%r)
bar.root.set(fill=(200, 100, 50, 255))
app.add_window(bar)
handle = app.thread_handle()
def go():
    time.sleep(2.0)
    print(json.dumps({"kind": "start"}), flush=True)
    handle.call_soon(lambda: bar.animate("margin_top", 300, 800,
                     on_complete=lambda: print(json.dumps({"kind": "done"}), flush=True)))
    time.sleep(3)
    os._exit(0)
threading.Thread(target=go, daemon=True).start()
app.run()
""" % name
    child = children(code)
    child.events(until="start", timeout=20)
    x0, y0, _, _ = outputs()[name]

    def top():
        shot = subprocess.run(["grim", "-t", "ppm", "-o", name, "-"], capture_output=True, timeout=20).stdout
        header, rest = shot.split(b"\n255\n", 1)
        width, height = (int(v) for v in header.split()[1:3])
        for y in range(height):
            i = (y * width + 10) * 3
            if tuple(rest[i : i + 3]) == (200, 100, 50):
                return y
        return None

    seen, began = [], time.time()
    while time.time() - began < 2.0:
        seen.append(top())
    positions = [y for y in seen if y is not None]
    assert positions[-1] == 300, positions[-5:]
    assert positions == sorted(positions), "the bar only moves one way"
    middle = {y for y in positions if 0 < y < 300}
    assert len(middle) >= 5, f"a smooth slide has many steps, saw {sorted(middle)}"
    assert child.events(until="done", timeout=5)


DRAG = """
import json, os, threading, time
from tre import App, LayerWindow
app = App()
bar = LayerWindow("drag-test", anchor=("top", "left"), size=(200, 60), exclusive_zone=0, output=%r,
                  margin=(100, 0, 0, 100))
bar.root.set(fill=(200, 100, 50, 255), window_region="drag_margin")
bar.on("moved", lambda e: print(json.dumps({"kind": "moved", "margin": list(e.margin)}), flush=True))
app.add_window(bar)
threading.Thread(target=lambda: (time.sleep(2.5), print(json.dumps({"kind": "ready"}), flush=True),
                                 time.sleep(7), os._exit(0)), daemon=True).start()
app.run()
"""


@pytest.mark.parametrize(
    "drag_to, expected",
    [
        # Inside the output: the margins follow the pointer exactly.
        ((350, 280), (250, 0, 0, 300)),
        # Toward the far corner, past what the surface can reach: stops with it on the output.
        ((1279, 719), (660, 0, 0, 1080)),
        # Toward the near corner: never negative.
        ((0, 0), (0, 0, 0, 0)),
    ],
)
def test_drag_margin_moves_the_surface_with_the_pointer_and_stops_at_the_output_edge(children, drag_to, expected):
    sys.path.insert(0, os.path.dirname(__file__))
    import wlr_pointer
    if not wlr_pointer.available():
        pytest.skip("needs pywayland (pip install pywayland) for a virtual pointer")
    name = next(iter(outputs()))
    _on_sway(name)
    x0, y0, _, _ = outputs()[name]
    layout = outputs().values()
    width = max(ox + ow for (ox, _, ow, _) in layout)
    height = max(oy + oh for (_, oy, _, oh) in layout)
    pointer = wlr_pointer.Pointer(os.environ)
    try:
        child = children(DRAG % name)
        child.events(until="ready", timeout=20)
        start = (150, 130)  # inside the bar: x 100..300, y 100..160
        pointer.move(x0 + start[0], y0 + start[1], width, height)
        pointer.press()
        steps = 20
        for i in range(1, steps + 1):
            pointer.move(
                x0 + round(start[0] + (drag_to[0] - start[0]) * i / steps),
                y0 + round(start[1] + (drag_to[1] - start[1]) * i / steps),
                width, height,
            )
            time.sleep(0.03)
        pointer.release()
        events = child.events(until="moved", timeout=10)
    finally:
        pointer.close()
    moved = [e for e in events if e["kind"] == "moved"]
    assert len(moved) == 1, events
    got = tuple(moved[0]["margin"])
    assert all(abs(a - b) <= 2 for a, b in zip(got, expected)), (got, expected)


def test_layer_windows_and_an_ordinary_window_run_in_one_app(children):
    names = list(outputs())
    bars = [dict(output=n) for n in names]
    child = bar_child(children, *bars, window=True, seconds=4)
    # The ordinary window stays open after the bars close, so the app has no "done";
    # read until every bar has closed.
    events, closed = [], 0
    while closed < len(names):
        batch = child.events(until="closed", timeout=20)
        assert batch, events
        events += batch
        closed = [e["kind"] for e in events].count("closed")
    assert [e["kind"] for e in events].count("configure") == len(names)


# ---- real input (TRE_LIVE_INPUT=1) ----

live_input = pytest.mark.skipif(not LIVE_INPUT, reason="set TRE_LIVE_INPUT=1 (and have write access to /dev/uinput)")


def drive(output_name, steps):
    """Runs `steps(device, x_of_bar, y_of_bar_top)` with a virtual device, once the bar is up."""
    sys.path.insert(0, os.path.dirname(__file__))
    import uinput_device

    x, y, width, height = outputs()[output_name]
    desktop_height = max(oy + oh for (_, oy, _, oh) in outputs().values())
    device = uinput_device.Device(max(ox + ow for (ox, _, ow, _) in outputs().values()), desktop_height)
    try:
        steps(device, x, y)
    finally:
        device.close()


@live_input
def test_real_pointer_scroll_and_key_events_reach_the_bars_nodes(children):
    name = next(iter(outputs()))
    child = bar_child(children, dict(output=name, keyboard="on_demand"), seconds=8)
    child.events(until="output")
    time.sleep(0.5)

    def steps(device, ox, oy):
        device.move(ox + 900, oy + 500)
        device.move(ox + 130, oy + 16)  # onto the button (x 100 to 160, y 4 to 28)
        time.sleep(0.4)
        device.click()
        device.scroll(-1)
        device.type("a")
        time.sleep(0.3)
        device.move(ox + 900, oy + 16)  # on the bar, off the button
        time.sleep(0.3)
        device.move(ox + 900, oy + 500)

    drive(name, steps)
    events = child.events(until="closed", timeout=15)
    kinds = [e["kind"] for e in events]
    for expected in ("enter", "click", "wheel", "key", "leave"):
        assert expected in kinds, (expected, kinds)
    assert {"kind": "key", "key": "a"} in events
    assert kinds.index("enter") < kinds.index("click") < kinds.index("leave")


@live_input
def test_keyboard_none_never_gets_keys(children):
    name = next(iter(outputs()))
    child = bar_child(children, dict(output=name, keyboard="none"), seconds=7)
    child.events(until="output")
    time.sleep(0.5)

    def steps(device, ox, oy):
        device.move(ox + 900, oy + 500)
        device.move(ox + 130, oy + 16)
        time.sleep(0.4)
        device.click()
        device.type("ab")
        time.sleep(0.3)
        device.move(ox + 900, oy + 500)

    drive(name, steps)
    events = child.events(until="closed", timeout=15)
    assert not [e for e in events if e["kind"] == "key"], events
    assert any(e["kind"] == "click" for e in events)


@live_input
def test_a_bar_moved_off_the_screen_by_a_negative_margin_gets_no_pointer_events(children):
    name = next(iter(outputs()))
    child = bar_child(children, dict(output=name, zone=0, margin=(-32, 0, 0, 0)), seconds=6)
    child.events(until="output")
    time.sleep(0.5)

    def steps(device, ox, oy):
        for y in (3, 12, 28):
            device.move(ox + 130, oy + y)
            time.sleep(0.2)
        device.move(ox + 900, oy + 500)

    drive(name, steps)
    events = child.events(until="closed", timeout=15)
    assert not [e for e in events if e["kind"] in ("enter", "click")], events


# ---- outputs (0.5.7, #202) ----


def test_outputs_agree_with_what_the_compositor_reports():
    expected = outputs()
    got = tre.App().outputs()
    assert {o.name for o in got} == set(expected)
    for o in got:
        x, y, width, height = expected[o.name]
        assert (o.x, o.y) == (x, y)
        assert o.width > 0 and o.height > 0 and o.scale_factor >= 1.0
    # Top to bottom, then left to right.
    assert [(o.y, o.x) for o in got] == sorted((o.y, o.x) for o in got)


def test_a_surface_bound_by_an_output_object_opens_on_that_output(children):
    name = list(outputs())[-1]
    child = children(
        "import json, os, threading, time\n"
        "from tre import App, LayerWindow\n"
        f"app = App(); out = [o for o in app.outputs() if o.name == {name!r}][0]\n"
        "bar = LayerWindow('tre-test-bar', anchor=('top','left','right'), size=(0, 24), exclusive_zone=24, output=out)\n"
        "bar.on('output_changed', lambda e: print(json.dumps({'kind': 'output', 'name': e.output}), flush=True))\n"
        "app.add_window(bar); h = app.thread_handle()\n"
        "threading.Thread(target=lambda: (time.sleep(3), h.call_soon(bar.close)), daemon=True).start()\n"
        "app.run()\n"
        "print(json.dumps({'kind': 'done'}), flush=True)\n"
    )
    events = child.events(until="done", timeout=20)
    assert {"kind": "output", "name": name} in events, events


def test_watching_outputs_changes_nothing_at_start_and_a_run_still_ends_cleanly(children):
    # The outputs that exist when the app starts are not "added"; with a listener the layer
    # thread starts at once, even with no layer window.
    child = children(
        "import json, os, threading, time\n"
        "from tre import App, Window\n"
        "app = App(); seen = []\n"
        "for e in ('output_added', 'output_removed', 'output_updated'):\n"
        "    app.on(e, lambda o, e=e: seen.append((e, o.name)))\n"
        "w = Window(width=100, height=60); app.add_window(w)\n"
        "app.run(max_frames=90)\n"
        "print(json.dumps({'kind': 'done', 'seen': seen}), flush=True)\n"
    )
    events = child.events(until="done", timeout=30)
    assert events[-1] == {"kind": "done", "seen": []}, events
