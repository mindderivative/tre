"""M30 Phase 5 Step 5 (§5, §7, §11.3): real, repeatable coverage of
`Window.add_search_bar`/`add_search_view`, closing Phase 5's own
component list. `add_search_bar`'s own input reuses `TextField`'s
already-real `NodeKind` directly, so every one of its existing typing/
focus capabilities works for free. `add_search_view`'s own panel is
opened/closed through the existing `Window.open_menu`/`close_menu`,
the same real reuse `test_tooltip.py` already proved for
`add_tooltip`'s own panel.
"""

import pytest

from tre import Node, Window


def test_add_search_bar_with_no_icons_returns_bar_and_field_only():
    window = Window(width=600, height=100)
    bar, field, leading, trailing = window.add_search_bar(placeholder="Search", width=400)
    assert isinstance(bar, Node)
    assert isinstance(field, Node)
    assert leading is None
    assert trailing == []


def test_add_search_bar_with_a_leading_icon_returns_a_real_leading_node():
    window = Window(width=600, height=100)
    bar, field, leading, trailing = window.add_search_bar(
        placeholder="Search", width=400, leading_icon="add"
    )
    assert isinstance(bar, Node)
    assert isinstance(field, Node)
    assert isinstance(leading, Node)
    assert trailing == []


def test_add_search_bar_with_trailing_icons_returns_one_node_each():
    window = Window(width=600, height=100)
    bar, field, leading, trailing = window.add_search_bar(
        placeholder="Search", width=400, trailing_icons=["add", "add"]
    )
    assert isinstance(bar, Node)
    assert leading is None
    assert len(trailing) == 2
    assert all(isinstance(t, Node) for t in trailing)


def test_a_themed_search_bar_does_not_raise():
    window = Window(width=600, height=100)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    bar, field, leading, trailing = window.add_search_bar(
        placeholder="Search", width=400, leading_icon="add", trailing_icons=["add"]
    )
    assert isinstance(bar, Node)
    assert isinstance(field, Node)


def test_an_unknown_leading_icon_name_raises_a_clear_value_error():
    window = Window(width=600, height=100)
    with pytest.raises(ValueError, match="unknown icon"):
        window.add_search_bar(placeholder="Search", width=400, leading_icon="not-a-real-icon-name")


def test_the_search_bars_own_text_field_is_a_real_textfield_node():
    """The real point of reusing TextField directly: the returned
    field must genuinely be a NodeKind::TextField, not a bare label --
    proven the same real way `test_text_field.py`'s own
    `test_get_text_rejects_a_non_text_field_node` does, from the other
    direction: `get_text()` only works on a real TextField node, and
    genuinely returns the placeholder seeded as its initial content.
    """
    window = Window(width=600, height=100)
    _bar, field, _leading, _trailing = window.add_search_bar(placeholder="Search", width=400)
    assert field.get_text() == "Search"


def test_add_search_view_returns_a_real_node():
    window = Window(width=600, height=400)
    view = window.add_search_view(width=400, height=200)
    assert isinstance(view, Node)


def test_a_themed_search_view_does_not_raise():
    window = Window(width=600, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    view = window.add_search_view(width=400, height=200)
    assert isinstance(view, Node)


def test_search_view_can_be_populated_and_opened_via_open_menu():
    """The real, deliberate reuse this step's own design makes: no
    dedicated open/close pair, just the existing `open_menu`/
    `close_menu` -- the same real contract `add_tooltip`'s own panel
    already has.
    """
    window = Window(width=600, height=400)
    bar, _field, _leading, _trailing = window.add_search_bar(placeholder="Search", width=400)
    view = window.add_search_view(width=400, height=200)

    suggestion = window.add_text(
        content="Recent search",
        background=(0, 0, 0, 0),
        width=380,
        height=20,
    )
    view.add_child(suggestion)

    window.open_menu(bar, view)  # must not raise
    window.close_menu(view)  # must not raise


def test_each_trailing_search_icon_is_a_real_independently_clickable_node():
    window = Window(width=600, height=100)
    _bar, _field, _leading, trailing = window.add_search_bar(
        placeholder="Search", width=400, trailing_icons=["add", "add"]
    )

    clicked = []
    for i, node in enumerate(trailing):
        node.enable_interaction()
        node.set_on_click(lambda i=i: clicked.append(i))

    window.click(trailing[1])
    assert clicked == [1], "clicking one trailing icon must reach only its own registered handler"
