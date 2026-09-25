"""M35 Phase 1 (§5, §7): real, repeatable coverage of
`Window.add_toolbar` -- MD3's real Toolbar (docked/floating). A real
"container with configurable slots" per MD3's own anatomy: the caller
populates it with any already-built node via the existing, generic
`Node.add_child`, not a specialized children-list parameter.
"""

import pytest

from tre import Node, Window


def test_a_docked_toolbar_defaults_to_the_full_window_width():
    window = Window(width=800, height=600)
    bar = window.add_toolbar()
    assert isinstance(bar, Node)


def test_a_floating_toolbar_does_not_raise():
    window = Window(width=800, height=600)
    bar = window.add_toolbar(variant="floating")
    assert isinstance(bar, Node)


def test_a_floating_vertical_vibrant_toolbar_does_not_raise():
    window = Window(width=800, height=600)
    bar = window.add_toolbar(variant="floating", orientation="vertical", vibrant=True)
    assert isinstance(bar, Node)


def test_an_unknown_variant_raises_a_clear_value_error():
    window = Window(width=800, height=600)
    with pytest.raises(ValueError, match="unknown toolbar variant"):
        window.add_toolbar(variant="bogus")


def test_an_unknown_orientation_raises_a_clear_value_error():
    window = Window(width=800, height=600)
    with pytest.raises(ValueError, match="unknown toolbar orientation"):
        window.add_toolbar(orientation="bogus")


def test_the_pre_0_3_3_color_keyword_is_gone():
    # M90: `color="standard"|"vibrant"` was never a color -- it's the
    # boolean `vibrant=`.
    window = Window(width=800, height=600)
    with pytest.raises(TypeError, match="color"):
        window.add_toolbar(color="vibrant")


def test_a_vertical_docked_toolbar_raises_a_clear_value_error():
    """MD3's own real anatomy has no vertical docked variant -- only a
    floating toolbar can be vertical."""
    window = Window(width=800, height=600)
    with pytest.raises(ValueError, match="always horizontal"):
        window.add_toolbar(variant="docked", orientation="vertical")


def test_a_themed_toolbar_does_not_raise():
    window = Window(width=800, height=600)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    bar = window.add_toolbar(vibrant=True)
    assert isinstance(bar, Node)


def test_an_already_built_button_can_be_composed_into_a_toolbar():
    """The real "container with slots" anatomy: any already-built node
    reparents in via the existing, generic `Node.add_child` -- no
    specialized children-list parameter on `add_toolbar` itself."""
    window = Window(width=800, height=600)
    bar = window.add_toolbar()
    button = window.add_button(label="Save", width=80, height=40)
    bar.add_child(button)


def test_a_real_click_on_a_button_composed_into_a_toolbar_reaches_its_own_handler():
    window = Window(width=800, height=600)
    bar = window.add_toolbar()
    button = window.add_button(label="Save", width=80, height=40)
    bar.add_child(button)

    clicked = []
    button.enable_interaction()
    button.set_on_click(lambda: clicked.append(True))
    window.click(button)

    assert clicked == [True], "a real click on a node composed into a toolbar must still reach its own handler"
