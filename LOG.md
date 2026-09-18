# Log: M27 Phase 3 — Motion & Custom Drawing Screen

`demo/showcase.py`'s "motion" placeholder replaced with a real screen:
four real animation triggers (`opacity`/`corner_radius`/`elevation`/
`shape` morph) on a shared demo card, each fired by a real dispatched
click on its own button — not auto-playing on a timer with nothing
driving it, per the milestone's own stated bar — plus a live `Canvas`
(a real node graph, the same shape `examples/node_graph.py` already
proves: edges/nodes as real `DrawCommand`s, one node with a real
`CustomHitTest::Circle`) with its own real `transform` pan/zoom,
`examples/pan_zoom.py`'s own mechanism applied to a `Canvas` node for
the first time.

**A real, connected fix this phase directly needed — exactly the
residual limitation M27 Phase 1's own log predicted, now hit for
real:** building this screen's `Canvas` from inside a nav-button click
handler (an entirely ordinary "click here, build new UI there"
pattern) panicked with `RuntimeError: Already borrowed`, because
`add_canvas`/`add_virtual_list` were the two `PyWindow` methods Phase 1
deliberately left `&mut self` — they write into `materializers`/
`canvas_draws`, the only two `PyWindow` fields not already behind a
`RefCell`. Fixed for real this time: wrapped both fields in their own
`RefCell<HashMap<...>>`, converted `add_virtual_list`/`add_canvas` to
`&self` (mirroring every other real `add_*` method), and updated their
own read sites (`set_virtual_list_window`/`redraw_canvas`) and the
`__traverse__`/`__clear__` GC hooks to borrow through the new
`RefCell` instead of accessing the map directly. No more residual
limitation — every `PyWindow` method can now be called from within any
other one's own call stack.

**Two more real bugs caught only by actually running it:**

1. A first draft's Tab-order verification assumed the gallery's own
   `Checkbox` was still the very first focusable control by the time
   its check ran — but Phase 3 moved the nav-button Tab-order proof to
   run *first* (a cleaner, more robust ordering in its own right,
   since real content on both screens now means no screen is ever
   "placeholder-only" anymore), which itself consumes the first two Tab
   stops. Fixed by reducing the gallery's own Slider Tab-press count
   from 4 to 2, with an explicit comment stating the real call-order
   dependency rather than leaving it implicit.
2. A first draft assumed `Node.set_hit_test_circle` only *narrows*
   hits elsewhere in a canvas's bounds while leaving the default
   rectangular hit-test intact for the canvas's own center — the
   opposite of its real, documented behavior (it *replaces* the
   default hit test entirely). A real dispatched click at the canvas's
   own center (all `Window.click` can target) missed the custom circle
   (centered on a specific graph node, not the canvas's own geometric
   center) and never fired the handler. Fixed by asserting the real,
   correct behavior instead of the wrong assumption — the click
   genuinely misses, proving the override took effect.

Full `cargo test --workspace --release` (all pre-existing suites
unmodified and passing — widening `&self` can't break a test that
never relied on exclusivity)/clippy `-D warnings`/fmt clean. `maturin
develop --release` + `pytest tests/` (187 passed, unchanged, 1
pre-existing skip), all 33 pre-existing examples, and the updated demo
confirmed clean with the real display.

M27 Phase 3 — Motion & Custom Drawing Screen is now complete. M27
continues with Phase 4 (data & layout screen: virtualized list +
docking + a declarative `View` panel).
