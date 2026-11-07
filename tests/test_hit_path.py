"""#270 (TRE-REQ-013 step 2, part B): `hit_path` says where a node is hit. A shader paints where the
CPU cannot see, so a node with a shader (or any box) takes clicks inside the shape it is given, in
its own pixels, and lets them through elsewhere."""

import pytest

from tess_re import Shader, Window

FILL = "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(1.0, 0.0, 0.0, 1.0);\n}\n"
# A 100 x 100 box hit only inside an L: the top bar and the left bar.
L_SHAPE = "M0,0 H100 V30 H30 V100 H0 Z"


def scene(**props):
    window = Window(width=240, height=240)
    box = window.create("box", width=100, height=100, x=50, y=50, position="absolute", **props)
    window.root.add_child(box)
    clicks = []
    box.on("click", lambda e: clicks.append("box"))
    window.root.on("click", lambda e: clicks.append("root"))
    return window, box, clicks


def click(window, clicks, x, y):
    clicks.clear()
    window.simulate("click", x=x, y=y)
    return clicks[0]


def test_the_default_is_the_box():
    window, box, clicks = scene()
    assert box.get("hit_path") is None
    assert click(window, clicks, 140, 140) == "box"


def test_a_shader_node_is_hit_inside_its_shape_and_not_in_the_notch():
    window, box, clicks = scene()
    box.set(shader=Shader(FILL), hit_path=L_SHAPE)
    assert click(window, clicks, 60, 60) == "box"  # the corner of the L
    assert click(window, clicks, 140, 70) == "box"  # the end of the top bar
    assert click(window, clicks, 70, 140) == "box"  # the end of the left bar
    assert click(window, clicks, 140, 140) == "root"  # the notch lets it through


def test_it_can_be_cleared_and_read_back():
    window, box, clicks = scene(hit_path=L_SHAPE)
    assert box.get("hit_path") is not None
    assert click(window, clicks, 140, 140) == "root"
    box.set(hit_path=None)
    assert box.get("hit_path") is None
    assert click(window, clicks, 140, 140) == "box"


def test_the_shape_cannot_reach_outside_the_box():
    window, box, clicks = scene(hit_path="M-100,-100 H300 V300 H-100 Z")
    assert click(window, clicks, 10, 10) == "root"
    assert click(window, clicks, 100, 100) == "box"


@pytest.mark.parametrize("bad", ["not a path", 3, ["M0,0"]])
def test_a_bad_shape_is_a_value_error(bad):
    window, box, _ = scene()
    with pytest.raises(ValueError, match="hit_path"):
        box.set(hit_path=bad)
