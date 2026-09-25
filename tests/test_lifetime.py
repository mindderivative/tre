"""M96 Phase 1: node lifetime and thread safety.

- A node made by `window.create` (or detached by `remove()`) is freed, with
  its listeners, once no Python handle points anywhere into its subtree. A
  listener's liveness is how these tests see it: `tre` drops a freed node's
  listeners, and a weak reference to the listener then goes dead.
- Issue #10: a `tre` object whose last reference sits in a cycle can be freed
  by Python's cyclic collector on whichever thread runs it. That must neither
  panic nor leak: the drop is finished on the owning thread.
"""

from __future__ import annotations

import gc
import sys
import threading
import weakref
from collections.abc import Callable, Iterator
from typing import Any

import pytest

import tre


class Listener:
    def __call__(self) -> None:
        pass


def listening(node: tre.Node) -> weakref.ref[Listener]:
    listener = Listener()
    node.on("click", listener)
    return weakref.ref(listener)


def test_a_created_node_with_no_handle_is_freed() -> None:
    w = tre.Window(200, 100, "lifetime")
    box = w.create("box")
    ref = listening(box)
    assert ref() is not None
    del box
    assert ref() is None


def test_a_handle_anywhere_in_the_subtree_keeps_it_alive() -> None:
    w = tre.Window(200, 100, "lifetime")
    box = w.create("box")
    child = w.create("box")
    box.add_child(child)
    ref = listening(box)
    del box
    assert ref() is not None, "the child's handle keeps the whole subtree"
    parent = child.parent()
    assert parent is not None
    del parent, child
    assert ref() is None


def test_an_attached_node_is_never_freed() -> None:
    w = tre.Window(200, 100, "lifetime")
    box = w.create("box")
    w.root.add_child(box)
    ref = listening(box)
    del box
    assert ref() is not None
    assert len(w.root.children()) == 1


def test_a_removed_node_is_freed_with_its_last_handle() -> None:
    w = tre.Window(200, 100, "lifetime")
    box = w.create("box")
    w.root.add_child(box)
    ref = listening(box)
    box.remove()
    assert ref() is not None
    del box
    assert ref() is None
    assert w.root.children() == []


@pytest.fixture
def unraisable() -> Iterator[list[str]]:
    seen: list[str] = []
    previous = sys.unraisablehook

    def hook(u: Any) -> None:
        seen.append(f"{type(u.exc_value).__name__}: {u.exc_value}")

    sys.unraisablehook = hook
    try:
        yield seen
    finally:
        sys.unraisablehook = previous


def collect_on_another_thread(make: Callable[[], object]) -> None:
    """Leaves the only reference to `make()`'s result in a cycle, then runs
    the cyclic collector on a background thread -- issue #10's repro."""
    gc.disable()
    try:

        class Holder:
            pass

        holder = Holder()
        holder.value = make()  # type: ignore[attr-defined]
        holder.me = holder  # type: ignore[attr-defined]
        del holder
        thread = threading.Thread(target=gc.collect)
        thread.start()
        thread.join()
    finally:
        gc.enable()


def test_a_window_and_node_freed_off_thread_neither_panic_nor_leak(
    unraisable: list[str],
) -> None:
    w = tre.Window(200, 100, "lifetime")
    refs: list[weakref.ref[Listener]] = []

    def make() -> object:
        box = w.create("box")
        refs.append(listening(box))
        return (tre.Window(100, 100, "other"), box)

    collect_on_another_thread(make)
    assert refs[0]() is not None, "not freed on the collector's thread"
    w.create("box")  # a safe point on the owning thread
    assert unraisable == []
    assert refs[0]() is None, "the off-thread drop was finished on this thread"


def test_a_view_freed_off_thread_neither_panics_nor_leaks(unraisable: list[str]) -> None:
    spec = {"id": "root", "kind": "Rect", "style": {"width": 10, "height": 10, "background": "#112233"}}
    collect_on_another_thread(lambda: tre.View(spec=spec))
    tre.Window(10, 10, "safe point").create("box")
    assert unraisable == []


def test_using_a_node_from_another_thread_still_raises() -> None:
    w = tre.Window(200, 100, "lifetime")
    box = w.create("box")
    errors: list[BaseException] = []

    def use() -> None:
        try:
            box.children()
        except BaseException as err:  # noqa: BLE001 -- pyo3 raises PanicException
            errors.append(err)

    thread = threading.Thread(target=use)
    thread.start()
    thread.join()
    assert len(errors) == 1
    assert "belongs to the thread that created it" in str(errors[0])
