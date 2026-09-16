# Plan: M4 Phase 5 — Real Ripple/Hover, End to End (§7.3)

## Context

`BUILD_TRACKER.md`'s corrected Phase 5 scope (fixed earlier this
session after finding a factual error in the prior version) says the
gap is "no `engine-py` API ever calls `interaction_mut`, so no real
Python-built node has `InteractionState` to animate." That's real, but
investigating further before writing code surfaced a second, bigger gap
in the same area that the tracker didn't yet name.

## Investigation before writing code

Re-read §7.3 in full, then read `interaction.rs`, `tree.rs`'s
`interaction_mut`/`update_hover`/`dispatch`, and — critically —
`engine-render`'s actual real-render-path function, `paint_node`
(what `build_tree_scene` calls, the function every real windowed app
and every FFI-driven render actually goes through), not just the
already-known standalone `build_ripple_scene` spike function.

Confirmed directly:

- `Tree::interaction_mut`/`Tree::dispatch`'s `PointerPressed` ripple
  spawn/`update_hover`'s hover animation are all real, complete, and
  already correctly wired at the `engine-core` level (unit-tested since
  M3 Phase 5 step 9). Nothing needs fixing here.
- **`paint_node` (`crates/engine-render/src/lib.rs`) never reads
  `node.interaction` at all** — confirmed by reading the full function.
  Only `build_ripple_scene`, a standalone spike with no `Tree`
  involved at all (its own doc comment says so explicitly), ever draws
  a ripple. This means even after adding a Python-facing opt-in, a real
  click on a real Python-built button would correctly *animate*
  `InteractionState` internally but produce **zero visible pixels** —
  an incomplete, unprovable claim if left as-is, not a real "it works."

**Scope widened accordingly**, still narrow and grounded in exactly
what's confirmed missing (not manufacturing anything beyond it):

1. `paint_node` gains real ripple/hover rendering for any node with
   `Some(interaction)`, reusing `build_ripple_scene`'s already-proven
   `push_layer(clip_path, ..., opacity, ...)` technique, moved into the
   real per-node walk instead of a synthetic single-button spike.
2. `engine-py` gets a real opt-in API.
3. Tests prove the real, complete path: opt in → real `Tree::dispatch`
   click → pixel-visible ripple, through the actual `build_tree_scene`
   pipeline, not the standalone spike.

**Real design decision, resolved:** should the opt-in be folded into
`Node.set_on_click`, or a separate method? Chose **separate**
(`Node.enable_interaction()`): a purely-hoverable, non-clickable node
is a legitimate, real, independent case (§7.3 describes hover
independently of click), and implicitly opting a node into extra
per-frame animation cost just because it got a click handler would be
a surprising side effect for a caller who only wanted the click.
Explicit opt-in for each independently matches Design Principle 6's own
"only a node that opts in pays the cost."

**Real, stated color narrowing:** the overlay uses a fixed neutral
(black) tint, not per-scheme MD3 "on-surface" color tokens — dynamic
color (`engine_md3::color::DynamicTheme`, real since M3 Phase 5 step
11) isn't wired into `paint_node` anywhere yet, for *any* property, not
just this one; that's a separate, larger, pre-existing gap (real
scheme-driven rendering), not something to solve as a side effect of
this phase. `dispatch.rs`'s own `interaction_config()` already
hardcodes MD3-value opacities the same deliberate way.

## Approach

1. **`engine-render`**: `paint_node` paints, for any node with
   `Some(interaction)` (after its own kind-specific fill, before
   recursing into children — state layers sit under content, matching
   real MD3): a flat hover overlay (`with_opacity(black, hover_opacity.
   current)` filled over the node's own rounded-rect bounds, a no-op at
   `0.0`) and each active ripple (`push_layer` clipped to the ripple's
   own growing circle, filled with the node's own rect — the
   intersection of clip and fill path is exactly the node-bounded,
   circle-clipped ripple, no nested clipping needed).
   New `engine-render` pixel-readback test: a real `Tree::dispatch`
   press on an opted-in node, rendered through the real
   `build_tree_scene`, shows the ripple color at the press point;
   released and ticked past its duration, the pixel returns to the
   node's own background.
2. **`engine-py`**: `Node.enable_interaction()` — calls
   `Tree::interaction_mut(self.id)` once, mirroring `set_on_click`'s own
   shape (a thin, direct call into the existing `engine-core` API,
   nothing new invented).
3. **Tests**: pytest coverage that `enable_interaction()` doesn't raise
   and a `Window.click()` on an opted-in node doesn't crash — the
   definitive pixel-level proof lives in the new `engine-render` test
   (step 1), matching this project's established split (Rust crate
   tests carry pixel proof; Python tests prove FFI wiring), the same
   way `test_splitter.py` deferred its own pixel proof to
   `splitter_drag_dispatch.rs`. A new real windowed example,
   `examples/ripple_button.py`, mirroring `resizable_panes.py`'s own
   stated pattern: automatable proof stops at construction/rendering,
   a human running it interactively sees the actual ripple/hover.

## Files to touch

- `crates/engine-render/src/lib.rs` — `paint_node`.
- `crates/engine-render/tests/` — new pixel-readback test file.
- `crates/engine-py/src/node.rs` — new `enable_interaction` method.
- `tests/test_interaction.py` — new.
- `examples/ripple_button.py` — new.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo fmt --check`.
- `maturin develop && python -m pytest tests/ -v` plus all examples run.
