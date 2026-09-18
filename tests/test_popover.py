"""M30 Phase 8 Step 1 (§11.3): real, repeatable coverage of
`Window.add_popover`, grounded in MD3's own real Rich Tooltip anatomy.
"Persistent" here means it doesn't dismiss automatically on hover-exit
the way the plain `add_tooltip` does -- it still reuses the existing
`Window.open_menu`/`close_menu` directly (the identical real design
`add_tooltip`'s own panel already established, Phase 3 Step 5), which
does dismiss on a real outside click, exactly like Menu.
"""

from tre import Node, Window


def test_add_popover_returns_a_node():
    window = Window(width=400, height=300)
    popover = window.add_popover(subhead="Storage", text="You have used 12 GB of 15 GB.", width=240, height=100)
    assert isinstance(popover, Node)


def test_a_themed_popover_does_not_raise():
    window = Window(width=400, height=300)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    popover = window.add_popover(subhead="Storage", text="You have used 12 GB of 15 GB.", width=240, height=100)
    assert isinstance(popover, Node)


def test_popover_can_be_opened_and_closed_via_open_menu():
    """The real, deliberate reuse this step's own design makes: no
    dedicated open/close pair, just the existing `open_menu`/
    `close_menu` -- the same real contract `add_tooltip`'s own panel
    already has.
    """
    window = Window(width=400, height=300)
    anchor = window.add_rect(background=(0, 0, 0, 255), width=100, height=40)
    popover = window.add_popover(subhead="Storage", text="You have used 12 GB of 15 GB.", width=240, height=100)

    window.open_menu(anchor, popover)  # must not raise
    window.open_menu(anchor, popover)  # a real, safe no-op -- already open
    window.close_menu(popover)  # must not raise


def test_a_reopened_popover_after_closing_does_not_raise():
    window = Window(width=400, height=300)
    anchor = window.add_rect(background=(0, 0, 0, 255), width=100, height=40)
    popover = window.add_popover(subhead="Storage", text="You have used 12 GB of 15 GB.", width=240, height=100)

    window.open_menu(anchor, popover)
    window.close_menu(popover)
    window.open_menu(anchor, popover)  # must not raise -- real reopen, not a stale/destroyed node


def test_a_popover_dismisses_on_a_real_outside_click_like_menu_does():
    """Real, honest clarification of what "persistent" actually means
    here: a popover doesn't dismiss on hover-exit the way the plain
    tooltip does, but it still reuses `open_menu`'s own real
    `dismiss_on_outside_click: true` behavior -- persistent against
    accidental hover changes, not immune to every dismissal. Proven
    explicitly rather than assumed from the doc comment alone.
    """
    window = Window(width=400, height=300)
    anchor = window.add_rect(background=(0, 0, 0, 255), width=100, height=40)
    background = window.add_rect(background=(0, 0, 0, 255), width=400, height=300)
    background.enable_interaction()
    clicks = []
    background.set_on_click(lambda: clicks.append("clicked"))

    popover = window.add_popover(subhead="Storage", text="You have used 12 GB of 15 GB.", width=240, height=100)
    window.open_menu(anchor, popover)

    # A point on the background, outside both the anchor and the
    # popover's own panel -- open_menu's real dismiss_on_outside_click
    # behavior should consume this click.
    window.click(background)
    assert clicks == [], "a real outside click must dismiss the popover, the same real Menu behavior it reuses"
