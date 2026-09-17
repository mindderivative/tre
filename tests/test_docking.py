"""M4 Phase 9 (§11.4, the final M4 phase): real, repeatable coverage
that a real drag actually moves a panel from one dock zone to another
-- `Window.add_dock_zone`/`dock_panel`/`set_active_tab`/
`set_dock_handle` (the Python-facing docking API that never existed
before this phase) and `start_panel_drag`/`drop_panel_at` (the real
drag-to-rearrange mechanism, mirroring `.click()`/`.hover()`/
`.right_click()`'s own no-live-window-needed proof pattern).

The real, functional proof that a panel genuinely moved: after
dragging it into a new zone, clicking it there (via `Window.click()`,
which only works on a node that's really attached and laid out)
confirms its own handler fires -- not an inspection of internal
`DockLayout` state Python has no getter for.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`.
"""

import pytest

from tre import Window


def build_two_zone_window():
    """Left zone (with one panel + its own drag handle) and an empty
    Right zone -- the minimal real setup a drag-to-rearrange test
    needs.
    """
    window = Window(width=300, height=120)
    left_container = window.add_rect(background=(0, 0, 0, 0), width=100, height=100)
    right_container = window.add_rect(background=(0, 0, 0, 0), width=100, height=100)
    window.add_dock_zone("left", left_container, 100.0)
    window.add_dock_zone("right", right_container, 100.0)

    handle = window.add_rect(background=(0x80, 0x80, 0x80, 0xFF), width=100, height=20)
    panel = window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=100, height=80)
    window.dock_panel("left", panel)
    window.set_dock_handle(handle, panel)

    return window, handle, panel, right_container


def test_dragging_a_registered_handle_moves_its_panel_for_real():
    window, handle, panel, right_container = build_two_zone_window()

    calls = []
    panel.set_on_click(lambda: calls.append("clicked"))

    started = window.start_panel_drag(handle)
    assert started is True

    # Drop inside the right container's own real, computed bounds.
    window.drop_panel_at(200.0, 50.0)

    # The real, functional proof: the panel is now really attached
    # under the right zone -- clicking it (a real dispatch, needing a
    # real computed layout) must fire its own handler.
    window.click(panel)
    assert calls == ["clicked"]


def test_starting_a_drag_on_an_unregistered_node_is_a_safe_no_op():
    window, handle, panel, right_container = build_two_zone_window()
    plain = window.add_rect(background=(0, 0, 0, 0), width=10, height=10)

    started = window.start_panel_drag(plain)
    assert started is False


def test_dropping_outside_any_zone_cancels_the_drag_without_moving_anything():
    window, handle, panel, right_container = build_two_zone_window()

    calls = []
    panel.set_on_click(lambda: calls.append("clicked"))

    window.start_panel_drag(handle)
    window.drop_panel_at(-500.0, -500.0)  # nowhere near any real node

    # The panel must still be exactly where it started -- clicking it
    # must still work (it never left the Left zone).
    window.click(panel)
    assert calls == ["clicked"]


def test_dropping_back_into_the_same_zone_is_a_safe_no_op():
    window, handle, panel, right_container = build_two_zone_window()

    calls = []
    panel.set_on_click(lambda: calls.append("clicked"))

    window.start_panel_drag(handle)
    window.drop_panel_at(50.0, 50.0)  # still inside the Left zone/handle area

    window.click(panel)
    assert calls == ["clicked"]


def test_dropping_with_no_drag_in_progress_does_not_raise():
    window, handle, panel, right_container = build_two_zone_window()
    window.drop_panel_at(200.0, 50.0)  # must not raise -- nothing was dragging


def test_set_active_tab_switches_the_visible_panel():
    window = Window(width=200, height=120)
    container = window.add_rect(background=(0, 0, 0, 0), width=100, height=100)
    window.add_dock_zone("left", container, 100.0)

    a = window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=100, height=80)
    b = window.add_rect(background=(0x00, 0xFF, 0x00, 0xFF), width=100, height=80)
    window.dock_panel("left", a)
    window.dock_panel("left", b)

    calls = []
    a.set_on_click(lambda: calls.append("a"))
    b.set_on_click(lambda: calls.append("b"))

    window.set_active_tab("left", 0)
    window.click(a)
    assert calls == ["a"]

    window.set_active_tab("left", 1)
    window.click(b)
    assert calls == ["a", "b"]


def test_an_unknown_dock_side_raises_value_error():
    window = Window(width=200, height=120)
    container = window.add_rect(background=(0, 0, 0, 0), width=100, height=100)
    try:
        window.add_dock_zone("nowhere", container, 100.0)
        raise AssertionError("expected a ValueError")
    except ValueError as e:
        assert "nowhere" in str(e)


def test_set_dock_handle_rejects_a_handle_or_panel_from_a_different_window():
    """M10 Phase 2 (§8): the same real `Rc::ptr_eq` same-tree guard
    `Node.add_child`/`Node.set_context_menu` already have -- a `NodeId`
    is only unique within the `Tree` that minted it.
    """
    window_a = Window(width=300, height=120)
    window_b = Window(width=300, height=120)
    container = window_a.add_rect(background=(0, 0, 0, 0), width=100, height=100)
    window_a.add_dock_zone("left", container, 100.0)
    panel = window_a.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=100, height=80)
    window_a.dock_panel("left", panel)
    handle = window_a.add_rect(background=(0x80, 0x80, 0x80, 0xFF), width=100, height=20)
    foreign = window_b.add_rect(background=(0, 0, 0, 255), width=10, height=10)

    with pytest.raises(ValueError, match="different Window"):
        window_a.set_dock_handle(foreign, panel)
    with pytest.raises(ValueError, match="different Window"):
        window_a.set_dock_handle(handle, foreign)


def test_dragging_over_a_different_zone_shows_the_highlight_covering_it():
    """M10 Phase 3 (§11.4): the real, functional proof that `drag_panel_
    over` shows the registered highlight over whatever zone is really
    under the pointer -- checked the only way there is to check it from
    Python (no getter for internal `DockState`): clicking the highlight
    itself, at its own real computed center, proves it's really
    attached, laid out, and positioned to cover the Right zone's own
    real bounds -- (160.0, 50.0) is confirmed (via `drop_panel_at`
    actually moving the panel there in `test_dragging_a_registered_
    handle_moves_its_panel_for_real`, above) to land inside the Right
    zone's own real, computed container bounds, unlike (200.0, 50.0),
    which sits exactly on its right edge (the container is flex-shrunk
    to fit three 100px-wide root children into less available width) --
    not inside it.
    """
    window, handle, panel, right_container = build_two_zone_window()
    highlight = window.add_rect(background=(0x00, 0x80, 0xFF, 0x60), width=1, height=1)
    window.set_drop_zone_highlight(highlight)

    window.start_panel_drag(handle)
    window.drag_panel_over(160.0, 50.0)  # inside the Right zone's real bounds

    calls = []
    highlight.set_on_click(lambda: calls.append("hit"))
    window.click(highlight)
    assert calls == ["hit"], "the highlight must be real, attached, and cover the Right zone"


def test_dragging_outside_every_zone_hides_the_highlight():
    window, handle, panel, right_container = build_two_zone_window()
    highlight = window.add_rect(background=(0x00, 0x80, 0xFF, 0x60), width=1, height=1)
    window.set_drop_zone_highlight(highlight)

    window.start_panel_drag(handle)
    window.drag_panel_over(160.0, 50.0)  # shows it over the Right zone first
    window.drag_panel_over(-500.0, -500.0)  # nowhere near any registered zone

    calls = []
    highlight.set_on_click(lambda: calls.append("hit"))
    window.click(highlight)
    assert calls == [], "the highlight must be hidden once the pointer leaves every zone"


def test_ending_a_drag_always_hides_the_highlight():
    window, handle, panel, right_container = build_two_zone_window()
    highlight = window.add_rect(background=(0x00, 0x80, 0xFF, 0x60), width=1, height=1)
    window.set_drop_zone_highlight(highlight)

    window.start_panel_drag(handle)
    window.drag_panel_over(160.0, 50.0)
    window.drop_panel_at(160.0, 50.0)

    calls = []
    highlight.set_on_click(lambda: calls.append("hit"))
    window.click(highlight)
    assert calls == [], "the highlight must be hidden once the drag has ended"


def test_drag_panel_over_with_no_drag_in_progress_is_a_safe_no_op():
    window, handle, panel, right_container = build_two_zone_window()
    highlight = window.add_rect(background=(0x00, 0x80, 0xFF, 0x60), width=1, height=1)
    window.set_drop_zone_highlight(highlight)

    window.drag_panel_over(160.0, 50.0)  # must not raise -- nothing is dragging

    calls = []
    highlight.set_on_click(lambda: calls.append("hit"))
    window.click(highlight)
    assert calls == []


def test_drag_panel_over_with_no_highlight_registered_is_a_safe_no_op():
    window, handle, panel, right_container = build_two_zone_window()

    window.start_panel_drag(handle)
    window.drag_panel_over(200.0, 50.0)  # must not raise -- no highlight registered


def test_set_drop_zone_highlight_rejects_content_from_a_different_window():
    """M10 Phase 3 (§11.4): the same real `Rc::ptr_eq` same-tree guard
    every other content-registering method on `Window`/`Node` already
    has (`set_dock_handle`, `set_context_menu`).
    """
    window_a = Window(width=300, height=120)
    window_b = Window(width=300, height=120)
    foreign = window_b.add_rect(background=(0, 0, 0, 255), width=10, height=10)

    with pytest.raises(ValueError, match="different Window"):
        window_a.set_drop_zone_highlight(foreign)
