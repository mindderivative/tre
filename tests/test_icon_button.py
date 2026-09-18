"""M30 Phase 1 Step 2 (§5, §7): real, repeatable coverage of `Window.
add_icon_button` -- the FFI boundary for MD3's four real Icon Button
variants. Mirrors `test_button.py`'s own established structure exactly
(same "FFI wiring only" split -- `engine-render`'s own pixel tests are
the definitive paint proof, not this file).
"""

import pytest

from tre import Node, Window


def test_add_icon_button_returns_a_node():
    window = Window(width=300, height=200)
    node = window.add_icon_button(icon="add")
    assert isinstance(node, Node)


def test_add_icon_button_defaults_to_standard_variant_and_40dp_size():
    window = Window(width=300, height=200)
    node = window.add_icon_button(icon="add")  # must not raise
    assert isinstance(node, Node)


@pytest.mark.parametrize("variant", ["filled", "filled_tonal", "outlined", "standard"])
def test_every_real_md3_icon_button_variant_is_accepted(variant):
    window = Window(width=300, height=200)
    node = window.add_icon_button(icon="add", size=40, variant=variant)
    assert isinstance(node, Node)


def test_an_unknown_variant_raises_a_clear_value_error():
    window = Window(width=300, height=200)
    with pytest.raises(ValueError, match="unknown icon button variant"):
        window.add_icon_button(icon="add", variant="not-a-real-variant")


def test_an_unknown_icon_name_raises_a_clear_value_error():
    window = Window(width=300, height=200)
    with pytest.raises(ValueError, match="unknown icon"):
        window.add_icon_button(icon="not-a-real-icon-name")


def test_a_themed_icon_button_does_not_raise():
    window = Window(width=300, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    for variant in ["filled", "filled_tonal", "outlined", "standard"]:
        node = window.add_icon_button(icon="add", variant=variant)
        assert isinstance(node, Node)


def test_the_already_generic_click_and_ripple_mechanism_works_on_an_icon_button():
    """The same real click-dispatch proof `test_button.py`'s own
    equivalent test establishes for `Button` -- this is the case that
    caught the original bug (a centered `Icon` child eating clicks
    meant for its own container), so this test is the real regression
    guard for `Tree::hit_test_at`'s `NodeKind::Icon(_) => false` arm.
    """
    window = Window(width=300, height=200)
    button = window.add_icon_button(icon="add")
    button.enable_interaction()  # must not raise

    calls = []
    button.set_on_click(lambda: calls.append("clicked"))
    window.click(button)
    assert calls == ["clicked"], (
        "a real click must reach an Icon Button's own registered handler, the same generic "
        "dispatch/ripple mechanism every other NodeKind already uses"
    )
