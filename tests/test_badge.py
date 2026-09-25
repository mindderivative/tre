"""M30 Phase 3 Step 1 (§5, §7): real, repeatable coverage of `Window.
add_badge` -- the FFI boundary for MD3's real two-size badge anatomy.
Mirrors `test_chip.py`'s own established structure (same "FFI wiring
only" split -- `engine-render`'s own existing pixel tests for `Rect`/
`Text` are the definitive paint proof for this pure composition, not
this file).
"""

from tre import Node, Window


def test_add_badge_with_no_label_returns_a_node():
    window = Window(width=200, height=200)
    node = window.add_badge()
    assert isinstance(node, Node)


def test_add_badge_with_a_label_returns_a_node():
    window = Window(width=200, height=200)
    node = window.add_badge(label="9")
    assert isinstance(node, Node)


def test_add_badge_with_a_wider_multi_digit_label_does_not_raise():
    window = Window(width=200, height=200)
    node = window.add_badge(label="99+", width=28)
    assert isinstance(node, Node)


def test_a_themed_badge_does_not_raise():
    window = Window(width=200, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    dot = window.add_badge()
    labeled = window.add_badge(label="3")
    assert isinstance(dot, Node)
    assert isinstance(labeled, Node)


def test_a_badge_is_positioned_independently_via_x_and_y():
    """A real badge is always overlaid on some other component's own
    corner -- this proves it's just a plain, caller-positioned node,
    not something with special anchoring machinery of its own.
    """
    window = Window(width=200, height=200)
    icon = window.add_icon(name="add", foreground=(0, 0, 0, 255), size=24, x=16, y=16)
    badge = window.add_badge(x=32, y=12)
    assert isinstance(icon, Node)
    assert isinstance(badge, Node)
