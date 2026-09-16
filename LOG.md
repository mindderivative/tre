# Log: M5 Phase 3 — `NodeKind::Canvas` + Custom Hit-Testing (§11.10, §11.11)

Corresponds to `BUILD_TRACKER.md` M5 Phase 3. Adds the last framework
primitive §11.10/§11.11 name that didn't exist: custom-drawn content
plus a custom hit-test override for it.

## Investigation before writing code

- **Neither `engine-core` nor `engine-render` can call into Python --
  confirmed via each crate's own `Cargo.toml` and the real crate DAG
  (§4): `engine-py` depends on `engine-render`/`engine-core`, never the
  reverse.** A literal "draw callback fired during paint" or "hit-test
  callback fired during `Tree::hit_test`" isn't buildable: `paint_node`
  and `hit_test_at` are pure-Rust, GIL-free recursions, and
  `update_hover`/`dispatch`'s internal calls to `self.hit_test` have no
  closure-injection point today without a much larger signature change
  reaching every caller.
- **The real, buildable design, confirmed by re-reading the
  `VirtualList` materializer precedent closely:** that callback isn't
  invoked during layout or paint either -- it fires at one well-defined,
  `engine-py`-owned sync point (`Window.set_virtual_list_window`,
  called explicitly by the app), storing its *result* (real `Node`s)
  into `Tree`. Canvas follows the identical shape: `Window.add_canvas`
  stores the Python `draw` callback (like `materializers`);
  `Window.redraw_canvas` is the real "materialize item N"-equivalent
  entry point, invoked explicitly by the app (not automatically every
  frame -- a real, deliberate scope narrowing, additive if a future
  need asks for it), passing a new `CanvasContext` pyclass and storing
  its *result* -- plain `Vec<DrawCommand>` + `Option<CustomHitTest>`,
  no `Py<PyAny>` anywhere -- via a new, ordinary `Tree::
  set_canvas_content`. `paint_node`/`hit_test_at` only ever read this
  inert data.
- **Custom hit-testing needed real distance-to-path math, not a
  hand-rolled approximation.** Checked `kurbo = "0.13.1"`'s own
  `ParamCurveNearest` trait directly in its vendored source: every
  `PathSeg` variant implements `nearest(point, accuracy) -> Nearest {
  distance_sq, t }`. A `BezPath`'s `segments()` plus this trait gives an
  exact "distance from a point to any path made of lines and/or true
  bezier curves" primitive for free. This phase's own `stroke_path`/
  `set_hit_test_path` only build straight-line polylines from Python
  (a real, stated scope narrowing -- no curve-authoring API yet), but
  the hit-test math underneath already generalizes to a true curve
  without any future replacement needed.
- **Draw command vocabulary kept to exactly what M5 Phase 4's own
  validation example needs:** `FillRect`, `FillCircle`, `StrokePath`.
  Not a general vector-drawing API.

## What happened

New `engine-core/src/canvas.rs`: `CanvasState { commands: Vec<
DrawCommand>, hit_test: Option<CustomHitTest> }`, `DrawCommand {
FillRect, FillCircle, StrokePath }`, `CustomHitTest { Circle, Path }` --
all plain, `Clone`-able, `Py<PyAny>`-free data. New `NodeKind::
Canvas(CanvasState)` variant (`node.rs`).

`engine-core/src/tree.rs`: `Tree::set_canvas_content` (the one ordinary
mutation `redraw_canvas` calls). `hit_test_at`'s leaf test now branches:
a `Canvas` with `Some(hit_test)` uses `CustomHitTest::Circle` (distance-
to-center vs. radius) or `Path` (minimum `segment.nearest(...).
distance_sq` across the path vs. `tolerance²`, via `ParamCurveNearest`);
every other node (including a `Canvas` with `None`) keeps the ordinary
local-space rect test from M5 Phase 2, factored into a new free function
`rect_contains` so both branches share it.

`engine-render/src/lib.rs`: `paint_node`'s match gains a `NodeKind::
Canvas` arm replaying `state.commands` via `scene.set_paint`/
`fill_path`/`set_stroke`/`stroke_path` (all confirmed real, already-
vendored `vello_hybrid = "0.2.0"` APIs), in the same node-local
coordinate space every other `NodeKind` already draws into -- so an
ancestor's animated `transform` (M5 Phase 1) reaches `Canvas` content
with zero special-casing, proven by the new pixel test.

New `engine-py/src/canvas.rs`: `CanvasContext` pyclass -- `fill_rect`/
`fill_circle`/`stroke_path`/`set_hit_test_circle`/`set_hit_test_path`,
all pushing into plain Rust fields (no GC-traversal obligation, same
reasoning `context_menus`/`dock` already established).

`engine-py/src/window.rs`: new `canvas_draws: HashMap<NodeId,
Py<PyAny>>` field (`__traverse__`/`__clear__` updated exactly like
`materializers`). `Window.add_canvas(width, height, draw) -> Node`
(store now, invoke later). `Window.redraw_canvas(canvas)` -- the real
invocation: builds a `CanvasContext`, calls `draw` exactly once
(a raised exception propagates as a real `PyErr` directly -- simpler
than `set_virtual_list_window`'s own per-index error-deferral, since
this is exactly one call, not N), then stores the result via
`Tree::set_canvas_content`. New `EngineError::NotACanvas` (`error.rs`),
mirroring `NotAVirtualList` exactly.

New tests: two `engine-core` unit tests (`canvas_custom_circle_hit_test_
overrides_the_default_rect`, `canvas_custom_path_hit_test_uses_real_
distance_to_path`) prove the override is real, not just a narrower rect
-- a point inside the node's own bounding box but outside the custom
shape genuinely misses. New `engine-render/tests/canvas_paint.rs`
pixel-readback test proves `DrawCommand`s paint at their correct,
transform-composed on-screen position. New `tests/test_canvas.py` (8
tests): the FFI wiring itself (callback invocation, both ownership
error paths, exception propagation, cyclic-GC participation) --
matching `test_splitter.py`/`splitter_drag_dispatch.rs`'s own established
Python-FFI-test/Rust-pixel-test split, since the definitive "did it
actually draw/hit-test correctly" proof has no meaningful Python-level
equivalent. New `examples/canvas.py`.

**A real naming bug caught by running the new pytest tests, not found
by review:** `CanvasContext`'s Rust methods originally took a parameter
named `color_` (to avoid shadowing a same-named free helper function) --
pyo3 exposes Python keyword arguments by their exact Rust parameter
name, so every call site's own documented `color=(r,g,b,a)` keyword
failed with `TypeError: unexpected keyword argument 'color'`. Fixed by
renaming the free helper to `to_color` instead, freeing up `color` as
the real parameter name every method (and this phase's own example/
tests) actually uses.

Full `cargo test --workspace --release` clean (`engine-core` 47 tests,
up from 45), `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check` all clean. `maturin develop --release` + full
`pytest tests/` (68 passed, up from 60, 1 skipped) and all seven
examples (six existing + new `canvas.py`) confirmed clean.
