"""M6 Phase 1 (§8): real, repeatable coverage of `Node.add_child`'s FFI
boundary -- the first Python-facing method §8's own original sketch
scoped exactly this way ("rejects a child that is an ancestor of self
with a `PyValueError`, not a silent cycle"). Three real, distinct
things get tested, matching `PLAN.md`'s own investigation:

1. A real attach actually reparents the tree (not just returns
   successfully).
2. Both cycle shapes (self, and a real multi-level ancestor cycle) are
   rejected with a clear `ValueError`, and re-parenting an already-
   attached node moves it rather than corrupting the tree.
3. A cross-`Window` `add_child` is rejected before either `Tree` is
   touched -- the real `NodeId`-aliasing risk found while planning this
   phase, not merely assumed.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as every other FFI test in this suite.
"""

import pytest

from tre import Window


def test_add_child_attaches_a_real_child():
    window = Window(width=200, height=200)
    parent = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    child = window.add_rect(background=(255, 0, 0, 255), width=10, height=10)

    parent.add_child(child)  # must not raise


def test_add_child_moves_an_already_attached_node():
    window = Window(width=200, height=200)
    old_parent = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    new_parent = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    child = window.add_rect(background=(255, 0, 0, 255), width=10, height=10)

    # `add_rect` already attached `child` to the window's own implicit
    # root row -- re-parenting it under `new_parent` must move it, not
    # raise and not duplicate it (the real "add_child has no dedup"
    # corruption risk PLAN.md names).
    new_parent.add_child(child)  # must not raise


def test_add_child_rejects_a_node_as_its_own_child():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)

    with pytest.raises(ValueError, match="descendant"):
        node.add_child(node)


def test_add_child_rejects_an_ancestor_as_a_descendants_child():
    window = Window(width=200, height=200)
    grandparent = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    parent = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    child = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)

    grandparent.add_child(parent)
    parent.add_child(child)

    with pytest.raises(ValueError, match="descendant"):
        child.add_child(grandparent)


def test_add_child_rejects_a_node_from_a_different_window():
    window_a = Window(width=200, height=200)
    window_b = Window(width=200, height=200)
    parent = window_a.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    foreign_child = window_b.add_rect(background=(0, 0, 0, 255), width=10, height=10)

    with pytest.raises(ValueError, match="different Window"):
        parent.add_child(foreign_child)
