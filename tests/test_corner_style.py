"""#269 (TRE-REQ-013 step 1, part B): `corner_style` makes a box's corner curve inward.

A concave corner is the box with a quarter disc about its own corner cut away, so a small box
with one concave corner is the fillet that fills the inside corner where two shapes meet.
Pixels come from `Window.snapshot()`, clicks from `Window.simulate`.
"""

import pytest

from tess_re import Window

BACK = (0x10, 0x20, 0x30, 255)
RED = (255, 0, 0, 255)


def pixel(shot, x, y):
    rgba, width, _ = shot
    i = (y * width + x) * 4
    return tuple(rgba[i : i + 4])


def scene(size=40, **props):
    """A 200x200 window with a `size` square red box at (80, 80)."""
    window = Window(width=200, height=200)
    window.root.set(fill=BACK)
    box = window.create("box", width=size, height=size, x=80, y=80, position="absolute",
                        fill=RED, **props)
    window.root.add_child(box)
    clicks = []
    box.on("click", lambda e: clicks.append("box"))
    window.root.on("click", lambda e: clicks.append("root"))
    return window, box, clicks


def test_the_default_is_round_and_a_box_is_unchanged():
    window, box, _ = scene(corner_radius=12)
    assert box.get("corner_style") == "round"
    assert pixel(window.snapshot(), 81, 81) == BACK  # the usual rounded corner is cut
    assert pixel(window.snapshot(), 100, 100) == RED


def test_a_concave_corner_curves_inward_about_the_box_corner():
    window, box, _ = scene(corner_radius=20, corner_style=("concave", "round", "round", "round"))
    assert box.get("corner_style") == ("concave", "round", "round", "round")
    shot = window.snapshot()
    # About the top-left corner (80, 80), radius 20: its inside is cut, the rest of the box stays.
    assert pixel(shot, 85, 85) == BACK and pixel(shot, 90, 90) == BACK
    assert pixel(shot, 105, 82) == RED and pixel(shot, 82, 105) == RED
    assert pixel(shot, 100, 100) == RED


def test_the_other_corners_keep_their_own_style():
    window, box, _ = scene(corner_radius=16, corner_style=("round", "concave", "round", "round"))
    shot = window.snapshot()
    assert pixel(shot, 115, 85) == BACK  # concave top-right, about (120, 80)
    assert pixel(shot, 81, 81) == BACK  # round top-left is cut
    assert pixel(shot, 90, 90) == RED
    assert pixel(shot, 118, 118) == BACK  # round bottom-right


def test_a_fillet_is_a_small_box_with_one_concave_corner():
    window, box, _ = scene(size=12, corner_radius=12, corner_style=("round", "round", "concave", "round"))
    box.set(corner_radius=(0, 0, 12, 0))
    shot = window.snapshot()
    assert pixel(shot, 81, 81) == RED and pixel(shot, 82, 81) == RED and pixel(shot, 81, 82) == RED
    assert pixel(shot, 89, 89) == BACK and pixel(shot, 91, 91) == BACK


def test_the_radius_animates_on_a_concave_corner():
    window, box, _ = scene(size=40, corner_radius=0, corner_style=("concave", "round", "round", "round"))
    assert pixel(window.snapshot(), 82, 82) == RED
    box.set(corner_radius=(20, 0, 0, 0))
    assert pixel(window.snapshot(), 82, 82) == BACK
    box.animate("corner_radius", (0, 0, 0, 0), 100)
    window.advance(200)
    assert pixel(window.snapshot(), 82, 82) == RED


def test_a_click_in_the_notch_falls_through_and_the_body_takes_it():
    window, box, clicks = scene(corner_radius=(20, 0, 0, 0), corner_style=("concave", "round", "round", "round"))
    window.simulate("click", x=85, y=85)
    assert clicks == ["root"]
    clicks.clear()
    window.simulate("click", x=110, y=110)
    assert clicks == ["box", "root"]


def test_a_border_follows_the_notch():
    window, box, _ = scene(corner_radius=(20, 0, 0, 0), corner_style=("concave", "round", "round", "round"),
                           stroke_color=(0, 255, 0, 255), stroke_width=3)
    shot = window.snapshot()
    assert pixel(shot, 100, 82) == (0, 255, 0, 255)  # along the top edge
    assert pixel(shot, 85, 85) == BACK


@pytest.mark.parametrize("bad", ["square", ("round",) * 3, ("round", "round", "round", 1), 3, None, ""])
def test_a_bad_style_is_a_value_error(bad):
    window, box, _ = scene()
    with pytest.raises(ValueError, match="corner_style"):
        box.set(corner_style=bad)
