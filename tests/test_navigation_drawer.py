"""M30 Phase 5 Step 2 (§5, §7, §11.3): real, repeatable coverage of
`Window.add_navigation_drawer`/`open_navigation_drawer`/
`close_navigation_drawer` -- the real desktop counterpart to Bottom
App Bar's own navigation role. Covers the same real behavioral fork
`Side Sheet` (Phase 4 Step 3) established (Standard = plain layout
participant, Modal = real blocking overlay reusing `Dialog`'s own
`OverlayMeta.modal`), plus the real multi-node return shape needed
for independently clickable destinations.
"""

import pytest

from tre import Node, Window


def test_add_navigation_drawer_standard_returns_container_and_items():
    window = Window(width=800, height=600)
    container, items = window.add_navigation_drawer(
        labels=["Inbox", "Starred", "Sent"],
        icons=["add", "add", "add"],
    )
    assert isinstance(container, Node)
    assert len(items) == 3
    assert all(isinstance(item, Node) for item in items)


def test_add_navigation_drawer_modal_returns_container_and_items():
    window = Window(width=800, height=600)
    container, items = window.add_navigation_drawer(
        labels=["Inbox", "Starred"],
        icons=["add", "add"],
        modal=True,
    )
    assert isinstance(container, Node)
    assert len(items) == 2


def test_a_themed_navigation_drawer_does_not_raise():
    window = Window(width=800, height=600)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    container, items = window.add_navigation_drawer(
        labels=["Inbox", "Starred"],
        icons=["add", "add"],
        selected=1,
    )
    assert isinstance(container, Node)
    assert len(items) == 2


def test_mismatched_labels_and_icons_raises_a_clear_value_error():
    window = Window(width=800, height=600)
    with pytest.raises(ValueError, match="one icon per label"):
        window.add_navigation_drawer(labels=["Inbox", "Starred"], icons=["add"])


def test_empty_labels_raises_a_clear_value_error():
    window = Window(width=800, height=600)
    with pytest.raises(ValueError, match="at least 1 item"):
        window.add_navigation_drawer(labels=[], icons=[])


def test_out_of_range_selected_raises_a_clear_value_error():
    window = Window(width=800, height=600)
    with pytest.raises(ValueError, match="out of range"):
        window.add_navigation_drawer(labels=["Inbox"], icons=["add"], selected=1)


def test_open_navigation_drawer_and_close_navigation_drawer_do_not_raise_for_modal():
    window = Window(width=800, height=600)
    container, _items = window.add_navigation_drawer(labels=["Inbox"], icons=["add"], modal=True)

    window.open_navigation_drawer(container)  # must not raise
    window.open_navigation_drawer(container)  # a real, safe no-op -- already open
    window.close_navigation_drawer(container)  # must not raise


def test_open_navigation_drawer_on_a_standard_drawer_is_a_real_explicit_no_op():
    window = Window(width=800, height=600)
    container, _items = window.add_navigation_drawer(labels=["Inbox"], icons=["add"], modal=False)
    window.open_navigation_drawer(container)  # must not raise
    window.close_navigation_drawer(container)  # must not raise


def test_open_navigation_drawer_rejects_a_drawer_from_a_different_window():
    window_a = Window(width=800, height=600)
    window_b = Window(width=800, height=600)
    foreign_container, _items = window_b.add_navigation_drawer(labels=["Inbox"], icons=["add"], modal=True)
    with pytest.raises(ValueError, match="different Window"):
        window_a.open_navigation_drawer(foreign_container)


def test_an_open_modal_navigation_drawer_blocks_a_real_click_on_the_background():
    """A third independent proof that `OverlayMeta.modal` (Dialog,
    Phase 4 Step 1) generalizes correctly, this time for a drawer
    docked to the left edge instead of Dialog's centered placement or
    Side Sheet's right edge.
    """
    window = Window(width=800, height=600)
    background_button = window.add_rect(background=(0, 0, 0, 255), width=780, height=580)
    background_button.enable_interaction()
    calls = []
    background_button.set_on_click(lambda: calls.append("clicked"))

    window.click(background_button)
    assert calls == ["clicked"]
    calls.clear()

    container, _items = window.add_navigation_drawer(labels=["Inbox"], icons=["add"], modal=True)
    window.open_navigation_drawer(container)

    window.click(background_button)
    assert calls == [], "a modal navigation drawer must block clicks on everything behind it"

    window.close_navigation_drawer(container)
    window.click(background_button)
    assert calls == ["clicked"], "the background must be clickable again once the drawer is closed"


def test_each_destination_is_a_real_independently_clickable_node():
    window = Window(width=800, height=600)
    container, items = window.add_navigation_drawer(
        labels=["Inbox", "Starred", "Sent"],
        icons=["add", "add", "add"],
    )

    clicked = []
    for i, item in enumerate(items):
        item.enable_interaction()
        item.set_on_click(lambda i=i: clicked.append(i))

    window.click(items[1])
    assert clicked == [1], "clicking one destination must reach only its own registered handler"


def test_a_standard_drawer_can_be_reparented_into_the_apps_own_layout():
    window = Window(width=800, height=600)
    shell = window.add_rect(background=(255, 255, 255, 255), width=800, height=600)
    container, _items = window.add_navigation_drawer(labels=["Inbox"], icons=["add"], modal=False)
    shell.add_child(container)  # must not raise -- a real re-parent, not a duplicate-child panic
