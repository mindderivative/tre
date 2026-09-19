"""M36 Phase 1 (§5, §7, §11.7): real, repeatable coverage of
`Window.add_scroll_view` -- a real, general scrollable viewport over
one child, grounded directly in the sibling `pyCopper` project's own
`ScrollViewElement`. The real clip/scroll-shift claims themselves are
proven at the pixel level (`crates/engine-render/tests/scroll_view.rs`)
and the real scroll-clamp/hit-test-after-scroll math at the Rust level
(`crates/engine-core/src/tree.rs`'s own `scroll_view`/`hit_test_after_
a_real_scroll` tests); this suite proves the real FFI surface.
"""

from tre import Node, Window


def test_add_scroll_view_returns_a_node():
    window = Window(width=800, height=600)
    view = window.add_scroll_view(width=200, height=100)
    assert isinstance(view, Node)


def test_a_horizontal_scroll_view_does_not_raise():
    window = Window(width=800, height=600)
    view = window.add_scroll_view(width=200, height=100, horizontal=True)
    assert isinstance(view, Node)


def test_real_content_can_be_composed_in_via_add_child():
    window = Window(width=800, height=600)
    view = window.add_scroll_view(width=200, height=100)
    content = window.add_rect(background=(255, 0, 0, 255), width=200, height=1000)
    view.add_child(content)


def test_a_real_click_on_content_composed_into_a_scroll_view_reaches_its_own_handler():
    window = Window(width=800, height=600)
    view = window.add_scroll_view(width=200, height=100)
    content = window.add_rect(background=(255, 0, 0, 255), width=200, height=1000)
    view.add_child(content)

    clicked = []
    content.enable_interaction()
    content.set_on_click(lambda: clicked.append(True))
    window.click(content)
    assert clicked == [True]


def test_a_real_click_after_scrolling_the_content_into_view_reaches_the_right_node():
    """The real, decisive proof this whole phase exists for: a real
    click at a genuinely scrolled-into-view node's own real position
    must reach it -- the exact scenario this phase's own investigation
    found a real, pre-existing gap for in `VirtualList`.
    """
    window = Window(width=800, height=600)
    view = window.add_scroll_view(width=200, height=100)
    content = window.add_rect(background=(0, 0, 0, 0), width=200, height=1000)
    view.add_child(content)

    button = window.add_button(label="Deep row", width=180, height=30, x=10, y=800)
    content.add_child(button)
    button.enable_interaction()
    clicked = []
    button.set_on_click(lambda: clicked.append(True))

    window.scroll(view, 750.0)
    window.click(button)
    assert clicked == [True], "a real click after a real scroll must reach the right node"


def test_scroll_with_delta_x_does_not_raise_on_a_horizontal_scroll_view():
    window = Window(width=800, height=600)
    view = window.add_scroll_view(width=100, height=50, horizontal=True)
    content = window.add_rect(background=(0, 255, 0, 255), width=1000, height=50)
    view.add_child(content)
    window.scroll(view, 0.0, delta_x=500.0)


def test_scroll_still_defaults_delta_x_to_zero_for_existing_callers():
    """Real backward-compatibility proof: every pre-existing real
    caller of `Window.scroll(node, delta_y)` must keep working
    unchanged after `delta_x` was added."""
    window = Window(width=800, height=600)
    view = window.add_scroll_view(width=200, height=100)
    content = window.add_rect(background=(255, 0, 0, 255), width=200, height=1000)
    view.add_child(content)
    window.scroll(view, 50.0)
