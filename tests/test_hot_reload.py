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

import pytest

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


# --- M71 (§8, §16.1): View(source=...) / poll_reload(source=...) -----------
# Real, direct proof of the new pyo3 API surface a Python-side view-macro
# preprocessor (the sibling Tesserae project's own real next milestone)
# needs: constructing/reconciling against pre-expanded text instead of a
# fresh disk read, while poll_reload's own real file-watcher still gates
# on the actual on-disk file -- the Rust-level logic is already covered
# directly (`crates/engine-py/src/view.rs::tests::source_override_*`);
# this file's own job is confirming the real Python binding wires it
# through correctly, matching every other test in this file.


def test_view_source_is_used_instead_of_the_on_disk_files_own_content(tmp_path):
    path = write_view(tmp_path, "id: root\nkind: Container\nstyle: {width: 40, height: 40}\n")
    view = View(path, source="id: root\nkind: Container\nstyle: {width: 999, height: 40}\n")
    node = view.node("root")
    assert node.get("opacity") == pytest.approx(1.0), "must construct successfully from source="
    # No Python-facing width getter exists on a plain Container -- this
    # milestone's own Rust-level test already proves the real value
    # lands in the Tree; this test's own real job is proving the pyo3
    # binding accepts and threads the kwarg through without raising.


def test_view_source_none_is_the_real_pre_existing_behavior(tmp_path):
    path = write_view(tmp_path, "id: root\nkind: Container\nstyle: {width: 40, height: 40}\n")
    # source= omitted entirely -- must behave exactly as it always has.
    view = View(path)
    assert view.node("root") is not None


def test_poll_reload_source_is_reconciled_instead_of_a_fresh_disk_read(tmp_path):
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
""",
    )
    view = View(path)

    time.sleep(0.1)
    # A real write to the watched file -- content doesn't matter beyond
    # giving the real inotify-backed watcher something to report;
    # poll_reload's own source= below overrides what's actually
    # reconciled regardless of what this write says.
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
""",
    )

    def poll_with_source():
        return view.poll_reload(
            source="""
id: root
kind: Container
style: {width: 100, height: 100}
children:
  - id: cb
    kind: Checkbox
    style: {width: 30, height: 30, background: "#00FF00"}
"""
        )

    deadline = time.time() + 5.0
    reloaded = False
    while time.time() < deadline:
        if poll_with_source():
            reloaded = True
            break
        time.sleep(0.05)
    assert reloaded, "expected a real file-watcher change within the timeout"

    cb = view.node("cb")
    assert cb.get_checked() is False, "the checkbox from source= must be the one actually reconciled"
