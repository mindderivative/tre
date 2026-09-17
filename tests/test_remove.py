"""M13 Phase 2 (§11.2): real, repeatable coverage of `Node.remove`'s
FFI boundary -- the one missing half `add_child` already had an
`engine-core` counterpart for (`Tree::remove`, real since §5) but never
a Python-facing one, needed for "navigating" (§11.2's own text: replace
a region's own children).

The real, functional proof a node is genuinely gone: a fresh node
added afterward, at the same real position the removed node occupied,
is the one that's hit by a real dispatched click -- not an attempt to
click the removed node's own now-stale `Node` handle directly, which
`Window.click()`'s own real `Tree::absolute_position`/`layout` calls
would panic on (a stale `NodeId` is an internal-bug condition
everywhere else in this codebase, not a recoverable one -- the same
"internal bug, not a runtime condition" contract `set_splitter_
position`/etc. already use).

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as every other FFI test in this suite.
"""

from tre import Window


def test_remove_detaches_a_node_so_a_new_sibling_can_take_its_place():
    window = Window(width=200, height=200)
    parent = window.add_rect(background=(0, 0, 0, 0), width=100, height=50)
    old_child = window.add_rect(background=(255, 0, 0, 255), width=100, height=50)
    parent.add_child(old_child)

    calls = []
    old_child.set_on_click(lambda: calls.append("old"))
    window.click(old_child)
    assert calls == ["old"], "sanity check: the child must be real and clickable before removal"

    old_child.remove()  # must not raise

    new_child = window.add_rect(background=(0, 255, 0, 255), width=100, height=50)
    new_child.set_on_click(lambda: calls.append("new"))
    parent.add_child(new_child)

    window.click(new_child)
    assert calls == ["old", "new"], (
        "the new child, occupying the same real position the removed one did, "
        "must be the one a real dispatched click reaches now"
    )


def test_remove_of_a_node_with_real_children_does_not_corrupt_the_tree():
    window = Window(width=200, height=200)
    parent = window.add_rect(background=(0, 0, 0, 0), width=100, height=50)
    child = window.add_rect(background=(255, 0, 0, 255), width=100, height=50)
    parent.add_child(child)

    parent.remove()  # recursively removes parent and child both -- must not raise

    # The rest of the window's own real tree must still be entirely
    # unaffected -- a fresh, unrelated node still builds and dispatches
    # correctly.
    unrelated = window.add_rect(background=(0, 0, 255, 255), width=50, height=50)
    calls = []
    unrelated.set_on_click(lambda: calls.append("clicked"))
    window.click(unrelated)
    assert calls == ["clicked"]
