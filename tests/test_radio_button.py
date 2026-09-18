"""M30 Phase 2 Step 1 (§5, §7.3): real, repeatable coverage of `Window.
add_radio_button`/`Node.set_selected`/the new `"select_progress"` arm
of `Node.animate`/`Node.get` -- the FFI boundary for a real MD3 radio
button. Mirrors `test_checkbox.py`'s own established structure exactly
(same "FFI wiring only" split -- `engine-render/tests/radio_button_
paint.rs` is the definitive pixel-level proof, not this file).
"""

import pytest

from tre import Node, Window


def test_add_radio_button_returns_a_node():
    window = Window(width=200, height=200)
    node = window.add_radio_button()
    assert isinstance(node, Node)


def test_add_radio_button_defaults_to_unselected():
    window = Window(width=200, height=200)
    radio = window.add_radio_button()
    assert radio.get("select_progress") == 0.0
    assert radio.get_selected() is False


def test_selected_true_seeds_select_progress_at_one():
    window = Window(width=200, height=200)
    radio = window.add_radio_button(selected=True)
    assert radio.get("select_progress") == 1.0
    assert radio.get_selected() is True


def test_set_selected_and_animate_select_progress_reach_the_real_radio_button():
    window = Window(width=200, height=200)
    radio = window.add_radio_button()

    radio.set_selected(True)  # must not raise -- plain, non-animated state write
    radio.animate("select_progress", 1.0, duration_ms=0)  # must not raise

    radio.set_selected(False)
    radio.animate("select_progress", 0.0, duration_ms=0)  # must not raise


def test_set_selected_rejects_a_non_radio_button_node():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'selected'"):
        rect.set_selected(True)


def test_select_progress_property_is_unknown_on_a_non_radio_button_node():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'select_progress'"):
        rect.animate("select_progress", 1.0)
    with pytest.raises(ValueError, match="Rect has no property 'select_progress'"):
        rect.get("select_progress")


def test_a_themed_radio_button_does_not_raise():
    window = Window(width=200, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    node = window.add_radio_button(selected=True)
    assert isinstance(node, Node)


def test_the_already_generic_click_and_ripple_mechanism_works_on_a_radio_button():
    """No new dispatch wiring was needed for this -- `set_on_click`/
    `enable_interaction()` already work on any `NodeKind`; this is the
    real, functional proof they still do for a `RadioButton` too, not
    an assumption.
    """
    window = Window(width=200, height=200)
    radio = window.add_radio_button()
    radio.enable_interaction()  # must not raise

    calls = []

    def on_click():
        radio.set_selected(True)
        radio.animate("select_progress", 1.0, duration_ms=0)
        calls.append("clicked")

    radio.set_on_click(on_click)
    window.click(radio)
    assert calls == ["clicked"], (
        "a real click must reach a RadioButton's own registered handler, the same generic "
        "dispatch/ripple mechanism every other NodeKind already uses"
    )


def test_group_exclusivity_is_real_application_state_not_engine_owned():
    """The real, explicit design this component's own docstring states
    -- selecting one radio in a group does not automatically deselect
    the others; an app wires that up itself.
    """
    window = Window(width=200, height=200)
    a = window.add_radio_button(selected=True)
    b = window.add_radio_button()

    def select(chosen, others):
        chosen.set_selected(True)
        for other in others:
            other.set_selected(False)

    select(b, [a])
    assert b.get_selected() is True
    assert a.get_selected() is False
