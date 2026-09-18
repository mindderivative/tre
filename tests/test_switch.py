"""M30 Phase 2 Step 2 (§5, §7.3): real, repeatable coverage of `Window.
add_switch`/`Node.set_on`/the new `"toggle_progress"` arm of `Node.
animate`/`Node.get` -- the FFI boundary for a real MD3 switch. Mirrors
`test_radio_button.py`'s own established structure exactly (same "FFI
wiring only" split -- `engine-render/tests/switch_paint.rs` is the
definitive pixel-level proof, not this file).
"""

import pytest

from tre import Node, Window


def test_add_switch_returns_a_node():
    window = Window(width=200, height=200)
    node = window.add_switch()
    assert isinstance(node, Node)


def test_add_switch_defaults_to_off():
    window = Window(width=200, height=200)
    switch = window.add_switch()
    assert switch.get("toggle_progress") == 0.0
    assert switch.get_on() is False


def test_on_true_seeds_toggle_progress_at_one():
    window = Window(width=200, height=200)
    switch = window.add_switch(on=True)
    assert switch.get("toggle_progress") == 1.0
    assert switch.get_on() is True


def test_set_on_and_animate_toggle_progress_reach_the_real_switch():
    window = Window(width=200, height=200)
    switch = window.add_switch()

    switch.set_on(True)  # must not raise -- plain, non-animated state write
    switch.animate("toggle_progress", 1.0, duration_ms=0)  # must not raise

    switch.set_on(False)
    switch.animate("toggle_progress", 0.0, duration_ms=0)  # must not raise


def test_set_on_rejects_a_non_switch_node():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'on'"):
        rect.set_on(True)


def test_toggle_progress_property_is_unknown_on_a_non_switch_node():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'toggle_progress'"):
        rect.animate("toggle_progress", 1.0)
    with pytest.raises(ValueError, match="Rect has no property 'toggle_progress'"):
        rect.get("toggle_progress")


def test_a_themed_switch_does_not_raise():
    window = Window(width=200, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    node = window.add_switch(on=True)
    assert isinstance(node, Node)


def test_the_already_generic_click_and_ripple_mechanism_works_on_a_switch():
    window = Window(width=200, height=200)
    switch = window.add_switch()
    switch.enable_interaction()  # must not raise

    calls = []

    def on_click():
        switch.set_on(True)
        switch.animate("toggle_progress", 1.0, duration_ms=0)
        calls.append("clicked")

    switch.set_on_click(on_click)
    window.click(switch)
    assert calls == ["clicked"], (
        "a real click must reach a Switch's own registered handler, the same generic "
        "dispatch/ripple mechanism every other NodeKind already uses"
    )
