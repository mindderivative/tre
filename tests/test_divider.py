"""M30 Phase 3 Step 4 (§5, §7): real, repeatable coverage of `Window.
add_divider` -- the FFI boundary for MD3's real 1dp separator line.
Mirrors `test_card.py`'s own established structure (same "FFI wiring
only" split -- `engine-render`'s own existing pixel tests for `Rect`
are the definitive paint proof for this plain shape, not this file).
"""

from tre import Node, Window


def test_add_divider_returns_a_node():
    window = Window(width=200, height=200)
    node = window.add_divider(length=150)
    assert isinstance(node, Node)


def test_add_divider_vertical_does_not_raise():
    window = Window(width=200, height=200)
    node = window.add_divider(length=100, orientation="vertical")
    assert isinstance(node, Node)


def test_a_themed_divider_does_not_raise():
    window = Window(width=200, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    node = window.add_divider(length=150)
    assert isinstance(node, Node)
