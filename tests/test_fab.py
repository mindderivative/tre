"""M30 Phase 1 Step 3 (§5, §7): real, repeatable coverage of `Window.
add_fab`/`Window.add_extended_fab` -- the FFI boundary for MD3's real
FAB/Extended FAB anatomy. Mirrors `test_button.py`/`test_icon_button.
py`'s own established structure (same "FFI wiring only" split --
`engine-render`'s own pixel tests are the definitive paint proof, not
this file).
"""

import pytest

from tre import Node, Window


def test_add_fab_returns_a_node():
    window = Window(width=300, height=200)
    node = window.add_fab(icon="add")
    assert isinstance(node, Node)


@pytest.mark.parametrize("size", ["small", "default", "large"])
def test_every_real_md3_fab_size_is_accepted(size):
    window = Window(width=300, height=200)
    node = window.add_fab(icon="add", size=size)
    assert isinstance(node, Node)


@pytest.mark.parametrize("variant", ["surface", "primary", "secondary", "tertiary"])
def test_every_real_md3_fab_variant_is_accepted(variant):
    window = Window(width=300, height=200)
    node = window.add_fab(icon="add", variant=variant)
    assert isinstance(node, Node)


def test_an_unknown_fab_size_raises_a_clear_value_error():
    window = Window(width=300, height=200)
    with pytest.raises(ValueError, match="unknown FAB size"):
        window.add_fab(icon="add", size="not-a-real-size")


def test_an_unknown_fab_variant_raises_a_clear_value_error():
    window = Window(width=300, height=200)
    with pytest.raises(ValueError, match="unknown FAB variant"):
        window.add_fab(icon="add", variant="not-a-real-variant")


def test_a_themed_fab_does_not_raise():
    window = Window(width=300, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    for variant in ["surface", "primary", "secondary", "tertiary"]:
        node = window.add_fab(icon="add", variant=variant)
        assert isinstance(node, Node)


def test_the_already_generic_click_and_ripple_mechanism_works_on_a_fab():
    window = Window(width=300, height=200)
    fab = window.add_fab(icon="add")
    fab.enable_interaction()  # must not raise

    calls = []
    fab.set_on_click(lambda: calls.append("clicked"))
    window.click(fab)
    assert calls == ["clicked"], (
        "a real click must reach a FAB's own registered handler, the same generic "
        "dispatch/ripple mechanism every other NodeKind already uses"
    )


def test_add_extended_fab_returns_a_node():
    window = Window(width=300, height=200)
    node = window.add_extended_fab(label="Compose", width=160, icon="add")
    assert isinstance(node, Node)


def test_add_extended_fab_without_an_icon_does_not_raise():
    """Real MD3's own label-only Extended FAB variant."""
    window = Window(width=300, height=200)
    node = window.add_extended_fab(label="Compose", width=140)
    assert isinstance(node, Node)


@pytest.mark.parametrize("variant", ["surface", "primary", "secondary", "tertiary"])
def test_every_real_md3_fab_variant_is_accepted_on_extended_fab(variant):
    window = Window(width=300, height=200)
    node = window.add_extended_fab(label="Compose", width=160, icon="add", variant=variant)
    assert isinstance(node, Node)


def test_an_unknown_extended_fab_variant_raises_a_clear_value_error():
    window = Window(width=300, height=200)
    with pytest.raises(ValueError, match="unknown FAB variant"):
        window.add_extended_fab(label="Compose", width=160, variant="not-a-real-variant")


def test_the_already_generic_click_and_ripple_mechanism_works_on_an_extended_fab():
    """The same real regression guard for `Tree::hit_test_at`'s
    `NodeKind::Text`/`NodeKind::Icon` arms `test_button.py`/`test_icon_
    button.py` already establish -- Extended FAB is a composite of
    both an `Icon` and a `Text` child at once.
    """
    window = Window(width=300, height=200)
    fab = window.add_extended_fab(label="Compose", width=160, icon="add")
    fab.enable_interaction()  # must not raise

    calls = []
    fab.set_on_click(lambda: calls.append("clicked"))
    window.click(fab)
    assert calls == ["clicked"], (
        "a real click must reach an Extended FAB's own registered handler, the same generic "
        "dispatch/ripple mechanism every other NodeKind already uses"
    )
