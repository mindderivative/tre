# LOG — M30 Phase 9 Step 2: Node Graph

- Read pyCopper's own real `NodeGraph` widget directly
  (`/home/phil/pyDev/projects/pyCopper/src/pycopper/widgets/
  nodegraph.py`, an additional working directory this session already
  has access to): draggable title-bar nodes, named ports, declared
  edges drawn as segments each paint, panning via scroll-offset --
  zoom deliberately excluded from its own v1 scope ("would distort
  glyph rasterisation... a real second feature").
- Read TRE's own real M5 Phase 4/M6 Phase 3 history directly in
  `BUILD_TRACKER.md` before designing anything: M5 Phase 4 found
  Python had no way to position a `Node` independently at all, so its
  own `examples/node_graph.py` drew the whole graph as `DrawCommand`s
  inside one `Canvas`. M6 Phase 3 closed that real gap (`x`/`y` on
  `add_rect`/`add_canvas`) and built `examples/positioned_graph.py` --
  real, independently-positioned, clickable nodes -- but still with
  interchangeable circles, no real node anatomy, no reparent/position
  automation an app didn't hand-roll itself.
- Investigated whether TRE's own machinery could support real,
  pannable/zoomable graph nodes without new engine-core/engine-render
  work. Confirmed via direct source read: `Node.add_child` (M6 Phase
  1) genuinely reparents (detach-then-attach via `Tree::try_add_child`)
  without touching the child's own `layout_style` -- so its existing
  `Position::Absolute` inset should re-resolve relative to its real
  new parent on the next layout pass, by ordinary taffy/CSS semantics.
  **Verified empirically before relying on it**, using the exact same
  real overlap-and-click hit-test technique `test_position.py` already
  established (a probe at the window's own default flow position, a
  child positioned to land exactly on that probe only if reparenting
  re-resolves its inset correctly) -- confirmed true on the first try.
- Investigated whether an ancestor's own `transform` composes into a
  reparented descendant's real `absolute_position`. A first attempt to
  verify this empirically via a synchronous Python script (no real
  render loop) gave a confusing negative result -- traced to a real,
  separate finding: `Node.animate(..., duration_ms=0)` does *not*
  apply synchronously at call time (`Animated::animate_to` only ever
  sets a pending `ActiveAnimation`, never writes `current` directly);
  it only takes effect on the *next* real `Tree::tick_all` pass, which
  only ever runs inside `App.run()`'s own render loop or `View`'s own
  reload path -- confirmed via grep, no other Python-facing call ticks
  it. Correctly resolved by trusting the already-existing, already-
  passing engine-core test `absolute_position_follows_an_ancestor_
  transform` (M6 Phase 4) instead, which proves the real composition
  directly at the Rust level, bypassing the animation-tick indirection
  entirely.
- Confirmed via direct source read: no general "clip children to this
  container's own bounds" mechanism exists for any `NodeKind` besides
  `VirtualList` (hardcoded, `engine-render/src/lib.rs`) -- a real,
  stated scope boundary for this step, not silently worked around:
  nodes/edges panned outside the graph's own viewport overflow
  visually, not clipped.
- Confirmed via direct source read: no Python-facing `PointerMoved`-
  while-pressed hook exists anywhere (`set_on_click`/`set_on_hover_*`
  are the only generic input hooks) -- a real, stated scope boundary:
  no drag-to-move mouse gesture, the same real constraint `Docking`'s
  own M4 Phase 9 "press+release only" scope boundary already found and
  documented for an analogous reason. `node.animate("transform", ...)`
  remains the real, available repositioning mechanism.
- Implemented `Window.add_node_graph(width, height, x, y)` in
  `crates/engine-py/src/window_factory.rs` -- a `surface_container_low`
  themed viewport, mechanically identical to `add_rect`.
- Implemented `Window.add_graph_node(graph, label, x, y, width,
  height)` -- a real composed node (a `surface_container` body under a
  `surface_container_high` title strip, `CARD_CORNER_RADIUS`, Title
  Small label reusing `TAB_LABEL_FONT_SIZE`/`_WEIGHT`), attached
  directly under `graph` (not `self.root`, a deliberate departure from
  every other `add_*` method's own convention, documented explicitly).
  Reused `open_menu`'s own `Rc::ptr_eq` cross-window safety check.
- **Ran a real empirical end-to-end check before writing any tests --
  it genuinely FAILED**: a click on a graph node's own real center
  point did not reach its own registered handler. Root-caused
  immediately (a repeat of an already-solved bug class this session):
  the title bar's own full-width `Rect` sits directly over the node's
  own vertical center (a 60dp-tall node's own center falls inside its
  own 32dp title band), so `hit_test_at`'s "recurse into children
  first" behavior let the title bar claim the click before the
  wrapper's own handler ever ran. Fixed with `tree.set_hit_testable(
  title_bar, false)`, the exact same real fix `Navigation Rail`'s
  active-indicator pill and `Tabs`'s content wrapper already applied
  this milestone. Re-verified empirically after the fix -- passed.
- Added `.pyi` stubs for both new methods.
- Wrote `tests/test_node_graph.py` (8 tests) -- all passed on the
  first run, including the real click-through proof (mirroring `test_
  position.py`'s own established technique) and a cross-window
  `ForeignNode` rejection test.
- Wrote a new example demonstrating four real graph nodes, a real
  edges `Canvas` reparented into the graph, a real graph-wide pan, and
  one node's own real reposition, all via `Node.animate("transform",
  ...)`.
- **Caught and corrected a real self-inflicted mistake**: the first
  draft of that example was written to `examples/node_graph.py` --
  which already existed (M5 Phase 4's own real, still-valid Canvas +
  `CustomHitTest::Circle` composition validation, committed at
  `28ce282`) -- silently overwriting it. Caught via `git status`
  showing the file as modified rather than new, before any commit.
  Restored the original byte-for-byte (`git show 28ce282:examples/
  node_graph.py`, rewritten back via Write after a required Read), and
  gave the new demonstration its own distinct name, `examples/
  graph_editor.py`. Both scripts re-run clean afterward.
- Fixed two real `mypy --strict` findings in `graph_editor.py`: an
  untyped `draw_edges(ctx)` callback parameter (annotated `ctx:
  CanvasContext`) and an unfixable lambda-default-argument type
  inference (replaced with a small named closure factory,
  `_make_click_handler`).
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (44 binaries green,
  unchanged -- this step is a pure `engine-py` composition, needing no
  new Rust unit test), `maturin develop --release`, `mypy --strict`
  clean against `examples/graph_editor.py`, `pytest tests/` (465
  passed, 1 skipped, up from 457), all 64 examples clean (including
  confirming the restored `examples/node_graph.py` still runs clean),
  showcase demo clean.
- Updated `BUILD_TRACKER.md` (Top Metrics row now 93%, Step 2 line,
  "Just closed"/"Up next" trailer), regenerated and republished the
  Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
