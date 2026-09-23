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


def test_mutation_through_event_node_is_visible_on_the_originally_registered_handle():
    """The real identity proof this milestone's own investigation
    settled on, given `Node` has no `__eq__`/`id` to compare more
    directly: `event.node` and the `Node` a handler was registered on
    must be two handles onto the exact same underlying engine node, so
    a write through one is immediately visible through the other.
    """
    window = Window(width=100, height=100)
    checkbox = window.add_checkbox(background=(0, 0, 0, 255), width=24, height=24, checked=False)

    events = []
    checkbox.set_on_click(lambda event: events.append(event))
    window.click(checkbox)

    events[0].node.set_checked(True)

    assert checkbox.get_checked() is True


def test_a_handler_can_call_back_on_event_node_immediately_without_a_reentrant_borrow_panic():
    """The real re-entrancy proof: `call_handler`'s own `make_event`
    closure fully completes -- `Event` built, `Py<Node>` included --
    before the Python handler itself runs, and building a `Node` costs
    only cheap `Rc` clones, never a live `Tree` borrow. A handler that
    immediately calls back into the tree through `event.node` (the
    exact same real pattern this codebase already trusts a handler
    touching *any* `Node` it holds to use) must not panic.
    """
    window = Window(width=100, height=100)
    checkbox = window.add_checkbox(background=(0, 0, 0, 255), width=24, height=24, checked=True)

    calls = []

    def on_click(event):
        event.node.animate("opacity", 0.5, duration_ms=1)
        calls.append(event.node.get_checked())
        event.node.set_on_hover_enter(lambda: None)

    checkbox.set_on_click(on_click)
    window.click(checkbox)  # must not raise

    assert calls == [True]


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


def test_a_shared_handler_distinguishes_callers_via_event_nodes_own_live_state():
    """The real scenario `Event.source`'s own bare opaque id could never
    fully serve without the caller keeping a separate id->Node lookup
    of its own: one handler, shared across several nodes, telling them
    apart by reading real live state straight off `event.node`.
    """
    window = Window(width=100, height=100)
    a = window.add_checkbox(background=(0, 0, 0, 255), width=20, height=20, checked=False)
    b = window.add_checkbox(background=(0, 0, 0, 255), width=20, height=20, checked=True, x=40)

    seen = []

    def shared_handler(event):
        seen.append(event.node.get_checked())

    a.set_on_click(shared_handler)
    b.set_on_click(shared_handler)

    window.click(a)
    window.click(b)

    assert seen == [False, True]
