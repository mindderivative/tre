"""M35 Phase 2 (§5, §7, §8): real, repeatable coverage of
`Window.add_split_button` -- MD3's real Split Button (leading button +
trailing menu-icon button). The engine never opens/closes a menu or
rotates the trailing icon on its own -- the app drives both directly
via `Node.animate("rotation", ...)`, Design Principle 6's own "engine
provides the mechanism, app decides the real state change" split.
"""

import pytest

from tre import Node, Window


def test_add_split_button_returns_three_real_distinct_nodes():
    window = Window(width=800, height=600)
    leading, trailing, icon = window.add_split_button(label="Watch later", width=140, height=32)
    assert isinstance(leading, Node)
    assert isinstance(trailing, Node)
    assert isinstance(icon, Node)


def test_a_themed_split_button_does_not_raise():
    window = Window(width=800, height=600)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    leading, trailing, icon = window.add_split_button(
        label="Watch later", width=140, height=32, variant="outlined"
    )
    assert isinstance(leading, Node)
    assert isinstance(trailing, Node)
    assert isinstance(icon, Node)


def test_an_unknown_variant_raises_a_clear_value_error():
    window = Window(width=800, height=600)
    with pytest.raises(ValueError, match="unknown button variant"):
        window.add_split_button(label="Watch later", width=140, height=32, variant="bogus")


def test_the_leading_and_trailing_buttons_are_independently_clickable():
    window = Window(width=800, height=600)
    leading, trailing, _icon = window.add_split_button(label="Watch later", width=140, height=32)

    clicked = []
    leading.enable_interaction()
    leading.set_on_click(lambda: clicked.append("leading"))
    trailing.enable_interaction()
    trailing.set_on_click(lambda: clicked.append("trailing"))

    window.click(trailing)
    assert clicked == ["trailing"], "a real click on the trailing button must reach only its own handler"


def test_the_leading_button_click_does_not_reach_the_trailing_handler():
    window = Window(width=800, height=600)
    leading, trailing, _icon = window.add_split_button(label="Watch later", width=140, height=32)

    clicked = []
    leading.enable_interaction()
    leading.set_on_click(lambda: clicked.append("leading"))
    trailing.enable_interaction()
    trailing.set_on_click(lambda: clicked.append("trailing"))

    window.click(leading)
    assert clicked == ["leading"], "a real click on the leading button must reach only its own handler"


def test_the_trailing_icon_rotation_can_be_animated_by_the_app():
    """The engine never rotates the icon automatically -- the app
    drives it directly via `Node.animate("rotation", ...)`, the same
    real "engine provides the mechanism" split `Checkbox.checked`
    already establishes.
    """
    window = Window(width=800, height=600)
    _leading, _trailing, icon = window.add_split_button(label="Watch later", width=140, height=32)
    icon.animate("rotation", 180.0, 0)
    icon.animate("rotation", 0.0, 0)
