"""0.5.7 (#179): a node placed against another node in the tree.

`anchor=`, `placement=` and `offset=` on an ordinary node put it at `placement` of
the anchor's box -- a badge on the corner of a distant button, a label beside a
field, a marker on a chart point -- laid out in the same pass as the anchor, so it
follows it with no lag. Unlike a layer it stays an ordinary child of its own
parent: clipped, ordered and hit as one, and events bubble through its parent.
"""

import pytest

import tre


def scene(**badge):
    """Two sibling boxes: the left holds the anchor, the right the badge.

    The anchor is 60 x 30, 30 px in and 40 down; the badge is 20 x 10 and is not
    the anchor's child. Returns (window, left, right, anchor, badge)."""
    window = tre.Window(500, 400, "anchored")
    window.root.set(padding=0, gap=0)
    left = window.create("box", width=200, height=300)
    right = window.create("box", width=200, height=300)
    window.root.add_child(left)
    window.root.add_child(right)
    anchor = window.create("box", position="absolute", x=30, y=40, width=60, height=30)
    left.add_child(anchor)
    node = window.create("box", width=20, height=10, anchor=anchor, **badge)
    right.add_child(node)
    window.advance(16)
    return window, left, right, anchor, node


def at(node):
    return (node.get("layout_x"), node.get("layout_y"))


@pytest.mark.parametrize(
    "placement, expected",
    [
        ("below", (30, 70)),
        ("above", (30, 30)),
        ("start", (10, 40)),
        ("end", (90, 40)),
        # A point: the badge's centre on it, so its top-left is (10, 5) less.
        ("top_start", (20, 35)),
        ("top_end", (80, 35)),
        ("bottom_start", (20, 65)),
        ("bottom_end", (80, 65)),
        ("center", (50, 50)),
    ],
)
def test_every_placement_against_an_anchor_in_another_subtree(placement, expected):
    _, _, _, _, badge = scene(placement=placement)
    assert at(badge) == expected


def test_the_default_placement_is_below():
    _, _, _, _, badge = scene()
    assert at(badge) == (30, 70) and badge.get("placement") == "below"


def test_offset_moves_the_attachment_point():
    _, _, _, _, badge = scene(placement="top_end", offset=(-8, 6))
    assert at(badge) == (72, 41)


def test_offset_is_in_the_anchors_coordinates_so_a_scaled_anchor_scales_it():
    window, _, right, anchor, plain = scene(placement="top_start")
    anchor.set(scale=2.0)
    shifted = window.create("box", width=20, height=10, anchor=anchor, placement="top_start", offset=(5, 5))
    right.add_child(shifted)
    window.advance(16)
    # The anchor draws twice as big, so 5 px in its coordinates is 10 px.
    assert (at(shifted)[0] - at(plain)[0], at(shifted)[1] - at(plain)[1]) == (10, 10)


def test_it_follows_the_anchor():
    window, _, _, anchor, badge = scene(placement="below")
    anchor.set(x=100, y=120)
    window.advance(16)
    assert at(badge) == (100, 150)
    anchor.set(height=50)
    window.advance(16)
    assert at(badge) == (100, 170)


def test_it_follows_an_anchor_that_scrolls():
    # The anchor is in a scroll view; scrolling moves it on screen, and the
    # badge, which is outside the scroll view, goes with it.
    window = tre.Window(500, 400, "scrolled")
    window.root.set(padding=0, gap=0)
    view = window.create("scroll_view", width=200, height=100)
    window.root.add_child(view)
    content = window.create("box", width=200, height=1000)
    view.add_child(content)
    anchor = window.create("box", position="absolute", x=10, y=300, width=60, height=30)
    content.add_child(anchor)
    badge = window.create("box", position="absolute", width=20, height=10, anchor=anchor, placement="below")
    window.root.add_child(badge)
    window.advance(16)
    before = at(badge)
    view.set(scroll_offset=50)
    window.advance(16)
    assert (at(badge)[0], at(badge)[1]) == (before[0], before[1] - 50)


def test_it_follows_a_parent_that_resizes():
    window, left, _, _, badge = scene(placement="end")
    left.set(width=300)
    window.advance(16)
    assert at(badge) == (90, 40)  # the anchor is in the left box, at 30 px: unmoved
    anchor = left.children()[0]
    anchor.set(x="50%")
    window.advance(16)
    assert at(badge)[0] == 150 + 60  # 50% of 300, then the anchor's width


def test_it_stays_a_child_of_its_own_parent_so_events_bubble_through_it():
    window, left, right, _, badge = scene(placement="below")
    assert badge.parent() == right
    heard = []
    left.on("click", lambda: heard.append("left"))
    right.on("click", lambda: heard.append("right"))
    window.simulate("click", node=badge)
    assert heard == ["right"]


def test_get_reads_the_anchoring_back_and_none_clears_it():
    window, _, _, anchor, badge = scene(placement="top_end", offset=(1, 2))
    assert badge.get("anchor") == anchor
    assert badge.get("placement") == "top_end"
    assert badge.get("offset") == (1, 2)
    badge.set(anchor=None)
    assert (badge.get("anchor"), badge.get("placement"), badge.get("offset")) == (None, None, None)


def test_clearing_the_anchor_puts_the_node_back_in_the_flow():
    window, _, _, anchor, badge = scene()
    badge.set(anchor=None)
    window.advance(16)
    assert badge.get("position") == "relative"
    assert at(badge) == (200, 0)  # first in its parent, the right box


def test_placement_and_offset_can_change_later():
    window, _, _, _, badge = scene(placement="below")
    badge.set(placement="center", offset=(0, 0))
    window.advance(16)
    assert at(badge) == (50, 50)


def test_placement_or_offset_without_an_anchor_is_refused():
    window = tre.Window(100, 100, "x")
    node = window.create("box", width=5, height=5)
    with pytest.raises(ValueError, match="need an `anchor`"):
        node.set(placement="below")
    with pytest.raises(ValueError, match="need an `anchor`"):
        window.create("box", offset=(1, 1))


def test_a_bad_value_is_refused():
    window, _, _, anchor, badge = scene()
    with pytest.raises(ValueError, match="placement"):
        badge.set(placement="middle")
    for bad in [(1,), "x", (float("nan"), 0), True]:
        with pytest.raises(ValueError, match="offset"):
            badge.set(offset=bad)
    with pytest.raises(ValueError, match="anchor"):
        badge.set(anchor="button")


def test_a_node_cannot_be_anchored_to_itself_or_to_a_node_inside_it():
    window, _, right, _, badge = scene()
    with pytest.raises(ValueError, match="itself or a node inside it"):
        badge.set(anchor=badge)
    with pytest.raises(ValueError, match="itself or a node inside it"):
        right.set(anchor=badge)


def test_a_node_of_another_window_is_refused():
    window, _, _, _, badge = scene()
    other = tre.Window(100, 100, "other")
    stranger = other.create("box", width=5, height=5)
    with pytest.raises(Exception, match="(?i)window|foreign"):
        badge.set(anchor=stranger)


def test_two_nodes_anchored_to_each_other_settle_without_error():
    window, _, _, anchor, badge = scene()
    # `anchor` against `badge` while `badge` is against `anchor`: no solution.
    anchor.set(anchor=badge, placement="end")
    window.advance(16)
    window.advance(16)
    for node in (anchor, badge):
        assert all(abs(v) < 10_000 for v in at(node))


def test_it_can_be_made_in_one_create_call():
    window = tre.Window(300, 300, "create")
    window.root.set(padding=0, gap=0)
    anchor = window.create("box", position="absolute", x=10, y=10, width=40, height=40)
    window.root.add_child(anchor)
    badge = window.create("box", width=10, height=10, anchor=anchor, placement="top_end", offset=(2, 3))
    window.root.add_child(badge)
    window.advance(16)
    assert at(badge) == (50 - 5 + 2, 10 - 5 + 3)
