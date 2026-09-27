"""`window.resize(width, height)`: growing, shrinking, and resizing an empty
window don't raise, and a node added before a resize still gets clicks
after it.
"""

from tre import Window
from helpers import add


def test_resize_does_not_raise():
    window = Window(width=400, height=300)
    window.resize(800, 600)


def test_resize_on_a_window_with_no_children_does_not_raise():
    window = Window(width=400, height=300)
    window.resize(1, 1)


def test_a_click_dispatched_after_a_resize_still_works():
    """`resize()` leaves the tree usable: a node added before it still
    gets a click afterward."""
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(255, 0, 0, 255), width=50, height=50)
    clicked = []
    rect.on("click", lambda: clicked.append(True))

    window.resize(800, 600)
    window.simulate("click", node=rect)

    assert clicked == [True], "a click after a resize must still reach its listener"


def test_resize_shrinking_the_window_does_not_raise():
    window = Window(width=800, height=600)
    add(window, "box", fill=(0, 255, 0, 255), width=50, height=50)
    window.resize(200, 150)


