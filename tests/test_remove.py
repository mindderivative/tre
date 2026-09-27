"""`Node.remove`: a removed node's place can be taken by a new sibling that
then gets the clicks, and removing a node with children leaves the rest of
the tree working.
"""

from tre import Window
from helpers import add


def test_remove_detaches_a_node_so_a_new_sibling_can_take_its_place():
    window = Window(width=200, height=200)
    parent = add(window, "box", fill=(0, 0, 0, 0), width=100, height=50)
    old_child = add(window, "box", fill=(255, 0, 0, 255), width=100, height=50)
    parent.add_child(old_child)

    calls = []
    old_child.on("click", lambda: calls.append("old"))
    window.simulate("click", node=old_child)
    assert calls == ["old"], "sanity check: the child must be clickable before removal"

    old_child.remove()  # must not raise

    new_child = add(window, "box", fill=(0, 255, 0, 255), width=100, height=50)
    new_child.on("click", lambda: calls.append("new"))
    parent.add_child(new_child)

    window.simulate("click", node=new_child)
    assert calls == ["old", "new"], (
        "the new child, occupying the position the removed one did, "
        "must be the one a click reaches now"
    )


def test_remove_of_a_node_with_children_does_not_corrupt_the_tree():
    window = Window(width=200, height=200)
    parent = add(window, "box", fill=(0, 0, 0, 0), width=100, height=50)
    child = add(window, "box", fill=(255, 0, 0, 255), width=100, height=50)
    parent.add_child(child)

    parent.remove()  # recursively removes parent and child both -- must not raise

    # The rest of the tree is unaffected -- a fresh, unrelated node still
    # builds and gets clicks.
    unrelated = add(window, "box", fill=(0, 0, 255, 255), width=50, height=50)
    calls = []
    unrelated.on("click", lambda: calls.append("clicked"))
    window.simulate("click", node=unrelated)
    assert calls == ["clicked"]
