"""`Event.target`: the `Node` a click, `pointer_enter`, `focus`, or `change`
listener is called for, read back through the handle it carries.
"""

from tre import Node, Window
from helpers import add


def test_event_target_is_a_node():
    window = Window(width=100, height=100)
    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)

    events = []
    button.on("click", lambda event: events.append(event))
    window.simulate("click", node=button)

    assert isinstance(events[0].target, Node)


def test_event_target_is_the_clicked_node():
    window = Window(width=100, height=100)
    field = add(window, "text_input", width=60, height=24)
    field.set(text="marker")

    seen = []
    field.on("click", lambda event: seen.append(event.target.get("text")))
    window.simulate("click", node=field)

    assert seen == ["marker"]


def test_event_target_is_the_entered_node():
    window = Window(width=100, height=100)
    field = add(window, "text_input", width=60, height=24)
    field.set(text="marker")

    seen = []
    field.on("pointer_enter", lambda event: seen.append(event.target.get("text")))
    window.simulate("pointer_move", node=field)

    assert seen == ["marker"]


def test_event_target_is_the_focused_node():
    window = Window(width=100, height=100)
    field = add(window, "text_input", width=60, height=24)
    field.set(text="marker")

    seen = []
    field.on("focus", lambda event: seen.append(event.target.get("text")))
    window.simulate("focus", node=field)

    assert seen == ["marker"]


def test_event_target_is_the_changed_node():
    window = Window(width=100, height=100)
    field = add(window, "text_input", width=60, height=24)

    seen = []
    field.on("change", lambda event: seen.append(event.target.get("text")))

    window.simulate("focus", node=field)
    window.simulate("input", text="hello")

    assert seen == ["hello"], "event.target must reflect the post-change text"


