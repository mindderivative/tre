# Plan: M7 Phase 5 — Container Transform (§7.6), closing the milestone

Corresponds to `BUILD_TRACKER.md` M7 Phase 5's own scoping: `Node::
computed_layout() -> taffy::Layout` (Step 1, the one genuinely new
`engine-core` primitive §7.6 names), plus the real five-step
choreography (capture, insert destination, drive a synchronized
animation set, staggered content cross-fade, teardown) as a single
`engine-md3` helper (Step 2).

## Investigation before writing code

- **Real finding: Step 1's "genuinely new primitive" already exists.**
  `ARCHITECTURE.md` §7.6's own Capture text (written before the real
  crate/API split) asks for "`engine-core` must expose computed layout
  output per node as a queryable value... e.g. `Node::computed_layout()
  -> taffy::Layout`." Read `crates/engine-core/src/tree.rs` directly:
  `Tree::layout(id) -> &Layout` (real position+size from Taffy's last
  pass, already `pub`, already used externally by `engine-py`'s
  `Window.click`/`hover`) and `Tree::absolute_position(id) -> (f64, f64)`
  (M6 Phase 4, transform-aware, composes the full ancestor chain) —
  together already expose everything Capture needs: real, computed,
  per-node position and size, queryable outside the paint pass. Nothing
  new needs building for Step 1; this phase only needs to *use* them.
- `PaintProperties` has no raw position/size fields at all (only
  `background`/`corner_radius`/`elevation`/`opacity`/`transform`/
  `shape`) — "the trigger's captured bounds" (step 2's own text) can
  only be represented through `transform`, the one field that can make
  a node's painted box occupy a rect other than its own real taffy
  layout. Confirmed `Interpolate for Affine`'s own doc comment: a plain
  componentwise coefficient lerp, correct for any zero-shear (diagonal)
  affine, not just the uniform-scale case this codebase has used so
  far (M6 Phase 2's `translate × uniform-scale`) — a real, independent
  non-uniform x/y scale is still safe to lerp, since both a diagonal
  start and end affine keep every intermediate `t` diagonal too
  (confirmed by reading the lerp's own per-coefficient loop). `Affine::
  new([xx, yx, xy, yy, x0, y0])` (the same 6-coefficient constructor
  `interpolate`'s own body already calls) builds one directly.
- `Animated<T>::animate_to`'s own real contract (confirmed by reading
  `animation.rs` directly): `from` is captured as `self.current.clone()`
  at call time, and `tick`'s `elapsed = now.saturating_duration_since(
  anim.start)` correctly holds at `from` (`elapsed == 0`, every real
  `MotionCurve` starts at `y = 0`, confirmed M7 Phase 1's own boundary
  test) for as long as `now < anim.start`. This is exactly the
  "construct an `ActiveAnimation` with a future `start: Instant`"
  mechanism §7.6's own step 4 text names for the staggered content
  cross-fade — already real, needs no new field or tick-loop change.
- `Node.children: Vec<NodeId>` is real, public, already how every
  parent/child relationship in this tree model is read. "The trigger's
  own content (icon/label)"/"the destination's real content" (step 4's
  own phrasing) is modeled the only way this engine can know what
  "content" means generically: each container's own direct children —
  matching this codebase's established principle of using existing,
  generic primitives rather than inventing a new "content" concept.
- **Real, confirmed gap, not silently worked around:** step 5's
  "on completion (via §5's queue-drain mechanism)" names
  `CompletionHandle`, confirmed via direct read of `interaction.rs`'s
  own doc comment to be "still just an unused struct field... no drain
  exists anywhere" — genuinely unwired anywhere in this codebase.
  Building that queue for real is separate, larger, unscoped work, not
  smuggled into this phase's own stated "single helper function" scope.
  Teardown is instead exposed as its own plain, explicit function
  (`container_transform::teardown`) the caller invokes once it knows
  the transition is done (e.g. after the same known `duration` has
  elapsed) — "an ordinary tree mutation" exactly as §7.6's own text
  says, using the already-real `Tree::detach`.
- `engine-md3` already depends on `engine-core` (§4) and already has
  `peniko` (for `color.rs`'s own `Color` usage, confirmed still present
  after Phase 4's `shape_morph` move) — `peniko::kurbo::Affine` is
  reachable with no new dependency. `engine-py` already depends on
  `engine-md3` (`window.rs` already imports `DynamicTheme`).

## Design

- New `engine-md3::container_transform` module: `ContainerTransform
  Config { duration: Duration, curve: MotionCurve, content_stagger:
  Duration }`, `pub fn begin(tree: &mut Tree, trigger: NodeId,
  destination: NodeId, config: &ContainerTransformConfig, now: Instant)`
  and `pub fn teardown(tree: &mut Tree, trigger: NodeId)`.
- `begin`: **Capture** — `tree.absolute_position(trigger)` +
  `tree.layout(trigger).size` (real, already-computed bounds) plus
  trigger's current `corner_radius`/`background`. Also captures
  destination's own real *target* `corner_radius`/`background`/
  `elevation` (whatever the caller already set) and its own natural
  bounds (`absolute_position`/`layout` called *before* touching its
  `transform`, since it still defaults to `Affine::IDENTITY` at this
  point — the real "pre-transform" position). **Insert/initialize** —
  overwrites destination's `.current` corner_radius/background/
  elevation to the trigger's captured from-state, and sets `transform.
  current` to the affine that maps destination's own natural rect onto
  the trigger's captured rect (scale by captured/natural width and
  height independently, translate by the position delta) — nothing
  visually changes yet, matching step 2's own claim. **Drive one
  synchronized animation set** — `animate_to` on all four (transform →
  `Affine::IDENTITY`, corner_radius/background/elevation → their real
  captured targets), one shared `now`/`duration`/`curve`. **Content
  cross-fade** — every trigger child's `opacity` animates to `0.0`
  starting `now`; every destination child's `opacity.current` is set to
  `0.0` then animated to `1.0` starting `now + content_stagger`.
- `teardown`: detaches `trigger` from its own parent via the already-
  real `Tree::detach` — "hidden or removed," the plain, stated-minimal
  reading, not a new visibility flag.
- `engine-py::PyWindow` gains `begin_container_transform(trigger,
  destination, duration_ms=300, content_stagger_ms=90)` and `end_
  container_transform(trigger)` — thin wrappers, checking `Rc::ptr_eq`
  on both nodes' own `tree` handles first (the same "must belong to
  this Window" check `add_child`/`set_context_menu` already use),
  calling `tree.compute_layout` first (so captured bounds are fresh),
  then the real `engine_md3::container_transform` functions. `curve`
  defaults to `MotionCurve::Emphasized` — §7.5's own text names this as
  container-transform's typical real curve.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt. New `engine-md3` tests:
  a real transition's destination starts at the trigger's captured
  bounds/appearance (not its own real target) and animates toward its
  own real target over the shared duration; trigger/destination
  children's opacity animations are registered with the correct
  (immediate vs. staggered) start times; `teardown` genuinely detaches
  the trigger (parent's `children` no longer contains it).
- `maturin develop --release` + `pytest tests/` + all examples. New
  `examples/container_transform.py`: a small trigger "card" expanding
  into a full destination "sheet," proving the whole call chain
  compiles and runs end to end through the real pipeline.
