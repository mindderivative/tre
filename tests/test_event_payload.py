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
from helpers import add


def test_a_bound_method_handler_with_only_self_still_works_zero_arg():
    """`inspect.signature` on a *bound* method already excludes `self`
    -- confirmed real here, not assumed: a bound method whose only
    parameter is `self` must arity-sniff as zero-required, the
    identical real shape every pre-existing bound-method handler in
    this project's own test suite (e.g. `test_click_dispatch.py`'s own
    GC-cycle test, `holder.on_click`) already relies on.
    """
    window = Window(width=100, height=100)
    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)

    class Handler:
        def __init__(self):
            self.calls = []

        def on_click(self):
            self.calls.append("bound method fired")

    handler = Handler()
    button.on("click", handler.on_click)
    window.simulate("click", node=button)

    assert handler.calls == ["bound method fired"]


def test_a_bound_method_handler_with_one_real_param_gets_the_event():
    window = Window(width=100, height=100)
    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)

    class Handler:
        def __init__(self):
            self.events = []

        def on_click(self, event):
            self.events.append(event)

    handler = Handler()
    button.on("click", handler.on_click)
    window.simulate("click", node=button)

    assert len(handler.events) == 1
    assert handler.events[0].type == "click"


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
        b = add(window, "box", fill=(0, 0, 0, 255), width=10, height=10, position="absolute", x=i * 15, y=0)
        b.on("click", lambda i=i: calls.append(i))
        window.simulate("click", node=b)

    assert calls == [0, 1, 2]


def test_a_keyword_only_parameter_does_not_make_the_event_required():
    """A handler with only a keyword-only parameter (`*, foo=None`) has
    no real *positional* slot for `Event` to fill -- arity-sniffs as
    zero-event, called plain, matching Python's own real calling
    convention (a keyword-only arg is never satisfied by a bare
    positional call anyway).
    """
    window = Window(width=100, height=100)
    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    calls = []

    def handler(*, extra=None):
        calls.append(extra)

    button.on("click", handler)
    window.simulate("click", node=button)

    assert calls == [None]


def test_event_target_distinguishes_two_nodes_sharing_one_handler():
    """One handler shared across two nodes tells them apart by
    `event.target` (M100: the legacy `source` id and `node` are gone)."""
    window = Window(width=100, height=100)
    a = add(window, "box", fill=(0, 0, 0, 255), width=20, height=20, position="absolute", x=0, y=0)
    b = add(window, "box", fill=(0, 0, 0, 255), width=20, height=20, position="absolute", x=40, y=0)

    targets = []

    def shared_handler(event):
        targets.append(event.target)

    a.on("click", shared_handler)
    b.on("click", shared_handler)
    window.simulate("click", node=a)
    window.simulate("click", node=b)

    assert targets == [a, b]
