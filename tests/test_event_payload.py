"""How a listener is called: with the `Event` if it has a required positional
parameter, without it otherwise (a bound method with only `self`, a
defaulted `i=i`, a keyword-only parameter), and one listener shared by two
nodes telling them apart by `event.target`.
"""

from tre import Window
from helpers import add


def test_a_bound_method_handler_with_only_self_still_works_zero_arg():
    """A bound method's signature already excludes `self`, so one taking
    only `self` is called without an event."""
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


def test_a_bound_method_handler_with_one_param_gets_the_event():
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
    """The `lambda i=i: ...` loop-index idiom: a defaulted parameter isn't
    required, so the listener is called without an event."""
    window = Window(width=100, height=100)
    calls = []
    for i in range(3):
        b = add(window, "box", fill=(0, 0, 0, 255), width=10, height=10, position="absolute", x=i * 15, y=0)
        b.on("click", lambda i=i: calls.append(i))
        window.simulate("click", node=b)

    assert calls == [0, 1, 2]


def test_a_keyword_only_parameter_does_not_make_the_event_required():
    """A keyword-only parameter (`*, extra=None`) has no positional slot
    for the event, so the listener is called without one."""
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
    `event.target`."""
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
