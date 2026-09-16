# Plan: M5 Phase 1 — Transform Composition (§11.9)

## Context

M4 closed entirely at `36521e4`. `BUILD_TRACKER.md` already scoped
Milestone 5 (§11.9, §11.10, §11.11) on 2026-09-15: `PaintProperties`
(`crates/engine-core/src/node.rs`) has no `transform` field at all,
blocking pan/zoom canvases, transform-aware hit-testing, and
`NodeKind::Canvas`. This phase tackles the first, foundational piece:
§11.9 itself — composing an animatable `kurbo::Affine` down the tree
during paint, "exactly like nested `<g transform>` in SVG."

## Investigation before writing code

- **`PaintProperties.transform` needs `Interpolate for kurbo::Affine`.**
  `animation.rs`'s own doc comment anticipated this ("`kurbo::Affine`...
  lands whenever a later step first animates a transform"), and
  `node.rs`'s doc comment calls it "non-trivial... matrix
  decomposition." Checked `kurbo = "0.13.1"`'s real `Affine::svd()`
  directly in its vendored source: it exists and does exactly what a
  rotation-aware decomposition needs (returns scale + rotation angle,
  used internally for ellipse-radius calculations) — but it's
  `pub(crate)`, not exported. A true rotation-aware `Interpolate` would
  have to reimplement that same SVD math by hand, for a rotation
  capability §11.9's own text never actually asks for ("animate a
  `Container`'s own `transform` (pan offset × zoom scale)" — no
  rotation). **Scope narrowed:** `Interpolate for Affine` is a plain
  componentwise lerp of the 6 coefficients (`as_coeffs`/`Affine::new`).
  This is not a hack — the subspace of affines with no rotation/shear
  (`a == d`, `b == c == 0`, i.e. uniform scale + translate) is convex,
  so a linear interpolation between two such affines stays in that same
  subspace for the whole animation: no shear or spurious rotation ever
  appears mid-flight for the pan/zoom case this milestone actually
  targets. A real rotation between two differently-rotated affines
  would look like a non-circular "morph" rather than sweeping through
  the correct arc — a genuine, named limitation, not silently
  incorrect, and not manufactured ahead of a real need for rotation.
- **Where composition has to happen.** `engine-core` has no painting
  code — `PaintProperties` is a plain data holder; the actual "compose
  parent's effective transform with its own" walk has to live in
  `engine-render::paint_node`, the one real per-node paint walk.
  Read `paint_node` end to end first: it currently accumulates a pure
  `(offset_x, offset_y)` translation pair down the tree (taffy's
  `layout.location` is parent-relative) and builds every path/glyph
  position in already-offset **absolute canvas coordinates**. Adding a
  transform on top of that scheme would double-apply position unless
  the whole coordinate model changes.
- **`vello_hybrid::Scene` already has exactly the right primitive.**
  `Scene::set_transform(Affine)` (confirmed directly in
  `vello_hybrid = "0.2.0"`'s vendored `scene.rs`) sets a matrix that
  both `fill_path` (`effective_path_transform`) and `glyph_run`
  (`self.transforms().scene_transform()`) genuinely respect — checked
  both call sites directly, not assumed. This means `paint_node` can be
  refactored to build every path/glyph run in **local, node-relative
  coordinates** (`(0, 0)` to `(w, h)`) and let one
  `scene.set_transform(composed)` call per node do the mapping into
  canvas space — for both `Rect`/`Splitter` paint and `Text`, uniformly,
  with no special-casing per `NodeKind`.
- **The composition itself:** `composed(node) = composed(parent) *
  Affine::translate(layout.location) * node.paint.transform.current`.
  Folding taffy's own layout-relative translation into the same
  product as the new animatable transform is what makes this "just
  ordinary matrix multiplication during the paint walk" per §11.9's own
  text, not a second parallel mechanism. When every node's `transform`
  is the default `Affine::IDENTITY`, this is byte-for-byte the same
  composed matrix as today's `offset_x/offset_y` accumulation
  (`translate(loc) * IDENTITY == translate(loc)`) — so every existing
  test/example is unaffected, confirmed by reasoning through the algebra
  before writing code, verified after by running the existing suite
  unchanged.
- **Hit-testing/`absolute_position` are deliberately untouched.**
  `Tree::hit_test`'s own doc comment already states "not
  transform-aware yet" and explicitly defers that to "once
  `PaintProperties.transform` lands" — this phase makes it land, but
  transform-aware hit-testing is its own named Phase 2, not folded in
  here. `Tree::absolute_position` (used by hit-testing, focus,
  `build_access_update`) stays pure layout-translation, unchanged.
  Consequence, stated not silent: a node that has *both* `interaction`
  (ripple/hover, §7.3) *and* a non-identity `transform` will paint its
  ripple/hover overlay under the same local-space composed transform as
  its own fill (consistent with everything else in `paint_node`), but
  the *pointer position* that produced that ripple was hit-tested in
  untransformed space — a real, carried-forward misalignment for that
  specific combination until Phase 2 lands. No existing node combines
  the two today, so nothing regresses.

## Approach

1. **`engine-core/src/animation.rs`**: `impl Interpolate for
   peniko::kurbo::Affine` — componentwise lerp of `as_coeffs()`, per
   the investigation above.
2. **`engine-core/src/node.rs`**: `PaintProperties` gains `pub
   transform: Animated<peniko::kurbo::Affine>`, initialized to
   `Affine::IDENTITY` in `PaintProperties::new`. `tick` ticks it
   alongside the other four fields.
3. **`engine-render/src/lib.rs`**: refactor `paint_node` to thread a
   `composed: Affine` accumulator instead of `(offset_x, offset_y)`.
   Each node: `let composed = parent_composed * Affine::translate((loc.x,
   loc.y)) * node.paint.transform.current;` then
   `scene.set_transform(composed)` before painting its own
   Rect/Splitter/Text/ripple/hover content in **local** `(0, 0, w, h)`
   coordinates; recurse into children with `composed` as their parent
   accumulator. `build_tree_scene`'s initial call passes
   `Affine::IDENTITY` as the root's parent-composed value (same as
   today's `scene.set_transform(Affine::IDENTITY)` before the walk).
4. **New `engine-render` pixel-readback test** (`transform_composition.rs`
   or inline in `lib.rs`'s own test module, matching this crate's
   existing convention): a `Container` with an animated `transform`
   (pan + scale) and a `Rect` child with no transform of its own —
   assert the child's painted pixels land at the *transformed* position,
   not its untransformed taffy layout position, proving composition
   really propagates to a descendant with zero per-descendant code.

## Files to touch

- `crates/engine-core/src/animation.rs` — `Interpolate for Affine`.
- `crates/engine-core/src/node.rs` — `PaintProperties.transform` field.
- `crates/engine-render/src/lib.rs` — `paint_node` coordinate-model
  refactor (local space + `set_transform`), new pixel test.
- `crates/engine-render/src/text.rs` — `TextPlacement.x/y` doc comment
  update (local-space now, not "absolute" — the field names/shape stay
  the same, only the coordinate space they're interpreted in changes).

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo fmt --check`.
- Full existing `engine-render` pixel-readback suite (rect, ripple/hover,
  overlay, docking, splitter-drag) must stay green unmodified — the
  algebraic no-op argument above needs to hold in practice, not just on
  paper.
- New transform-composition pixel test passes.
- No `engine-py`/Python-facing surface changes this phase — nothing in
  §11.9 asks for a Python API yet; `NodeKind::Canvas` (Phase 3) is what
  will actually need one to *set* a transform from Python. Confirmed via
  grep: no existing Python API sets any `PaintProperties` field directly
  by name anyway (colors/etc. go through kwargs at node-creation time),
  so this is consistent with the existing pattern, not a new gap.
