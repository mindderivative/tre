"""Scroll views: `window.create("scroll_view", ...)` in either orientation,
content attached with `add_child` and still clickable, wheel input on both
axes, and rejecting an unknown orientation. Clipping and scroll offset are
covered by `engine-render`'s `scroll_view.rs` and `engine-core`'s tree tests.
"""

import pytest

from tre import Node, Window
from helpers import add


def test_create_scroll_view_returns_a_node():
    window = Window(width=800, height=600)
    view = add(window, "scroll_view", width=200, height=100)
    assert isinstance(view, Node)


def test_a_horizontal_scroll_view_does_not_raise():
    window = Window(width=800, height=600)
    view = add(window, "scroll_view", width=200, height=100, orientation="horizontal")
    assert isinstance(view, Node)


def test_content_can_be_attached_with_add_child():
    window = Window(width=800, height=600)
    view = add(window, "scroll_view", width=200, height=100)
    content = add(window, "box", fill=(255, 0, 0, 255), width=200, height=1000)
    view.add_child(content)


def test_a_click_on_scroll_view_content_reaches_its_listener():
    window = Window(width=800, height=600)
    view = add(window, "scroll_view", width=200, height=100)
    content = add(window, "box", fill=(255, 0, 0, 255), width=200, height=1000)
    view.add_child(content)

    clicked = []
    content.on("click", lambda: clicked.append(True))
    window.simulate("click", node=content)
    assert clicked == [True]


def test_wheel_with_delta_x_does_not_raise_on_a_horizontal_scroll_view():
    window = Window(width=800, height=600)
    view = add(window, "scroll_view", width=100, height=50, orientation="horizontal")
    content = add(window, "box", fill=(0, 255, 0, 255), width=1000, height=50)
    view.add_child(content)
    window.simulate("wheel", node=view, delta_y=0.0, delta_x=500.0)


def test_wheel_with_only_delta_y_does_not_raise():
    window = Window(width=800, height=600)
    view = add(window, "scroll_view", width=200, height=100)
    content = add(window, "box", fill=(255, 0, 0, 255), width=200, height=1000)
    view.add_child(content)
    window.simulate("wheel", node=view, delta_y=50.0)


def test_an_unknown_orientation_is_a_clear_error():
    with pytest.raises(ValueError, match="`orientation` must be one of"):
        add(Window(), "scroll_view", width=100, height=50, orientation="diagonal")
