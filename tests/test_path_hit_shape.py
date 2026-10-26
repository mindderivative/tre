"""0.5.9 (#236): `hit_shape` on a `path` node, `"box"` (the default) or `"painted"`.

A painted ring takes clicks on its ring and lets them through in its hole; a painted notch
stays a notch; a stroke-only line is hit on the line. The region a layer window hands the
compositor is built from the same shape (see `tests/test_layer_window_live.py` and the engine
tests); these check the node events, headless.
"""

import pytest

from tre import Window

# A 100 x 100 frame: the outer square, and an inner square wound the other way, so it is a hole.
RING = "M0,0 H100 V100 H0 Z M25,25 V75 H75 V25 Z"
L_SHAPE = "M0,0 H100 V50 H50 V100 H0 Z"


def scene(data, hit_shape=None, **props):
    window = Window(width=240, height=240)
    path = window.create("path", data=data, x=50, y=50, width=100, height=100, position="absolute",
                         fill=(200, 40, 40, 255), **props)
    window.root.add_child(path)
    if hit_shape is not None:
        path.set(hit_shape=hit_shape)
    clicks = []
    path.on("click", lambda e: clicks.append("path"))
    window.root.on("click", lambda e: clicks.append("root"))
    return window, path, clicks


def click(window, clicks, x, y):
    clicks.clear()
    window.simulate("click", x=x, y=y)
    return list(clicks)


def test_the_default_is_the_box_so_nothing_changes_for_existing_paths():
    window, path, clicks = scene(RING)
    assert path.get("hit_shape") == "box"
    assert click(window, clicks, 100, 100)[0] == "path", "the hole is still the path's, as before"


def test_a_painted_ring_takes_clicks_on_the_ring_and_lets_the_hole_through():
    window, path, clicks = scene(RING, "painted")
    assert path.get("hit_shape") == "painted"
    assert click(window, clicks, 56, 100)[0] == "path"
    assert click(window, clicks, 100, 56)[0] == "path"
    assert click(window, clicks, 100, 100) == ["root"]
    assert click(window, clicks, 30, 30) == ["root"], "outside the node"


def test_a_painted_notch_stays_a_notch():
    window, path, clicks = scene(L_SHAPE, "painted")
    assert click(window, clicks, 60, 140)[0] == "path"
    assert click(window, clicks, 140, 60)[0] == "path"
    assert click(window, clicks, 130, 130) == ["root"]


def test_a_stroke_only_path_is_hit_on_its_line():
    window = Window(width=240, height=240)
    line = window.create("path", data="M10,10 L90,90", x=50, y=50, width=100, height=100, position="absolute",
                         stroke_color=(255, 255, 255, 255), stroke_width=6, hit_shape="painted")
    window.root.add_child(line)
    clicks = []
    line.on("click", lambda e: clicks.append("line"))
    window.root.on("click", lambda e: clicks.append("root"))
    assert click(window, clicks, 100, 100)[0] == "line"
    assert click(window, clicks, 125, 75) == ["root"], "off the line, inside the box"


def test_a_path_with_nothing_painted_takes_no_clicks():
    window, path, clicks = scene(RING, "painted")
    path.set(fill=(0, 0, 0, 0))
    assert click(window, clicks, 56, 100) == ["root"]


def test_it_can_be_changed_while_the_window_runs_and_back():
    window, path, clicks = scene(RING, "painted")
    assert click(window, clicks, 100, 100) == ["root"]
    path.set(hit_shape="box")
    assert click(window, clicks, 100, 100)[0] == "path"
    path.set(hit_shape="painted")
    assert click(window, clicks, 100, 100) == ["root"]


def test_it_follows_the_nodes_transform():
    window, path, clicks = scene(RING, "painted")
    path.set(translate_x=60)  # the ring now spans x 110 to 210; its hole is x 135 to 185
    assert click(window, clicks, 56, 100) == ["root"], "where the left side was"
    assert click(window, clicks, 116, 100)[0] == "path", "where it is now"
    assert click(window, clicks, 160, 100) == ["root"], "the hole moved with it"
    assert click(window, clicks, 195, 100)[0] == "path", "the right side"


def test_bad_values_and_other_kinds_are_refused():
    window, path, _ = scene(RING)
    with pytest.raises(ValueError, match="hit_shape"):
        path.set(hit_shape="filled")
    with pytest.raises(ValueError, match="hit_shape"):
        path.set(hit_shape=True)
    box = window.create("box", width=10, height=10)
    with pytest.raises(ValueError, match="path"):
        box.set(hit_shape="painted")
    with pytest.raises(ValueError, match="path"):
        box.get("hit_shape")
