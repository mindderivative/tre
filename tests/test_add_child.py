"""`Node.add_child`: attaching and re-parenting a node, and rejecting a cycle
(a node as its own child or its ancestor's child) or a node from another
window with `ValueError`.
"""

import pytest

from tre import Window
from helpers import add


def test_add_child_attaches_a_child():
    window = Window(width=200, height=200)
    parent = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    child = add(window, "box", fill=(255, 0, 0, 255), width=10, height=10)

    parent.add_child(child)  # must not raise


def test_add_child_moves_an_already_attached_node():
    window = Window(width=200, height=200)
    old_parent = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    new_parent = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    child = add(window, "box", fill=(255, 0, 0, 255), width=10, height=10)

    # `add` already attached `child` to the root -- re-parenting it under
    # `new_parent` must move it, not raise and not duplicate it.
    new_parent.add_child(child)  # must not raise


def test_add_child_rejects_a_node_as_its_own_child():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)

    with pytest.raises(ValueError, match="descendant"):
        node.add_child(node)


def test_add_child_rejects_an_ancestor_as_a_descendants_child():
    window = Window(width=200, height=200)
    grandparent = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    parent = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    child = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)

    grandparent.add_child(parent)
    parent.add_child(child)

    with pytest.raises(ValueError, match="descendant"):
        child.add_child(grandparent)


def test_add_child_rejects_a_node_from_a_different_window():
    window_a = Window(width=200, height=200)
    window_b = Window(width=200, height=200)
    parent = add(window_a, "box", fill=(0, 0, 0, 255), width=50, height=50)
    foreign_child = add(window_b, "box", fill=(0, 0, 0, 255), width=10, height=10)

    with pytest.raises(ValueError, match="different Window"):
        parent.add_child(foreign_child)
