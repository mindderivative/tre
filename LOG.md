# Log: M3 Phase 5, Step 9 — Ripple/State-Layer via `push_layer` (§14 step 9)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 5, step 9 of 4 (steps 8-11).

## What happened

**Checked whether §7.3's own dispatch story ("hover falls out of
hit-testing, run every pointer-move"; ripples "spawned from real press
events") had anything to attach to before writing code.** Grepped the
whole workspace for `InputEvent`/`AppHandler`/`hit_test` first: none of
it exists anywhere yet, only forward-reference comments in
`engine-core`'s and `engine-platform`'s own module docs ("lands once...",
"No `AppHandler`/`InputEvent` dispatch yet"). This is the same situation
step 7 hit with keyboard Tab/Enter dispatch ("no interactive component
exists yet to dispatch to") -- so this step applies the identical scope
narrowing: build and prove the animation/rendering mechanisms
standalone, defer wiring them to a real dispatch source until one
exists.

**`engine_core::interaction`**: `RippleState { origin, radius:
Animated<f64>, opacity: Animated<f64> }` and `InteractionState {
ripples: SmallVec<[RippleState; 4]>, hover_opacity, focus_ring }`,
matching §7.3's own struct sketch exactly. `smallvec` added as a direct
dependency -- already resolved at `1.16.1` transitively (checked in
`Cargo.lock` first), so this is a zero-cost addition to the dependency
graph, the same "reuse what's already there" move as `slotmap` at step
3. `Node` gained `interaction: Option<InteractionState>` (the `Option`
was always the plan, per §7.3's own text and this crate's own
"add the field when its own step needs it" comments going back to step
3). `Tree::interaction_mut` lazily opts a node in, mirroring
`set_access`'s "nothing until a caller opts in" shape; `Tree::tick_all`
now also ticks a node's interaction state if it has one.

**Deliberately did not build a completion-queue for ripple pruning**,
despite §7.3's own text framing removal as reusing "the same
completion-queue mechanism §5 already defines for `on_complete`".
Checked directly: that queue doesn't exist as real code anywhere --
`CompletionHandle`/`on_complete` are still just an unused struct field
from step 2, with no drain mechanism (`engine-py`'s future job, for
handing a finished animation to a *Python-visible* callback). Nothing
about ripple pruning needs to be Python-visible, so `InteractionState::
tick` prunes finished ripples directly via `SmallVec::retain` checking
each `RippleState`'s own `Animated::tick` return value -- the same
underlying fact the queue would ultimately be built from, without
manufacturing unused machinery ahead of a caller that needs it
(exactly the class of premature abstraction LESSONS_LEARNED.md §1 warns
against).

**Verified `vello_hybrid` 0.2.0's real `Scene::push_layer` signature
directly in source** before writing the render spike: `push_layer(
clip_path: Option<&BezPath>, blend_mode: Option<BlendMode>, opacity:
Option<f32>, mask: Option<Mask>, filter: Option<Filter>)` -- confirmed
its doc comment ("Panics if `mask` is provided") and that `clip_path`
and `opacity` compose in one call (no need to nest two separate
`push_layer` calls for clip + opacity).

**`engine_render::build_ripple_scene`**: one ripple over one solid
"button" background -- a base fill, then `push_layer(Some(&circle),
None, Some(opacity), None, None)`, a second fill covering the same
bounds, then `pop_layer()`. Deliberately as narrow as `build_shadow_
scene` (step 8): parameterized over origin/radius/opacity/colors, no
`Tree`, no `InteractionState` wiring, no MD3 ripple-color token (real
MD3 ripple color is a theme-role lookup that doesn't exist as code
until step 11's `material-colors` wiring). Wiring ripples into
`build_tree_scene`'s actual per-node paint pass is deferred to whenever
both a real color source (step 11) and a real press source (§4/§11.10,
still unscheduled) exist -- inventing either ahead of that need was
the exact premature-abstraction trap this project keeps deliberately
avoiding.

**The actual proof** (`crates/engine-render/tests/ripple_spike.rs`, two
tests, same headless render+readback discipline as `shadow_spike.rs`):
one test isolates opacity blending (sampled dead center, inside any
nonzero radius, so only opacity is under test) and shows the red
channel strictly between the base and ripple colors -- not fully one or
the other. The second isolates the clip: the *same* fixed point, 50px
from the ripple's origin, reads pure base color under a 20px-radius
ripple and a real blended color under a 100px-radius one -- since the
exact same `fill_path` call runs in both renders, only `push_layer`'s
own `clip_path` argument can explain the difference. This is the
falsifiable claim the spike exists to make: a `push_layer` that ignored
`clip_path` (or a broken build) would either blend everywhere regardless
of radius, or nowhere at all -- either failure mode fails one of the
two tests.

## Verification

```
$ cargo test -p engine-core          # 15 passed, incl. 3 new
                                       # interaction unit tests + 1 new
                                       # Tree::interaction_mut/tick_all
                                       # composition test
$ cargo test -p engine-render --test ripple_spike -- --nocapture
test ripple_opacity_genuinely_blends_the_ripple_color_into_the_background ... ok
test ripple_clip_confines_the_blend_to_the_circles_own_radius ... ok

$ cargo test --workspace             # all green
$ cargo clippy --workspace --all-targets -- -D warnings
                                       # one real finding, fixed: a
                                       # collapsible-if in tick_all,
                                       # rewritten as a let-chain
$ cargo fmt --check                   # clean
```

## Next

`BUILD_TRACKER.md` updated: Phase 5 step 9 done (2 of 4 steps in this
phase). Next: step 10 -- the shape morph module (§7.4), the one MD3
component with no library to lean on ("equalize point count, then lerp"
plus the correspondence/alignment search the architecture's own review
note flags as the harder, easy-to-skip half).
