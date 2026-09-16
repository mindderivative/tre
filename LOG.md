# Log: M5 Phase 4 — Node Graphs & Charts Groundwork (§11.11), closing M5

Corresponds to `BUILD_TRACKER.md` M5 Phase 4, the last M5 phase.
Validation only, per §11.11's own text: prove `Canvas` + transform
composition + custom hit-testing + virtualization actually compose in
one real tree, not separately-built follow-up machinery.

## Investigation before writing anything

- **§11.8 culling was never actually built** — confirmed via grep
  (`cull`/`clip_region`/`visible_region`: zero hits in `engine-render`/
  `engine-core`). `BUILD_TRACKER.md`'s own "Known gaps" already states
  this honestly, predating M5 and outside M5's own scoped sections
  (§11.9/§11.10/§11.11, not §11.8). This phase cannot demonstrate real
  engine-level culling because none exists — §11.11's own text already
  permits the honest alternative: "full level-of-detail/decimation
  logic ... is an application or library concern, not something this
  framework needs to own."
- **The idiomatic graph shape, reasoned through before building
  anything:** simple, convex, clickable graph nodes don't need `Canvas`
  at all — an ordinary `Rect` (`corner_radius` for a circle) plus the
  already-real `Node.set_on_click`/rect hit-test is correct and
  existing. `Canvas` + `CustomHitTest::Path` is for *edges* — open
  curves no other `NodeKind` can hit-test precisely, exactly what
  §11.11's own text names.
- **A real, load-bearing Python-API gap found while planning the live
  example, before writing it:** `Node.animate()`'s real property list
  (`engine-py/src/node.rs`, read directly) is `opacity`/`corner_radius`/
  `elevation`/`background` only — no `transform`. And `Window.add_rect`/
  `add_canvas` both attach as flex-row children of the window's own
  root — no `Position::Absolute` is exposed to Python at all. So a
  Python example with several independently-positioned, click-target
  `Rect` "nodes" joined by separately-positioned `Canvas` "edges" (the
  shape the Rust-level pixel test builds, since Rust has direct
  `taffy::Style`/`Position::Absolute` access) isn't actually buildable
  from Python today. **Scope narrowed, not silently worked around:**
  the live example draws the whole graph — nodes *and* edges — as
  `DrawCommand`s inside one `Canvas`, a real and legitimate way to
  build a node graph (arguably the more realistic shape for "many
  nodes," per §11.11's own culling discussion: batching into one
  `Canvas` rather than one real `Tree` node per graph node). Adding
  Python-facing `transform`/absolute-positioning support would be real,
  additive `engine-py` surface — explicitly not built here, since Phase
  4's own charter is "no new framework mechanism," and no real consumer
  has asked for it yet.

## What happened

New `crates/engine-render/tests/graph_composition.rs`: one tree, one
shared "camera" `Container` with a real (non-identity) animated
`transform`, holding three children of three different kinds — a
`Rect` graph "node," a `Canvas` graph "edge" (`StrokePath` +
`CustomHitTest::Path`), and a `NodeKind::VirtualList` with one
materialized item (standing in for "many graph nodes, windowed"). Each
item's pure local position is captured via `Tree::absolute_position`
*before* setting `camera`'s transform (transform-independent, per M5
Phase 1/2's own established boundary — this avoids needing to hand-
compute taffy's own block-layout arithmetic for the `VirtualList`
item), then all three are asserted to paint AND hit-test correctly at
their transformed positions after a real translate is applied, plus
that the edge's custom hit-test still genuinely excludes an off-path
point inside its own bounding box — re-confirming M5 Phase 3's
"override, not narrowing" claim holds inside a combined scene, not just
in isolation. Passed on the first run.

New `examples/node_graph.py`: five circular graph nodes + six stroked
edges, all real `DrawCommand`s inside one `Canvas`, with a real
`CustomHitTest::Circle` on one specific node (§11.10's own "a specific
plotted data point" example, verbatim) — the live, human-runnable
counterpart to `graph_composition.rs`'s pixel-level proof, working
within the real Python-API constraints found above rather than
silently assuming a fuller API existed.

No `engine-core`/`engine-render`/`engine-py` production code changes
this phase — confirmed correct by the investigation above: every
primitive M5 Phase 4 needs already exists after Phases 1-3.

**M5 (Transform Composition & Custom Drawing, §11.9/§11.10/§11.11) is
now complete — all 4 phases done.** Full `cargo test --workspace
--release` clean (48 total engine-render/engine-core tests combined
with the new composition test), `cargo clippy --workspace --all-targets
-- -D warnings`, `cargo fmt --check` all clean. `maturin develop
--release` + full `pytest tests/` (68 passed, 1 skipped — unchanged
from Phase 3, as expected: no production code touched) and all eight
examples (seven existing + new `node_graph.py`) confirmed clean.
