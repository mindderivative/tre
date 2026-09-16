# Log: M6 Phase 4 — `Tree::absolute_position` transform-awareness, closing M6

Corresponds to `BUILD_TRACKER.md` M6 Phase 4, the last M6 phase. The
post-M5 code review finding: `absolute_position` never composed
`PaintProperties.transform`, deliberately, per M5's own stated scope
each time (`hit_test`/`paint_node` alone became transform-aware). Its
real callers — `open_overlay`'s anchor placement, `splitter_geometry`'s
drag math, every `engine-py` synthetic-point entry point — silently
computed the wrong canvas position for a node inside a panned/zoomed
`Container`.

## Investigation before writing code

- **Every real caller of `absolute_position`, enumerated via grep,
  before deciding how to fix it:** two in `engine-core` (`open_overlay`,
  `splitter_geometry`) and six in `engine-py` (`window.rs`'s `click`/
  `hover`/`right_click`, `view.rs`'s equivalents) — a small, fully
  enumerated set, and *every one of them* wants the transform-aware
  answer. No caller specifically wants the old pure-translation
  behavior.
- **This is why the fix rewrites `absolute_position` in place, rather
  than adding a parallel checked method the way M6 Phase 1
  (`try_add_child`) and M5 Phase 2 (`hit_test_at`) did.** Those cases
  had a large, trusted existing caller set (`add_child`'s ~80,
  `hit_test`'s hot per-frame path) that genuinely couldn't accept the
  new behavior or cost; `absolute_position` doesn't — none of its real
  callers run once per node per frame (an overlay opens once per
  interaction, a drag reads this once per pointer move), so the small
  extra composition cost is not the same concern `paint_node`/
  `hit_test_at`'s own per-frame walks have.
- **An `Affine` only composes correctly root-to-node, the opposite
  order of the old bottom-up accumulation** — so the rewrite collects
  the chain from `id` up to the root first, then walks it in reverse,
  composing the identical `parent * translate(layout.location) *
  own_transform` product `paint_node`/`hit_test_at` already compose
  (M5 Phase 1/2), applied to the node's own local origin.

## What happened

`engine-core/src/tree.rs`: `absolute_position` rewritten in place --
public signature (`&self, id: NodeId) -> (f64, f64)`) unchanged, so
every real caller gets the fix for free, the same "mechanism reaches
everywhere at once" pattern this project has used repeatedly. New
`engine-core` unit test proves a node under a real ancestor transform
reports its actual transformed position — passed on the first run; the
full pre-existing suite (every overlay/splitter-drag/docking pixel
test, all of which depend on `absolute_position` internally) passed
unmodified, confirming the "no behavior change for identity-transform
content" claim held in practice, the same standard M5 Phase 1
established.

New pytest test: `Window.click(node)` (which resolves its dispatch
point via `absolute_position`) still finds a node after `Node.animate
("transform", ...)` (M6 Phase 2) has moved it — this test would have
failed before this phase's fix (a stale, untransformed dispatch point
would miss the node's real, transform-aware hit-test bounds entirely,
since hit-testing itself has been transform-aware since M5 Phase 2).
Passed on the first run.

**M6 (Imperative API Completeness) is now complete — all 4 phases
done.** Full `cargo test --workspace --release` clean (`engine-core` 51
tests, up from 50), `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo fmt --check` all clean. `maturin develop --release` +
full `pytest tests/` (78 passed, up from 77, 1 skipped) and all ten
examples confirmed clean.
