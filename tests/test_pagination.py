"""M30 Phase 8 Step 4 (§5, §7): real, repeatable coverage of
`Window.add_pagination`. MD3 has no official Pagination page
(confirmed via the same directory-listing technique this milestone
already uses).
"""

import pytest

from tre import Node, Window


def test_add_pagination_returns_previous_pages_and_next():
    window = Window(width=500, height=100)
    previous, pages, next_ = window.add_pagination(page_count=5)
    assert isinstance(previous, Node)
    assert len(pages) == 5
    assert all(isinstance(p, Node) for p in pages)
    assert isinstance(next_, Node)


def test_add_pagination_with_a_current_page_does_not_raise():
    window = Window(width=500, height=100)
    previous, pages, next_ = window.add_pagination(page_count=5, current=2)
    assert len(pages) == 5


def test_a_themed_pagination_does_not_raise():
    window = Window(width=500, height=100)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    previous, pages, next_ = window.add_pagination(page_count=3)
    assert len(pages) == 3


def test_zero_pages_raises_a_clear_value_error():
    window = Window(width=500, height=100)
    with pytest.raises(ValueError, match="at least 1 page"):
        window.add_pagination(page_count=0)


def test_out_of_range_current_raises_a_clear_value_error():
    window = Window(width=500, height=100)
    with pytest.raises(ValueError, match="out of range"):
        window.add_pagination(page_count=3, current=3)


def test_each_page_is_a_real_independently_clickable_node():
    window = Window(width=500, height=100)
    _previous, pages, _next = window.add_pagination(page_count=4)

    clicked = []
    for i, page in enumerate(pages):
        page.enable_interaction()
        page.set_on_click(lambda i=i: clicked.append(i))

    window.click(pages[2])
    assert clicked == [2], "clicking one page must reach only its own registered handler"


def test_previous_and_next_are_each_real_independently_clickable_nodes():
    window = Window(width=500, height=100)
    previous, _pages, next_ = window.add_pagination(page_count=4, current=1)

    calls = []
    previous.enable_interaction()
    previous.set_on_click(lambda: calls.append("previous"))
    next_.enable_interaction()
    next_.set_on_click(lambda: calls.append("next"))

    window.click(next_)
    assert calls == ["next"], "clicking next must reach only its own registered handler"
