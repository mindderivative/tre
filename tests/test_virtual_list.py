"""§14 step 15, Stage C (§11.7): real, repeatable coverage of the
`Window.add_virtual_list`/`Window.set_virtual_list_window` FFI boundary --
the "materialize item N" callback pattern, its two real error paths (not
a `VirtualList` at all / not owned by this `Window`, and the callback
itself raising), and that `engine-core`'s own recycling actually reaches
Python-visible behavior (a scrolled-away index's callback isn't called
again for free -- it has to be re-materialized).

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`. `test_virtual_list_benchmark.py`
is the separate, `skip`-by-default real GIL-overhead measurement --
this file is the fast, no-window-needed regression coverage.
"""

import gc
import weakref

import pytest

from tre import Node, Window


def test_add_virtual_list_returns_a_node():
    window = Window(width=200, height=200)
    node = window.add_virtual_list(
        item_count=100_000, item_extent=20.0, materialize=lambda idx: (0, 0, 0, 255)
    )
    assert isinstance(node, Node)


def test_set_virtual_list_window_calls_materialize_only_for_newly_visible_indices():
    window = Window(width=200, height=200)
    calls = []

    def materialize(idx):
        calls.append(idx)
        return (idx % 256, 0, 0, 255)

    vl = window.add_virtual_list(item_count=100_000, item_extent=20.0, materialize=materialize)

    window.set_virtual_list_window(vl, 0, 5)
    assert calls == [0, 1, 2, 3, 4]

    calls.clear()
    # Scroll: 0..5 -> 3..8. Indices 3, 4 stayed in the window and must
    # NOT be re-materialized; only the genuinely new indices 5, 6, 7
    # should trigger a real callback.
    window.set_virtual_list_window(vl, 3, 8)
    assert calls == [5, 6, 7]

    calls.clear()
    # Scroll back: 3..8 -> 0..5. Indices 0, 1, 2 were fully recycled
    # (real Tree::remove, not just hidden) when they first left the
    # window, so they're real, freshly-materialized calls again here --
    # recycling has a real, Python-visible cost, not a free cache hit.
    window.set_virtual_list_window(vl, 0, 5)
    assert calls == [0, 1, 2]


def test_set_virtual_list_window_rejects_a_node_that_is_not_a_virtual_list():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=10, height=10)
    with pytest.raises(ValueError, match="not a VirtualList"):
        window.set_virtual_list_window(rect, 0, 1)


def test_set_virtual_list_window_rejects_a_virtual_list_from_another_window():
    window_a = Window(width=200, height=200)
    window_b = Window(width=200, height=200)
    vl = window_a.add_virtual_list(
        item_count=10, item_extent=10.0, materialize=lambda idx: (0, 0, 0, 255)
    )
    with pytest.raises(ValueError, match="not a VirtualList"):
        window_b.set_virtual_list_window(vl, 0, 1)


def test_a_materializer_exception_propagates_as_a_real_python_error():
    window = Window(width=200, height=200)

    def materialize(idx):
        if idx == 2:
            raise RuntimeError(f"item {idx} is cursed")
        return (0, 0, 0, 255)

    vl = window.add_virtual_list(item_count=10, item_extent=10.0, materialize=materialize)
    with pytest.raises(RuntimeError, match="item 2 is cursed"):
        window.set_virtual_list_window(vl, 0, 5)


def test_window_participates_in_cyclic_gc_when_its_materializer_captures_it_back():
    """The real reason `PyWindow` implements `__traverse__`/`__clear__`
    (§8's own review note, engine-py/src/window.rs's module doc comment):
    a materializer that captures the very `Window` it was registered on
    -- a plausible, real pattern, e.g. a bound method reading other state
    off the window -- forms a reference cycle plain refcounting can never
    break. Without real GC participation this leaks forever; this proves
    CPython's cyclic collector actually reclaims it, not just that the
    methods exist and don't crash.

    Uses a bound method as the materializer, not a plain nested-function
    closure over a local variable: a closure over an enclosing function's
    local shares one cell with that local (confirmed directly -- `del`
    on the local clears the shared cell immediately, so it can never
    outlive its own defining scope's `del`, meaning it can't be used to
    prove a cycle survives past that point at all). A bound method's
    `__self__` is a genuine, independent strong reference living on the
    method object itself, which is what a real materializer capturing
    "the window it belongs to" would actually look like in practice.
    """

    class Holder:
        """A plain Python object to close the cycle through and to
        weak-reference -- a bare `Window` (`#[pyclass(unsendable, name =
        "Window")]`, no `weakref` flag) doesn't support `weakref.ref`
        itself, but `Holder`'s own liveness is exactly tied to the
        cycle's: it's reachable only via the bound method `window`'s
        materializer dict stores, and `window` is reachable only via
        `holder.window`.
        """

        def __init__(self):
            self.window = None

        def materialize(self, idx):
            # Touching `self.window` is what makes this a real "reads
            # other state off the window" materializer, per this
            # function's own docstring -- not load-bearing for the cycle
            # itself, which is `__self__` alone.
            self.window
            return (0, 0, 0, 255)

    holder = Holder()
    window = Window(width=50, height=50)
    holder.window = window

    # window -> materializers -> holder.materialize (bound method) ->
    # __self__ -> holder -> .window -> window.
    window.add_virtual_list(item_count=10, item_extent=10.0, materialize=holder.materialize)

    holder_ref = weakref.ref(holder)
    del window
    del holder
    assert holder_ref() is not None, (
        "sanity check: a real reference cycle must survive plain refcounting alone "
        "(if this fails, the test itself isn't constructing a real cycle)"
    )

    gc.collect()
    assert holder_ref() is None, (
        "the cycle (window <-> bound-method materializer <-> holder) must be "
        "collected by CPython's cyclic GC once nothing outside it references any "
        "part of it -- if this fails, __traverse__/__clear__ aren't actually making "
        "window's own side of the cycle visible to the collector"
    )


def test_window_scroll_reaches_a_virtual_lists_real_scroll_offset_with_no_error():
    """M8 Phase 3 (§11.7): `Window.scroll` is the no-live-window-needed
    proof pattern `click()`/`hover()` already established -- dispatches
    a real `InputEvent::Scroll` at the given node's own center point.
    The definitive proof that this genuinely moves/clamps a real
    `VirtualList`'s own `scroll_offset` is `engine-core::tree`'s own
    `dispatch_scroll_over_a_virtual_lists_child_updates_its_real_
    scroll_offset` and `engine-render`'s own `virtual_list_scroll.rs`
    pixel tests, not this one -- there's no Python-level way to read
    `scroll_offset` back (no consumer needs one yet), so this proves
    only that the real FFI call chain (Python -> Tree::dispatch's real
    scroll-bubbling -> `NodeKind::VirtualList`) runs without error: a
    scroll dispatched directly over the list's own root pixel (`Tree::
    dispatch`'s "walk up to the nearest VirtualList ancestor" finds it
    immediately, itself), and a scroll that hits a plain `Rect` with no
    `VirtualList` ancestor at all (a true no-op, must not raise either).
    """
    window = Window(width=200, height=100)
    vl = window.add_virtual_list(
        item_count=20, item_extent=20.0, materialize=lambda idx: (0, 0, 0, 255)
    )
    window.set_virtual_list_window(vl, 0, 5)

    window.scroll(vl, 50.0)
    window.scroll(vl, -1000.0)

    plain_rect = window.add_rect(background=(0, 0, 0, 255), width=10, height=10)
    window.scroll(plain_rect, 50.0)
