"""M30 Phase 1 (§5, §7): real, repeatable coverage of `Window.
add_button` -- the FFI boundary for MD3's five real button variants.

The definitive pixel-level proof that each variant's container/label/
border colors and `Elevated`'s real shadow are genuinely correct is
`engine-render`'s own pixel tests (`border_paint.rs`, `text_align.rs`,
and this phase's own future `button_paint.rs`), not this file -- the
same "FFI wiring only" split `test_checkbox.py`/`test_slider.py`
already establish. This file proves: `add_button` returns a real,
usable `Node` for every real variant string; an unknown variant raises
a clear `ValueError` rather than silently falling back to something;
and the already-generic click/ripple mechanism works unmodified on a
button's own container node, exactly as it does for every other
`NodeKind`.
"""

import pytest

from tre import Node, Window


def test_add_button_returns_a_node():
    window = Window(width=300, height=200)
    node = window.add_button(label="Ok", width=120, height=40)
    assert isinstance(node, Node)


def test_add_button_defaults_to_filled_variant():
    """`variant="filled"` is the real default in both `window_factory.
    rs`'s own `#[pyo3(signature = ...)]` and this stub -- calling
    without `variant` at all must not raise.
    """
    window = Window(width=300, height=200)
    window.add_button(label="Ok", width=120, height=40)  # must not raise


@pytest.mark.parametrize(
    "variant", ["elevated", "filled", "filled_tonal", "outlined", "text"]
)
def test_every_real_md3_variant_is_accepted(variant):
    window = Window(width=300, height=200)
    node = window.add_button(label="Ok", width=120, height=40, variant=variant)
    assert isinstance(node, Node)


def test_an_unknown_variant_raises_a_clear_value_error():
    window = Window(width=300, height=200)
    with pytest.raises(ValueError, match="unknown button variant"):
        window.add_button(label="Ok", width=120, height=40, variant="not-a-real-variant")


def test_a_themed_button_does_not_raise():
    """`add_button` created after `set_theme` must resolve every real
    MD3 role it needs (`primary`/`on_primary`/`secondary_container`/
    `outline`/etc) through `ThemeState::role` without raising, for
    every real variant -- the construction-time half of the same real
    "themed at construction" contract `theme.py` already established
    for `Checkbox`/`Slider`/`TextField`.
    """
    window = Window(width=300, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    for variant in ["elevated", "filled", "filled_tonal", "outlined", "text"]:
        node = window.add_button(label="Ok", width=120, height=40, variant=variant)
        assert isinstance(node, Node)


def test_the_already_generic_click_and_ripple_mechanism_works_on_a_button():
    """No new dispatch wiring was needed for this -- `set_on_click`/
    `enable_interaction()` already work on any `NodeKind`; this is the
    real, functional proof they still do for a button's own container
    node, not an assumption.
    """
    window = Window(width=300, height=200)
    button = window.add_button(label="Save", width=120, height=40)
    button.enable_interaction()  # must not raise

    calls = []
    button.set_on_click(lambda: calls.append("clicked"))
    window.click(button)
    assert calls == ["clicked"], (
        "a real click must reach a Button's own registered handler, the same generic "
        "dispatch/ripple mechanism every other NodeKind already uses"
    )
