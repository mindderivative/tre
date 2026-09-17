"""§14 step 6: real, repeatable coverage of `engine-py`'s FFI boundary --
node creation, the one property setter (`Node.animate`), and its two
`EngineError` paths (`UnknownProperty` -> `ValueError`, `TypeMismatch` ->
`TypeError`, §8's own design). `App.run()` itself isn't exercised here
(it blocks and opens a real window) -- `examples/animate_rect.py` and
`examples/two_windows.py` are that proof; this file is the fast,
no-window-needed regression coverage that should never need a display
to run.

§14 step 14 (§11.1) split `Window` back out of `App` -- node creation
now happens on a `Window`, not `App` itself; `App` only collects
`Window`s and drives them. `test_app_requires_at_least_one_window`
covers the one new `App`-level behavior that split introduced.

Requires `maturin develop` to have installed the compiled extension into
the active environment first -- these tests import the real `tre`
package, not a mock.
"""

import gc
import weakref

import pytest

from tre import App, Node, Window


def test_add_rect_returns_a_node():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0x67, 0x50, 0xA4, 0xFF), width=50, height=50)
    assert isinstance(node, Node)


def test_animate_accepts_each_known_paint_property():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    # None of these should raise -- registering an animation is a
    # fire-and-forget call (§8: "registers work and returns, never
    # blocks"), so success is simply the absence of an exception.
    node.animate("opacity", 0.5, duration_ms=100)
    node.animate("corner_radius", 12.0, duration_ms=100)
    node.animate("elevation", 4.0, duration_ms=100)
    node.animate("background", (255, 255, 255, 255), duration_ms=100)
    node.animate("transform", (10.0, 20.0, 1.5), duration_ms=100)


def test_animate_defaults_duration_to_an_instant_snap():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.animate("opacity", 0.2)  # duration_ms omitted -- must not raise


def test_unknown_property_raises_value_error_naming_the_node_kind():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError, match="Rect has no property 'not_a_real_property'"):
        node.animate("not_a_real_property", 1.0)


def test_type_mismatch_raises_type_error_naming_expected_and_actual():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(TypeError, match="expects a float, got str"):
        node.animate("opacity", "not a float")


def test_background_requires_a_four_tuple_not_a_float():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(TypeError, match="expects an \\(r, g, b, a\\) tuple"):
        node.animate("background", 0.5)


def test_transform_requires_a_translate_x_translate_y_scale_three_tuple():
    """M6 Phase 2 (§8): `transform` is `(translate_x, translate_y,
    scale)`, not a raw affine-coefficient tuple -- matches `Interpolate
    for Affine`'s own real limitation (M5 Phase 1, `PLAN.md`)."""
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(TypeError, match="expects a \\(translate_x, translate_y, scale\\) tuple"):
        node.animate("transform", 0.5)


def test_app_requires_at_least_one_window():
    app = App()
    with pytest.raises(RuntimeError, match="add_window"):
        app.run()


def test_app_run_tracing_subscriber_init_does_not_panic_across_multiple_calls():
    """M16 Phase 1 (§3, §9): `App.run()` calls `tracing_subscriber::fmt
    ::try_init()` at its own real top, before even the "no windows"
    check above -- a global `tracing` subscriber can only ever be
    installed once per process, so a second real call (this whole
    pytest process already made one, via the test above, and every
    other test file in this suite that calls `.run()`) must be a
    silent no-op, not a panic. `try_init` (not `init`) is the real,
    load-bearing choice this test actually exercises -- calling `.run()`
    a second time here, real proof, not just reasoning about the API.
    """
    app = App()
    with pytest.raises(RuntimeError, match="add_window"):
        app.run()  # must not panic on subscriber re-init


def test_animate_accepts_a_real_on_complete_callback():
    """M9 Phase 2 (§5): `on_complete`, when given, must not raise --
    registering it is a fire-and-forget call, the same as `animate()`
    itself (§8's own design rule). The definitive proof it actually
    *fires* needs a real, running `App.run()` loop (this file's own
    docstring: that needs a real display, so it isn't exercised here,
    `examples/animation_completion.py` is that real, live proof) --
    `engine-core`'s own tests (M9 Phase 1) already proved the tick-level
    mechanics exhaustively; this is the FFI boundary's own smoke test.
    """
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.animate("opacity", 0.5, duration_ms=100, on_complete=lambda: None)


def test_begin_container_transform_accepts_a_real_on_complete_callback():
    """M9 Phase 3 (§5): `Window.begin_container_transform`'s own new
    `on_complete` parameter, the same FFI-smoke-test scope as `Node.
    animate`'s own `on_complete` tests above (§5) -- `App.run()` needs a
    real display to prove live firing, so that's `examples/
    container_transform.py`'s own job, not this file's. The definitive
    proof the handle actually reaches `Tree::tick_all`'s own real drain
    is `engine-md3`'s own `a_real_on_complete_handle_reaches_tick_alls_
    own_drain_when_the_transition_finishes` test (M9 Phase 3).
    """
    window = Window(width=200, height=200)
    trigger = window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=40, height=40)
    destination = window.add_rect(background=(0x00, 0x00, 0xFF, 0xFF), width=100, height=100)
    window.begin_container_transform(
        trigger, destination, duration_ms=100, on_complete=lambda: None
    )


def test_animate_still_works_with_on_complete_omitted():
    # Every other test in this file already omits `on_complete` and
    # still passes -- this one states that regression explicitly.
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.animate("opacity", 0.5, duration_ms=100)


def test_window_participates_in_cyclic_gc_when_an_on_complete_callback_captures_it_back():
    """M9 Phase 2 (§5): the same real reason `PyWindow` already
    implements `__traverse__`/`__clear__` for `handlers`/`materializers`
    (`test_virtual_list.py`'s own analogous test) applies to
    `completions` too -- it holds real `Py<PyAny>` callbacks the same
    way. An `on_complete` callback that captures the very `Window` it
    was registered on (a plausible, real pattern -- a bound method
    reading other state off the window) forms a reference cycle plain
    refcounting can never break; this proves CPython's cyclic collector
    actually reclaims it, not just that the methods exist and don't
    crash. Uses a bound method, not a closure over a local, for the same
    real reason `test_virtual_list.py`'s own analogous test does (a
    closure over an enclosing local shares one cell with it, so it can't
    outlive that scope's own `del`).
    """

    class Holder:
        def __init__(self):
            self.window = None

        def on_complete(self):
            self.window

    holder = Holder()
    window = Window(width=200, height=200)
    holder.window = window
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)

    # window -> completions -> holder.on_complete (bound method) ->
    # __self__ -> holder -> .window -> window.
    node.animate("opacity", 0.5, duration_ms=100, on_complete=holder.on_complete)

    holder_ref = weakref.ref(holder)
    del window
    del node
    del holder
    assert holder_ref() is not None, (
        "sanity check: a real reference cycle must survive plain refcounting alone "
        "(if this fails, the test itself isn't constructing a real cycle)"
    )

    gc.collect()
    assert holder_ref() is None, (
        "the cycle (window <-> bound-method on_complete callback <-> holder) must be "
        "collected by CPython's cyclic GC once nothing outside it references any part of "
        "it -- if this fails, completions' own __traverse__/__clear__ aren't actually "
        "making window's own side of the cycle visible to the collector"
    )
