# LOG — M39 Phase 2 Step 1: Loading Indicator

- User's own explicit instruction: "Start" continued into M39 Phase 2
  (item 1 from the gap-sweep answer, second in the user's own chosen
  order): "Loading Indicator + Time Picker Dial." This entry covers
  Step 1 only -- Loading Indicator.
- Real research first, the same discipline every prior MD3 phase in
  this catalog has already established (the M3 site's own spec page
  is JS-rendered, no fetchable static content): found via a real,
  cited open-source port's own README that MD3 Expressive's real
  loading indicator is genuinely NOT a simple spinner -- a real
  looping morph across seven named shapes (Circle, Cookie 4/6/7/9/12,
  Pentagon, Pill, Sunny, Oval, ...) with genuine spring physics and a
  dual counter-rotation formula.
- Paused via `AskUserQuestion` before writing any code: full real
  fidelity (sourcing/authoring 7 real shape vertex sets, a new
  spring-physics motion primitive, a new looping-animation concept --
  none of which this codebase has any precedent for) vs. a real,
  honest v1 simplification (a smaller set of procedurally-generated
  real shapes, morphed via the already-proven `Animated<ShapeKey>`
  machinery with plain easing rather than springs). User chose the
  simplified v1 explicitly.
- New `NodeKind::LoadingIndicator(LoadingIndicatorState)` (`node.rs`)
  -- `LoadingIndicatorState { shapes: Vec<ShapeKey>, current_shape:
  usize }`. `shapes` is built once at construction to the real node's
  own `w x h` (since `ShapeKey` carries no scale transform of its
  own) via a new `LoadingIndicatorState::new(w, h)` constructor.
- New `crate::shape_morph::loading_indicator_shapes` module -- four
  real, procedurally-generated shapes, all built via trigonometry, no
  sourced SVG vertex data: `pentagon` (a regular 5-gon via a new
  private `ngon_path` helper), `cookie` (a real 12-vertex soft-
  scalloped shape via a new private `scalloped_path` helper), `pill`
  (a `RoundedRect` tessellated at a loose `1.0` tolerance to keep its
  vertex count comparable to the hand-authored polygon shapes, for a
  better `ShapeKey` morph correspondence), `oval` (a 2:1 `Ellipse`,
  matching MD3's own real Oval shape's proportions, same loose
  tolerance). A real, recognizable, in-spirit subset of MD3
  Expressive's own seven shapes -- not sourced vertex-for-vertex.
- New `Tree::LOADING_INDICATOR_SHAPE_DURATION: Duration = 650ms` --
  real, cited timing kept from the research even though the physics
  model itself was simplified to plain easing.
- The real automatic-loop mechanism, added to `Tree::tick_all` right
  after `node.paint.tick(now, &mut completed)` runs for each node:
  ```rust
  if let NodeKind::LoadingIndicator(state) = &mut node.kind
      && node.paint.shape.active.is_none()
      && !state.shapes.is_empty()
  {
      state.current_shape = (state.current_shape + 1) % state.shapes.len();
      let next = state.shapes[state.current_shape].clone();
      node.paint.shape.animate_to(next, Self::LOADING_INDICATOR_SHAPE_DURATION, MotionCurve::Linear, now);
      any_active = true;
  }
  ```
  This exploits a real, already-existing identity in `Animated<T>::
  tick`'s own implementation: `active` is set to `None` BOTH when a
  field was never animating AND the exact tick a transition just
  crosses its duration threshold and settles -- there is no separate
  "just completed" state to branch on. Checking `active.is_none()`
  immediately after `paint.tick()` runs, every single frame, is
  therefore sufficient to both kick off the very first transition on
  a freshly-constructed indicator AND immediately retarget to the
  next shape the instant the previous one finishes -- all with zero
  app-side wiring; the loop starts and keeps running the instant a
  node is constructed.
- `engine-render::paint_node`'s own existing `NodeKind::Rect |
  NodeKind::Splitter(_)` shape-morph-aware fill match arm widened to
  `NodeKind::Rect | NodeKind::Splitter(_) | NodeKind::LoadingIndicator(_)`
  -- a `LoadingIndicator`'s entire real visual appearance is
  `PaintProperties.shape`, already fully handled by the existing fill
  path, so this needed zero new paint code.
- `Window.add_loading_indicator(size, color, x, y) -> Node`
  (`engine-py::window_factory.rs`): initializes `paint.shape` directly
  to `state.shapes[0]` at construction time (never left at `ShapeKey::
  empty()`), the identical "avoid a first-tick flash-from-empty
  visual bug" discipline Split Button already established in M38
  Phase 4. Color defaults to the current theme's own `primary` role.
- Fixed two real, non-exhaustive-match compile errors surfaced
  immediately by `cargo check` after adding the new `NodeKind`
  variant: `engine-render/src/lib.rs`'s own `paint_node` match, and
  `engine-py/src/node.rs`'s own `kind_name` function (given its own
  `"LoadingIndicator"` string).
- **Real bug found and fixed in my own new test's timing model --**
  the same "verify, don't assume" discipline this whole project has
  already applied repeatedly to its OWN code, not just the engine's:
  the first draft of `tick_all_advances_a_loading_indicator_to_the_
  next_shape_once_settled_and_wraps_at_the_end` called `tree.
  tick_all(now)` then `tree.tick_all(now + 700ms)` per loop iteration
  with a FIXED `now` that never advanced across iterations -- this
  double-counted transitions, since settle-and-immediately-retrigger
  genuinely happens within a SINGLE `tick_all` call the instant
  `active` crosses to `None` (confirmed by tracing `Animated::tick`'s
  own real implementation plus the new check's own placement right
  after it, in the same call). Fixed by redesigning the test around
  one monotonically-advancing `now` variable, calling `tree.
  tick_all(now)` exactly ONCE per iteration and advancing `now +=
  700ms` only between iterations -- correctly reproduces the real
  0->1->2->3->0 cycle across exactly 4 calls.
- Full verification chain, all green: `cargo check --workspace --all-
  targets`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo fmt` + `cargo fmt --check`; `cargo test --workspace --
  release` (`engine-core`: 201 passed, +3 from this step); `maturin
  develop --release`; `pytest tests/` (569 passed, 1 skipped, up from
  564, +5 new tests in `tests/test_loading_indicator.py`); all 76
  examples including the new `examples/loading_indicator.py` (a real,
  live 200-frame `App().run()` loop -- at 650ms/shape this guarantees
  several genuine transitions actually happen over real simulated
  time, the decisive proof the loop keeps advancing rather than
  stalling after the first one); showcase demo.
- `python/tre/_core.pyi` given a full, paired `add_loading_indicator`
  stub, including the "real, honest v1 simplification" note.
- `BUILD_TRACKER.md` Phase 2's own Step 1 bullet flipped to done with
  a real completion note; parser re-confirmed balanced (39
  milestones, 127 phases, 218 items, unchanged); artifact regenerated
  and republished. **Phase 2 itself remains open -- Step 2 (Time
  Picker Dial) has not been started.**
