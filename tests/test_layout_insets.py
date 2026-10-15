"""0.5.6.2 (#178): `left`, `top`, `right`, `bottom` and `inset`.

An absolute node used to be placed by `x` and `y` alone, so "8 pixels from each
side" could not be said. With both edges of an axis set and the size `auto`, the
node is stretched between them, from the parent's edge; `x` and `y` are `left` and
`top` under their old names.
"""

import pytest

from tre import Window


def scene(parent_width=300, parent_height=200, **child):
    """A parent box with one absolute child, laid out; returns (window, parent, child)."""
    window = Window(width=600, height=400)
    parent = window.create("box", width=parent_width, height=parent_height, padding=10)
    window.root.add_child(parent)
    child.setdefault("position", "absolute")
    node = window.create("box", **child)
    parent.add_child(node)
    window.advance(16)
    return window, parent, node


def box_in(parent, node):
    """The child's box relative to the parent's edge: (x, y, width, height)."""
    return (
        node.get("layout_x") - parent.get("layout_x"),
        node.get("layout_y") - parent.get("layout_y"),
        node.get("layout_width"),
        node.get("layout_height"),
    )


def test_left_and_right_stretch_an_auto_width_between_the_edges():
    _, parent, node = scene(left=8, right=8, top=0, height=20)
    assert box_in(parent, node) == (8, 0, 284, 20)


def test_top_and_bottom_stretch_an_auto_height_between_the_edges():
    _, parent, node = scene(top=5, bottom=15, left=0, width=30)
    assert box_in(parent, node) == (0, 5, 30, 180)


def test_all_four_insets_fill_the_parent_less_the_strip():
    _, parent, node = scene(inset=0)
    assert box_in(parent, node) == (0, 0, 300, 200)
    node.set(left=16, right=16)
    parent.get("layout_x")
    assert box_in(parent, node) == (16, 0, 268, 200)


def test_insets_are_from_the_parents_edge_not_its_padding():
    # The parent has padding=10; the child still starts at the edge.
    _, parent, node = scene(left=0, right=0, top=0, height=10)
    assert box_in(parent, node) == (0, 0, 300, 10)


def test_percentage_insets():
    _, parent, node = scene(left="10%", right="10%", top=0, height=20)
    assert box_in(parent, node) == (30, 0, 240, 20)


def test_right_alone_places_a_sized_node_from_the_right_edge():
    _, parent, node = scene(right=8, top=0, width=100, height=20)
    assert box_in(parent, node) == (192, 0, 100, 20)


def test_bottom_alone_places_a_sized_node_from_the_bottom_edge():
    _, parent, node = scene(bottom=8, left=0, width=100, height=20)
    assert box_in(parent, node) == (0, 172, 100, 20)


def test_margins_shrink_a_node_stretched_between_insets():
    _, parent, node = scene(left=8, right=8, top=0, height=20, margin_left=4, margin_right=4)
    assert box_in(parent, node) == (12, 0, 276, 20)


def test_a_size_that_is_not_auto_beats_the_right_inset():
    _, parent, node = scene(left=8, right=8, top=0, width="100%", height=20)
    assert box_in(parent, node) == (8, 0, 300, 20)


def test_the_node_follows_its_parent_when_it_is_resized():
    window, parent, node = scene(left=8, right=8, top=0, height=20)
    parent.set(width=400)
    window.advance(16)
    assert box_in(parent, node) == (8, 0, 384, 20)


def test_a_relative_node_is_offset_by_right():
    window = Window(width=600, height=400)
    node = window.create("box", width=50, height=20)
    window.root.add_child(node)
    window.advance(16)
    x = node.get("layout_x")
    node.set(right=8)
    window.advance(16)
    assert node.get("layout_x") == x - 8


def test_x_and_y_are_left_and_top():
    window = Window(width=100, height=100)
    node = window.create("box", position="absolute", x=5, y=6)
    assert (node.get("left"), node.get("top")) == (5, 6)
    node.set(left=7, top=8)
    assert (node.get("x"), node.get("y")) == (7, 8)


def test_get_reads_each_inset_back_as_set():
    window = Window(width=100, height=100)
    node = window.create("box", position="absolute", left=1, top="5%", right=3, bottom="auto")
    assert node.get("left") == 1
    assert node.get("top") == "5%"
    assert node.get("right") == 3
    assert node.get("bottom") == "auto"


def test_inset_sets_all_four_and_reads_back_one_value_or_four():
    window = Window(width=100, height=100)
    node = window.create("box", position="absolute", inset=4)
    assert node.get("inset") == 4
    assert [node.get(side) for side in ("left", "top", "right", "bottom")] == [4, 4, 4, 4]
    node.set(right=9)
    assert node.get("inset") == (4, 9, 4, 4)  # top, right, bottom, left
    node.set(inset=None)
    assert node.get("inset") == "auto"


@pytest.mark.parametrize("name", ["left", "top", "right", "bottom", "inset"])
@pytest.mark.parametrize("bad", ["wide", True, [1]])
def test_a_bad_inset_is_refused(name, bad):
    window = Window(width=100, height=100)
    with pytest.raises(ValueError, match=name):
        window.create("box", **{name: bad})
