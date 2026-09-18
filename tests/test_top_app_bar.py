"""M30 Phase 5 Step 3 (§5, §7): real, repeatable coverage of
`Window.add_top_app_bar` -- MD3's real *Small* Top App Bar variant.
Reuses `Icon Button`'s own exact real anatomy for its leading/trailing
actions (a `Rect` container with a centered, deferring `Icon` child),
so each is independently clickable with no `Navigation Rail`-style
hit-test risk.
"""

import pytest

from tre import Node, Window


def test_add_top_app_bar_with_no_icons_returns_bar_and_no_leading_and_empty_trailing():
    window = Window(width=800, height=100)
    bar, leading, trailing = window.add_top_app_bar(title="Inbox")
    assert isinstance(bar, Node)
    assert leading is None
    assert trailing == []


def test_add_top_app_bar_with_a_leading_icon_returns_a_real_leading_node():
    window = Window(width=800, height=100)
    bar, leading, trailing = window.add_top_app_bar(title="Inbox", leading_icon="add")
    assert isinstance(bar, Node)
    assert isinstance(leading, Node)
    assert trailing == []


def test_add_top_app_bar_with_trailing_icons_returns_one_node_each():
    window = Window(width=800, height=100)
    bar, leading, trailing = window.add_top_app_bar(title="Inbox", trailing_icons=["add", "add"])
    assert isinstance(bar, Node)
    assert leading is None
    assert len(trailing) == 2
    assert all(isinstance(t, Node) for t in trailing)


def test_a_themed_top_app_bar_does_not_raise():
    window = Window(width=800, height=100)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    bar, leading, trailing = window.add_top_app_bar(
        title="Inbox", leading_icon="add", trailing_icons=["add", "add"]
    )
    assert isinstance(bar, Node)
    assert isinstance(leading, Node)
    assert len(trailing) == 2


def test_an_unknown_leading_icon_name_raises_a_clear_value_error():
    window = Window(width=800, height=100)
    with pytest.raises(ValueError, match="unknown icon"):
        window.add_top_app_bar(title="Inbox", leading_icon="not-a-real-icon-name")


def test_an_unknown_trailing_icon_name_raises_a_clear_value_error():
    window = Window(width=800, height=100)
    with pytest.raises(ValueError, match="unknown icon"):
        window.add_top_app_bar(title="Inbox", trailing_icons=["not-a-real-icon-name"])


def test_the_leading_node_is_a_real_independently_clickable_node():
    window = Window(width=800, height=100)
    _bar, leading, _trailing = window.add_top_app_bar(title="Inbox", leading_icon="add")

    calls = []
    assert leading is not None
    leading.enable_interaction()
    leading.set_on_click(lambda: calls.append("leading"))
    window.click(leading)
    assert calls == ["leading"], "a real click on the leading icon must reach its own registered handler"


def test_each_trailing_node_is_a_real_independently_clickable_node():
    window = Window(width=800, height=100)
    _bar, _leading, trailing = window.add_top_app_bar(title="Inbox", trailing_icons=["add", "add"])

    clicked = []
    for i, node in enumerate(trailing):
        node.enable_interaction()
        node.set_on_click(lambda i=i: clicked.append(i))

    window.click(trailing[1])
    assert clicked == [1], "clicking one trailing icon must reach only its own registered handler"
