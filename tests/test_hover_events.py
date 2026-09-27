"""`pointer_enter`/`pointer_leave` listeners driven by simulated pointer moves:
entering a node, leaving it for a sibling, the Event's window position, and
hovering a node with no listener.
"""

from tre import Window
from helpers import add


def test_pointer_enter_fires_when_the_pointer_arrives_on_the_node():
    window = Window(width=120, height=60)
    button = add(window, "box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)

    calls = []
    button.on("pointer_enter", lambda: calls.append("entered"))

    window.simulate("pointer_move", node=button)

    assert calls == ["entered"]


def test_pointer_leave_fires_when_the_pointer_moves_to_a_sibling():
    window = Window(width=120, height=60)
    a = add(window, "box", fill=(0xFF, 0x00, 0x00, 0xFF), width=40, height=40)
    b = add(window, "box", fill=(0x00, 0x00, 0xFF, 0xFF), width=40, height=40)

    calls = []
    a.on("pointer_leave", lambda: calls.append("a exited"))
    b.on("pointer_enter", lambda: calls.append("b entered"))

    window.simulate("pointer_move", node=a)
    assert calls == []  # first entry onto `a` -- no leave yet, and `a` has no enter listener

    window.simulate("pointer_move", node=b)
    assert calls == ["a exited", "b entered"]


def test_pointer_enter_and_leave_give_a_one_arg_listener_the_position():
    """Both carry the pointer's window position; `button`, `old_value`
    and `new_value` read `None`."""
    window = Window(width=120, height=60)
    a = add(window, "box", fill=(0xFF, 0x00, 0x00, 0xFF), width=40, height=40, position="absolute", x=0, y=0)
    b = add(window, "box", fill=(0x00, 0x00, 0xFF, 0xFF), width=40, height=40, position="absolute", x=60, y=0)

    events = []
    a.on("pointer_leave", lambda event: events.append(event))
    b.on("pointer_enter", lambda event: events.append(event))

    window.simulate("pointer_move", node=a)
    window.simulate("pointer_move", node=b)

    assert len(events) == 2
    exit_event, enter_event = events
    # Both fire from the one move onto `b`, so the leave event's position
    # is where the pointer now is (over `b`), not `a`'s center.
    assert exit_event.type == "pointer_leave"
    assert (exit_event.window_x, exit_event.window_y) == (80.0, 20.0)
    assert enter_event.type == "pointer_enter"
    assert (enter_event.window_x, enter_event.window_y) == (80.0, 20.0)  # b's center
    for event in events:
        assert event.button is None
        assert event.old_value is None
        assert event.new_value is None


def test_hovering_a_node_with_no_listener_is_a_safe_no_op():
    window = Window(width=120, height=60)
    button = add(window, "box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)

    window.simulate("pointer_move", node=button)  # must not raise



