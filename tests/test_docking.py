"""Docking: zones, docked panels, the active panel, and a drag that reports
where it is and where it ended. The framework draws the handle and the
target highlight. Drags are driven by `window.simulate` pointer events,
through the same input pipeline `App.run()` uses.
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


# -- dock_panel on a docked panel moves it (issue #14) -------------------------


def labels(zone):
    return [n.get("label") for n in zone.children()]


def two_panels_left():
    """Issue #14's setup: A and B docked left, B shown; an empty right zone."""
    window = Window(width=600, height=300)
    left = add(window, "box", width=200, height=200)
    right = add(window, "box", width=200, height=200)
    window.add_dock_zone("left", left, 200.0)
    window.add_dock_zone("right", right, 200.0)
    a = window.create("box", width=10, height=10, label="A")
    b = window.create("box", width=10, height=10, label="B")
    window.dock_panel("left", a)
    window.dock_panel("left", b)
    return window, left, right, a, b


def test_dock_panel_moves_a_panel_docked_in_another_zone():
    window, left, right, a, b = two_panels_left()
    window.dock_panel("right", a)
    assert (labels(left), labels(right)) == (["B"], ["A"])
    assert a.parent() == right and b.parent() == left

    # The old zone lists one panel now: re-showing it never steals A back.
    window.set_active_panel("left", 0)
    assert (labels(left), labels(right)) == (["B"], ["A"])
    with pytest.raises(ValueError, match="out of range"):
        window.set_active_panel("left", 1)


def test_moving_the_shown_panel_shows_the_next_one_left_behind():
    window, left, right, a, b = two_panels_left()
    window.dock_panel("right", b)  # B was shown in the left zone
    assert (labels(left), labels(right)) == (["A"], ["B"])


def test_a_moved_panel_drags_back_from_its_new_zone():
    window, left, right, a, b = two_panels_left()
    window.dock_panel("right", a)
    window.start_panel_drag(a)
    window.simulate("pointer_move", node=left)
    window.simulate("pointer_up", node=left)
    assert labels(left) == ["A"] and labels(right) == []
    window.set_active_panel("left", 0)  # the zone's panels are [B, A]
    assert labels(left) == ["B"] and a.parent() is None


def test_dock_panel_into_its_own_zone_shows_it():
    window, left, right, a, b = two_panels_left()
    window.dock_panel("left", a)
    assert labels(left) == ["A"]
    window.set_active_panel("left", 1)
    assert labels(left) == ["B"], "still two panels, not a duplicate"
    with pytest.raises(ValueError, match="out of range"):
        window.set_active_panel("left", 2)


def test_a_keyboard_move_menu_needs_no_simulated_input():
    """What issue #14 asked for: moving a panel with no drag at all."""
    window, left, right, a, b = two_panels_left()
    for side, zone in (("right", right), ("left", left), ("right", right)):
        window.dock_panel(side, a)
        assert a.parent() == zone
        assert sum(z.children().count(a) for z in (left, right)) == 1


# -- undock_panel takes a panel out of docking (issue #16) ---------------------


def three_panels_left():
    """A, B and C docked left, C shown; an empty right zone."""
    window, left, right, a, b = two_panels_left()
    c = window.create("box", width=10, height=10, label="C")
    window.dock_panel("left", c)
    return window, left, right, a, b, c


def test_undock_panel_takes_it_out_of_its_zone_for_good():
    """Issue #16's own repro: set_active_panel can't bring it back."""
    window, left, right, a, b = two_panels_left()
    window.undock_panel(b)
    assert labels(left) == ["A"] and b.parent() is None
    with pytest.raises(ValueError, match="out of range"):
        window.set_active_panel("left", 1)
    window.set_active_panel("left", 0)
    assert labels(left) == ["A"] and b.parent() is None


def test_undocking_the_shown_panel_shows_the_next_one():
    window, left, right, a, b, c = three_panels_left()
    window.set_active_panel("left", 1)  # B
    window.undock_panel(b)
    assert labels(left) == ["C"]


def test_undocking_the_last_shown_panel_shows_the_previous_one():
    window, left, right, a, b, c = three_panels_left()  # C shown
    window.undock_panel(c)
    assert labels(left) == ["B"]


def test_undocking_an_earlier_panel_keeps_the_shown_one():
    window, left, right, a, b, c = three_panels_left()
    window.set_active_panel("left", 1)  # B
    window.undock_panel(a)
    assert labels(left) == ["B"]
    window.set_active_panel("left", 1)  # indexes shifted down: C
    assert labels(left) == ["C"]


def test_moving_an_earlier_panel_away_keeps_the_shown_one():
    """The same shift through dock_panel's move, which used to show the
    panel after the shown one."""
    window, left, right, a, b, c = three_panels_left()
    window.set_active_panel("left", 1)  # B
    window.dock_panel("right", a)
    assert (labels(left), labels(right)) == (["B"], ["A"])


def test_undocking_a_zones_only_panel_leaves_it_empty():
    window, left, right, panel = two_zones()
    window.undock_panel(panel)
    assert left.children() == [] and panel.parent() is None
    with pytest.raises(ValueError, match="out of range"):
        window.set_active_panel("left", 0)


def test_an_undocked_panel_can_be_docked_again():
    window, left, right, a, b = two_panels_left()
    window.undock_panel(a)
    window.dock_panel("right", a)
    assert (labels(left), labels(right)) == (["B"], ["A"])
    window.start_panel_drag(a)  # docked again, so it drags
    window.simulate("pointer_up", node=left)
    assert labels(left) == ["A"] and labels(right) == []


def test_undocking_the_dragged_panel_cancels_its_drag():
    window, left, right, a, b = two_panels_left()
    seen = record(window)
    window.start_panel_drag(a)
    window.undock_panel(a)
    window.simulate("pointer_move", node=right)
    window.simulate("pointer_up", node=right)
    assert seen == []
    assert labels(right) == [] and a.parent() is None


def test_undock_panel_needs_a_docked_panel():
    window, left, right, a, b = two_panels_left()
    window.undock_panel(a)
    with pytest.raises(ValueError, match="isn't a docked panel"):
        window.undock_panel(a)
    plain = add(window, "box", fill=(0, 0, 0, 0), width=10, height=10)
    with pytest.raises(ValueError, match="isn't a docked panel"):
        window.undock_panel(plain)


def test_undock_panel_rejects_a_node_from_another_window():
    window, left, right, panel = two_zones()
    other = Window(width=100, height=100)
    foreign = add(other, "box", fill=(0, 0, 0, 255), width=10, height=10)
    with pytest.raises(ValueError, match="different Window"):
        window.undock_panel(foreign)


def test_a_destroyed_panel_leaves_its_zone():
    """It used to stay listed, and showing it again panicked."""
    window, left, right, a, b, c = three_panels_left()
    window.set_active_panel("left", 1)  # B
    a.destroy()
    window.set_active_panel("left", 1)  # indexes shifted down: C
    assert labels(left) == ["C"]
    with pytest.raises(ValueError, match="out of range"):
        window.set_active_panel("left", 2)


def test_destroying_the_shown_panel_shows_the_next_one():
    window, left, right, a, b, c = three_panels_left()
    window.set_active_panel("left", 1)  # B
    b.destroy()
    window.dock_panel("right", a)  # any docking call catches up
    assert (labels(left), labels(right)) == (["C"], ["A"])


def test_destroying_the_dragged_panel_ends_its_drag():
    window, left, right, a, b = two_panels_left()
    seen = record(window)
    window.start_panel_drag(a)
    a.destroy()
    window.simulate("pointer_move", node=right)
    window.simulate("pointer_up", node=right)
    assert seen == [] and labels(right) == []
