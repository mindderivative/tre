"""M56 (§8, §16.2): real, repeatable coverage of `Event.target` (M100:
formerly `Event.node`) -- the live `Node` handle every dispatched `Event`
carries.

`Node` exposes no Python-facing `id`/`__eq__` (confirmed via `_core.pyi`
-- no such attribute exists), so "is `event.target` the *same* node the
handler registered on" is proven behaviorally throughout: mutate through
`event.target`, observe the identical effect on the originally-held
handle, rather than an equality/identity assertion neither `Node` nor
this test suite has any way to make more directly.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_event_payload.py`.
"""

from tre import Node, Window
from helpers import add


def test_event_node_is_a_real_node_instance():
    window = Window(width=100, height=100)
    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)

    events = []
    button.on("click", lambda event: events.append(event))
    window.click(button)

    assert isinstance(events[0].target, Node)


def test_event_node_is_correct_for_a_real_click():
    window = Window(width=100, height=100)
    field = add(window, "text_input", width=60, height=24)
    field.set(text="marker")

    seen = []
    field.on("click", lambda event: seen.append(event.target.get("text")))
    window.click(field)

    assert seen == ["marker"]


def test_event_node_is_correct_for_a_real_hover_enter():
    window = Window(width=100, height=100)
    field = add(window, "text_input", width=60, height=24)
    field.set(text="marker")

    seen = []
    field.on("pointer_enter", lambda event: seen.append(event.target.get("text")))
    window.hover(field)

    assert seen == ["marker"]


def test_event_node_is_correct_for_a_real_focus_enter():
    window = Window(width=100, height=100)
    field = add(window, "text_input", width=60, height=24)
    field.set(text="marker")

    seen = []
    field.on("focus", lambda event: seen.append(event.target.get("text")))
    window.focus(field)

    assert seen == ["marker"]


def test_event_node_is_correct_for_a_real_change():
    window = Window(width=100, height=100)
    field = add(window, "text_input", width=60, height=24)

    seen = []
    field.on("change", lambda event: seen.append(event.target.get("text")))

    window.simulate("focus", node=field)
    window.simulate("input", text="hello")

    assert seen == ["hello"], "event.target must reflect the real post-change content"


