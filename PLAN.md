# Plan: M8 Phase 1 — Engine-Level Paint Culling (§11.8)

Corresponds to `BUILD_TRACKER.md` M8 Phase 1's own scoping: `build_tree_
scene`/`paint_node` gain a real "current visible rect," and a node whose
own composed absolute bounds don't intersect it skips Vello scene-
encoding for its *entire* subtree.

## Investigation before writing code

- `ARCHITECTURE.md` §11.8's own text, read again directly: "The paint
  pass (§6) skips Vello scene-encoding entirely for any subtree whose
  computed layout bounds... transformed into viewport space (§11.9)...
  don't intersect the current visible/clip region." This is exactly
  what this phase builds — nothing more.
- Confirmed via grep: zero "cull" hits anywhere in the codebase before
  this phase.
- `paint_node` (`crates/engine-render/src/lib.rs`) already computes
  `composed = parent_transform * translate(layout.location) *
  node.paint.transform.current` as its very first real step, and
  already knows `w`/`h` (`layout.size`) — everything needed to compute
  a node's own real, composed, absolute bounding box already exists at
  exactly the right point in the walk; nothing new needs reading from
  `Tree`.
- `peniko::kurbo::Rect::overlaps(&self, other: Rect) -> bool` (verified
  directly in kurbo `0.13.1`'s vendored source) is an inclusive-boundary
  intersection test — exactly the check needed, already imported in
  this file (`Rect` is already in `paint_node`'s own `use` list).
- `paint_node`'s recursion into `node.children` is the *only* place a
  subtree ever gets painted — `tree: &Tree` is an immutable reference
  throughout the whole walk (confirmed via its own signature), so
  skipping a subtree's recursion has zero side effects to lose; this is
  a pure rendering optimization, not a behavior change for anything
  that isn't visually off-screen.
- `paint_node` is a private function (confirmed via grep: only called
  from `build_tree_scene` and recursively from itself) — its signature
  can change freely; `build_tree_scene`'s own public signature
  (`width`/`height` unchanged) needs no change at all, so no external
  caller (`engine-py`'s `App::run`, every Rust pixel test) needs
  updating.
- The "current visible rect" needs to be a real parameter threaded
  through the recursion (not just computed once at the top and left
  unused) because M8 Phase 2 (`VirtualList` real clipping) will need to
  *narrow* it on the way into a list's own materialized children — this
  phase establishes the threading; Phase 2 is the first real consumer
  of narrowing it.
- **Real, stated scope limit, not silently worked around:** the
  culling check uses a node's own plain layout bounds, not padded for
  elevation's own shadow spread (M7 Phase 2) — a node whose shadow
  extends past its own box could show a hard edge exactly at the cull
  boundary if the node itself is just barely off-screen but its shadow
  isn't. `ARCHITECTURE.md` §11.8's own text doesn't call for shadow-
  aware padding, and no real, currently-shipping app in this workspace
  scrolls/pans content near a cull boundary with visible elevation —
  the plain "simplest thing that could work" version, not manufactured
  precision ahead of a real, observed case.

## Design

- `paint_node` gains a `visible: Rect` parameter (canvas-space,
  inclusive-boundary). Right after computing `composed`/`w`/`h`
  (before anything else — shadow, fill, interaction, children), computes
  this node's own composed absolute bounding box by transforming its
  four local corners `(0,0)`, `(w,0)`, `(0,h)`, `(w,h)` through
  `composed` and taking their min/max x/y (not just two opposite
  corners — correct even if a future rotation-capable `Affine` ever
  lands, not just today's shear/rotation-free subspace). If that
  bounding box doesn't `overlap` `visible`, returns immediately —
  nothing painted, no recursion into children.
- `build_tree_scene` computes `visible = Rect::new(0.0, 0.0,
  f64::from(width), f64::from(height))` once, passes it to the initial
  `paint_node` call.
- The recursive call into `node.children` passes `visible` through
  unchanged for this phase (no `NodeKind` narrows it yet — that's
  Phase 2's own job for `VirtualList`).

## Verification plan

- `cargo test --workspace --release`/clippy/fmt — the full, unmodified
  pre-existing pixel-test suite must still pass (every real on-screen
  node in every existing test is, by construction, inside its own
  scene's viewport, so this must be a true no-op for all of them,
  matching every additive feature's own "provably a no-op" standard
  since M5 Phase 1). New tests: a node whose real bounds don't overlap
  the viewport paints nothing (a pixel that would show its fill color
  stays plain background); a node whose bounds DO overlap paints
  normally, including when only partially overlapping (edge case: a
  node straddling the viewport edge still paints its visible portion —
  proving culling is a whole-subtree skip decision, not a per-pixel
  clip, so a partially-visible node isn't wrongly dropped).
- `maturin develop --release` + `pytest tests/` + all examples — this
  phase touches no `engine-py`/Python-facing API at all, so this is a
  pure regression check (every existing example must still render
  identically, none of them place content meaningfully off-screen).
