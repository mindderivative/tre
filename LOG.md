# Log: M4 Phase 5 — Real Ripple/Hover, End to End (§7.3)

Corresponds to `BUILD_TRACKER.md` M4 Phase 5. Started from a scope this
session had itself corrected before the phase began: "no `engine-py` API
ever calls `interaction_mut`." That's real, but investigating further
before writing code surfaced a second, larger gap the tracker didn't
yet name, which changed this phase's real scope.

## Investigation before writing code

Re-read §7.3, then read `interaction.rs`, `tree.rs`'s `interaction_mut`/
`update_hover`/`dispatch`, and — critically — `engine-render`'s actual
real-render-path function `paint_node` (what `build_tree_scene` calls,
the function every real windowed app goes through), not just the
already-known standalone `build_ripple_scene` spike.

Two real, confirmed findings changed the plan:

1. **`paint_node` never read `node.interaction` at all.** Only the
   standalone `build_ripple_scene` spike (no real `Tree`, its own doc
   comment says so) ever drew a ripple. Even with a Python-facing
   opt-in, a real click would correctly animate `InteractionState`
   internally but produce zero visible pixels — an unprovable claim if
   left as-is.
2. **Ripple already fires with no opt-in at all, and this phase's own
   premise was half wrong.** `Tree::dispatch`'s `PointerPressed` arm
   calls `self.interaction_mut(node)` unconditionally on whatever node
   it hits, and `interaction_mut` *always* lazily creates
   `InteractionState` — there's no gating check. So a real primary
   click on *any* node already ripples internally, with zero
   opt-in, unlike `update_hover`, which its own doc comment states
   explicitly "never lazily creates one just because a node happened to
   be hovered." This is a real, confirmed asymmetry in already-shipped
   M3/M4 code, not something this phase introduces — and it means
   `enable_interaction()`'s real, necessary job is enabling *hover*,
   not ripple.

## What happened

**`engine-render`**: `paint_node` now paints, for any node with
`Some(interaction)`, after its own kind-specific fill and before
recursing into children (state layers sit under content, matching real
MD3): a flat hover overlay (`with_opacity(black, hover_opacity.
current)`, filled over the node's own rounded-rect bounds — a true
no-op at `0.0`) and each active ripple (`push_layer` clipped to the
ripple's own growing circle, filled with the node's own bounds rect —
the intersection of clip and fill path is exactly "this ripple, bounded
to this node," no nested clipping needed). A fixed neutral (black) tint,
not per-scheme MD3 tokens — dynamic color isn't wired into `paint_node`
for *any* property yet, a separate, larger, pre-existing gap, not
solved here as a side effect.

**`engine-py`**: `Node.enable_interaction()` — a thin call into
`Tree::interaction_mut`, mirroring `set_on_click`'s own shape. Kept
deliberately separate from `set_on_click` (not folded in): a
purely-hoverable, non-clickable node is a real, independent case §7.3
itself describes, and implicitly paying extra per-frame animation cost
just because a node got a click handler would be a surprising side
effect for a caller who only wanted the click.

## Verification

New `crates/engine-render/tests/ripple_hover_dispatch.rs` — the
definitive pixel-level proof, through the real `build_tree_scene`
pipeline: (1) a real `Tree::dispatch` press on a node that never called
`enable_interaction` still shows a visible ripple at the press point
(proving the confirmed no-opt-in-needed fact above) while a point
outside the ripple's radius stays untouched; (2) a real `PointerMoved`
dispatch directly over a node that never opted in shows *no* hover
tint at all, and the identical dispatch after a real opt-in shows a
uniform tint across the whole node. Both passed after one real bug in
the test itself, not the production code: the first attempt dispatched
`PointerMoved` twice at the same point, and `update_hover`'s own
"repeated call with the same result is a no-op" contract correctly
suppressed the second call — fixed by moving off-node between the two
dispatches, matching how a real cursor would actually have to move to
re-trigger hover.

3 new pytest tests in `tests/test_interaction.py` (the FFI-wiring half
of the split this project already uses for `test_splitter.py`/
`splitter_drag_dispatch.rs` — no pixel-buffer access exists from
Python, so the definitive proof lives in the Rust test above). New
`examples/ripple_button.py`, mirroring `resizable_panes.py`'s own
stated honesty about what's automatable vs. what needs a human.

```
$ cargo build --workspace                                    # clean
$ cargo clippy --workspace --all-targets -- -D warnings       # clean
$ cargo fmt --check                                           # clean (after one real fmt fix)
$ cargo test --workspace                                      # all green, 2 new engine-render tests included

$ maturin develop
$ python -m pytest tests/ -v
44 passed, 1 skipped (the TRE_RUN_BENCHMARK-gated benchmark)

$ python examples/animate_rect.py       # exited cleanly, unaffected
$ python examples/resizable_panes.py    # exited cleanly, unaffected
$ python examples/ripple_button.py      # exited cleanly, new
$ python examples/two_windows.py        # exited cleanly, unaffected
```

## Next

M4 Phase 6: introduce a minimal `Event`/`EventKind` type (confirmed via
grep to not exist anywhere yet) so `HoverEnter`/`HoverExit` (§7.3) has
something real to fire through — the same generalization `View`'s
`on_click`-only wiring (M4 Phase 4) already named as its own next step.
Real, stated-not-silent gaps unchanged: `paint_node`'s ripple/hover tint
is a fixed neutral color, not per-scheme MD3 tokens (dynamic color
isn't wired into real rendering anywhere yet — separate, pre-existing,
larger gap); ripple's own radius stays `dispatch.rs`'s existing flat
100.0 approximation, not computed per-node from its own size; two-phase
press/hold/release ripple timing (grow-and-hold on press, fade only on
release) is still the single-shot combined animation `interaction.rs`'s
own doc comment named as deferred since M3.
