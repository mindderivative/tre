# Log: M5 Phase 1 — Transform Composition (§11.9)

Corresponds to `BUILD_TRACKER.md` M5 Phase 1, the first phase of the new
Milestone 5 (§11.9, §11.10, §11.11), scoped 2026-09-15 right after M4
closed. `PaintProperties` had no `transform` field at all before this
phase — this is what makes it land.

## Investigation before writing code

- **`Interpolate for kurbo::Affine` can't be a real rotation-aware
  decomposition through the public API.** `kurbo = "0.13.1"`'s own
  `Affine::svd()` computes exactly the scale+rotation decomposition a
  general-purpose `Interpolate` would want — confirmed by reading its
  vendored source directly — but it's `pub(crate)`, not exported.
  Reimplementing that same SVD math by hand would be real, additive
  work for a capability §11.9's own text never actually asks for ("pan
  offset × zoom scale," no rotation). **Scope narrowed:** a plain
  componentwise lerp of the 6 coefficients. Not a hack — the subspace
  of affines with no rotation/shear is convex, so lerping between two
  such affines never introduces spurious shear/rotation mid-flight;
  exact for this milestone's actual pan/zoom scope, a real named
  limitation only for a future rotation need that doesn't exist yet.
- **Where the actual composition had to live.** `engine-core` has no
  painting code — `PaintProperties` is inert data. Read `engine-render::
  paint_node` end to end before touching anything: it accumulated a
  pure `(offset_x, offset_y)` translation down the tree and built every
  path/glyph position in already-offset absolute canvas coordinates.
  Confirmed directly in `vello_hybrid = "0.2.0"`'s vendored source that
  `Scene::set_transform(Affine)` is respected by both `fill_path`
  (`effective_path_transform`) and `glyph_run`
  (`self.transforms().scene_transform()`) — the real primitive needed
  to refactor `paint_node` into a local-space coordinate model with
  zero per-`NodeKind` special-casing.
- **Reasoned through the "no behavior change for existing content"
  claim algebraically before writing code, then verified it held.**
  `composed(node) = composed(parent) * Affine::translate(layout.location)
  * node.paint.transform.current`. When `transform` is the default
  identity (true for every node before this phase), this reduces to
  exactly the same accumulated translation the old `offset_x/offset_y`
  scheme computed. The full pre-existing pixel-readback suite (rect,
  ripple/hover, splitter-drag ×2, docking, overlay, text, virtual list)
  passed unmodified on the first run after the refactor — the algebra
  held in practice, not just on paper.
- **`Tree::hit_test`/`Tree::absolute_position` are deliberately
  untouched.** `hit_test`'s own doc comment already named
  "`PaintProperties.transform` doesn't exist yet" as the reason it
  isn't transform-aware — this phase makes the field exist, but
  transform-aware hit-testing is M5 Phase 2, not folded in here. Stated
  consequence, not silent: a node with both `interaction` (ripple/hover)
  and a non-identity `transform` will paint its overlay under the same
  local-space transform as its own fill, but the pointer position that
  produced it was hit-tested in untransformed space — a real
  misalignment for that specific, currently-nonexistent combination
  until Phase 2 lands.

## A real bug found and fixed mid-implementation, not by the pixel test

`InteractionState::ripples`' `origin` field is a real pointer coordinate
captured by `Tree::dispatch` in absolute canvas space (`interaction.rs`).
Once `paint_node` moved to drawing every node's own content in *local*
space under `scene.set_transform(composed)`, using `ripple.origin`
directly (still absolute-space) would have silently drawn every ripple
in the wrong place for any node under a non-identity ancestor transform,
and even for identity-transform nodes it would have been offset by
exactly that node's own absolute position — a real regression the
existing `ripple_hover_dispatch.rs` test doesn't exercise (no node in
that test combines interaction with a nonzero ancestor transform, since
the feature didn't exist before this phase). Caught by re-reading
`spawn_ripple`'s own caller in `Tree::dispatch` before wiring the paint
side, not by a failing test. Fixed by mapping `ripple.origin` through
`composed.inverse()` before use, converting it from canvas space into
the same local space every other path in that node's paint now uses.

## What happened

`engine-core/src/animation.rs`: `impl Interpolate for peniko::kurbo::
Affine` (componentwise coefficient lerp, per the investigation above).
`engine-core/src/node.rs`: `PaintProperties` gains `pub transform:
Animated<peniko::kurbo::Affine>`, defaulting to `Affine::IDENTITY` in
`PaintProperties::new`; `tick` ticks it alongside the other four fields.

`engine-render/src/lib.rs`: `paint_node` refactored to thread a
`composed: Affine` accumulator instead of `(offset_x, offset_y)` —
`composed = parent_transform * Affine::translate(layout.location) *
node.paint.transform.current`, set via `scene.set_transform(composed)`
before painting. `Rect`/`Splitter` fill, `Text` placement, and the
ripple/hover overlay all now build their paths in local `(0, 0, w, h)`
coordinates (`ripple.origin` mapped back via `composed.inverse()` per
the bug above); children recurse with `composed` as their own parent
accumulator. `build_tree_scene`'s initial call passes `Affine::IDENTITY`
for the root, unchanged in spirit from before.
`engine-render/src/text.rs`: `TextPlacement.x`/`.y` doc comment updated
to reflect the new local-space meaning; fields/shape unchanged.

New `engine-render/tests/transform_composition.rs`: a `Rect` background,
a full-canvas `Container` ("camera") whose own `transform` is animated
toward a combined pan+zoom target (`Affine::translate((60,40)) *
Affine::scale(0.5)`), and a `Rect` child with no `transform` of its own.
Two claims: before any transform, the child renders at its plain
untransformed position; halfway through the animation (linear, t=0.5),
the child has moved to the exact interpolated transformed position
*and* no longer occupies its old spot — proving composition reaches an
untouched descendant for free, and that `Interpolate for Affine`
genuinely drives it mid-flight, not just at either endpoint. First
version of the test failed for an unrelated reason: the background was
a `Container`, which paints nothing by design, so the "child moved away"
half of the assertion compared against a transparent pixel instead of a
real background color — fixed by using a `Rect` background, not a
change to the composition logic itself, which was already correct.

Full `cargo test --workspace --release` (44 total, up from 43), `cargo
clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --check`
all clean. `maturin develop --release` + full `pytest tests/` (60
passed, 1 skipped) confirmed unaffected, as expected — this phase
touches no `engine-py` code, and §11.9 itself doesn't ask for a Python
API yet (`NodeKind::Canvas`, Phase 3, is what will actually need one to
*set* a transform from Python).
