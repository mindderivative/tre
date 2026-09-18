"""M30 Phase 5 Step 1 (§5, §7): real, repeatable coverage of
`Window.add_navigation_rail` -- the real desktop counterpart to
Navigation Bar. Built as a plain composition, the same real dividing
line `Segmented Button`/`Filter Chip` already established: a rail's
own "active item" is app-owned group-select state, not a new
engine-owned toggle, so this returns one real, independently
clickable `Node` per item (`Vec<Node>`, `Segmented Button`'s own real
return shape) rather than a new stateful `NodeKind`.
"""

import pytest

from tre import Node, Window


def test_add_navigation_rail_returns_one_node_per_item():
    window = Window(width=400, height=600)
    items = window.add_navigation_rail(
        labels=["Home", "Search", "Profile"],
        icons=["add", "add", "add"],
    )
    assert len(items) == 3
    assert all(isinstance(item, Node) for item in items)


def test_add_navigation_rail_with_a_selected_index_does_not_raise():
    window = Window(width=400, height=600)
    items = window.add_navigation_rail(
        labels=["Home", "Search", "Profile"],
        icons=["add", "add", "add"],
        selected=1,
    )
    assert len(items) == 3


def test_a_themed_navigation_rail_does_not_raise():
    window = Window(width=400, height=600)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    items = window.add_navigation_rail(labels=["Home", "Search"], icons=["add", "add"])
    assert len(items) == 2


def test_mismatched_labels_and_icons_raises_a_clear_value_error():
    window = Window(width=400, height=600)
    with pytest.raises(ValueError, match="one icon per label"):
        window.add_navigation_rail(labels=["Home", "Search"], icons=["add"])


def test_empty_labels_raises_a_clear_value_error():
    window = Window(width=400, height=600)
    with pytest.raises(ValueError, match="at least 1 item"):
        window.add_navigation_rail(labels=[], icons=[])


def test_out_of_range_selected_raises_a_clear_value_error():
    window = Window(width=400, height=600)
    with pytest.raises(ValueError, match="out of range"):
        window.add_navigation_rail(labels=["Home"], icons=["add"], selected=1)


def test_an_unknown_icon_name_raises_a_clear_value_error():
    window = Window(width=400, height=600)
    with pytest.raises(ValueError, match="unknown icon"):
        window.add_navigation_rail(labels=["Home"], icons=["not-a-real-icon-name"])


def test_each_item_is_a_real_independently_clickable_node():
    """The real point of this component's own `Vec<Node>` shape: each
    item must be clickable on its own, independent of its siblings.
    """
    window = Window(width=400, height=600)
    items = window.add_navigation_rail(
        labels=["Home", "Search", "Profile"],
        icons=["add", "add", "add"],
    )

    clicked = []
    for i, item in enumerate(items):
        item.enable_interaction()
        item.set_on_click(lambda i=i: clicked.append(i))

    window.click(items[1])
    assert clicked == [1], "clicking one item must reach only its own registered handler"
