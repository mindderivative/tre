# Plan: M5 Phase 3 — `NodeKind::Canvas` + Custom Hit-Testing (§11.10, §11.11)

## Context

M5 Phases 1-2 (§11.9 transform composition, §11.10's transform-aware
default hit-test) are done. This phase adds the last missing primitive
§11.10/§11.11 name: `NodeKind::Canvas` for custom-drawn content, plus
the custom hit-test override §11.10's own text describes ("a bezier
curve within N pixels of the point, a specific plotted data point... 
overrides the default rect test for that node only").

## Investigation before writing code

- **The real architectural constraint: neither `engine-core` nor
  `engine-render` can call into Python.** `engine-core` has zero `pyo3`
  dependency (confirmed via its `Cargo.toml`); `engine-render` depends
  on `engine-core` only, not `engine-py` (§4's crate DAG runs the other
  direction: `engine-py` depends on `engine-render`/`engine-core`, never
  the reverse). So a live "draw callback fired during the paint walk"
  or "hit-test callback fired during `Tree::hit_test`" is not buildable
  as literally stated -- both `paint_node` and `hit_test_at` are deep,
  hot-path, pure-Rust recursions with no GIL, no `Python<'_>` token,
  and (for `hit_test_at` specifically) no closure-injection point today
  (`update_hover`/`dispatch`'s `PointerPressed`/`PointerReleased` arms
  call `self.hit_test` internally, with no opportunity to thread a
  per-call Python closure through without a much larger signature
  change touching every caller).
- **The real, buildable design, confirmed by re-reading the
  `VirtualList` materializer precedent closely (§11.7/§8):** that
  callback isn't invoked during layout or paint either -- it's invoked
  at one well-defined, `engine-py`-owned synchronization point
  (`Window.set_virtual_list_window`, called explicitly by the app, e.g.
  on scroll), which stores its *result* (real `Node`s) into `Tree`.
  Nothing downstream (`paint_node`, `hit_test_at`) ever touches Python.
  Canvas follows the identical shape: `Window.add_canvas(...)` stores
  the Python draw callback (like `materializers`); a new
  `Window.redraw_canvas(node)` is the actual "materialize item N"-
  equivalent entry point -- invoked explicitly by the app (on data
  change, not automatically every frame; building an automatic
  per-frame Python round-trip would be new, disproportionate scope this
  phase doesn't need and no consumer has asked for, matching the "don't
  build ahead of a real need" discipline `ItemExtent`'s own doc comment
  already established) -- calls the stored callback *once*, passing it
  a new `CanvasContext` pyclass exposing an imperative drawing API
  (`fill_rect`/`fill_circle`/`stroke_path`/`set_hit_test_circle`/
  `set_hit_test_path`), and stores the *result* -- plain, inert
  `Vec<DrawCommand>` + `Option<CustomHitTest>` data, no `Py<PyAny>`
  anywhere in it -- into the node's own `CanvasState` via a new,
  ordinary (non-callback) `Tree::set_canvas_content` method. `paint_node`
  and `hit_test_at` then only ever read this inert Rust data, never
  Python, keeping both hot paths exactly as free of `pyo3` as they are
  today.
- **Custom hit-testing needs real distance-to-path math, not a hand-
  rolled approximation.** Checked `kurbo = "0.13.1"`'s own
  `ParamCurveNearest` trait directly in its vendored source: every
  `PathSeg` variant (`Line`/`Quad`/`Cubic`) implements `nearest(point,
  accuracy) -> Nearest { distance_sq, t }`. A `BezPath`'s own
  `segments()` iterator plus this trait gives an exact, real
  "distance from a point to any path made of lines and/or true bezier
  curves" primitive for free -- no hand-rolled polyline-distance math,
  and it already supports genuine curves if a future need ever
  authors one from Python, even though this phase's own `stroke_path`/
  `set_hit_test_path` only builds straight-line `PathSeg::Line`
  segments from a list of points (a real, stated scope narrowing:
  Python doesn't get bezier *authoring* this phase, only straight
  polylines -- but the hit-test math underneath is the same real
  mechanism a curve would use, not a special case that would need
  replacing later).
- **Draw command vocabulary kept to the minimum this phase's own
  validation needs (Phase 4's node-graph/chart example):** `FillRect`,
  `FillCircle`, `StrokePath` -- circles for graph nodes/data points,
  strokes for edges/chart lines, rects for simple backgrounds/bars.
  Not a general vector-drawing API; additive if a real future need asks
  for more (fill paths, gradients, images), matching this codebase's
  own "don't build ahead of need" discipline throughout.
- **`vello_hybrid::Scene::stroke_path`/`set_stroke` are real, already
  vendored APIs** (confirmed directly in `vello_hybrid = "0.2.0"`'s
  source) -- `paint_node`'s new `Canvas` arm uses them directly, no new
  rendering mechanism.
- **Custom hit-test overrides the default rect test *only when
  present*** -- a `Canvas` node with no `hit_test` set still uses the
  ordinary transform-aware rect test from Phase 2, matching §11.10's
  own "nodes with no custom hit-test use the rect default" text exactly.

## Approach

1. **New `engine-core/src/canvas.rs`**: `CanvasState { commands:
   Vec<DrawCommand>, hit_test: Option<CustomHitTest> }`,
   `DrawCommand { FillRect, FillCircle, StrokePath }`, `CustomHitTest {
   Circle, Path }`. `NodeKind::Canvas(CanvasState)` new variant in
   `node.rs`.
2. **`engine-core/src/tree.rs`**: `Tree::set_canvas_content(&mut self,
   id, commands, hit_test)`. `hit_test_at`'s leaf test branches: a
   `Canvas` node with `Some(hit_test)` uses it (`Circle`: distance-to-
   center vs. radius; `Path`: minimum `segment.nearest(...).distance_sq`
   across the path vs. `tolerance²`); every other node (including a
   `Canvas` with no custom hit-test) keeps the existing local-space rect
   test from Phase 2, unchanged.
3. **`engine-render/src/lib.rs`**: `paint_node`'s match gains a
   `NodeKind::Canvas(state)` arm, replaying `state.commands` via
   `scene.set_paint`/`fill_path`/`set_stroke`/`stroke_path`, in the same
   node-local coordinate space (M5 Phase 1) every other `NodeKind`
   already draws into.
4. **New `engine-py/src/canvas.rs`**: `CanvasContext` pyclass (no
   `Py<PyAny>` fields -- no GC-traversal obligation, matching
   `context_menus`'/`dock`'s own reasoning), `#[pymethods]`
   `fill_rect`/`fill_circle`/`stroke_path`/`set_hit_test_circle`/
   `set_hit_test_path` pushing into plain Rust fields.
5. **`engine-py/src/window.rs`**: `canvas_draws: HashMap<NodeId,
   Py<PyAny>>` (new field, `__traverse__`/`__clear__` updated exactly
   like `materializers`). `Window.add_canvas(width, height, draw) ->
   Node` (stores `draw`, inserts a `NodeKind::Canvas` node -- mirrors
   `add_virtual_list`'s own "store now, invoke later" shape exactly).
   `Window.redraw_canvas(node)` (the real invocation entry point --
   builds a `CanvasContext`, calls `draw` once, propagates a raised
   exception as a real `PyErr` -- simpler than the materializer's own
   per-index error-deferral, since this is exactly one call, not N --
   then stores the result via `Tree::set_canvas_content`).
6. **`engine-py/src/error.rs`**: `EngineError::NotACanvas`, mirroring
   `NotAVirtualList` exactly, for `redraw_canvas` called on a non-
   `Canvas`/foreign node.
7. **New `engine-render` pixel-readback test**: a `Canvas` node's
   `FillCircle`/`StrokePath` commands paint real pixels at their
   expected on-screen position, transform-composed like every other
   `NodeKind` (proves Phase 1's composition reaches `Canvas` too, with
   zero special-casing).
8. **New `engine-core` unit tests**: `hit_test` finds a `Canvas` node
   via its custom `Circle` hit-test at a point outside its bounding
   rect-but-inside-the-circle (and misses just outside the circle but
   inside the rect -- the concrete proof the override is real, not
   just "still passes the rect test"); same shape for `Path`.
9. **New pytest tests + example**: `Window.add_canvas`/`redraw_canvas`
   round-trip (`CanvasContext` methods actually populate `CanvasState`,
   readable back via the real pixel test above being the definitive
   proof, matching `test_splitter.py`/`splitter_drag_dispatch.rs`'s own
   Rust-pixel-test/Python-FFI-test split); a raised exception inside
   `draw` is caught/logged/non-fatal, matching §9's established policy.

## Files to touch

- `crates/engine-core/src/canvas.rs` -- new.
- `crates/engine-core/src/node.rs` -- `NodeKind::Canvas` variant.
- `crates/engine-core/src/tree.rs` -- `set_canvas_content`, `hit_test_at`
  custom-hit-test branch.
- `crates/engine-core/src/lib.rs` -- register + re-export `canvas`.
- `crates/engine-render/src/lib.rs` -- `paint_node`'s `Canvas` arm, new
  test.
- `crates/engine-py/src/canvas.rs` -- new, `CanvasContext`.
- `crates/engine-py/src/window.rs` -- `canvas_draws`, `add_canvas`,
  `redraw_canvas`, GC methods.
- `crates/engine-py/src/error.rs` -- `NotACanvas`.
- `crates/engine-py/src/lib.rs` -- register `CanvasContext`.
- `tests/test_canvas.py` -- new.
- `examples/canvas.py` -- new (a small static scene: a couple of
  circles + a stroked polyline "edge" between them, proving the real
  end-to-end path before Phase 4's own node-graph validation example
  composes it with transform/virtualization).

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo fmt --check`.
- Full existing pixel-readback suite must stay green unmodified (purely
  additive: a new `NodeKind` variant, a new match arm, no existing arm
  touched).
- `maturin develop && python -m pytest tests/ -v` plus all examples run.
