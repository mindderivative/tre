"""M54 Phase 2/3 (§8, §16.2): real, repeatable coverage of the
cross-cutting mechanism the new `Event` payload rests on -- arity-
sniffing at registration (`dispatch::wants_event_payload`), which
decides whether a handler gets called `handler()` (every pre-existing
handler in this project's own `tests`/`examples`) or `handler(event)`
(any new one that declares it wants the real payload). Per-`EventKind`
field correctness (`Click`'s position/button, `Change`'s old/new value,
`HoverEnter`/`HoverExit`'s position) is covered where each already has
an established home: `test_click_dispatch.py`, `test_change_event.py`,
`test_hover_events.py`.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`.
"""

from tre import Window


def test_a_bound_method_handler_with_only_self_still_works_zero_arg():
    """`inspect.signature` on a *bound* method already excludes `self`
    -- confirmed real here, not assumed: a bound method whose only
    parameter is `self` must arity-sniff as zero-required, the
    identical real shape every pre-existing bound-method handler in
    this project's own test suite (e.g. `test_click_dispatch.py`'s own
    GC-cycle test, `holder.on_click`) already relies on.
    """
    window = Window(width=100, height=100)
    button = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)

    class Handler:
        def __init__(self):
            self.calls = []

        def on_click(self):
            self.calls.append("bound method fired")

    handler = Handler()
    button.set_on_click(handler.on_click)
    window.click(button)

    assert handler.calls == ["bound method fired"]


def test_a_bound_method_handler_with_one_real_param_gets_the_event():
    window = Window(width=100, height=100)
    button = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)

    class Handler:
        def __init__(self):
            self.events = []

        def on_click(self, event):
            self.events.append(event)

    handler = Handler()
    button.set_on_click(handler.on_click)
    window.click(button)

    assert len(handler.events) == 1
    assert handler.events[0].kind == "click"


def test_a_defaulted_parameter_counts_as_not_required_like_lambda_i_equals_i():
    """The real, pervasive `lambda i=i: ...` pattern this project's own
    catalog already relies on (`examples/segmented_button.py`, `tests/
    test_date_picker.py`, etc., to close over a loop index) must keep
    working unmodified -- a parameter with a real default is not a
    required positional, so this handler still arity-sniffs as
    zero-event, exactly its own pre-existing behavior.
    """
    window = Window(width=100, height=100)
    calls = []
    for i in range(3):
        b = window.add_rect(background=(0, 0, 0, 255), width=10, height=10, x=i * 15, y=0)
        b.set_on_click(lambda i=i: calls.append(i))
        window.click(b)

    assert calls == [0, 1, 2]


def test_a_keyword_only_parameter_does_not_make_the_event_required():
    """A handler with only a keyword-only parameter (`*, foo=None`) has
    no real *positional* slot for `Event` to fill -- arity-sniffs as
    zero-event, called plain, matching Python's own real calling
    convention (a keyword-only arg is never satisfied by a bare
    positional call anyway).
    """
    window = Window(width=100, height=100)
    button = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    calls = []

    def handler(*, extra=None):
        calls.append(extra)

    button.set_on_click(handler)
    window.click(button)

    assert calls == [None]


def test_a_time_picker_dial_change_gives_a_real_hour_minute_tuple():
    """`ChangedValue::Time` (M54 Phase 1) -- the one real `Changed`
    value shape that's neither a plain number nor a string, confirmed
    to cross the FFI boundary as a real Python `(hour, minute)` tuple.
    """
    window = Window(width=200, height=200)
    dial = window.add_time_picker_dial(hour=3, minute=15, size=160)
    dial.enable_interaction()

    events = []
    dial.set_on_change(lambda event: events.append(event))

    window.click(dial)  # a zero-movement press+release, a real drag start/end

    assert len(events) == 1
    event = events[0]
    assert event.kind == "change"
    assert event.old_value == (3, 15)
    assert event.new_value == (3, 15)


def test_event_source_is_a_stable_value_distinguishing_two_real_nodes():
    """`Event.source` (a plain opaque `u64`, M54 scoping's own resolved
    design fork) is real and usable for at least the one thing a bare
    id is good for: telling two different real nodes' own events apart
    when one handler is shared across both. `Event.node` (M56) is now
    the real live-handle counterpart for the same scenario -- see
    `test_event_node.py` -- but `source` itself is unchanged, kept
    exactly as shipped here.
    """
    window = Window(width=100, height=100)
    a = window.add_rect(background=(0, 0, 0, 255), width=20, height=20, x=0, y=0)
    b = window.add_rect(background=(0, 0, 0, 255), width=20, height=20, x=40, y=0)

    events = []

    def shared_handler(event):
        events.append(event.source)

    a.set_on_click(shared_handler)
    b.set_on_click(shared_handler)
    window.click(a)
    window.click(b)

    assert len(events) == 2
    assert events[0] != events[1]
    assert isinstance(events[0], int)
    assert isinstance(events[1], int)
