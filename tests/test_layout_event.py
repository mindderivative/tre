"""0.5.7 (#178): the node `layout` event.

A node with a `layout` listener is told when its laid-out size changes, and when
it is first laid out: `event.width`/`event.height` are the new size,
`event.old_value` is `None` the first time and the old `(width, height)` after,
`event.new_value` the new `(width, height)`. Whatever changed the size -- a
`set`, a parent's resize, a calc size, new text -- the event comes once a frame.
"""

from tre import Window


def watched(window, node):
    events = []
    node.on("layout", events.append)
    return events


def make(window, parent_width=300, **child):
    parent = window.create("box", width=parent_width, height=200)
    window.root.add_child(parent)
    child.setdefault("height", 20)
    node = window.create("box", **child)
    parent.add_child(node)
    return parent, node


def test_the_first_layout_is_reported_with_no_old_size():
    window = Window(width=600, height=400)
    _, node = make(window, width=50)
    events = watched(window, node)
    assert events == []  # nothing is laid out until a frame
    window.advance(16)
    assert len(events) == 1
    e = events[0]
    assert (e.width, e.height) == (50, 20)
    assert e.old_value is None and e.new_value == (50, 20)
    assert e.type == "layout" and e.target == node


def test_nothing_more_is_reported_while_the_size_holds():
    window = Window(width=600, height=400)
    _, node = make(window, width=50)
    events = watched(window, node)
    for _ in range(4):
        window.advance(16)
    assert len(events) == 1


def test_a_set_reports_the_old_and_new_sizes():
    window = Window(width=600, height=400)
    _, node = make(window, width=50)
    events = watched(window, node)
    window.advance(16)
    node.set(width=80, height=30)
    window.advance(16)
    assert len(events) == 2
    assert events[1].old_value == (50, 20) and events[1].new_value == (80, 30)
    assert (events[1].width, events[1].height) == (80, 30)


def test_a_parent_resizing_is_reported_for_a_child_sized_by_it():
    window = Window(width=600, height=400)
    parent, node = make(window, width="50%")
    events = watched(window, node)
    window.advance(16)
    assert events[-1].width == 150
    parent.set(width=400)
    window.advance(16)
    assert len(events) == 2 and events[1].old_value == (150, 20) and events[1].width == 200


def test_a_calc_size_reports_where_it_settles():
    window = Window(width=600, height=400)
    parent, node = make(window, width="100% - 16")
    events = watched(window, node)
    window.advance(16)
    assert len(events) == 1 and events[0].width == 284
    parent.set(width=400)
    window.advance(16)
    assert events[-1].old_value == (284, 20) and events[-1].width == 384


def test_it_is_how_a_view_learns_its_parent_changed_size_the_motivating_case():
    # #178: with no node event for a size change, Tesserae could only follow a
    # parent by polling each frame.
    window = Window(width=600, height=400)
    parent, node = make(window, width=10)
    seen = []
    parent.on("layout", lambda e: seen.append((e.width, e.height)))
    window.advance(16)
    parent.set(width=320, height=180)
    window.advance(16)
    assert seen == [(300, 200), (320, 180)]


def test_new_text_in_a_fit_content_label_is_reported():
    window = Window(width=600, height=400)
    label = window.create("text", text="Hello", font_size=16, width="fit-content", height="fit-content")
    window.root.add_child(label)
    events = watched(window, label)
    window.advance(16)
    first = events[0].width
    label.set(text="Hello, a much longer label")
    window.advance(16)
    assert len(events) == 2 and events[1].width > first


def test_off_stops_the_events_and_on_replaces_the_handler():
    window = Window(width=600, height=400)
    _, node = make(window, width=50)
    first, second = [], []
    node.on("layout", first.append)
    node.on("layout", second.append)  # replaces
    window.advance(16)
    assert first == [] and len(second) == 1
    node.off("layout")
    node.set(width=60)
    window.advance(16)
    assert len(second) == 1


def test_it_does_not_bubble():
    window = Window(width=600, height=400)
    parent, node = make(window, width=50)
    from_parent = watched(window, parent)
    node.on("layout", lambda: None)
    window.advance(16)
    node.set(width=70)
    window.advance(16)
    # The parent hears about its own layout only: one event, for itself.
    assert len(from_parent) == 1 and from_parent[0].target == parent


def test_a_handler_may_take_no_parameters():
    window = Window(width=600, height=400)
    _, node = make(window, width=50)
    calls = []
    node.on("layout", lambda: calls.append(1))
    window.advance(16)
    assert calls == [1]


def test_a_removed_node_stops_being_watched_without_error():
    window = Window(width=600, height=400)
    parent, node = make(window, width=50)
    events = watched(window, node)
    window.advance(16)
    node.remove()
    window.advance(16)
    window.advance(16)
    assert len(events) == 1
