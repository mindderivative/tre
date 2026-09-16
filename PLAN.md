# Plan: M6 Phase 4 — `Tree::absolute_position` transform-awareness

## Context

The post-M5 code review finding, now the milestone's final phase:
`Tree::absolute_position` still doesn't know `PaintProperties.transform`
exists — deliberately, per M5's own stated scope (`hit_test`/
`paint_node` alone became transform-aware; `absolute_position` was
explicitly left as pure layout-translation both times). Its real
callers — `open_overlay`'s anchor placement, `splitter_geometry`'s drag
math, and every `engine-py` synthetic-point entry point — will silently
compute the wrong canvas position for a node inside a panned/zoomed
`Container`.

## Investigation before writing code

- **Every real caller of `absolute_position`, enumerated via grep,
  before deciding how to fix it:** `engine-core::tree.rs` itself
  (`open_overlay`'s anchor placement, `splitter_geometry`'s flanking-
  sibling math) and `engine-py` (`window.rs`'s `click`/`hover`/
  `right_click`, `view.rs`'s equivalents) — all resolve "where is this
  node, in canvas space" for a purpose that needs the *real*,
  transform-composed position, exactly the same claim `hit_test`
  already makes correctly since M5 Phase 2.
- **Changing `absolute_position`'s own behavior in place, not adding a
  parallel method, is the correct fix here** — unlike `hit_test`
  (M5 Phase 2) and `add_child` (M6 Phase 1), where a *new* checked
  method was correct because the existing infallible one had ~80
  trusted callers that structurally couldn't need the new behavior,
  `absolute_position` has a small, fully-enumerated set of real callers
  (five, all listed above), and *every one of them* wants the
  transform-aware answer — there is no caller that specifically wants
  the old pure-translation behavior. Confirmed by reading each call
  site: none of them are hot-path-per-frame the way `paint_node`/
  `hit_test_at` are (an overlay opens once per interaction, a splitter
  drag reads it once per pointer move, not once per node per frame),
  so the small extra cost of composing a transform is not a real
  concern the way it would be for a mass per-frame walk.
- **The composition formula is the same product Phases 1/2 already
  established** — `composed(node) = composed(parent) *
  Affine::translate(layout.location) * node.paint.transform.current`
  — but `absolute_position` returns an `(f64, f64)` point, not an
  `Affine`. The real fix: compose the full `Affine` walking down from
  the root (mirroring `hit_test_at`'s own top-down recursion, not
  `absolute_position`'s current bottom-up parent-chain walk, since an
  `Affine` composes correctly only in root-to-node order) and apply it
  to the node's own local origin `Point::ORIGIN` to get its real,
  transformed canvas position.
- **This does change `absolute_position`'s answer for any node under a
  non-identity ancestor transform** — a real, intentional behavior
  change, not a silent one: every existing call site's own tests (this
  phase's own new tests, plus the full pre-existing suite) must
  confirm the identity-transform case (every node before this phase,
  and everywhere `transform` is never set) is byte-for-byte unchanged,
  the same "provably a no-op" standard M5 Phase 1 held itself to.

## Approach

1. **`engine-core/src/tree.rs`**: rewrite `absolute_position` to walk
   top-down from the root (find the root via repeated `.parent` lookups
   first, matching the *shape* of the existing bottom-up walk but
   composing an `Affine` instead of accumulating `(f64, f64)`), then
   return `(composed * Point::ORIGIN).x, (composed * Point::ORIGIN).y)`.
   Its public signature (`&self, id: NodeId) -> (f64, f64)`) is
   unchanged — every caller keeps working, just gets the correct answer.
2. **New `engine-core` tests**: a node under a real ancestor transform
   reports its actual transformed position via `absolute_position`, not
   its untransformed layout position; the full pre-existing
   `absolute_position` test suite (identity-transform case) stays green
   unmodified.
3. **No `engine-py` changes needed** — every real caller (`open_overlay`,
   `splitter_geometry`, `click`/`hover`/`right_click` and their `View`
   equivalents) already calls the same public `absolute_position`, so
   the fix reaches all of them for free, the same "mechanism reaches
   everywhere at once" pattern this project has used repeatedly.

## Files to touch

- `crates/engine-core/src/tree.rs` — `absolute_position` rewrite +
  tests.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo fmt --check`.
- Full pre-existing suite (including every overlay/splitter-drag/docking
  pixel test) must stay green unmodified — the "no behavior change for
  identity-transform content" claim, held to the same standard M5
  Phase 1 established.
- `maturin develop && python -m pytest tests/ -v` plus all examples run.
