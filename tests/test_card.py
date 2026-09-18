"""M30 Phase 3 Step 3 (§5, §7): real, repeatable coverage of `Window.
add_card` -- the FFI boundary for MD3's three real card variants.
Mirrors `test_chip.py`'s own established structure (same "FFI wiring
only" split -- `engine-render`'s own existing pixel tests for `Rect`
are the definitive paint proof for this plain container, not this
file).
"""

import pytest

from tre import Node, Window


def test_add_card_returns_a_node():
    window = Window(width=300, height=300)
    node = window.add_card(width=200, height=120)
    assert isinstance(node, Node)


@pytest.mark.parametrize("variant", ["elevated", "filled", "outlined"])
def test_every_real_md3_card_variant_is_accepted(variant):
    window = Window(width=300, height=300)
    node = window.add_card(width=200, height=120, variant=variant)
    assert isinstance(node, Node)


def test_an_unknown_variant_raises_a_clear_value_error():
    window = Window(width=300, height=300)
    with pytest.raises(ValueError, match="unknown card variant"):
        window.add_card(width=200, height=120, variant="not-a-real-variant")


def test_a_themed_card_does_not_raise():
    window = Window(width=300, height=300)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    for variant in ["elevated", "filled", "outlined"]:
        node = window.add_card(width=200, height=120, variant=variant)
        assert isinstance(node, Node)


def test_a_card_accepts_arbitrary_child_content():
    """A real card is a plain container -- this proves the already-
    generic `Node.add_child` reaches it exactly like any other node,
    no special anatomy-specific API needed.
    """
    window = Window(width=300, height=300)
    card = window.add_card(width=200, height=120)
    label = window.add_text(content="Title", background=(0, 0, 0, 255), width=180, height=24)
    card.add_child(label)  # must not raise


def test_the_already_generic_click_and_ripple_mechanism_works_on_a_card():
    window = Window(width=300, height=300)
    card = window.add_card(width=200, height=120)
    card.enable_interaction()  # must not raise

    calls = []
    card.set_on_click(lambda: calls.append("clicked"))
    window.click(card)
    assert calls == ["clicked"], (
        "a real click must reach a Card's own registered handler, the same generic "
        "dispatch/ripple mechanism every other NodeKind already uses"
    )
