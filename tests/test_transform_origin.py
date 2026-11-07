"""#269 (TRE-REQ-013 step 1): `transform_origin` is the point `scale` and
`rotation_deg` turn about, as fractions of the box. Pixels come from
`Window.snapshot()`."""

import pytest

from tess_re import Window

BACK = (0x10, 0x20, 0x30, 255)
RED = (255, 0, 0, 255)


def pixel(shot, x, y):
    rgba, width, _ = shot
    i = (y * width + x) * 4
    return tuple(rgba[i : i + 4])


def scene(**props):
    """A 200x200 window with a 40x40 red box at (80, 80)."""
    window = Window(width=200, height=200)
    window.root.set(fill=BACK)
    box = window.create("box", width=40, height=40, x=80, y=80, position="absolute", fill=RED, **props)
    window.root.add_child(box)
    return window, box


def test_the_default_origin_is_the_centre():
    window, box = scene(scale=2.0)
    assert box.get("transform_origin") == (0.5, 0.5)
    shot = window.snapshot()
    assert pixel(shot, 62, 100) == RED and pixel(shot, 138, 100) == RED
    assert pixel(shot, 58, 100) == BACK and pixel(shot, 142, 100) == BACK


def test_scale_about_the_top_left_corner_keeps_that_corner_fixed():
    window, box = scene(scale=2.0, transform_origin=(0.0, 0.0))
    shot = window.snapshot()
    assert pixel(shot, 82, 82) == RED and pixel(shot, 158, 158) == RED
    assert pixel(shot, 78, 100) == BACK and pixel(shot, 100, 78) == BACK
    assert pixel(shot, 162, 100) == BACK


def test_scale_about_the_bottom_right_corner():
    window, box = scene(scale=2.0, transform_origin=(1.0, 1.0))
    shot = window.snapshot()
    assert pixel(shot, 82, 82) == RED and pixel(shot, 118, 118) == RED
    assert pixel(shot, 38, 100) == BACK and pixel(shot, 122, 100) == BACK


def test_a_quarter_turn_about_a_corner_swings_the_box_round_that_corner():
    window, box = scene(rotation_deg=90.0, transform_origin=(0.0, 0.0))
    shot = window.snapshot()
    # Clockwise about (80, 80): the box now covers x 40..80, y 80..120.
    assert pixel(shot, 60, 100) == RED
    assert pixel(shot, 100, 100) == BACK


def test_the_origin_can_change_after_creation():
    window, box = scene(scale=2.0)
    box.set(transform_origin=(0.0, 0.0))
    assert box.get("transform_origin") == (0.0, 0.0)
    assert pixel(window.snapshot(), 158, 158) == RED


@pytest.mark.parametrize("bad", [1.0, (0.5,), (0.5, "a"), (float("nan"), 0), None, "centre"])
def test_a_bad_origin_is_a_value_error(bad):
    window, box = scene()
    with pytest.raises(ValueError, match="transform_origin"):
        box.set(transform_origin=bad)
