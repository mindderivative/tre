"""0.5.6 (#164): the `mask` node property clips a node and its subtree to a
circle, a rounded box or a path. Pixels come from `Window.snapshot()`."""

import pytest

from tre import Window

BACK = (0x10, 0x20, 0x30, 255)
RED = (255, 0, 0, 255)


def pixel(shot, x, y):
    rgba, width, _ = shot
    i = (y * width + x) * 4
    return tuple(rgba[i : i + 4])


def scene(**box_props):
    """A 100x100 window with an 80x80 red box at (10, 10)."""
    window = Window(width=100, height=100)
    window.root.set(fill=BACK)
    box = window.create("box", width=80, height=80, x=10, y=10, position="absolute",
                        fill=RED, **box_props)
    window.root.add_child(box)
    return window, box


def test_no_mask_paints_the_whole_box():
    window, _ = scene()
    shot = window.snapshot()
    assert pixel(shot, 11, 11) == RED and pixel(shot, 88, 88) == RED


def test_a_circle_clips_the_corners_and_keeps_the_middle():
    window, box = scene(mask="circle")
    shot = window.snapshot()
    assert pixel(shot, 50, 50) == RED
    assert pixel(shot, 11, 11) == BACK and pixel(shot, 88, 11) == BACK
    assert pixel(shot, 11, 88) == BACK and pixel(shot, 88, 88) == BACK
    assert pixel(shot, 50, 12) == RED                      # top of the circle
    box.set(mask=None)
    assert pixel(window.snapshot(), 11, 11) == RED


def test_a_rounded_mask_is_independent_of_the_nodes_corner_radius():
    window, box = scene(mask={"rounded": 30})
    shot = window.snapshot()
    assert box.get("corner_radius") == 0
    assert pixel(shot, 11, 11) == BACK and pixel(shot, 50, 11) == RED
    assert pixel(shot, 30, 30) == RED


def test_a_rounded_mask_takes_a_radius_for_each_corner():
    window, _ = scene(mask={"rounded": (40, 0, 40, 0)})
    shot = window.snapshot()
    assert pixel(shot, 11, 11) == BACK and pixel(shot, 88, 88) == BACK     # rounded corners
    assert pixel(shot, 88, 11) == RED and pixel(shot, 11, 88) == RED        # square corners


def test_a_path_mask_is_fitted_into_the_box():
    # A triangle in a 10x10 view box: apex top middle, base along the bottom.
    mask = {"path": "M5 0 L10 10 L0 10 Z", "view_box": (0, 0, 10, 10)}
    window, _ = scene(mask=mask)
    shot = window.snapshot()
    assert pixel(shot, 50, 60) == RED                        # inside
    assert pixel(shot, 15, 20) == BACK                       # outside, upper left
    assert pixel(shot, 85, 20) == BACK                       # outside, upper right
    assert pixel(shot, 50, 85) == RED                        # base


def test_a_mask_clips_the_subtree():
    window, box = scene(mask="circle")
    child = window.create("box", width=200, height=200, x=-50, y=-50, position="absolute",
                          fill=(0, 255, 0, 255))
    box.add_child(child)
    shot = window.snapshot()
    assert pixel(shot, 50, 50) == (0, 255, 0, 255)          # the child, inside the circle
    assert pixel(shot, 11, 11) == BACK                       # and cut off at the corner
    assert pixel(shot, 2, 2) == BACK                         # and nowhere outside the node


def test_a_mask_follows_the_nodes_transform():
    window, box = scene(mask="circle")
    box.set(translate_x=0)
    shot = window.snapshot()
    box.set(scale=0.5)
    small = window.snapshot()
    assert pixel(shot, 25, 50) == RED and pixel(small, 25, 50) == BACK
    assert pixel(small, 50, 50) == RED


def test_it_reads_back_as_set():
    _, box = scene()
    assert box.get("mask") is None
    for mask in ("circle", {"rounded": 12.0}, {"rounded": (1.0, 2.0, 3.0, 4.0)},
                 {"path": "M0,0 L10,0 L10,10 Z", "view_box": (0.0, 0.0, 10.0, 10.0)}):
        box.set(mask=mask)
        assert box.get("mask") == mask


@pytest.mark.parametrize("value", [
    "square", 5, {"rounded": -1}, {"rounded": "x"}, {"oval": 1}, {"path": "M0 0"},
    {"path": "not a path", "view_box": (0, 0, 1, 1)},
    {"path": "M0 0 L1 1", "view_box": (0, 0, 0, 1)},
    {"path": "M0 0 L1 1", "view_box": (0, 0, 1)},
    {"rounded": 4, "path": "M0 0", "view_box": (0, 0, 1, 1)},
])
def test_a_bad_mask_is_rejected_and_changes_nothing(value):
    _, box = scene(mask="circle")
    with pytest.raises(ValueError):
        box.set(mask=value, label="x")
    assert box.get("mask") == "circle" and box.get("label") is None


def test_a_mask_changes_the_partial_redraw():
    """A change of mask repaints (the damage fingerprint sees it)."""
    window, box = scene()
    window.snapshot()
    box.set(mask="circle")
    assert pixel(window.snapshot(), 11, 11) == BACK


def test_a_mask_also_clips_the_backdrop_blur():
    window = Window(width=100, height=100)
    window.root.set(fill=BACK)
    stripes = window.create("box", width=100, height=10, x=0, y=10, position="absolute",
                            fill=(255, 255, 255, 255))
    glass = window.create("box", width=80, height=80, x=10, y=10, position="absolute",
                          fill=(0, 0, 0, 0), backdrop_blur=6.0, mask="circle")
    window.root.add_child(stripes)
    window.root.add_child(glass)
    shot = window.snapshot()
    # In the corner of the glass box (outside the circle) the stripe is still sharp.
    assert pixel(shot, 12, 12) == (255, 255, 255, 255)
    assert pixel(shot, 12, 19) == (255, 255, 255, 255)
    # Inside the circle it is blurred: the edge of the stripe is no longer crisp.
    assert pixel(shot, 50, 19) != (255, 255, 255, 255)
