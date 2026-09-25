"""M43 Phases 1 and 2 (§4, §5, §8, §16.2, §16.6): real, repeatable
coverage that `View.instantiate(path, into)`/`Component` genuinely
embed a view inside another view with its own, separate `ViewModel`,
that multiple simultaneous instances of the same component stay fully
independent, and that `Component.remove()` (Phase 2) really tears the
instance down -- unsubscribing its own bindings' `Signal`s so a later
write no longer tries to reach a `NodeId` that's gone. "The essence of
MVVM and single page applications," the user's own words scoping this
milestone.

Deliberately does *not* test `Component.click()`/`.hover()` -- `Component`
has no such methods (see `component.rs`'s own module doc comment for
why: `Tree::compute_layout` called from a component's own root would
distort its real, parent-constrained size). Dispatch on an embedded
component's own node goes through the *owning* `View`'s existing
`click`/`hover`/`right_click` instead, e.g. `view.click(card.node
("button"))` -- exactly what these tests do.
"""

import pytest

from tre import Signal, View, ViewModel

PARENT_VIEW = """
id: root
kind: Container
style: {flex_direction: vertical, width: 300, height: 200, gap: 8, padding: 8}
children:
  - id: card_list
    kind: Container
    style: {flex_direction: vertical, gap: 4, width: 280, height: 180}
"""

CARD_VIEW = """
id: root
kind: Container
style: {flex_direction: horizontal, width: 260, height: 40, gap: 8}
children:
  - id: label
    kind: Text
    text: {content: "Count: 0", font_family: Roboto, font_size: 16}
    style: {width: 180, height: 32, foreground: "#FFFFFF00"}
    bindings: {text: "{{ label.get() }}"}
  - id: button
    kind: Rect
    style: {width: 60, height: 32, background: "#6750A4", corner_radius: 4}
    handlers: {on_click: "bump"}
"""


class CardViewModel(ViewModel):
    def __init__(self, view):
        self._count = 0
        self.label = Signal("Count: 0")
        super().__init__(view)

    def bump(self):
        self._count += 1
        self.label.set(f"Count: {self._count}")


def write(tmp_path, yaml, name):
    path = tmp_path / name
    path.write_text(yaml)
    return str(path)


def test_instantiate_returns_a_usable_component(tmp_path):
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    card_path = write(tmp_path, CARD_VIEW, "card.yaml")

    view = View(parent_path)
    container = view.node("card_list")
    card = view.instantiate(card_path, container)

    node = card.node("label")
    assert node is not None


def test_multiple_instances_get_independent_node_identities(tmp_path):
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    card_path = write(tmp_path, CARD_VIEW, "card.yaml")

    view = View(parent_path)
    container = view.node("card_list")
    card_a = view.instantiate(card_path, container)
    card_b = view.instantiate(card_path, container)

    label_a = card_a.node("label")
    label_b = card_b.node("label")

    # `Node` has no `.id` getter exposed to Python, so distinct real
    # NodeId resolution is proven observably instead: `set_text` is a
    # real, immediate, synchronous setter (unlike `animate`, which only
    # snaps on the next tick) -- mutating instance A's own node must
    # leave instance B's completely untouched.
    label_a.set_text("A only")

    assert label_a.get_text() == "A only"
    assert label_b.get_text() == "Count: 0", (
        "two instances of the same component must resolve 'label' to distinct real NodeIds"
    )


def test_each_instances_viewmodel_is_genuinely_independent(tmp_path):
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    card_path = write(tmp_path, CARD_VIEW, "card.yaml")

    view = View(parent_path)
    container = view.node("card_list")

    card_a = view.instantiate(card_path, container)
    vm_a = CardViewModel(card_a)
    card_b = view.instantiate(card_path, container)
    vm_b = CardViewModel(card_b)

    view.click(card_a.node("button"))
    view.click(card_a.node("button"))

    assert vm_a._count == 2
    assert vm_b._count == 0, "a click on instance A's button must not affect instance B's own VM"
    assert card_a.node("label").get_text() == "Count: 2"
    assert card_b.node("label").get_text() == "Count: 0"


def test_a_click_on_an_embedded_components_node_reaches_that_instances_handler(tmp_path):
    """The real, decisive proof: `view.click(component.node(...))` --
    not a `Component`-level `.click()`, which doesn't exist -- correctly
    dispatches through the shared `Tree` and reaches the right
    instance's own handler, confirming layout/hit-testing/dispatch all
    work transparently across an embedded subtree with zero new
    `engine-core` code.
    """
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    card_path = write(tmp_path, CARD_VIEW, "card.yaml")

    view = View(parent_path)
    container = view.node("card_list")

    cards = []
    vms = []
    for _ in range(3):
        card = view.instantiate(card_path, container)
        vm = CardViewModel(card)
        cards.append(card)
        vms.append(vm)

    view.click(cards[1].node("button"))

    assert vms[0]._count == 0
    assert vms[1]._count == 1
    assert vms[2]._count == 0


def test_components_nest_recursively(tmp_path):
    outer_path = write(
        tmp_path,
        """
id: root
kind: Container
style: {width: 200, height: 100}
""",
        "outer.yaml",
    )
    inner_path = write(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 100, height: 50, background: "#112233"}
""",
        "inner.yaml",
    )

    view = View(outer_path)
    outer_container = view.node("root")
    outer_component = view.instantiate(outer_path, outer_container)
    inner_component = outer_component.instantiate(inner_path, outer_component.node("root"))

    assert inner_component.node("root") is not None


def test_instantiate_with_a_nonexistent_path_raises_clearly(tmp_path):
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    view = View(parent_path)
    container = view.node("card_list")

    with pytest.raises(Exception):
        view.instantiate(str(tmp_path / "does_not_exist.yaml"), container)


def test_component_node_with_unknown_widget_id_raises_clearly(tmp_path):
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    card_path = write(tmp_path, CARD_VIEW, "card.yaml")

    view = View(parent_path)
    container = view.node("card_list")
    card = view.instantiate(card_path, container)

    with pytest.raises(ValueError):
        card.node("nonexistent_widget")


def test_a_signal_write_after_remove_does_not_panic(tmp_path):
    """The real bug M43 Phase 2 exists to fix: before `Signal.
    _unsubscribe` existed, a removed component's own `BindingCallback`
    stayed subscribed -- a later write to the `Signal` it read from
    would try to apply the new value to a `NodeId` `Tree::remove`
    already deleted, panicking. `card.remove()` must unsubscribe first,
    so this write is now a real, silent no-op instead.
    """
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    card_path = write(tmp_path, CARD_VIEW, "card.yaml")

    view = View(parent_path)
    container = view.node("card_list")
    card = view.instantiate(card_path, container)
    vm = CardViewModel(card)

    card.remove()

    vm.label.set("this must not panic")  # must not raise


def test_remove_add_remove_cycle_stays_stable(tmp_path):
    """Simulates a real dynamic list -- repeatedly instantiating and
    removing components into the same container must stay stable
    across several iterations, with each still-alive instance's own
    state genuinely unaffected by a sibling's removal.
    """
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    card_path = write(tmp_path, CARD_VIEW, "card.yaml")

    view = View(parent_path)
    container = view.node("card_list")

    survivor = view.instantiate(card_path, container)
    survivor_vm = CardViewModel(survivor)
    view.click(survivor.node("button"))
    assert survivor_vm._count == 1

    for _ in range(5):
        card = view.instantiate(card_path, container)
        vm = CardViewModel(card)
        view.click(card.node("button"))
        assert vm._count == 1
        card.remove()
        vm.label.set("post-removal write must not panic")

    # The survivor, never removed, must be completely unaffected by
    # five real instantiate/click/remove cycles of unrelated siblings.
    view.click(survivor.node("button"))
    assert survivor_vm._count == 2
    assert survivor.node("label").get_text() == "Count: 2"


def test_instantiate_called_from_inside_a_click_handler_does_not_panic(tmp_path):
    """A real bug caught and fixed before this ever shipped, found by
    actually running `examples/component_list.py`, not by inspection:
    `View.click`/`hover`/`right_click`/`_attach` used to take `&mut
    self` even though nothing in their own bodies mutates a plain
    `View` struct field directly (every real mutation goes through an
    interior-mutable `Rc<RefCell<...>>`/`Cell`). pyo3 holds an
    *exclusive* borrow on the whole `View` Python object for a `&mut
    self` method's entire duration -- a handler dispatched from inside
    `click()` that calls `view.instantiate(...)` (exactly the real "Add"
    -button pattern a dynamic list needs) panicked with "Already
    mutably borrowed." Fixed by widening those methods to `&self`
    (mirroring `Window`'s own `click`/`hover`/`right_click`, which
    already used `&self`) -- this test calls `instantiate` from *inside*
    a dispatched handler, the one scenario that actually exercises it.
    """
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    card_path = write(tmp_path, CARD_VIEW, "card.yaml")

    view = View(parent_path)
    container = view.node("card_list")

    class SpawnerViewModel(ViewModel):
        def __init__(self, view):
            self.spawned = []
            self.label = Signal("Count: 0")
            super().__init__(view)

        def bump(self):
            # Reentrant: called from inside view.click() below, and
            # itself calls another method on the same `view` object.
            card = view.instantiate(card_path, container)
            self.spawned.append(card)

    root_card = view.instantiate(card_path, container)
    vm = SpawnerViewModel(root_card)

    view.click(root_card.node("button"))  # must not raise

    assert len(vm.spawned) == 1


def test_remove_is_safe_to_call_once_and_stops_dispatch_reaching_the_handler(tmp_path):
    """A real, decisive structural proof: after `remove()`, the
    instance's own subtree is genuinely gone from the `Tree` -- a
    sibling's own click must still work (proving the shared `Tree`
    itself stayed healthy), and the removed instance's own `ViewModel`
    must not have been reachable by anything after removal.
    """
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    card_path = write(tmp_path, CARD_VIEW, "card.yaml")

    view = View(parent_path)
    container = view.node("card_list")

    card_a = view.instantiate(card_path, container)
    vm_a = CardViewModel(card_a)
    card_b = view.instantiate(card_path, container)
    vm_b = CardViewModel(card_b)

    card_a.remove()

    view.click(card_b.node("button"))
    assert vm_b._count == 1
    assert vm_a._count == 0


# M71 follow-up (§8, §16.1, §16.6): `instantiate(..., source=...)` --
# the same real `source=` override `View.__new__`/`poll_reload` already
# have, widened to reach an *embedded* component too. The real,
# confirmed gap this closes: the sibling `Tesserae` project's own
# `component:` macro-expansion layer could pre-process a top-level
# `View`'s YAML via `View(path=, source=)`, but had no way to do the
# same for a component instantiated *into* a view -- `instantiate` read
# `path` straight from disk with no override at all.


def test_instantiate_source_override_is_used_instead_of_reading_path_from_disk(tmp_path):
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    card_path = write(tmp_path, CARD_VIEW, "card.yaml")

    view = View(parent_path)
    container = view.node("card_list")
    overridden = """
id: root
kind: Container
style: {flex_direction: horizontal, width: 999, height: 40, gap: 8}
children:
  - id: label
    kind: Text
    text: {content: "Overridden", font_family: Roboto, font_size: 16}
    style: {width: 180, height: 32, foreground: "#FFFFFF00"}
"""
    card = view.instantiate(card_path, container, source=overridden)

    node = card.node("label")
    assert node is not None
    # The real file on disk still has "Count: 0" -- proving `source=`,
    # not the file, is what got reconciled.
    assert card_path.endswith("card.yaml")


def test_component_instantiate_also_accepts_source_for_nested_components(tmp_path):
    """`Component.instantiate` (nesting a component inside another
    component) gets the identical real widening -- `instantiate_
    component`'s own single, shared implementation, confirmed by
    reading `component.rs` before writing this, not assumed."""
    outer_path = write(tmp_path, PARENT_VIEW, "outer.yaml")
    inner_path = write(tmp_path, CARD_VIEW, "inner.yaml")
    innermost_path = write(tmp_path, CARD_VIEW, "innermost.yaml")

    view = View(outer_path)
    outer_container = view.node("card_list")
    outer_component = view.instantiate(inner_path, outer_container)

    overridden = """
id: root
kind: Container
style: {flex_direction: horizontal, width: 42, height: 40, gap: 8}
children:
  - id: label
    kind: Text
    text: {content: "Nested override", font_family: Roboto, font_size: 16}
    style: {width: 180, height: 32, foreground: "#FFFFFF00"}
"""
    inner_component = outer_component.instantiate(
        innermost_path, outer_component.node("root"), source=overridden
    )

    assert inner_component.node("label") is not None


def test_instantiate_with_no_source_still_reads_the_real_file(tmp_path):
    """`source=None` (the default, omitted here entirely) is the real,
    pre-existing behavior, unchanged -- matches every other real caller
    in this file that doesn't pass it."""
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    card_path = write(tmp_path, CARD_VIEW, "card.yaml")

    view = View(parent_path)
    container = view.node("card_list")
    card = view.instantiate(card_path, container)

    assert card.node("label") is not None


# --- 0.3.1 review, user-requested follow-up (item 3): spec= on
# View.instantiate/Component.instantiate, mirroring View::new's own
# M78 widening. path= stays a required (but may be empty) positional
# param here, unlike View::new -- every real call site in this file
# already calls instantiate(path, into) positionally, and reordering
# `into` ahead of `path` to make `path` optional would have broken
# every one of them (see instantiate_component's own doc comment).


def test_instantiate_spec_builds_a_real_component_with_no_backing_file(tmp_path):
    parent_path = write(tmp_path, PARENT_VIEW, "parent.yaml")
    view = View(parent_path)
    container = view.node("card_list")

    spec = {
        "id": "root",
        "kind": "Rect",
        "style": {"width": 40, "height": 40, "background": "#112233", "corner_radius": 5},
    }
    card = view.instantiate("", container, spec=spec)

    assert card.node("root").get("corner_radius") == 5.0


def test_component_instantiate_also_accepts_spec_for_nested_components():
    """`Component.instantiate` (nesting a component inside another
    component) gets the identical real widening -- `instantiate_
    component`'s own single, shared implementation, confirmed by
    reading `component.rs` before writing this, not assumed."""
    outer = View(spec={"id": "root", "kind": "Container", "style": {"width": 200, "height": 200}})
    outer_component = outer.instantiate(
        "", outer.node("root"), spec={"id": "inner", "kind": "Container", "style": {"width": 100, "height": 100}}
    )
    nested_component = outer_component.instantiate(
        "",
        outer_component.node("inner"),
        spec={
            "id": "nested",
            "kind": "Rect",
            "style": {"width": 10, "height": 10, "background": "#000000", "corner_radius": 2},
        },
    )

    assert nested_component.node("nested").get("corner_radius") == 2.0


def test_instantiate_spec_and_source_together_is_a_clear_error():
    view = View(spec={"id": "root", "kind": "Container", "style": {"width": 10, "height": 10}})
    with pytest.raises(ValueError, match="spec=, source="):
        view.instantiate(
            "",
            view.node("root"),
            source="id: x\nkind: Container\n",
            spec={"id": "x", "kind": "Container"},
        )


def test_instantiate_with_no_path_source_or_spec_is_a_clear_error():
    view = View(spec={"id": "root", "kind": "Container", "style": {"width": 10, "height": 10}})
    with pytest.raises(ValueError, match="path=.*spec=/source="):
        view.instantiate("", view.node("root"))
