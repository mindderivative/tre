"""Absolute positioning: a box or canvas created with `position="absolute"`,
`x`, `y` wins the hit test over the in-flow node it overlaps, and a simulated
click still finds a node its translation has moved. Hit testing is the probe,
since the click lands at the target node's center.
"""

from tre import Window
from helpers import add


def test_an_absolute_box_overlaps_the_default_flow_position():
    window = Window(width=200, height=200)

    hits = []
    default_positioned = add(window, "box", fill=(0, 0, 0, 255), width=40, height=40)
    default_positioned.on("click", lambda: hits.append("default"))

    # 16 is the root's padding -- the first in-flow child's position.
    explicitly_positioned = add(window, "box", fill=(255, 0, 0, 255), width=40, height=40, position="absolute", x=16.0, y=16.0)
    explicitly_positioned.on("click", lambda: hits.append("explicit"))

    window.simulate("click", node=default_positioned)

    assert hits == ["explicit"], (
        "the explicitly-positioned (topmost) node must win hit-testing at the "
        f"default node's own center point, got {hits!r}"
    )


def test_a_box_without_x_or_y_stays_in_flow():
    """Without `x`/`y` a node stays in the root's flex row, clickable at
    its laid-out position."""
    window = Window(width=200, height=200)
    hits = []
    node = add(window, "box", fill=(0, 0, 0, 255), width=40, height=40)
    node.on("click", lambda: hits.append(True))

    window.simulate("click", node=node)

    assert hits == [True]


def test_an_absolute_canvas_overlaps_the_default_flow_position():
    window = Window(width=200, height=200)

    hits = []
    default_positioned = add(window, "box", fill=(0, 0, 0, 255), width=40, height=40)
    default_positioned.on("click", lambda: hits.append("default"))

    explicitly_positioned = add(window, "canvas", width=40, height=40, draw=lambda ctx: None, position="absolute", x=16.0, y=16.0)
    explicitly_positioned.on("click", lambda: hits.append("explicit"))

    window.simulate("click", node=default_positioned)

    assert hits == ["explicit"], (
        "an explicitly-positioned canvas (topmost) must win hit-testing at the "
        f"default node's own center point, got {hits!r}"
    )


def test_click_still_finds_a_node_after_its_translation_moves_it():
    """A simulated click aims at the node's transformed position, not its
    untransformed layout box."""
    window = Window(width=200, height=200)
    hits = []
    node = add(window, "box", fill=(0, 0, 0, 255), width=40, height=40)
    node.on("click", lambda: hits.append(True))

    # duration_ms=0 -- an instant snap.
    window.advance(0)
    node.animate("translate_x", 60.0, duration_ms=0)
    node.animate("translate_y", 60.0, duration_ms=0)
    window.advance(1)

    window.simulate("click", node=node)

    assert hits == [True], (
        "click must still find the node at its transformed position, not its "
        f"stale untransformed one, got {hits!r}"
    )
