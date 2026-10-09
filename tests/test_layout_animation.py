"""0.5.6 (#161): layout properties animate (in pixels), with layout run again
each frame."""

import pytest

from tre import Window


def scene(**first):
    """A horizontal flex row: a box, then a second box that follows it."""
    window = Window(width=400, height=200)
    row = window.create("box", width=400, height=100, flex_direction="horizontal",
                        fill=(0, 0, 0, 0))
    window.root.add_child(row)
    a = window.create("box", width=100, height=50, fill=(255, 0, 0, 255), **first)
    b = window.create("box", width=40, height=50, fill=(0, 255, 0, 255))
    row.add_child(a)
    row.add_child(b)
    window.advance(0)
    return window, row, a, b


def test_width_animates_and_moves_what_follows():
    window, row, a, b = scene()
    def gap():
        return b.get("layout_x") - a.get("layout_x")

    assert a.get("layout_width") == 100 and gap() == 100
    a.animate("width", 200, 100)
    window.advance(50)
    assert 100 < a.get("layout_width") < 200
    assert gap() == pytest.approx(a.get("layout_width"), abs=0.01)
    window.advance(60)
    assert a.get("layout_width") == 200 and gap() == 200
    assert a.get("width") == 200


def test_get_returns_the_current_value_and_get_target_the_end():
    window, _, a, _ = scene()
    a.animate("width", 300, 100)
    window.advance(40)
    assert 100 < a.get("width") < 300
    assert a.get_target("width") == 300
    window.advance(100)
    assert a.get_target("width") == 300 and a.get("width") == 300


def test_auto_size_starts_from_what_layout_gave_it():
    window = Window(width=400, height=200)
    box = window.create("box", width=120, fill=(255, 0, 0, 255))
    box.set(height="auto")
    window.root.add_child(box)
    window.advance(0)
    box.animate("width", 60, 100)
    window.advance(50)
    assert 60 < box.get("layout_width") < 120


def test_other_properties_in_pixels():
    window, row, a, b = scene()
    a.set(padding_left=0, margin_left=0, margin_top=0)
    row.set(gap=0)
    a.animate("padding_left", 20, 50)
    a.animate("margin_left", 10, 50)
    row.animate("gap", 12, 50)
    window.advance(100)
    assert a.get("padding_left") == 20 and a.get("margin_left") == 10
    assert row.get("gap") == 12
    assert b.get("layout_x") - a.get("layout_x") == pytest.approx(100 + 12, abs=0.01)
    assert a.get("layout_x") - row.get("layout_x") >= 10


def test_a_shorthand_animates_every_side():
    window, _, a, _ = scene()
    a.set(padding=0)
    a.animate("padding", 8, 50)
    window.advance(100)
    assert a.get("padding") == 8


def test_x_and_y_move_an_absolute_node():
    window = Window(width=400, height=200)
    box = window.create("box", width=50, height=50, x=10, y=10, position="absolute",
                        fill=(255, 0, 0, 255))
    window.root.add_child(box)
    window.advance(0)
    box.animate("x", 110, 100)
    box.animate("y", -20, 100)
    window.advance(50)
    assert 10 < box.get("layout_x") < 110 and -20 < box.get("layout_y") < 10
    window.advance(60)
    assert (box.get("x"), box.get("y")) == (110, -20)


def test_an_interrupted_animation_retargets_from_where_it_is():
    window, _, a, _ = scene()
    seen = []
    a.on("animation_end", lambda e: seen.append((e.property, e.finished)))
    a.animate("width", 300, 1000)
    window.advance(250)
    mid = a.get("width")
    assert 100 < mid < 300
    a.animate("width", 50, 100)
    window.advance(10)
    assert seen == [("width", False)]
    assert a.get("width") <= mid
    window.advance(200)
    assert a.get("width") == 50 and seen[-1] == ("width", True)


def test_setting_the_property_cancels_the_animation():
    window, _, a, _ = scene()
    seen = []
    a.on("animation_end", lambda e: seen.append((e.property, e.finished)))
    a.animate("width", 300, 1000)
    window.advance(100)
    a.set(width=150)
    window.advance(100)
    assert a.get("width") == 150 and seen == [("width", False)]


def test_stop_animation_leaves_it_where_it_is():
    window, _, a, _ = scene()
    a.animate("width", 300, 1000)
    window.advance(300)
    mid = a.get("width")
    a.stop_animation("width")
    window.advance(500)
    assert a.get("width") == mid


def test_on_complete_and_easings_work():
    window, _, a, _ = scene()
    done = []
    a.animate("width", 160, 100, easing=(0.0, 0.0, 0.2, 1.0), on_complete=lambda: done.append(1))
    window.advance(200)
    assert done == [1] and a.get("width") == 160


def test_a_spring_settles_on_its_target():
    window, _, a, _ = scene()
    a.animate("width", 220, 0, easing="spring")
    window.advance(3000)
    assert a.get("width") == pytest.approx(220, abs=0.01)


def test_it_cannot_animate_to_auto_or_a_percentage_or_a_negative_size():
    _, _, a, _ = scene()
    for bad in ("auto", "50%", None, True):
        with pytest.raises(ValueError, match="pixels"):
            a.animate("width", bad, 100)
    with pytest.raises(ValueError, match="negative"):
        a.animate("width", -5, 100)
    a.animate("margin_left", -5, 100)            # a margin may be negative


def test_it_cannot_start_from_auto_or_a_percentage_except_width_and_height():
    _, _, a, _ = scene()
    a.set(padding_left="10%")
    with pytest.raises(ValueError, match="no number to animate from"):
        a.animate("padding_left", 5, 100)
    a.set(margin_left="auto")
    with pytest.raises(ValueError, match="no number to animate from"):
        a.animate("margin_left", 5, 100)
    a.set(width="50%")
    a.animate("width", 80, 100)                   # width starts from the laid-out size


def test_font_size_still_does_not_animate():
    _, _, a, _ = scene()
    with pytest.raises(ValueError, match="isn't animatable"):
        a.animate("font_size", 20, 100)


def test_an_animation_that_fails_registers_nothing():
    window, _, a, _ = scene()
    a.set(padding_left="10%")
    with pytest.raises(ValueError):
        a.animate("padding_left", 5, 100, on_complete=lambda: None)
    seen = []
    a.on("animation_end", lambda e: seen.append(e))
    window.advance(500)
    assert seen == []


LIVE = """
import json
from tre import App, Window

window = Window(width=300, height=150)
a = window.create("box", width=60, height=40, fill=(255, 0, 0, 255))
window.root.add_child(a)
seen = []
def ended(event):
    seen.append((event.property, event.finished, a.get("layout_width")))
    window.close()
a.on("animation_end", ended)
a.animate("width", 180, 200)
app = App()
app.add_window(window)
app.run()
print(json.dumps({"seen": seen, "frames": window.frame_stats()["frames"]}))
"""


def test_a_live_window_runs_a_layout_animation_to_its_end():
    import json
    import os
    import subprocess
    import sys

    if not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")):
        pytest.skip("needs a display")
    result = subprocess.run([sys.executable, "-c", LIVE], capture_output=True, text=True, timeout=60)
    assert result.returncode == 0, result.stderr
    data = json.loads(result.stdout.strip().splitlines()[-1])
    assert data["seen"] == [["width", True, 180.0]], data
    assert data["frames"] >= 3
