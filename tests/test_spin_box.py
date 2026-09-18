"""M30 Phase 8 Step 3 (§5, §7): real, repeatable coverage of
`Window.add_spin_box`. Deliberately named *SpinBox*, not *Stepper* --
MD3's own vocabulary already uses "Stepper" for a completely different
real component (a multi-step flow indicator), pyCopper's own real
prior naming-risk finding, reused directly.
"""

from tre import Node, Window


def test_add_spin_box_returns_field_and_two_buttons():
    window = Window(width=300, height=100)
    field, decrement, increment = window.add_spin_box(value="1")
    assert isinstance(field, Node)
    assert isinstance(decrement, Node)
    assert isinstance(increment, Node)


def test_a_themed_spin_box_does_not_raise():
    window = Window(width=300, height=100)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    field, decrement, increment = window.add_spin_box(value="1")
    assert isinstance(field, Node)


def test_the_spin_box_field_is_a_real_textfield_node():
    window = Window(width=300, height=100)
    field, _decrement, _increment = window.add_spin_box(value="5")
    assert field.get_text() == "5"


def test_typing_into_a_focused_spin_box_field_edits_its_real_content():
    window = Window(width=300, height=100)
    field, _decrement, _increment = window.add_spin_box(value="1")

    window.press_key("tab")
    field.set_text("")
    window.type_text("42")
    assert field.get_text() == "42"


def test_decrement_and_increment_are_each_real_independently_clickable_nodes():
    window = Window(width=300, height=100)
    field, decrement, increment = window.add_spin_box(value="1")

    counter = {"value": 1}

    def apply_delta(delta: int) -> None:
        counter["value"] += delta
        field.set_text(str(counter["value"]))

    decrement.enable_interaction()
    decrement.set_on_click(lambda: apply_delta(-1))
    increment.enable_interaction()
    increment.set_on_click(lambda: apply_delta(1))

    window.click(increment)
    window.click(increment)
    window.click(decrement)
    assert counter["value"] == 2
    assert field.get_text() == "2"
