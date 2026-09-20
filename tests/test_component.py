"""M43 Phase 1 (§4, §5, §8, §16.2, §16.6): real, repeatable coverage
that `View.instantiate(path, into)`/`Component` genuinely embed a view
inside another view with its own, separate `ViewModel`, and that
multiple simultaneous instances of the same component stay fully
independent -- "the essence of MVVM and single page applications," the
user's own words scoping this milestone.

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
style: {flex_direction: Column, width: 300, height: 200, gap: 8, padding: 8}
children:
  - id: card_list
    kind: Container
    style: {flex_direction: Column, gap: 4, width: 280, height: 180}
"""

CARD_VIEW = """
id: root
kind: Container
style: {flex_direction: Row, width: 260, height: 40, gap: 8}
children:
  - id: label
    kind: Text
    text: {content: "Count: 0", font_family: Roboto, font_size: 16}
    style: {width: 180, height: 32, background: "#FFFFFF00"}
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
