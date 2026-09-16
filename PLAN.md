# Plan: M5 Phase 4 — Node Graphs & Charts Groundwork (§11.11)

## Context

The last phase of Milestone 5. §11.11's own text: "no new framework
mechanism — both compose entirely from what's already specified:
`NodeKind::Canvas` for custom-drawn content, pan/zoom via transform
composition (§11.9), `kurbo` path drawing for links/chart geometry, and
custom hit-testing (§11.10) ... virtualization/culling (§11.7/§11.8)."
This phase is validation only: prove those pieces actually compose in
one real tree, not separately-built follow-up machinery.

## Investigation before writing anything

- **§11.8 culling was never actually built** -- confirmed via grep
  (`cull`/`clip_region`/`visible_region`: zero hits anywhere in
  `engine-render`/`engine-core`). `BUILD_TRACKER.md`'s own "Known gaps"
  already states this honestly ("§11.8 aren't built"), predating M5
  entirely and out of M5's own scoped sections (§11.9/§11.10/§11.11,
  not §11.8). This phase cannot demonstrate real engine-level culling
  because it doesn't exist -- claiming otherwise would be exactly the
  "manufactured, not real" mistake this project's own discipline avoids
  throughout. What §11.11's own text actually permits stands in for it:
  "full level-of-detail/decimation logic ... is an application or
  library concern, not something this framework needs to own" -- so an
  app deciding which graph nodes to draw based on a computed visible
  range (exactly the same shape `VirtualList`'s own materializer
  already requires the app to do) is the real, honest answer, not a
  gap this phase needs to close.
- **What genuinely *is* real and reusable:** `NodeKind::Canvas` (Phase
  3), transform composition (Phase 1), transform-aware hit-testing incl.
  the custom override (Phases 2-3), and `NodeKind::VirtualList` (M3 step
  15 Stage C) for windowed/large datasets. None of these have been
  proven together in one tree before -- every existing test isolates
  one dimension (Phase 1's own transform test uses a plain `Rect` child,
  Phase 3's own canvas test has no `VirtualList`, etc.).
- **The idiomatic graph shape, reasoned through before building the
  example:** individual graph *nodes* (simple, convex, clickable
  circles) don't need `Canvas`/custom hit-testing at all -- an ordinary
  `Rect` with `corner_radius` set to half its size (a real circle look)
  plus the already-real `Node.set_on_click`/rect hit-test (M3/M4) is the
  correct, existing mechanism, reused verbatim. `Canvas` +
  `CustomHitTest::Path` is for exactly what §11.11's own text names:
  *edges/links* -- open curves no other `NodeKind` can hit-test
  precisely. Building the example this way (real `Rect` nodes + real
  `Canvas` edges, not everything crammed into one `Canvas`) is itself
  part of what "compose from what's already specified" means -- not
  reinventing node hit-testing inside `Canvas` when a real mechanism
  for it already exists.
- **A `CanvasState` holds exactly one `CustomHitTest`,** confirmed by
  its own Phase 3 shape -- correct for one edge's own precise hit-test,
  but means "many independently clickable edges" needs one `Canvas`
  node per edge (not a limitation this phase needs to lift; a small
  graph's edge count is exactly the kind of "not a general vector API"
  scope Phase 3 already named).

## Approach

1. **New `engine-render` pixel-readback test**
   (`tests/graph_composition.rs`): one tree, one shared "camera"
   `Container` with a real (non-identity) animated `transform` --
   a `Rect` "node," a `Canvas` "edge" (`StrokePath` +
   `CustomHitTest::Path`), and a `NodeKind::VirtualList` with one
   materialized item, all as `camera`'s children. Captures each item's
   pure local position via `Tree::absolute_position` *before* setting
   `camera`'s transform (transform-independent, per Phase 1/2's own
   established boundary), then asserts, after setting a real translate
   transform: (a) each of the three paints at its correctly-transformed
   position, (b) `hit_test` finds each of the three at that same
   transformed position, and (c) the edge's custom hit-test still
   genuinely excludes a point off the path but inside the edge's own
   bounding box (re-confirming Phase 3's "override, not narrowing"
   claim holds inside a combined scene, not just in isolation).
2. **New `examples/node_graph.py`**: a small, real graph -- a handful of
   real `Rect` "node" circles (`corner_radius` = half size,
   `set_on_click`) plus a couple of `Canvas` "edges" between them
   (`stroke_path` + `set_hit_test_path`), all inside a "camera"
   `Container` whose `transform` animates a pan across frames --
   the live, human-runnable counterpart to the pixel test above.
3. **No `engine-core`/`engine-render`/`engine-py` production code
   changes** -- per §11.11's own "no new framework mechanism" text,
   confirmed correct by the investigation above: every primitive this
   phase needs already exists.

## Files to touch

- `crates/engine-render/tests/graph_composition.rs` -- new.
- `examples/node_graph.py` -- new.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo fmt --check`.
- `maturin develop && python -m pytest tests/ -v` (unaffected -- no
  Python-facing API changes) plus all examples (including the new one)
  run clean.
