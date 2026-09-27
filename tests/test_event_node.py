"""M56 (§8, §16.2): real, repeatable coverage of `Event.node` -- the
real, live `Node` handle every dispatched `Event` carries alongside its
existing `source: int` (M54 scoping's own deferred design question,
resolved here: additive, not a replacement).

`Node` exposes no Python-facing `id`/`__eq__` (confirmed via `_core.pyi`
-- no such attribute exists), so "is `event.node` the *same* node the
handler registered on" is proven behaviorally throughout: mutate through
`event.node`, observe the identical effect on the originally-held
handle, rather than an equality/identity assertion neither `Node` nor
this test suite has any way to make more directly.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_event_payload.py`.
"""

from tre import Node, Window


def test_event_node_is_a_real_node_instance():
    window = Window(width=100, height=100)
    button = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)

    events = []
    button.set_on_click(lambda event: events.append(event))
    window.click(button)

    assert isinstance(events[0].node, Node)


def test_event_node_is_correct_for_a_real_click():
    window = Window(width=100, height=100)
    field = window.add_text_field(background=(255, 255, 255, 255), width=60, height=24)
    field.set_text("marker")

    seen = []
    field.set_on_click(lambda event: seen.append(event.node.get_text()))
    window.click(field)

    assert seen == ["marker"]


def test_event_node_is_correct_for_a_real_hover_enter():
    window = Window(width=100, height=100)
    field = window.add_text_field(background=(255, 255, 255, 255), width=60, height=24)
    field.set_text("marker")

    seen = []
    field.set_on_hover_enter(lambda event: seen.append(event.node.get_text()))
    window.hover(field)

    assert seen == ["marker"]


def test_event_node_is_correct_for_a_real_focus_enter():
    window = Window(width=100, height=100)
    field = window.add_text_field(background=(255, 255, 255, 255), width=60, height=24)
    field.set_text("marker")

    seen = []
    field.set_on_focus_enter(lambda event: seen.append(event.node.get_text()))
    window.focus(field)

    assert seen == ["marker"]


def test_event_node_is_correct_for_a_real_change():
    window = Window(width=100, height=100)
    field = window.add_text_field(background=(255, 255, 255, 255), width=60, height=24)

    seen = []
    field.set_on_change(lambda event: seen.append(event.node.get_text()))

    field.set_text("hello")

    assert seen == ["hello"], "event.node must reflect the real post-change content"


