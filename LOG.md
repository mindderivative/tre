# Log: M5 Phase 2 — Transform-Aware Hit-Testing (completing §11.10)

Corresponds to `BUILD_TRACKER.md` M5 Phase 2. `Tree::hit_test`'s own
doc comment has said "not transform-aware yet" since M4 Phase 1 step 1,
explicitly naming `PaintProperties.transform` not existing as the
reason -- M5 Phase 1 (`508be50`) made it exist. This phase makes
`hit_test` actually use it.

## Investigation before writing code

- **`absolute_position` cannot be reused as the transform-aware
  primitive.** Confirmed via grep: it's also used by `open_overlay`
  (anchor placement), `splitter_geometry` (drag math), and several
  `engine-py` synthetic-point entry points (`Window`/`View`'s
  `.click()`/`.hover()`/`.right_click()`). All of those are explicitly
  out of this phase's scope (unchanged since Phase 1's own boundary
  decision) -- correctly kept as a separate, pure-translation
  primitive, not repurposed.
- **The composition formula has to match `paint_node`'s exactly.**
  `composed(node) = composed(parent) * Affine::translate(layout.
  location) * node.paint.transform.current` -- the same product Phase 1
  established for rendering. `hit_test` already recurses top-down
  through children in reverse paint order, so threading an accumulated
  `parent_transform: Affine` through that same recursion needed no new
  tree walk. This does mean the formula now lives in two places
  (`engine-core::Tree::hit_test` and `engine-render::paint_node`) since
  `engine-core` can't depend on `engine-render` (§4's crate-boundary
  rule) and duplicating `vello_hybrid::Scene` plumbing just to share
  four lines of `Affine` arithmetic would be disproportionate -- a
  real, accepted duplication, documented with cross-referencing comments
  on both sides so a future change to one is a signal to check the
  other.
- **`NodeKind::Canvas` custom hit-testing is explicitly not this
  phase's job** -- `Canvas` doesn't exist until M5 Phase 3, and §11.10's
  own text describes the custom override as something layered on top of
  the now-transform-aware default rect test, not a replacement for it.
- **Public API unchanged.** `Tree::hit_test(&self, root, point) ->
  Option<NodeId>`'s signature is untouched -- every existing caller
  (`update_hover`, `dispatch`'s press/release arms, `engine-py::dock`'s
  drag-drop, `engine-py::app`'s click path) reaches the new
  transform-awareness for free, the same "mechanism reaches everywhere
  at once" pattern M4 Phase 3 step 1 established for splitter-drag
  dispatch.

## A real test-design gap found while writing the new tests, not a bug in the implementation

The first version of both new tests expected a point that misses a
moved child to return `None`. Both failed immediately -- correctly, as
it turned out: `Container` nodes are themselves real, always-valid
hit-test targets by their own full bounds (existing, intentional
behavior, matching `hit_test_misses_entirely_outside_every_nodes_
bounds`'s own comment: "every node is a valid hit target by its own
bounds"). A point that misses a translated/scaled child still falls
through to the enclosing `Container`'s own bounds -- which, since a
node's own `transform` also moves *itself* (§11.9's "exactly like
nested `<g transform>` in SVG," not just its descendants), had also
moved. Fixed by correcting the test's expected values to `Some(camera)`
/ `Some(root)` (whichever untransformed ancestor bounds the point still
falls inside) instead of `None`, and documenting why in each test's own
comments. The composition/hit-testing implementation itself needed no
changes -- this was purely a wrong expectation in the test, caught by
running it rather than assumed correct on paper.

## What happened

`engine-core/src/tree.rs`: `Tree::hit_test` is now a thin public entry
point (`Affine::IDENTITY` as the initial parent transform) delegating
to a new private `hit_test_at(&self, id, point, parent_transform)`,
which composes the same formula `paint_node` uses, recurses into
children with the composed value, and on no child hit, maps the
caller's point into local space via `composed.inverse()` before testing
it against the untransformed local layout `Rect` -- mirroring
`paint_node`'s own local-space refactor from Phase 1 exactly.

Two new `engine-core` unit tests: `hit_test_follows_an_ancestor_
translate_transform` (pure translation -- exact integer-arithmetic
expected positions) and `hit_test_follows_an_ancestor_scale_and_
translate_transform` (a combined pan+zoom, the same "translate * scale"
shape Phase 1's own pixel test used) -- both prove a real,
non-identity ancestor `transform` genuinely shifts/grows the
hit-testable area, matching where `paint_node` actually draws it.

No `engine-render`/`engine-py` changes needed -- `hit_test`'s public
signature is unchanged, so every existing caller reaches
transform-awareness automatically.

Full `cargo test --workspace --release` clean (`engine-core` alone at
45 tests, up from 43: the two new hit-test tests), `cargo clippy
--workspace --all-targets -- -D warnings`, and `cargo fmt --check` all
clean. `maturin develop
--release` + full `pytest tests/` (60 passed, 1 skipped) and all six
examples confirmed unaffected, as expected -- this phase touches no
`engine-py` code and hit-testing's public contract is unchanged for
every existing (identity-transform) node.
