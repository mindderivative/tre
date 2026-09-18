"""M30 Phase 2 Step 3 (§5, §7): real, repeatable coverage of `Window.
add_chip` -- the FFI boundary for MD3's four real chip variants.
Mirrors `test_button.py`/`test_fab.py`'s own established structure
(same "FFI wiring only" split -- `engine-render`'s own existing pixel
tests for `Rect`/`Text`/`Icon` are the definitive paint proof for
this pure composition, not this file).
"""

import pytest

from tre import Node, Window


def test_add_chip_returns_a_node():
    window = Window(width=300, height=200)
    node = window.add_chip(label="Assist", width=120)
    assert isinstance(node, Node)


@pytest.mark.parametrize("variant", ["assist", "filter", "input", "suggestion"])
def test_every_real_md3_chip_variant_is_accepted(variant):
    window = Window(width=300, height=200)
    node = window.add_chip(label="Chip", width=120, variant=variant)
    assert isinstance(node, Node)


def test_an_unknown_variant_raises_a_clear_value_error():
    window = Window(width=300, height=200)
    with pytest.raises(ValueError, match="unknown chip variant"):
        window.add_chip(label="Chip", width=120, variant="not-a-real-variant")


def test_a_chip_with_a_leading_icon_does_not_raise():
    window = Window(width=300, height=200)
    node = window.add_chip(label="Assist", width=120, variant="assist", icon="add")
    assert isinstance(node, Node)


def test_a_selected_filter_chip_shows_a_real_checkmark_not_the_custom_icon():
    """Real MD3 behavior: a selected Filter Chip's checkmark replaces
    any custom leading icon -- this must not raise even when a custom
    `icon` is also passed (it's simply not the one that renders).
    """
    window = Window(width=300, height=200)
    node = window.add_chip(label="Active", width=120, variant="filter", icon="add", selected=True)
    assert isinstance(node, Node)


def test_a_removable_input_chip_does_not_raise():
    window = Window(width=300, height=200)
    node = window.add_chip(label="Contact", width=140, variant="input", icon="add", removable=True)
    assert isinstance(node, Node)


def test_an_unknown_icon_name_raises_a_clear_value_error():
    window = Window(width=300, height=200)
    with pytest.raises(ValueError, match="unknown icon"):
        window.add_chip(label="Chip", width=120, icon="not-a-real-icon-name")


def test_a_themed_chip_does_not_raise():
    window = Window(width=300, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    for variant in ["assist", "filter", "input", "suggestion"]:
        node = window.add_chip(label="Chip", width=120, variant=variant)
        assert isinstance(node, Node)


def test_the_already_generic_click_and_ripple_mechanism_works_on_a_chip():
    """The same real regression guard `test_button.py` already
    establishes for `Tree::hit_test_at`'s `NodeKind::Text`/`NodeKind::
    Icon` arms -- a chip is a composite `Rect` container with `Text`
    (and, with an icon, `Icon`) children sized to fill its own
    clickable area.
    """
    window = Window(width=300, height=200)
    chip = window.add_chip(label="Assist", width=120, variant="assist", icon="add")
    chip.enable_interaction()  # must not raise

    calls = []
    chip.set_on_click(lambda: calls.append("clicked"))
    window.click(chip)
    assert calls == ["clicked"], (
        "a real click must reach a Chip's own registered handler, the same generic "
        "dispatch/ripple mechanism every other NodeKind already uses"
    )
