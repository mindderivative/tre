"""0.5.4 (#110): `blur`, `backdrop_blur` and `blend_mode` on a node."""

import pytest

from tre import Window

BG = (40, 90, 200, 255)
RED = (255, 0, 0, 255)
YELLOW = (255, 200, 0, 255)


def scene():
    window = Window(width=100, height=100)
    window.root.set(fill=BG, padding=0)
    return window


def box(window, x=30, y=30, w=40, h=40, **props):
    node = window.create("box", width=w, height=h, x=x, y=y, position="absolute", **props)
    window.root.add_child(node)
    return node


def at(window, x, y):
    rgba, width, _ = window.snapshot()
    i = (y * width + x) * 4
    return tuple(rgba[i : i + 4])


def test_the_defaults_are_no_effect():
    node = box(scene(), fill=RED)
    assert node.get("blur") == 0.0
    assert node.get("backdrop_blur") == 0.0
    assert node.get("blend_mode") == "normal"


def test_a_blur_softens_edges_and_spreads_past_the_box():
    window = scene()
    box(window, fill=RED, blur=4)
    assert at(window, 50, 50)[:3] == (255, 0, 0)
    edge = at(window, 30, 50)
    assert 90 < edge[0] < 220
    assert at(window, 25, 50)[0] > 40
    assert at(window, 5, 5) == BG


def test_blur_is_settable_readable_and_animatable():
    window = scene()
    node = box(window, fill=RED)
    node.set(blur=2.5)
    assert node.get("blur") == 2.5
    node.animate("blur", 10, 1000)
    window.advance(500)
    assert node.get("blur") == pytest.approx(6.25, abs=0.01)
    assert node.get_target("blur") == 10.0
    window.advance(600)
    assert node.get("blur") == 10.0


def test_multiply_mixes_with_what_is_behind():
    window = scene()
    box(window, fill=YELLOW, blend_mode="multiply")
    got = at(window, 50, 50)
    assert all(abs(g - w) <= 2 for g, w in zip(got[:3], (40, 71, 0)))


def test_every_blend_mode_name_is_accepted_and_read_back():
    window = scene()
    node = box(window, fill=YELLOW)
    for name in ("normal", "multiply", "screen", "overlay", "darken", "lighten",
                 "color_dodge", "color_burn", "hard_light", "soft_light",
                 "difference", "exclusion", "hue", "saturation", "color", "luminosity"):
        node.set(blend_mode=name)
        assert node.get("blend_mode") == name
        window.snapshot()  # none panics


def test_a_backdrop_blur_frosts_what_is_behind_a_panel():
    window = scene()
    box(window, x=45, y=0, w=10, h=100, fill=RED)
    box(window, fill=(0, 0, 0, 0), backdrop_blur=4)
    assert at(window, 50, 10)[:3] == (255, 0, 0)   # outside the panel: hard
    assert at(window, 42, 50)[0] > 70               # inside: the bar bleeds
    assert at(window, 50, 50)[0] < 250              # and thins out


@pytest.mark.parametrize("prop", ["blur", "backdrop_blur"])
@pytest.mark.parametrize("value", [-1, "4", None, float("nan")])
def test_a_bad_blur_is_a_value_error(prop, value):
    node = box(scene(), fill=RED)
    with pytest.raises(ValueError, match=prop):
        node.set(**{prop: value})


def test_an_unknown_blend_mode_lists_the_valid_ones():
    node = box(scene(), fill=RED)
    with pytest.raises(ValueError, match="multiply"):
        node.set(blend_mode="plus")
    with pytest.raises(ValueError, match="blend_mode"):
        node.set(blend_mode=3)
    assert node.get("blend_mode") == "normal"


def test_a_snapshot_with_effects_is_repeatable():
    window = scene()
    box(window, x=45, y=0, w=10, h=100, fill=RED)
    box(window, fill=(255, 255, 255, 90), backdrop_blur=3, blur=1, blend_mode="screen")
    assert window.snapshot() == window.snapshot()
