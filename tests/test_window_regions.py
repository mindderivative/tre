"""0.5.0 M3 (issue #28, #38): a framework's own title bar and borders --
which presses move or resize the window. The move itself is `winit`'s and
needs a real display and a real press, but the decision, and the
`pointer_cancel` that ends a press taken for it, run the same without one.
"""

import pytest

from tre import Window


EVENTS = ("pointer_down", "pointer_up", "click", "pointer_cancel")


def title_bar(window):
    """A 300x40 drag region at the window's top, holding a 30x30 node."""
    bar = window.create("box", width=300, height=40, window_region="drag")
    inner = window.create("box", width=30, height=30)
    bar.add_child(inner)
    window.root.add_child(bar)
    return bar, inner


def press(window, node, **fields):
    window.simulate("pointer_down", node=node, **fields)
    window.simulate("pointer_up", node=node, **fields)


def test_window_region_reads_back_as_set():
    node = Window().create("box")
    assert node.get("window_region") is None
    node.set(window_region="drag")
    assert node.get("window_region") == "drag"
    node.set(window_region="none")
    assert node.get("window_region") == "none"
    node.set(window_region=None)
    assert node.get("window_region") is None
    with pytest.raises(ValueError, match="window_region"):
        node.set(window_region="title")


def test_a_press_on_the_drag_region_moves_the_window_instead_of_clicking():
    window = Window(width=400, height=300)
    bar, _ = title_bar(window)
    heard = []
    for event in EVENTS:
        bar.on(event, lambda event=event: heard.append(event))
    press(window, bar, x=200, y=20)
    assert heard == ["pointer_down", "pointer_cancel"], (
        "the press reaches its listeners, then ends: no pointer_up or click follows"
    )


def test_a_non_interactive_node_inside_drags():
    window = Window(width=400, height=300)
    _, inner = title_bar(window)
    heard = []
    inner.on("pointer_down", lambda: heard.append("down"))  # a tooltip's anchor, say
    inner.on("pointer_cancel", lambda: heard.append("cancel"))
    press(window, inner)
    assert heard == ["down", "cancel"], "pointer_down alone doesn't make a node interactive"


@pytest.mark.parametrize(
    "make_interactive",
    [
        lambda node: node.set(focusable=True),
        lambda node: node.on("click", lambda: None),
        lambda node: node.on("pointer_down", lambda: node.capture_pointer()),
    ],
    ids=["focusable", "click listener", "pointer capture"],
)
def test_an_interactive_node_inside_is_pressed_not_dragged(make_interactive):
    window = Window(width=400, height=300)
    _, inner = title_bar(window)
    make_interactive(inner)
    cancelled = []
    inner.on("pointer_cancel", lambda: cancelled.append(True))
    press(window, inner)
    assert cancelled == [], "a button in the title bar stays a button"


def test_none_turns_dragging_off_and_drag_turns_it_back_on():
    window = Window(width=400, height=300)
    bar, inner = title_bar(window)
    inner.set(window_region="none")
    cancelled = []
    inner.on("pointer_cancel", lambda: cancelled.append("inner"))
    press(window, inner)
    assert cancelled == []

    button = window.create("box", width=60, height=30, focusable=True)
    icon = window.create("box", width=16, height=16, window_region="drag")
    button.add_child(icon)
    bar.add_child(button)
    icon.on("pointer_cancel", lambda: cancelled.append("icon"))
    press(window, icon)
    assert cancelled == ["icon"], "a drag region inside a button overrides it"


def test_nothing_above_the_drag_region_is_looked_at():
    window = Window(width=400, height=300)
    bar, _ = title_bar(window)
    window.root.set(focusable=True)
    window.root.on("click", lambda: None)
    window.root.on("pointer_down", lambda: None)  # the side-button listener, say
    cancelled = []
    bar.on("pointer_cancel", lambda: cancelled.append(True))
    press(window, bar, x=200, y=20)
    assert cancelled == [True]


def test_without_a_drag_region_a_press_is_ordinary():
    window = Window(width=400, height=300)
    box = window.create("box", width=100, height=40)
    window.root.add_child(box)
    heard = []
    for event in EVENTS:
        box.on(event, lambda event=event: heard.append(event))
    press(window, box)
    assert heard == ["pointer_down", "pointer_up", "click"]


def test_the_next_press_after_a_drag_is_ordinary():
    window = Window(width=400, height=300)
    bar, _ = title_bar(window)
    box = window.create("box", width=100, height=40)
    window.root.add_child(box)
    window.simulate("pointer_down", node=bar, x=200, y=20)  # the release never came
    heard = []
    for event in EVENTS:
        box.on(event, lambda event=event: heard.append(event))
    press(window, box)
    assert heard == ["pointer_down", "pointer_up", "click"]


def test_a_secondary_press_on_the_drag_region_does_not_drag():
    window = Window(width=400, height=300)
    bar, _ = title_bar(window)
    cancelled = []
    bar.on("pointer_cancel", lambda: cancelled.append(True))
    window.simulate("pointer_down", node=bar, x=200, y=20, button="secondary")
    assert cancelled == []
