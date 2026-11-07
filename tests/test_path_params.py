"""#272 (TRE-REQ-013 step 4): a path written once with named parameters, which the engine
evaluates again as `animate_param` moves them, with no Python per frame. Pixels come from
`Window.snapshot()`, time from `Window.advance()`."""

import re

import pytest

from tess_re import Window

TEMPLATE = (
    "M {r},0 H {w - r} A {r} {r} 0 0 1 {w},{r} V {h - r} A {r} {r} 0 0 1 {w - r},{h} "
    "H {r} A {r} {r} 0 0 1 0,{h - r} V {r} A {r} {r} 0 0 1 {r},0 Z"
)


def rounded(w, h, r):
    return (
        f"M{r},0 H{w - r} A{r},{r} 0 0 1 {w},{r} V{h - r} A{r},{r} 0 0 1 {w - r},{h} "
        f"H{r} A{r},{r} 0 0 1 0,{h - r} V{r} A{r},{r} 0 0 1 {r},0 Z"
    )


def numbers(data):
    return [float(n) for n in re.findall(r"-?\d+(?:\.\d+)?", data)]


def scene(**props):
    window = Window(width=120, height=80)
    window.root.set(fill=(0, 0, 0, 255), padding=0)
    path = window.create("path", data=TEMPLATE, params={"w": 100, "h": 60, "r": 0},
                         fill=(255, 0, 0, 255), width=100, height=60, **props)
    window.root.add_child(path)
    return window, path


def plain_data(window, w, h, r):
    return window.create("path", data=rounded(w, h, r)).get("data")


def test_a_template_reads_as_the_data_it_fills_in():
    window, path = scene()
    assert path.get("params") == {"w": 100.0, "h": 60.0, "r": 0.0}
    assert numbers(path.get("data")) == pytest.approx(numbers(plain_data(window, 100, 60, 0)))


def test_animating_a_parameter_redraws_the_exact_path_each_frame_and_ends_equal_to_plain_data():
    window, path = scene()
    path.animate_param("r", 12, 100, easing="linear")
    path.animate_param("w", 60, 100, easing="linear")
    window.advance(50)
    mid = path.get("data")
    assert numbers(mid) == pytest.approx(numbers(plain_data(window, 80, 60, 6)), abs=0.05)
    assert path.get("params")["r"] == pytest.approx(6, abs=0.1)
    window.advance(500)
    assert path.get("params") == {"w": 60.0, "h": 60.0, "r": 12.0}
    assert path.get("data") == plain_data(window, 60, 60, 12)


def test_the_corner_is_an_arc_at_every_frame():
    window, path = scene()
    path.animate_param("r", 20, 100, easing="linear")
    window.advance(50)  # r = 10: a corner 14.1 from the arc's centre is outside, 7.1 inside
    rgba, width, _ = window.snapshot()
    ox, oy = int(path.get("layout_x")), int(path.get("layout_y"))
    pixel = lambda x, y: rgba[((oy + y) * width + ox + x) * 4]  # noqa: E731
    assert pixel(1, 1) == 0 and pixel(3, 3) > 200
    assert len(re.findall(r"C", path.get("data"))) == 4, "four curves, not a polygon"


def test_setting_plain_data_ends_the_template_and_params_stay():
    window, path = scene()
    path.set(data=rounded(50, 50, 5))
    path.animate_param("r", 12, 50)
    window.advance(200)
    assert path.get("data") == plain_data(window, 50, 50, 5), "the parameter no longer drives it"
    assert path.get("params")["r"] == 12.0


def test_set_params_stops_an_animation_and_redraws_at_once():
    window, path = scene()
    path.animate_param("r", 12, 100)
    window.advance(20)
    path.set(params={"r": 4})
    window.advance(500)
    assert path.get("params")["r"] == 4.0
    assert path.get("data") == plain_data(window, 100, 60, 4)


def test_a_template_and_its_params_can_be_set_in_one_call():
    window, path = scene()
    path.set(data="M0,0 H {a * 2} V {a} Z", params={"a": 10})
    assert path.get("data") == window.create("path", data="M0,0 H 20 V 10 Z").get("data")


@pytest.mark.parametrize(
    "props, match",
    [
        ({"data": "M0,0 H {nope}"}, "nope"),
        ({"data": "M0,0 H {w / 2}"}, "template"),
        ({"data": "M0,0 H {w"}, "template"),
        ({"data": "X {w}"}, "path data"),
        ({"params": {"w": "wide"}}, "params"),
        ({"params": {"w": float("nan")}}, "params"),
        ({"params": {"1w": 1}}, "params"),
        ({"params": [1, 2]}, "params"),
    ],
)
def test_a_bad_template_or_params_raise_and_change_nothing(props, match):
    window, path = scene()
    before = (path.get("data"), path.get("params"))
    with pytest.raises(ValueError, match=match):
        path.set(**props)
    assert (path.get("data"), path.get("params")) == before


def test_animating_an_unknown_parameter_or_the_data_of_a_template_is_an_error():
    window, path = scene()
    with pytest.raises(ValueError, match="no parameter `nope`"):
        path.animate_param("nope", 1, 100)
    with pytest.raises(ValueError, match="template"):
        path.animate("data", "M0,0 L1,1")
    with pytest.raises(ValueError, match="finite"):
        path.animate_param("r", float("inf"), 100)
    with pytest.raises(ValueError, match="path"):
        window.create("box", width=1, height=1).animate_param("r", 1, 100)


def test_create_checks_the_template_too():
    window = Window(width=50, height=50)
    with pytest.raises(ValueError, match="`r`"):
        window.create("path", data="M0,0 H {r}", params={})
