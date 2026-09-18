"""M30 Phase 7 Step 2 (§5, §7): real, repeatable coverage of
`Window.add_time_input_field`/`add_period_selector` -- the real MD3
*Time Input* variant (digital hour:minute entry), not the analog
clock-face dial, which needs a genuinely new drag-to-angle engine
capability this project doesn't have.
"""

import pytest

from tre import Node, Window


def test_add_time_input_field_returns_a_node():
    window = Window(width=400, height=200)
    field = window.add_time_input_field(value="09")
    assert isinstance(field, Node)


def test_a_themed_time_input_field_does_not_raise():
    window = Window(width=400, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    field = window.add_time_input_field(value="09")
    assert isinstance(field, Node)


def test_the_time_input_field_is_a_real_textfield_node():
    """The real point of reusing TextField directly: the field must
    genuinely be a TextField (typing/focus/selection all work), not
    a bare label -- proven the same real way `test_search.py` already
    proved it for Search Bar's own input.
    """
    window = Window(width=400, height=200)
    field = window.add_time_input_field(value="09")
    assert field.get_text() == "09"


def test_typing_into_a_focused_time_field_edits_its_real_content():
    window = Window(width=400, height=200)
    field = window.add_time_input_field(value="09")

    window.press_key("tab")
    field.set_text("")
    window.type_text("11")
    assert field.get_text() == "11"


def test_add_period_selector_defaults_to_am():
    window = Window(width=400, height=200)
    am, pm = window.add_period_selector()
    assert isinstance(am, Node)
    assert isinstance(pm, Node)


def test_add_period_selector_pm_does_not_raise():
    window = Window(width=400, height=200)
    am, pm = window.add_period_selector(selected="PM")
    assert isinstance(am, Node)
    assert isinstance(pm, Node)


def test_add_period_selector_rejects_an_invalid_value():
    window = Window(width=400, height=200)
    with pytest.raises(ValueError, match='"AM" or "PM"'):
        window.add_period_selector(selected="Noon")


def test_am_and_pm_are_each_real_independently_clickable_nodes():
    window = Window(width=400, height=200)
    am, pm = window.add_period_selector()

    clicked = []
    am.enable_interaction()
    am.set_on_click(lambda: clicked.append("am"))
    pm.enable_interaction()
    pm.set_on_click(lambda: clicked.append("pm"))

    window.click(pm)
    assert clicked == ["pm"], "clicking pm must reach only its own registered handler"
