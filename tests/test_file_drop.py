"""0.5.4 (#114): files dragged from the OS -- `file_hover`, `file_hover_cancel`
and `file_drop`, on the node under the pointer and on the window.

`window.simulate("file_drop", paths=[...], node=...)` delivers what the
platform's `DroppedFile` events become, so none of this needs a file manager.
"""

import pytest

from tre import Window


def scene():
    window = Window(width=300, height=200)
    window.root.set(padding=0)
    zone = window.create("box", width=100, height=100, x=20, y=20, position="absolute")
    other = window.create("box", width=100, height=100, x=170, y=20, position="absolute")
    for node in (zone, other):
        window.root.add_child(node)
    log = []
    for name, node in (("zone", zone), ("other", other)):
        for event in ("file_hover", "file_hover_cancel", "file_drop"):
            node.on(event, lambda e, name=name, event=event: log.append((name, event, e)))
    for event in ("file_hover", "file_hover_cancel", "file_drop"):
        window.on(event, lambda e, event=event: log.append(("window", event, e)))
    return window, zone, other, log


def test_a_drop_reaches_the_node_under_the_pointer_and_the_window():
    window, zone, other, log = scene()
    window.simulate("file_drop", paths=["/tmp/a.txt"], node=zone)
    who = [(n, ev) for n, ev, _ in log]
    assert ("zone", "file_drop") in who and ("window", "file_drop") in who
    assert ("other", "file_drop") not in who
    event = next(e for n, ev, e in log if n == "zone")
    assert event.path == "/tmp/a.txt" and event.paths == ["/tmp/a.txt"]
    assert event.target == zone


def test_several_files_are_one_event_with_all_their_paths():
    window, zone, _, log = scene()
    window.simulate("file_drop", paths=["/tmp/a.txt", "/tmp/b.png", "/tmp/c.md"], node=zone)
    drops = [e for n, ev, e in log if n == "zone" and ev == "file_drop"]
    assert len(drops) == 1
    assert drops[0].paths == ["/tmp/a.txt", "/tmp/b.png", "/tmp/c.md"]
    assert drops[0].path == "/tmp/a.txt"


def test_hover_then_cancel_tells_the_same_node():
    window, zone, other, log = scene()
    window.simulate("file_hover", paths=["/tmp/a.txt"], node=zone)
    window.simulate("file_hover_cancel")
    node_events = [(n, ev) for n, ev, _ in log if n != "window"]
    assert node_events == [("zone", "file_hover"), ("zone", "file_hover_cancel")]
    window_events = [ev for n, ev, _ in log if n == "window"]
    assert window_events == ["file_hover", "file_hover_cancel"]


def test_a_drop_after_a_hover_is_not_also_a_cancel():
    window, zone, _, log = scene()
    window.simulate("file_hover", paths=["/tmp/a.txt"], node=zone)
    window.simulate("file_drop", paths=["/tmp/a.txt"], node=zone)
    assert [ev for n, ev, _ in log if n == "zone"] == ["file_hover", "file_drop"]
    # A later cancel has no node to tell.
    window.simulate("file_hover_cancel")
    assert [ev for n, ev, _ in log if n == "zone"] == ["file_hover", "file_drop"]


def test_events_bubble_to_ancestors_and_carry_the_position():
    window = Window(width=300, height=200)
    window.root.set(padding=0)
    outer = window.create("box", width=200, height=200, x=0, y=0, position="absolute")
    inner = window.create("box", width=50, height=50, x=10, y=10, position="absolute")
    window.root.add_child(outer)
    outer.add_child(inner)
    heard = []
    outer.on("file_drop", lambda e: heard.append((e.target == inner, e.window_x, e.window_y)))
    window.simulate("file_drop", paths=["/tmp/x"], x=30, y=30)
    assert heard == [(True, 30.0, 30.0)]


def test_a_drop_where_nothing_listens_is_harmless():
    window = Window(width=100, height=100)
    window.simulate("file_drop", paths=["/tmp/a"], x=5, y=5)
    window.simulate("file_hover", path="/tmp/a", x=5, y=5)
    window.simulate("file_hover_cancel")


def test_a_single_path_keyword_is_accepted():
    window, zone, _, log = scene()
    window.simulate("file_drop", path="/tmp/one.txt", node=zone)
    event = next(e for n, ev, e in log if n == "zone")
    assert event.paths == ["/tmp/one.txt"]


@pytest.mark.parametrize("fields, message", [
    ({}, "needs `paths` or `path`"),
    ({"paths": []}, "needs `paths` or `path`"),
    ({"paths": "nope"}, "list of str paths"),
    ({"paths": [1, 2]}, "list of str paths"),
])
def test_bad_fields_are_value_errors(fields, message):
    window, zone, _, _ = scene()
    with pytest.raises(ValueError, match=message):
        window.simulate("file_drop", node=zone, **fields)
