"""M4 Phase 2 (§10): real, repeatable coverage of `Window.press_key` --
`Window.click()`'s own keyboard counterpart, and the first Python-facing
entry point for Tab/Shift-Tab focus movement and Enter/Space activation.
Neither had one before this: `Window` only exposed `click()` (pointer
press+release).

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_click_dispatch.py`.
"""

from tre import Window
from helpers import add


def test_tab_then_enter_activates_the_first_interactive_node():
    window = Window(width=200, height=200)
    calls = []
    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50, focusable=True)
    button.on("click", lambda: calls.append("clicked"))

    window.simulate("key_down", key="tab")
    window.simulate("key_down", key="enter")

    assert calls == ["clicked"]


def test_tab_then_space_also_activates_the_focused_node():
    window = Window(width=200, height=200)
    calls = []
    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50, focusable=True)
    button.on("click", lambda: calls.append("clicked"))

    window.simulate("key_down", key="tab")
    window.simulate("key_down", key="space")

    assert calls == ["clicked"]


def test_tab_cycles_between_two_interactive_nodes_and_wraps():
    window = Window(width=200, height=200)
    calls = []
    a = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50, focusable=True)
    b = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50, focusable=True)
    a.on("click", lambda: calls.append("a"))
    b.on("click", lambda: calls.append("b"))

    window.simulate("key_down", key="tab")
    window.simulate("key_down", key="enter")
    window.simulate("key_down", key="tab")
    window.simulate("key_down", key="enter")
    window.simulate("key_down", key="tab")  # wraps back to a
    window.simulate("key_down", key="enter")

    assert calls == ["a", "b", "a"]


def test_shift_tab_moves_focus_backward():
    window = Window(width=200, height=200)
    calls = []
    a = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50, focusable=True)
    b = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50, focusable=True)
    a.on("click", lambda: calls.append("a"))
    b.on("click", lambda: calls.append("b"))

    # Shift-Tab from nothing focused wraps to the *last* interactive node.
    window.simulate("key_down", key="tab", shift=True)
    window.simulate("key_down", key="enter")

    assert calls == ["b"]


def test_enter_with_nothing_focused_activates_nothing():
    window = Window(width=200, height=200)
    calls = []
    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50, focusable=True)
    button.on("click", lambda: calls.append("clicked"))

    window.simulate("key_down", key="enter")  # no prior Tab -- nothing is focused yet

    assert calls == []
