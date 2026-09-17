# Log: M8 Phase 1 — Engine-Level Paint Culling (§11.8)

Corresponds to `BUILD_TRACKER.md` M8 Phase 1. `build_tree_scene`/
`paint_node` gain a real "current visible rect," and a node whose own
composed absolute bounds don't intersect it skips Vello scene-encoding
for its entire subtree.

## Investigation before writing code

- `ARCHITECTURE.md` §11.8's own text, read again directly, specifies
  exactly this mechanism: skip Vello scene-encoding entirely for any
  subtree whose computed layout bounds, transformed into viewport
  space, don't intersect the current visible/clip region. Confirmed via
  grep: zero "cull" hits anywhere in the codebase before this phase.
- `paint_node` already computes `composed`/`w`/`h` as its very first
  real step — everything needed for the culling check already exists
  at exactly the right point in the walk, nothing new to read from
  `Tree`.
- `peniko::kurbo::Rect::overlaps(&self, other: Rect) -> bool` (verified
  directly in kurbo `0.13.1`'s vendored source) is an inclusive-boundary
  intersection test, already the exact check needed; `Rect` was already
  imported in this file.
- `paint_node` is private (confirmed via grep — only called from
  `build_tree_scene` and recursively from itself); its signature could
  change freely with zero external callers to update. `tree: &Tree` is
  immutable throughout the whole walk, so skipping a subtree's
  recursion has no side effects to lose — a pure rendering optimization.
- The "current visible rect" is threaded as a real parameter (not just
  computed once and left unused) because M8 Phase 2 (`VirtualList` real
  clipping) will need to *narrow* it on the way into a list's own
  materialized children — established here, consumed there.
- Real, stated scope limit: the culling check uses a node's own plain
  layout bounds, not padded for elevation's own shadow spread (M7 Phase
  2) — a node whose shadow extends past its own box could show a hard
  edge exactly at the cull boundary. `ARCHITECTURE.md` §11.8's own text
  doesn't call for shadow-aware padding, and no real app in this
  workspace pans content near a cull boundary with visible elevation —
  the simplest thing that could work, not manufactured precision.

## What happened

`build_tree_scene` computes `visible = Rect::new(0.0, 0.0, width,
height)` once and threads it into `paint_node`. `paint_node` gains a
`visible: Rect` parameter; right after computing `composed`/`w`/`h`, it
transforms all four local corners `(0,0)`/`(w,0)`/`(0,h)`/`(w,h)`
through `composed`, takes their min/max x/y to build this node's own
real composed bounding box, and returns immediately (no shadow, fill,
interaction, or recursion into children) if that box doesn't `overlap`
`visible`. The recursive call into `node.children` passes `visible`
through unchanged for this phase (no `NodeKind` narrows it yet — that's
Phase 2's own job).

New `crates/engine-render/tests/paint_culling.rs`: the real,
distinguishing behavior culling adds (a parent positioned 10,000px
off-screen, whose child's own `transform` would otherwise bring it back
onto visible canvas, never paints — proving the parent's whole subtree
was skipped before the child's own transform was ever computed, not
just that off-screen content happens not to rasterize); a node only
partially overlapping the viewport (straddling its edge) still paints
its real, visible portion (proving this is a bounding-box-overlap
decision, not an all-or-nothing "must be fully inside" test). Both
passed on the first run.

Full `cargo test --workspace --release` clean (`engine-render` gains 2
new tests — every prior test passed unmodified, confirming the no-op
claim for on-screen content), `cargo clippy --workspace --all-targets
-- -D warnings`, `cargo fmt --check` all clean. This phase touches no
`engine-py`/Python-facing API at all — `maturin develop --release` +
full `pytest tests/` (78 passed, 1 skipped, unaffected) and all fourteen
examples confirmed clean, a pure regression check.
