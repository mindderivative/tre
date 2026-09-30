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


# --- 0.4.2 M12 (issue #24): keyboard scrolling ---------------------------------


def scroller(window, orientation="vertical", length=1000, parent=None):
    """A 200x100 scroll view over one content box `length` long, holding a
    focusable item near the top."""
    vertical = orientation == "vertical"
    view = window.create("scroll_view", width=200, height=100, orientation=orientation)
    content = window.create(
        "box",
        width=200 if vertical else length,
        height=length if vertical else 100,
        flex_shrink=0,
        flex_direction="vertical" if vertical else "horizontal",
    )
    item = window.create("box", width=50, height=20, focusable=True)
    content.add_child(item)
    view.add_child(content)
    (parent or window.root).add_child(view)
    return view, content, item


def offset(view):
    return view.get("scroll_offset")


def test_keys_scroll_the_scroll_view_around_the_focused_node():
    window = Window(width=800, height=600)
    view, _, item = scroller(window)
    item.focus()
    for key, expected in [
        ("arrow_down", 40.0),
        ("page_down", 140.0),
        ("arrow_up", 100.0),
        ("end", 900.0),
        ("page_down", 900.0),  # already at the end
        ("home", 0.0),
        ("arrow_up", 0.0),  # already at the top
    ]:
        window.simulate("key_down", key=key)
        assert offset(view) == expected, key


def test_a_text_input_keeps_its_arrows_but_not_page_keys():
    window = Window(width=800, height=600)
    view, content, _ = scroller(window)
    field = window.create("text_input", text="hello", width=150, height=24)
    content.add_child(field)
    field.focus()
    window.simulate("key_down", key="arrow_down")
    window.simulate("key_down", key="end")
    assert offset(view) == 0.0, "the text input used them"
    window.simulate("key_down", key="page_down")
    assert offset(view) == 100.0


def test_a_key_down_listener_on_the_way_keeps_the_keys():
    window = Window(width=800, height=600)
    view, _, item = scroller(window)
    item.on("key_down", lambda: None)
    item.focus()
    window.simulate("key_down", key="arrow_down")
    assert offset(view) == 0.0, "the item handles its own keys"

    other_view, _, other_item = scroller(window)
    other_view.on("key_down", lambda: None)
    other_item.focus()
    window.simulate("key_down", key="page_down")
    assert offset(other_view) == 0.0, "the scroll view's own listener handles them"


def test_each_key_scrolls_the_nearest_scroll_view_along_its_axis():
    window = Window(width=800, height=600)
    page, page_content, _ = scroller(window)
    carousel, _, card = scroller(window, "horizontal", parent=page_content)
    card.focus()
    window.simulate("key_down", key="arrow_right")
    assert (offset(carousel), offset(page)) == (40.0, 0.0)
    window.simulate("key_down", key="arrow_down")
    assert (offset(carousel), offset(page)) == (40.0, 40.0), "up/down skip the carousel"


def test_a_focused_scroll_view_scrolls_itself():
    window = Window(width=800, height=600)
    view, _, _ = scroller(window)
    view.set(focusable=True)
    view.focus()
    window.simulate("key_down", key="page_down")
    assert offset(view) == 100.0


def test_keys_scroll_nothing_without_focus():
    window = Window(width=800, height=600)
    view, _, _ = scroller(window)
    window.simulate("key_down", key="page_down")
    assert offset(view) == 0.0


# 0.4.3 M14: a key with Ctrl, Alt, or Meta held is a shortcut, not a scroll.
@pytest.mark.parametrize("modifier", ["ctrl", "alt", "meta"])
@pytest.mark.parametrize("key", ["arrow_down", "page_down", "end"])
def test_a_shortcut_modifier_keeps_keys_from_scrolling(modifier, key):
    window = Window(width=800, height=600)
    view, _, item = scroller(window)
    item.focus()
    window.simulate("key_down", key=key, **{modifier: True})
    assert offset(view) == 0.0
    window.simulate("key_down", key=key)  # released: the same key scrolls
    assert offset(view) > 0.0


def test_shift_still_scrolls():
    window = Window(width=800, height=600)
    view, _, item = scroller(window)
    item.focus()
    window.simulate("key_down", key="page_down", shift=True)
    assert offset(view) == 100.0


# --- 0.4.2 M12 (issue #24): scroll_into_view and revealing focus -------------------


def item_at(window, content, y, height=20, focusable=True):
    """A box placed at `y` in `content` (absolute, so rows don't shrink)."""
    node = window.create(
        "box", position="absolute", x=0, y=y, width=50, height=height, focusable=focusable
    )
    content.add_child(node)
    return node


def test_scroll_into_view_scrolls_just_enough():
    window = Window(width=800, height=600)
    view, content, _ = scroller(window)
    below = item_at(window, content, 500)
    below.scroll_into_view()
    assert offset(view) == 420.0, "its bottom (520) at the viewport's bottom"
    below.scroll_into_view()
    assert offset(view) == 420.0, "already in view: nothing moves"
    above = item_at(window, content, 300)
    above.scroll_into_view()
    assert offset(view) == 300.0, "its top at the viewport's top"


def test_the_accessibility_action_scrolls_into_view():
    window = Window(width=800, height=600)
    view, content, _ = scroller(window)
    below = item_at(window, content, 700)
    heard = []
    below.on("a11y_action", lambda e: heard.append(e.action))
    window.simulate("a11y_action", node=below, action="scroll_into_view")
    assert heard == ["scroll_into_view"]
    assert offset(view) == 620.0


def test_focus_reveals_the_focused_node():
    window = Window(width=800, height=600)
    view, content, first = scroller(window)
    far = item_at(window, content, 800)
    far.focus()
    assert offset(view) == 720.0
    first.focus()
    assert offset(view) == 0.0


def test_tab_reveals_the_next_node():
    window = Window(width=800, height=600)
    view, content, first = scroller(window)
    item_at(window, content, 600)
    first.focus()
    window.simulate("key_down", key="tab")
    assert offset(view) == 520.0


def test_nested_scroll_views_reveal_together():
    window = Window(width=800, height=600)
    outer, outer_content, _ = scroller(window)
    inner = window.create("scroll_view", width=200, height=100)
    inner.set(position="absolute", x=0, y=400)
    inner_content = window.create("box", width=200, height=1000, flex_shrink=0)
    inner.add_child(inner_content)
    outer_content.add_child(inner)
    deep = item_at(window, inner_content, 300)
    deep.scroll_into_view()
    assert offset(inner) == 220.0, "the inner view shows it"
    assert offset(outer) == 400.0, "the outer view shows the inner one's view of it"


# --- 0.4.2 M12 (issue #24): the scroll event --------------------------------------


def listen(view):
    seen = []
    view.on("scroll", lambda e: seen.append((e.old_value, e.new_value)))
    return seen


def test_scroll_fires_for_every_cause_with_old_and_new_offsets():
    window = Window(width=800, height=600)
    view, content, item = scroller(window)
    seen = listen(view)

    item.focus()  # already in view: no scroll
    window.simulate("wheel", node=view, delta_y=50.0)
    window.simulate("key_down", key="arrow_down")
    item_at(window, content, 700).scroll_into_view()
    view.set(scroll_offset=0.0)
    assert seen[0][0] == 0.0 and seen[0][1] > 0.0, "wheel"
    assert [new for _, new in seen[1:]] == [seen[0][1] + 40.0, 620.0, 0.0]
    assert all(old == prev_new for (old, _), (_, prev_new) in zip(seen[1:], seen)), (
        "each old value is the last one reported"
    )


def test_an_animated_scroll_fires_as_it_moves():
    window = Window(width=800, height=600)
    view, _, _ = scroller(window)
    seen = listen(view)
    window.advance(0)
    view.animate("scroll_offset", 300.0, 100)
    window.advance(50)
    window.advance(50)
    assert len(seen) == 2
    assert 0.0 < seen[0][1] < 300.0 and seen[1] == (seen[0][1], 300.0)


def test_scroll_is_quiet_when_nothing_moves_and_does_not_bubble():
    window = Window(width=800, height=600)
    view, _, item = scroller(window)
    seen = listen(view)
    root_heard = []
    window.root.on("scroll", lambda e: root_heard.append(e))
    item.focus()
    window.simulate("key_down", key="arrow_up")  # already at the top
    window.advance(16)
    assert seen == []
    window.simulate("key_down", key="page_down")
    assert seen == [(0.0, 100.0)]
    assert root_heard == [], "scroll stays on the scroll view"


# --- 0.4.3 M15: scroll_offset clamped when it's set -------------------------------


def test_setting_past_the_end_clamps_at_once_with_one_event():
    window = Window(width=800, height=600)
    view, _, _ = scroller(window)  # 1000 of content in a 100 view: 900 of travel
    seen = listen(view)
    view.set(scroll_offset=5000.0)
    assert offset(view) == 900.0, "reads back clamped, before any frame"
    window.advance(16)
    assert seen == [(0.0, 900.0)], "one event, with the clamped value"


def test_setting_inside_the_range_is_kept():
    window = Window(width=800, height=600)
    view, _, _ = scroller(window)
    view.set(scroll_offset=300.0)
    assert offset(view) == 300.0


def test_shrinking_content_clamps_the_offset():
    window = Window(width=800, height=600)
    view, content, _ = scroller(window)
    view.set(scroll_offset=800.0)
    seen = listen(view)
    content.set(height=500)
    window.advance(16)
    assert offset(view) == 400.0
    assert seen == [(800.0, 400.0)]


def test_an_animation_past_the_end_eases_to_the_end():
    window = Window(width=800, height=600)
    view, _, _ = scroller(window)
    window.advance(0)
    view.animate("scroll_offset", 5000.0, 100, easing="linear")
    window.advance(50)
    assert offset(view) == 450.0, "halfway to 900, not clamped from halfway to 5000"
    window.advance(50)
    assert offset(view) == 900.0
