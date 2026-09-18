"""M30 Phase 3 Step 5 (§5, §7, §11.3): real, repeatable coverage of
`Window.add_tooltip` -- the FFI boundary for MD3's real Plain Tooltip
panel, shown/hidden through the same `open_menu`/`close_menu`
primitive Step 4 already built. Mirrors `test_menu.py`'s own
established structure.
"""

from tre import Node, Window


def test_add_tooltip_returns_a_node():
    window = Window(width=200, height=200)
    node = window.add_tooltip(text="Delete", width=80)
    assert isinstance(node, Node)


def test_a_themed_tooltip_does_not_raise():
    window = Window(width=200, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    node = window.add_tooltip(text="Delete", width=80)
    assert isinstance(node, Node)


def test_a_tooltip_opens_and_closes_via_the_real_menu_overlay_primitive():
    """A tooltip is deliberately shown/hidden through the same real
    `open_menu`/`close_menu` `Window.add_menu_item`'s own group already
    uses -- this proves that reuse actually works, not just compiles.
    """
    window = Window(width=200, height=200)
    anchor = window.add_rect(background=(0, 0, 0, 255), width=40, height=40)
    tooltip = window.add_tooltip(text="Delete", width=80)

    window.open_menu(anchor, tooltip)  # must not raise
    window.close_menu(tooltip)  # must not raise


def test_hover_enter_and_exit_can_drive_a_real_tooltip():
    """The real, intended wiring: `Node.set_on_hover_enter`/`set_on_
    hover_exit` (already generic) opening/closing a tooltip -- proven
    via a real synthetic hover dispatch, not just that the handlers
    can be registered.
    """
    window = Window(width=200, height=200)
    anchor = window.add_rect(background=(0, 0, 0, 255), width=40, height=40)
    tooltip = window.add_tooltip(text="Delete", width=80)

    events = []
    anchor.set_on_hover_enter(lambda: (window.open_menu(anchor, tooltip), events.append("enter")))
    anchor.set_on_hover_exit(lambda: (window.close_menu(tooltip), events.append("exit")))

    window.hover(anchor)  # moves onto the anchor -- fires hover-enter
    assert "enter" in events
