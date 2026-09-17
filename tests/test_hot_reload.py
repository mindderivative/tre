"""M19 Phase 1 (§16.4): real, repeatable coverage that `View.
poll_reload()` genuinely reaches a real `ViewWatcher`/`Reconciler`
pair, not just that the API compiles. `engine-spec`'s own tests
already proved `ViewWatcher` detects a real file write and
`Reconciler::reconcile` preserves an unchanged widget's own `NodeId`
in isolation -- this file proves both reach the real Python FFI
surface together, for the first time, through `View`.

`View` has no live-window/render-loop concept of its own (`view.rs`'s
own module doc comment) -- `poll_reload` is a real, explicit method
the caller invokes, not something that fires automatically every
"frame." Filesystem watchers deliver asynchronously, so every test
below polls in a bounded loop rather than a single fixed sleep,
matching `engine-spec::watch::tests::watcher_detects_a_real_write_to_
the_watched_file`'s own established discipline -- failing definitively
(not flakily) if no change ever arrives within a generous window.
"""

import time

from tre import View


def write_view(tmp_path, yaml):
    path = tmp_path / "view.yaml"
    path.write_text(yaml)
    return str(path)


def poll_until_changed(view, timeout=5.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        if view.poll_reload():
            return True
        time.sleep(0.05)
    return False


def test_poll_reload_reports_no_change_before_any_write(tmp_path):
    path = write_view(tmp_path, "id: root\nkind: Container\nstyle: {width: 100, height: 100}\n")
    view = View(path)
    assert view.poll_reload() is False
    assert view.poll_reload() is False, "a second poll with still no change must also be False"


def test_a_real_file_edit_reaches_the_live_tree(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Container
style: {width: 100, height: 100}
children:
  - id: box
    kind: Rect
    style: {width: 20, height: 20, background: "#111111", corner_radius: 0}
""",
    )
    view = View(path)
    assert view.poll_reload() is False

    time.sleep(0.1)
    write_view(
        tmp_path,
        """
id: root
kind: Container
style: {width: 100, height: 100}
children:
  - id: box
    kind: Rect
    style: {width: 20, height: 20, background: "#111111", corner_radius: 12}
""",
    )

    assert poll_until_changed(view), "poll_reload never reported the real write within 5s"

    node = view.node("box")
    assert node.get("corner_radius") == 12.0


def test_an_unchanged_widget_keeps_its_real_node_id_across_a_reload(tmp_path):
    """The real end-to-end proof of `Reconciler::reconcile`'s own
    documented claim ("an unchanged node keeps its real `NodeId`... so
    its focus, scroll, and any in-flight `ActiveAnimation` survive a
    reload"), reached through `View.poll_reload` for the first time --
    `engine-spec`'s own unit tests already proved this at the
    `Reconciler` level directly, this proves the real FFI path too.
    Mutates `cb` through a `Node` obtained *before* the reload, then
    reads it back through a *fresh* `Node` obtained *after* -- if
    reconciling had genuinely replaced `cb`'s own `NodeId` (a
    stale-reference bug), the write would either raise or silently not
    show up on the fresh read.
    """
    path = write_view(
        tmp_path,
        """
id: root
kind: Container
style: {width: 100, height: 100}
children:
  - id: cb
    kind: Checkbox
    style: {width: 24, height: 24, background: "#6750A4"}
  - id: other
    kind: Rect
    style: {width: 10, height: 10, background: "#000000"}
""",
    )
    view = View(path)
    cb_before = view.node("cb")
    cb_before.set_checked(True)

    time.sleep(0.1)
    # Only "other" changes -- "cb" itself is byte-for-byte identical.
    write_view(
        tmp_path,
        """
id: root
kind: Container
style: {width: 100, height: 100}
children:
  - id: cb
    kind: Checkbox
    style: {width: 24, height: 24, background: "#6750A4"}
  - id: other
    kind: Rect
    style: {width: 10, height: 10, background: "#FFFFFF"}
""",
    )
    assert poll_until_changed(view)

    cb_after = view.node("cb")
    assert cb_after.get_checked() is True, (
        "an unchanged widget must keep its real NodeId across a reload -- a stale/replaced id "
        "would have lost the pre-reload mutation"
    )
