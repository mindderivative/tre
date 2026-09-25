"""M96 Phase 1: tree structure -- `insert_child`, `children`, `parent`,
`remove` (detach, R5), and `destroy` -- and the move semantics a keyed
reconciler relies on.
"""

from __future__ import annotations

import pytest

import tre

BLACK = (0, 0, 0, 255)


def window() -> tre.Window:
    return tre.Window(300, 200, "structure")


def parent_with(w: tre.Window, n: int) -> tuple[tre.Node, list[tre.Node]]:
    parent = w.create("box")
    kids = [w.create("box") for _ in range(n)]
    for kid in kids:
        parent.add_child(kid)
    return parent, kids


def test_children_and_parent() -> None:
    w = window()
    parent, kids = parent_with(w, 3)
    assert parent.children() == kids
    assert kids[1].parent() == parent
    assert parent.parent() is None
    w.root.add_child(parent)
    assert parent.parent() == w.root


def test_insert_child_index_is_the_final_position() -> None:
    w = window()
    parent, (a, b, c) = parent_with(w, 3)
    parent.insert_child(2, a)
    assert parent.children() == [b, c, a]
    parent.insert_child(0, a)
    assert parent.children() == [a, b, c]
    d = w.create("box")
    parent.insert_child(1, d)
    assert parent.children() == [a, d, b, c]


def test_insert_child_moves_between_parents() -> None:
    w = window()
    first, (a,) = parent_with(w, 1)
    second, (b,) = parent_with(w, 1)
    second.insert_child(0, a)
    assert first.children() == []
    assert second.children() == [a, b]


def test_insert_child_rejects_an_index_past_the_end() -> None:
    w = window()
    parent, (a, b) = parent_with(w, 2)
    with pytest.raises(IndexError, match="must be 0 to 1"):
        parent.insert_child(2, a)
    with pytest.raises(IndexError, match="must be 0 to 2"):
        parent.insert_child(3, w.create("box"))
    assert parent.children() == [a, b]


def test_insert_child_rejects_a_cycle() -> None:
    w = window()
    parent, (a,) = parent_with(w, 1)
    with pytest.raises(ValueError, match="own descendant"):
        a.insert_child(0, parent)


def test_a_move_keeps_listeners_and_focus() -> None:
    w = window()
    list_box = w.add_rect(BLACK, 250, 150)
    fields = [w.add_text_field((255, 255, 255, 255), 100, 30) for _ in range(3)]
    for field in fields:
        list_box.add_child(field)
    typed: list[str | None] = []
    fields[0].on("input", lambda e: typed.append(e.text))
    w.simulate("focus", node=fields[0])

    list_box.insert_child(2, fields[0])
    w.simulate("input", text="x")
    assert typed == ["x"], "still focused, still listening, after the move"
    assert fields[0].get_text() == "x"


def test_remove_detaches_and_keeps_the_node_alive() -> None:
    w = window()
    parent, (a, b) = parent_with(w, 2)
    a.remove()
    assert parent.children() == [b]
    assert a.parent() is None
    parent.add_child(a)
    assert parent.children() == [b, a]


def test_destroy_frees_the_subtree() -> None:
    w = window()
    parent, (a,) = parent_with(w, 1)
    w.root.add_child(parent)
    parent.destroy()
    assert w.root.children() == []
    with pytest.raises(ValueError, match="destroyed"):
        a.children()
    with pytest.raises(ValueError, match="destroyed"):
        parent.destroy()


def test_proof_keyed_reorder_preserves_identity_listeners_and_animation() -> None:
    """A keyed reconciler reordering rows: the same nodes, in the new order,
    still listening, and an animation started before the move finishes
    after it without restarting."""
    w = window()
    w.advance(0)
    list_box = w.create("box")
    w.root.add_child(list_box)
    rows = {key: w.create("box", width=100, height=20, opacity=0.0) for key in "abcde"}
    for row in rows.values():
        list_box.add_child(row)
    clicks: list[str] = []
    for key, row in rows.items():
        row.on("click", lambda key=key: clicks.append(key))
    rows["c"].animate("opacity", 1.0, 100)
    w.advance(50)

    for index, key in enumerate("edcba"):  # the reconciler's keyed pass
        list_box.insert_child(index, rows[key])

    assert list_box.children() == [rows[key] for key in "edcba"]
    w.simulate("click", node=rows["a"])
    w.simulate("click", node=rows["e"])
    assert clicks == ["a", "e"]
    assert rows["c"].get("opacity") == pytest.approx(0.5)
    w.advance(50)
    assert rows["c"].get("opacity") == 1.0


def test_a_screen_swap_unfocuses_and_keeps_the_screen_as_it_was_left() -> None:
    """App.show(): `old.remove(); root.add_child(new)`, and back again."""
    w = window()
    w.advance(0)
    shell = w.create("box")
    w.root.add_child(shell)
    first, second = w.create("box"), w.create("box")
    shell.add_child(first)
    field = w.create("text_input", text="draft", width=100, height=30)
    view = w.create("scroll_view", scroll_offset=40)
    fading = w.create("box", opacity=0.0)
    for node in (field, view, fading):
        first.add_child(node)
    unfocuses: list[object] = []
    shell.on("unfocus", lambda e: unfocuses.append(e.target))
    w.simulate("focus", node=field)
    field.set(selection=(1, 3))
    fading.animate("opacity", 1.0, 100)

    first.remove()
    shell.add_child(second)
    assert unfocuses == [field], "unfocus bubbles through the tree as it was"
    assert field.get("focused") is False
    w.simulate("input", text="x")
    assert field.get("text") == "draft", "keys don't reach a detached screen"
    w.advance(100)

    second.remove()
    shell.add_child(first)
    assert field.get("text") == "draft"
    assert field.get("selection") == (1, 3)
    assert view.get("scroll_offset") == 40.0
    assert fading.get("opacity") == 1.0, "animations kept advancing while detached"


def test_destroy_unfocuses_before_freeing() -> None:
    w = window()
    shell = w.create("box")
    w.root.add_child(shell)
    field = w.create("text_input")
    shell.add_child(field)
    unfocuses: list[object] = []
    shell.on("unfocus", lambda e: unfocuses.append(e.target))
    w.simulate("focus", node=field)
    field.destroy()
    assert len(unfocuses) == 1
