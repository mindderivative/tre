"""§14 step 15 Stage C (§11.7), M100: real, repeatable coverage of
`window.create("virtual_list", ...)` at the FFI boundary -- what
`tests/test_kinds.py`'s row-building tests don't cover: a raising
`size_hint` or `materialize` is logged and non-fatal, the window takes
part in cyclic GC when its materializer captures it, and wheel input
over the list or a plain node is safe. M100 removed `add_virtual_list`
and `set_virtual_list_window`: `tre` builds the visible rows itself.
"""

import gc
import weakref

from tre import Node, Window
from helpers import add


def box(window):
    return window.create("box", fill=(0, 0, 0, 255))


def test_a_virtual_list_with_item_extent_or_size_hint_is_a_node():
    window = Window(width=200, height=200)
    fixed = add(window, "virtual_list", item_count=10, item_extent=20.0,
                materialize=lambda _i: box(window), width=100, height=100)
    varied = add(window, "virtual_list", item_count=10, size_hint=lambda idx: 10.0 + idx,
                 materialize=lambda _i: box(window), width=100, height=100)
    assert isinstance(fixed, Node)
    assert isinstance(varied, Node)


def test_a_raising_size_hint_is_logged_and_non_fatal(capfd):
    window = Window(width=200, height=200)

    def size_hint(idx):
        if idx == 2:
            raise RuntimeError(f"item {idx} is cursed")
        return 10.0

    rows = add(window, "virtual_list", item_count=10, size_hint=size_hint,
               materialize=lambda _i: box(window), width=100, height=100)
    window.advance(0)
    assert "item 2 is cursed" in capfd.readouterr().err
    # M100: its offsets are unresolved, so it has no extent -- a wheel over
    # it once panicked the engine.
    window.simulate("wheel", node=rows, delta_y=50)


def test_a_raising_materializer_is_logged_and_non_fatal(capfd):
    window = Window(width=200, height=200)

    def materialize(idx):
        if idx == 2:
            raise RuntimeError(f"item {idx} is cursed")
        return box(window)

    rows = add(window, "virtual_list", item_count=10, item_extent=10.0,
               materialize=materialize, width=100, height=100)
    window.advance(0)
    assert "item 2 is cursed" in capfd.readouterr().err
    assert rows.get("layout_height") == 100.0


def test_window_participates_in_cyclic_gc_when_its_materializer_captures_it_back():
    """A materializer that captures the window it was registered on --
    a bound method reading other state off the window -- forms a cycle
    plain refcounting can never break; CPython's cyclic collector must
    reclaim it (`PyWindow`'s `__traverse__`/`__clear__`). A bound method
    rather than a closure: its `__self__` is an independent strong
    reference, as a real materializer's would be.
    """

    class Holder:
        def __init__(self):
            self.window = None

        def materialize(self, idx):
            return self.window.create("box")

    holder = Holder()
    window = Window(width=50, height=50)
    holder.window = window

    # window -> callbacks -> holder.materialize -> __self__ -> holder -> .window -> window
    add(window, "virtual_list", item_count=10, item_extent=10.0,
        materialize=holder.materialize, width=50, height=50)

    holder_ref = weakref.ref(holder)
    del window
    del holder
    assert holder_ref() is not None, "sanity check: a real cycle survives refcounting"

    gc.collect()
    assert holder_ref() is None, "the cycle must be collected by the cyclic GC"


def test_wheel_input_over_a_virtual_list_or_a_plain_node_is_safe():
    """The definitive proof that a wheel moves and clamps a list's
    `scroll_offset` is `engine-core::tree`'s own tests and
    `engine-render`'s `virtual_list_scroll.rs`; this proves only that the
    FFI chain runs, over the list and over a node with no list above it.
    """
    window = Window(width=200, height=100)
    rows = add(window, "virtual_list", item_count=20, item_extent=20.0,
               materialize=lambda _i: box(window), width=100, height=100)
    window.simulate("wheel", node=rows, delta_y=50.0)
    window.simulate("wheel", node=rows, delta_y=-1000.0)

    plain = add(window, "box", fill=(0, 0, 0, 255), width=10, height=10)
    window.simulate("wheel", node=plain, delta_y=50.0)
