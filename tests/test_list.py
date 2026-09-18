"""M30 Phase 6 Step 1 (§5, §7): real, repeatable coverage of
`Window.add_list_item`/`add_list` -- a plain, non-virtualized list for
small real collections (`VirtualList`, already real, stays the choice
for large ones). Covers this step's own proactive fix: the two-line
variant's headline+supporting-text wrapper is opted out of hit-testing
immediately via `Tree::set_hit_testable` (`Navigation Rail`'s own
capability, already confirmed reusable by `Tabs`), verified here for a
third time rather than assumed safe by precedent alone.
"""

import pytest

from tre import Node, Window


def test_add_list_item_one_line_returns_a_node():
    window = Window(width=400, height=400)
    item = window.add_list_item(headline="Inbox")
    assert isinstance(item, Node)


def test_add_list_item_two_line_returns_a_node():
    window = Window(width=400, height=400)
    item = window.add_list_item(headline="Inbox", supporting_text="12 unread")
    assert isinstance(item, Node)


def test_add_list_item_with_leading_and_trailing_icons_does_not_raise():
    window = Window(width=400, height=400)
    item = window.add_list_item(headline="Inbox", leading_icon="add", trailing_icon="add")
    assert isinstance(item, Node)


def test_a_themed_list_item_does_not_raise():
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    item = window.add_list_item(headline="Inbox", supporting_text="12 unread")
    assert isinstance(item, Node)


def test_an_unknown_leading_icon_name_raises_a_clear_value_error():
    window = Window(width=400, height=400)
    with pytest.raises(ValueError, match="unknown icon"):
        window.add_list_item(headline="Inbox", leading_icon="not-a-real-icon-name")


def test_add_list_groups_items_into_one_frame():
    window = Window(width=400, height=400)
    items = [window.add_list_item(headline=label) for label in ["Inbox", "Starred", "Sent"]]
    frame = window.add_list(items)
    assert isinstance(frame, Node)


def test_add_list_with_no_items_raises_a_clear_value_error():
    window = Window(width=400, height=400)
    with pytest.raises(ValueError, match="at least 1 item"):
        window.add_list([])


def test_add_list_rejects_an_item_from_a_different_window():
    window_a = Window(width=400, height=400)
    window_b = Window(width=400, height=400)
    foreign_item = window_b.add_list_item(headline="Inbox")
    with pytest.raises(ValueError, match="different Window"):
        window_a.add_list([foreign_item])


def test_a_one_line_item_is_a_real_independently_clickable_node():
    window = Window(width=400, height=400)
    item = window.add_list_item(headline="Inbox")

    calls = []
    item.enable_interaction()
    item.set_on_click(lambda: calls.append("clicked"))
    window.click(item)
    assert calls == ["clicked"], "a real click on the item must reach its own registered handler"


def test_a_two_line_item_is_a_real_independently_clickable_node():
    """The real point of this step's own proactive fix: a click at
    the item's own center (which falls inside the real headline/
    supporting-text wrapper) must still reach the item's own handler,
    not be intercepted by that wrapper the way `Navigation Rail`'s own
    decorative Rect first did.
    """
    window = Window(width=400, height=400)
    item = window.add_list_item(headline="Inbox", supporting_text="12 unread")

    calls = []
    item.enable_interaction()
    item.set_on_click(lambda: calls.append("clicked"))
    window.click(item)
    assert calls == ["clicked"], "a real click on a two-line item must reach its own registered handler"


def test_each_item_in_a_list_remains_independently_clickable():
    window = Window(width=400, height=400)
    items = [window.add_list_item(headline=label) for label in ["Inbox", "Starred", "Sent"]]
    window.add_list(items)

    clicked = []
    for i, item in enumerate(items):
        item.enable_interaction()
        item.set_on_click(lambda i=i: clicked.append(i))

    window.click(items[1])
    assert clicked == [1], "clicking one item must reach only its own registered handler"
