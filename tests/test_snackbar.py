"""M30 Phase 4 Step 2 (§5, §7, §11.3): real, repeatable coverage of
`Window.add_snackbar`/`open_snackbar`/`close_snackbar` -- a real MD3
transient notification, and the real, deliberate anatomy departure
this step makes: unlike every prior component in this catalog,
`add_snackbar` returns up to three independent real `Node`s
(container, action, close), each independently clickable, since a
real snackbar action must be clickable on its own -- not the purely
decorative sub-icon shape `Chip`'s own `removable` icon already
established for the other case.
"""

import pytest

from tre import Node, Window


def test_add_snackbar_returns_the_container_node():
    window = Window(width=400, height=300)
    container, action, close = window.add_snackbar(text="File deleted.", width=320)
    assert isinstance(container, Node)
    assert action is None
    assert close is None


def test_add_snackbar_with_an_action_label_returns_a_real_action_node():
    window = Window(width=400, height=300)
    container, action, close = window.add_snackbar(text="File deleted.", width=320, action_label="Undo")
    assert isinstance(container, Node)
    assert isinstance(action, Node)
    assert close is None


def test_add_snackbar_closable_returns_a_real_close_node():
    window = Window(width=400, height=300)
    container, action, close = window.add_snackbar(text="File deleted.", width=320, closable=True)
    assert isinstance(container, Node)
    assert action is None
    assert isinstance(close, Node)


def test_add_snackbar_with_both_action_and_close_returns_both_real_nodes():
    window = Window(width=400, height=300)
    container, action, close = window.add_snackbar(
        text="File deleted.", width=320, action_label="Undo", closable=True
    )
    assert isinstance(container, Node)
    assert isinstance(action, Node)
    assert isinstance(close, Node)


def test_a_themed_snackbar_does_not_raise():
    window = Window(width=400, height=300)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    container, _action, _close = window.add_snackbar(
        text="File deleted.", width=320, action_label="Undo", closable=True
    )
    assert isinstance(container, Node)


def test_open_snackbar_and_close_snackbar_do_not_raise():
    window = Window(width=400, height=300)
    container, _action, _close = window.add_snackbar(text="Saved.", width=280)

    window.open_snackbar(container)  # must not raise
    window.open_snackbar(container)  # a real, safe no-op -- already open
    window.close_snackbar(container)  # must not raise


def test_reopening_a_snackbar_after_closing_it_does_not_raise():
    window = Window(width=400, height=300)
    container, _action, _close = window.add_snackbar(text="Saved.", width=280)

    window.open_snackbar(container)
    window.close_snackbar(container)
    window.open_snackbar(container)  # must not raise -- real reopen, not a stale/destroyed node


def test_open_snackbar_rejects_a_snackbar_from_a_different_window():
    window_a = Window(width=400, height=300)
    window_b = Window(width=400, height=300)
    foreign_container, _a, _c = window_b.add_snackbar(text="Saved.", width=280)
    with pytest.raises(ValueError, match="different Window"):
        window_a.open_snackbar(foreign_container)


def test_the_action_node_is_a_real_clickable_node_independent_of_the_container():
    """The real point of this step's own multi-node return shape: the
    action must be clickable on its own, distinct from the snackbar's
    own body (which isn't a button in real MD3).
    """
    window = Window(width=400, height=300)
    container, action, _close = window.add_snackbar(text="File deleted.", width=320, action_label="Undo")
    window.open_snackbar(container)

    calls = []
    action.enable_interaction()
    action.set_on_click(lambda: calls.append("undo"))
    window.click(action)
    assert calls == ["undo"], "a real click on the action node must reach its own registered handler"


def test_the_close_node_is_a_real_clickable_node_independent_of_the_action():
    """The same real independence proven a second time for `close`,
    distinct from both the container and the action.
    """
    window = Window(width=400, height=300)
    container, action, close = window.add_snackbar(
        text="File deleted.", width=320, action_label="Undo", closable=True
    )
    window.open_snackbar(container)

    action_calls = []
    close_calls = []
    action.enable_interaction()
    action.set_on_click(lambda: action_calls.append("undo"))
    close.enable_interaction()
    close.set_on_click(lambda: window.close_snackbar(container) or close_calls.append("closed"))

    window.click(close)
    assert close_calls == ["closed"]
    assert action_calls == [], "clicking close must not also fire the action's own handler"
