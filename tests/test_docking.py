"""Docking (§11.4), reduced to D10's bare bones in M99: zones, docked
panels, the active panel, and a drag that reports where it is and where
it ended. The framework draws the handle and the target highlight.

Drags are driven the way a real pointer drives them -- `window.simulate`
pointer events through the same `process_input` pipeline `App.run()`
uses -- so these tests cover the live path, not a test-only one.
"""

import pytest

from tre import Window
from helpers import add


def two_zones():
    """A left zone holding one panel, and an empty right zone."""
    window = Window(width=300, height=120)
    left = add(window, "box", fill=(0, 0, 0, 0), width=100, height=100)
    right = add(window, "box", fill=(0, 0, 0, 0), width=100, height=100)
    window.add_dock_zone("left", left, 100.0)
    window.add_dock_zone("right", right, 100.0)
    panel = add(window, "box", fill=(0xFF, 0x00, 0x00, 0xFF), width=100, height=80)
    window.dock_panel("left", panel)
    return window, left, right, panel


def record(window):
    """Records every `dock_target`/`dock_drop` event's fields."""
    seen = []
    window.on("dock_target", lambda e: seen.append(("target", e.side)))
    window.on("dock_drop", lambda e: seen.append(("drop", e.side, e.panel)))
    return seen


def test_a_drag_reports_the_zone_under_the_pointer_and_moves_the_panel():
    window, left, right, panel = two_zones()
    seen = record(window)

    window.start_panel_drag(panel)
    window.simulate("pointer_move", node=right)
    window.simulate("pointer_up", node=right)

    assert seen == [("target", "right"), ("drop", "right", panel)]
    assert right.children() == [panel]
    assert left.children() == []
    # Attached and laid out in its new place: a click lands on it.
    calls = []
    panel.on("click", lambda: calls.append("clicked"))
    window.simulate("click", node=panel)
    assert calls == ["clicked"]


def test_dock_target_fires_only_when_the_zone_changes():
    window, left, right, panel = two_zones()
    seen = record(window)

    window.start_panel_drag(panel)
    window.simulate("pointer_move", node=left)
    window.simulate("pointer_move", node=left, x=10, y=10)  # same zone: no event
    window.simulate("pointer_move", node=right)
    window.simulate("pointer_move", x=-500, y=-500)  # over no zone

    assert seen == [("target", "left"), ("target", "right"), ("target", None)]


def test_dropping_outside_every_zone_leaves_the_panel_where_it_was():
    window, left, right, panel = two_zones()
    seen = record(window)

    window.start_panel_drag(panel)
    window.simulate("pointer_up", x=-500, y=-500)

    assert seen == [("drop", None, panel)]
    assert left.children() == [panel]


def test_dropping_back_into_the_same_zone_changes_nothing():
    window, left, right, panel = two_zones()

    window.start_panel_drag(panel)
    window.simulate("pointer_up", node=left)

    assert left.children() == [panel]
    assert right.children() == []


def test_without_a_drag_pointer_events_report_nothing():
    window, left, right, panel = two_zones()
    seen = record(window)

    window.simulate("pointer_move", node=right)
    window.simulate("pointer_up", node=right)

    assert seen == []
    assert left.children() == [panel]


def test_a_secondary_release_does_not_end_the_drag():
    window, left, right, panel = two_zones()
    seen = record(window)

    window.start_panel_drag(panel)
    window.simulate("pointer_up", node=right, button="secondary")
    assert seen == []
    window.simulate("pointer_up", node=right)
    assert seen == [("drop", "right", panel)]


def test_a_handle_starts_the_drag_from_its_own_pointer_down():
    """How a framework wires its handle: `start_panel_drag` from the
    handle's `pointer_down`, then the press-move-release a user makes."""
    window, left, right, panel = two_zones()
    handle = add(window, "box", fill=(0x80, 0x80, 0x80, 0xFF), width=20, height=20)
    handle.on("pointer_down", lambda: window.start_panel_drag(panel))
    seen = record(window)

    window.simulate("pointer_down", node=handle)
    window.simulate("pointer_move", node=right)
    window.simulate("pointer_up", node=right)

    assert seen == [("target", "right"), ("drop", "right", panel)]
    assert right.children() == [panel]


def test_the_drag_ends_once_dropped():
    window, left, right, panel = two_zones()
    seen = record(window)

    window.start_panel_drag(panel)
    window.simulate("pointer_up", node=right)
    window.simulate("pointer_move", node=left)
    window.simulate("pointer_up", node=left)

    assert len(seen) == 1
    assert right.children() == [panel]


def test_moving_the_active_panel_out_shows_the_next_one():
    window, left, right, a = two_zones()
    b = add(window, "box", fill=(0x00, 0xFF, 0x00, 0xFF), width=100, height=80)
    window.dock_panel("left", b)  # b is now the active panel

    window.start_panel_drag(b)
    window.simulate("pointer_up", node=right)

    assert left.children() == [a]
    assert right.children() == [b]


def test_start_panel_drag_needs_a_docked_panel():
    window, left, right, panel = two_zones()
    plain = add(window, "box", fill=(0, 0, 0, 0), width=10, height=10)

    with pytest.raises(ValueError, match="isn't a docked panel"):
        window.start_panel_drag(plain)


def test_start_panel_drag_rejects_a_node_from_another_window():
    window, left, right, panel = two_zones()
    other = Window(width=100, height=100)
    foreign = add(other, "box", fill=(0, 0, 0, 255), width=10, height=10)

    with pytest.raises(ValueError, match="different Window"):
        window.start_panel_drag(foreign)


def test_set_active_panel_switches_the_visible_panel():
    window = Window(width=200, height=120)
    container = add(window, "box", fill=(0, 0, 0, 0), width=100, height=100)
    window.add_dock_zone("left", container, 100.0)
    a = add(window, "box", fill=(0xFF, 0x00, 0x00, 0xFF), width=100, height=80)
    b = add(window, "box", fill=(0x00, 0xFF, 0x00, 0xFF), width=100, height=80)
    window.dock_panel("left", a)
    window.dock_panel("left", b)

    window.set_active_panel("left", 0)
    assert container.children() == [a]
    window.set_active_panel("left", 1)
    assert container.children() == [b]


def test_set_active_panel_rejects_an_index_out_of_range():
    window, left, right, panel = two_zones()
    with pytest.raises(ValueError, match="out of range"):
        window.set_active_panel("left", 1)


def test_an_unknown_dock_side_raises_value_error():
    window = Window(width=200, height=120)
    container = add(window, "box", fill=(0, 0, 0, 0), width=100, height=100)
    with pytest.raises(ValueError, match="nowhere"):
        window.add_dock_zone("nowhere", container, 100.0)


def test_the_dock_events_are_window_events():
    window = Window(width=100, height=100)
    window.on("dock_target", lambda: None)
    window.on("dock_drop", lambda: None)
    window.off("dock_target")
    window.off("dock_drop")
