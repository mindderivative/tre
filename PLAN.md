# Plan: M3 Phase 5, Step 9 — Ripple/State-Layer via `push_layer` (§14 step 9)

Corresponds to `BUILD_TRACKER.md` M3 Phase 5, step 9 of 4 (steps 8-11).

## Goal

Per §14 step 9: "Ripple/state-layer via `push_layer` + animated alpha."
§7.3 sketches the full model: `InteractionState { ripples: SmallVec<
[RippleState; 4]>, hover_opacity: Animated<f64>, focus_ring: Animated<
f64> }` living on `Node` as `Option<InteractionState>`, each ripple an
`origin: kurbo::Point` plus `radius`/`opacity: Animated<f64>`, painted
via `Scene::push_layer(clip_path, ...)`.

## Scope narrowing (checked directly before writing anything)

§7.3's full text describes hover falling out of hit-testing (§11.10,
"run every pointer-move") and ripples spawned from real press events
via `AppHandler`/`InputEvent` (§4). Grepped the whole workspace for
`InputEvent`/`AppHandler`/`hit_test` before starting: **none of it
exists yet** — every hit is a forward-reference comment ("lands once
engine-core exists to route to", `engine-platform`'s own module doc:
"No `AppHandler`/`InputEvent` dispatch yet"). There is nothing to wire
hover-from-hit-testing or press-from-pointer-events *to* yet.

This mirrors step 7's own precedent exactly (keyboard Tab/Enter
dispatch deferred because "no interactive component exists yet to
dispatch to") — so this step builds and proves the two genuinely new,
risky mechanisms standalone, deferring real dispatch wiring to whichever
later build-order step first lands real pointer/keyboard input:

In scope:
- `engine-core::interaction`: real `InteractionState`/`RippleState`
  types matching §7.3's sketch, `Option<InteractionState>` on `Node`,
  `Tree::interaction_mut` (lazy opt-in, mirroring `set_access`'s
  shape), `Tree::tick_all` extended to also tick each node's
  interaction state. Unit-tested standalone (mirrors `Animated<T>`'s
  own step-2 precedent): a spawned ripple's radius/opacity animate
  correctly, a finished ripple is pruned from the `SmallVec`, and
  `tick_all` reports "still active" while a ripple is running.
- `engine-render::build_ripple_scene`: standalone spike proving
  `vello_hybrid` 0.2.0's real `Scene::push_layer(clip_path, blend_mode,
  opacity, mask, filter)` genuinely clips a fill to a circle *and*
  applies opacity to it — the two mechanisms the whole ripple visual is
  built on. One ripple over one solid "button" background, parameterized
  over origin/radius/opacity/colors, like `build_shadow_scene` (step 8)
  was over its own parameters. No `Tree` involved — same reasoning as
  step 8's own scope note.
- `crates/engine-render/tests/ripple_spike.rs`: headless render+readback
  proving both mechanisms independently — a sample point inside a large
  radius shows real color+opacity blending (proves opacity), the same
  point excluded by a smaller radius shows pure background (proves the
  clip actually constrains where the fill lands, not just that
  *something* got drawn).

Out of scope (deferred, not a gap in this step):
- Hover derived from real hit-testing (§11.10) — doesn't exist.
- Ripples spawned from real pointer-press events (§4's `InputEvent`/
  `AppHandler`) — doesn't exist. `Tree::interaction_mut(...).
  spawn_ripple(...)` is a real, callable API; nothing calls it from a
  real input source yet.
- Wiring `InteractionState`/ripples into `build_tree_scene`'s per-node
  paint pass. A ripple's real on-screen color is an MD3 theme token
  (§7.1), which doesn't exist as real code until step 11
  (`material-colors`) — wiring ripple rendering into the actual `Tree`
  paint pass now would mean either hardcoding an arbitrary color ahead
  of the real token system, or inventing a `PaintProperties` field
  ahead of a caller that needs it. `build_ripple_scene` proves the
  primitive works today; a real button component wires it into
  `build_tree_scene` once both a color source (step 11) and a press
  source (§4/§11.10, unscheduled) exist.
- A completion-queue-based ripple removal, despite §7.3's text framing
  it as reusing "the same completion-queue mechanism §5 already defines
  for `on_complete`". Checked directly: no such queue exists anywhere
  in the codebase yet — `CompletionHandle`/`on_complete` are still just
  an unused struct field (§5's own queue-drain design is `engine-py`'s
  job, still unbuilt). That queue exists to hand a finished animation
  to a *Python-visible* callback; nothing about ripple pruning is
  Python-visible, so building that queue now, only to have this step be
  its first and only caller, would be exactly the "real, maintained
  machinery for a consumer that doesn't exist yet" LESSONS_LEARNED.md
  §1 warns against. Pruning instead checks each `RippleState`'s own
  `Animated::tick` return value directly (the same mechanical fact the
  queue would ultimately be built from), via `SmallVec::retain`.

## Verification

`cargo test -p engine-core` (new interaction unit tests) and
`cargo test -p engine-render --test ripple_spike` both pass with real
measured claims, not compiles-and-assumed. `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check` all clean.
