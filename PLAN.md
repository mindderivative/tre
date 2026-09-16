# Plan: M5 Phase 2 — Transform-Aware Hit-Testing (completing §11.10)

## Context

M5 Phase 1 (§11.9, `508be50`) landed `PaintProperties.transform` and
taught `engine-render::paint_node` to compose it down the tree during
paint. `Tree::hit_test`'s own doc comment has explicitly deferred
transform-awareness since M4 Phase 1 step 1, naming exactly this
precondition ("`PaintProperties.transform` doesn't exist in this
codebase yet ... Revisit this method once each lands"). It now exists.
This phase makes `hit_test` actually use it, per §11.10's own text:
"transforming the pointer position into each candidate's local space
via the inverse of its composed transform... before testing containment
against `Node::computed_layout()`."

## Investigation before writing code

- **`hit_test`'s current implementation, read in full.** `Tree::hit_test`
  recurses into children in reverse order (topmost-paints-last-so-test-
  first, matching paint order), then for a leaf/no-hit-child case,
  calls `self.absolute_position(root)` (a bottom-up parent-chain walk
  accumulating pure `layout.location` translation) and tests a
  **canvas-space** `Rect` against the caller's **canvas-space** `point`.
- **`absolute_position` cannot become the transform-aware primitive.**
  It's used by three other things that are explicitly out of this
  phase's scope (unchanged since Phase 1's own boundary, confirmed via
  grep): overlay anchor placement (`open_overlay`), splitter drag
  geometry (`splitter_geometry`), and several `engine-py` call sites
  (`Window`/`View`'s synthetic `.click()`/`.hover()`/`.right_click()`
  entry points, which compute a node's center point directly).
  Changing its behavior would silently change all of those too --
  correctly kept as a separate, pure-translation primitive.
- **The composition formula has to match `paint_node`'s exactly, or
  hit-testing and rendering would disagree about where a node actually
  is.** `composed(node) = composed(parent) * Affine::translate(layout.
  location) * node.paint.transform.current` -- the same product Phase 1
  already established. `hit_test` already recurses top-down through
  children, so threading an accumulated `parent_transform: Affine`
  parameter through that same recursion is a direct, minimal change --
  no new tree walk needed. This does mean the formula now lives in two
  places (`engine-core::Tree::hit_test` and `engine-render::paint_node`)
  since `engine-core` cannot depend on `engine-render` (crate-boundary
  rule, §4) and has no reason to duplicate `vello_hybrid::Scene`
  plumbing just to share four lines of `Affine` arithmetic. A real,
  accepted duplication, not an oversight -- noted with a doc comment on
  both sides pointing at each other so a future change to one is a
  signal to check the other.
- **Containment testing moves from canvas-space to local-space,
  mirroring `paint_node`'s own local-space refactor exactly.** Instead
  of building a canvas-space `Rect` from `absolute_position` and testing
  the caller's canvas-space `point` directly, `hit_test` now maps
  `point` into the candidate's local space via `composed.inverse() *
  point` and tests it against a local `Rect::new(0.0, 0.0, w, h)` --
  the untransformed layout box, exactly as `paint_node` draws it.
- **`NodeKind::Canvas` custom hit-testing is explicitly NOT this
  phase's job.** §11.10's own text describes it as a *further*
  override "for that node only" once `Canvas` exists -- `Canvas`
  doesn't exist until M5 Phase 3. This phase only makes the *default*
  rect test transform-aware; Phase 3 adds the override hook.
- **Public API is unchanged.** `Tree::hit_test(&self, root: NodeId,
  point: Point) -> Option<NodeId>`'s signature stays exactly as every
  existing caller (`update_hover`, `dispatch`'s press/release arms,
  `engine-py::dock`'s drag-drop, `engine-py::app`'s click-through-
  dispatch path) already calls it -- the transform-awareness is
  entirely internal, a private recursive helper carries the new
  `parent_transform` accumulator.

## Approach

1. **`engine-core/src/tree.rs`**: import `peniko::kurbo::Affine`
   alongside the existing `Point`/`Rect` import. `hit_test` becomes a
   thin public entry point calling a new private `hit_test_at(&self,
   id: NodeId, point: Point, parent_transform: Affine) -> Option<NodeId>`,
   which composes `composed = parent_transform * Affine::translate(...)
   * node.paint.transform.current`, recurses into children with
   `composed`, and on no child hit, maps `point` through `composed.
   inverse()` and tests it against the local-space layout `Rect`.
2. **New `engine-core` unit tests**: a node under a translated-only
   ancestor transform still hits correctly (regression proof the
   existing untransformed-node behavior is unchanged, mirroring Phase
   1's own "pure no-op for identity transform" verification); a node
   under a real (translate * scale) ancestor transform is only hit at
   its *transformed* on-screen position, not its untransformed layout
   position -- the direct `engine-core`-level equivalent of Phase 1's
   `engine-render` pixel test, proving the same claim one layer down
   (hit-testing agrees with where the pixels actually landed).
3. **No `engine-render`/`engine-py` changes needed this phase** --
   `dispatch`/`update_hover`/the Python-facing click/hover/right-click/
   drag entry points all call the same public `hit_test`, so
   transform-awareness reaches the full stack for free, the same
   "mechanism reached everywhere at once" pattern M4 Phase 3 step 1
   already established for splitter-drag dispatch.

## Files to touch

- `crates/engine-core/src/tree.rs` -- `hit_test` refactor + new tests.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo fmt --check`.
- Full existing test suite (engine-core unit tests, all engine-render
  pixel-readback tests, full pytest suite, all examples) must stay
  green unmodified -- same "provably a no-op for every existing
  identity-transform node" standard Phase 1 held itself to.
