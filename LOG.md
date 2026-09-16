# Log: M7 Phase 5 — Container Transform (§7.6), closing M7 entirely

Corresponds to `BUILD_TRACKER.md` M7 Phase 5, the milestone's own final
phase. Two steps: `Node::computed_layout() -> taffy::Layout` (the one
"genuinely new" `engine-core` primitive §7.6 names); the real five-step
choreography as a single `engine-md3` helper.

## Investigation before writing code

- **Real finding: Step 1's "genuinely new primitive" already exists.**
  `ARCHITECTURE.md` §7.6's own Capture text (written before the real
  crate/API split) asks for `engine-core` to expose computed layout as
  a queryable value. Read `crates/engine-core/src/tree.rs` directly:
  `Tree::layout(id) -> &Layout` (real position+size, already `pub`,
  already used externally) and `Tree::absolute_position(id) -> (f64,
  f64)` (M6 Phase 4, transform-aware) together already expose
  everything Capture needs. Nothing new needed building for Step 1 —
  this phase only uses them.
- `PaintProperties` has no raw position/size fields — "the trigger's
  captured bounds" can only be represented through `transform`.
  Confirmed `Interpolate for Affine`'s own doc comment: a plain
  componentwise coefficient lerp, safe for any zero-shear (diagonal)
  affine, not just the uniform-scale case used so far — a real
  independent x/y scale stays diagonal at every interpolated `t`.
  `Affine::new([xx, yx, xy, yy, x0, y0])` (the same constructor
  `interpolate`'s own body calls) builds one directly.
- `Animated<T>::animate_to`'s real contract (confirmed by reading
  `animation.rs`): `from` is captured at call time, and `tick`'s
  `saturating_duration_since` correctly holds at `from` for as long as
  `now < start` — exactly the "construct an `ActiveAnimation` with a
  future `start: Instant`" mechanism §7.6's own step 4 names for the
  staggered content fade. Already real, no new field/tick-loop change.
- `Node.children: Vec<NodeId>` models "content" the only way this
  engine can know generically: each container's own direct children.
- **Real, confirmed, stated gap:** step 5's "queue-drain mechanism"
  (`CompletionHandle`) is confirmed via `interaction.rs`'s own doc
  comment to be unwired anywhere in this codebase. Building that queue
  for real is separate, unscoped work — `teardown` is instead a plain,
  explicit function the caller invokes once it knows the transition is
  done, "an ordinary tree mutation" exactly as §7.6's own text says.

## What happened

New `engine-md3::container_transform` module: `ContainerTransform
Config { duration, curve, content_stagger }`, `begin(tree, trigger,
destination, config, now)` (capture trigger's real bounds/appearance,
capture destination's real targets, initialize destination to the
trigger's captured from-state via a computed non-uniform scale+translate
`Affine`, drive one synchronized `animate_to` set on transform/
corner_radius/background/elevation, cross-fade trigger/destination
children with an immediate vs. staggered start), `teardown(tree,
trigger)` (detaches trigger from its parent via the already-real `Tree::
detach`). `engine-md3/Cargo.toml` gains a test-only `taffy` dev-
dependency (production code never touches it directly).

`engine-py::PyWindow` gains `begin_container_transform(trigger,
destination, duration_ms=300, content_stagger_ms=90)` and `end_
container_transform(trigger)` — same-tree `Rc::ptr_eq` guards mirroring
`Node::add_child`'s own `ForeignNode` check, `tree.compute_layout` first
so captured bounds are fresh, `MotionCurve::Emphasized` as the default
curve (§7.5's own text names this as container-transform's typical real
curve).

New `engine-md3` tests: the destination genuinely starts at the
trigger's captured corner_radius/background/elevation/transform (not
its own real target), and ticking to completion lands on its own real
target, not the captured one; trigger children fade out immediately
while destination children stay pinned at 0.0 until their staggered
start arrives, then reach 1.0; `teardown` genuinely detaches the trigger
from its parent while leaving the destination untouched. New
`examples/container_transform.py`: a small trigger card expanding into
a full destination sheet, end to end through the real pipeline.

Full `cargo test --workspace --release` clean (`engine-md3` gains 3
tests: 4 → 7), `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check` all clean — every prior test passed unmodified.
`maturin develop --release` + full `pytest tests/` (78 passed, 1
skipped, unaffected) and all fourteen examples (thirteen existing + new
`container_transform.py`) confirmed clean.

M7 — MD3 Visual Fidelity is now complete: all 5 phases done.
