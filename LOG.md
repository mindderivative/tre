# Log: M3 Phase 5, Step 8 — MD3 Shadow Spike (§14 step 8)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 5, step 8 of 4 (steps 8-11) -- opens Phase 5.

## What happened

**Corrected an assumption before writing any code.** Memory recorded
after step 7 predicted this step would be "the first step to touch
`engine-md3` for real." Re-reading ARCHITECTURE.md §4's crate-boundary
rule and §15's Risk Register mitigation text directly ("confine all
Vello calls to `engine-render`") shows that's wrong: `engine-md3`
depends only on `engine-core`, never on `vello_hybrid`/`wgpu`, so a
spike of a `vello_hybrid` API has to live in `engine-render` -- the same
crate step 1's original "one static rounded rect through `vello_hybrid`"
spike lived in, for the identical reason. `engine-md3` stays an empty
skeleton for now; nothing in §14 step 8's own text actually requires
touching it, only the earlier memory note did.

**Verified `vello_hybrid` 0.2.0's real current API directly in its
vendored source** (`~/.cargo/registry/src/.../vello_hybrid-0.2.0/src/
scene.rs`) before writing anything, per this project's established
per-Linebender-crate discipline. Confirmed: `Scene::fill_blurred_
rounded_rect(&mut self, rect: &kurbo::Rect, radius: f32, std_dev: f32,
invert: bool)` -- takes a plain `Rect` plus a separate `radius`, not a
pre-built `RoundedRect` like `fill_path` does elsewhere in this crate.
Its own doc comment: "When `invert` is `true`, the inverse (`1 - alpha`)
of the blur coverage is painted... implement inset box shadows" -- not
needed yet (`invert: false` here), but confirms §7.2's "with `invert`
for inset shadows" claim against the real source, not just the
architecture doc's own prose. Also confirmed the render path handles the
resulting `EncodedPaint` generically (`encode_blurred_rounded_rect_paint`
exists in both the wgpu and webgl backends) -- no extra caller-side
setup needed beyond the ordinary `FrameRenderer::render` call every
other scene in this crate already uses. Single resolved `kurbo` version
across the whole workspace (`0.13.1`, checked in `Cargo.lock`) confirmed
`peniko::kurbo::Rect` (already imported in `engine-render`) and
`vello_hybrid`'s own `Rect` parameter are the same type -- no
conversion needed.

**`engine_render::build_shadow_scene`**: one blurred rounded rect, fixed
60px margin, parameterized over `color`/`corner_radius`/`std_dev` --
deliberately as narrow as step 1's own `build_rect_scene`, which it
sits directly alongside in `lib.rs`. No `Tree`, no layout, no MD3
elevation-level presets: this step only needs to prove the primitive
itself works against the exact pinned version, not build the table of
MD3 elevation levels on top of it (steps 9/11's job).

**The actual proof** (`crates/engine-render/tests/shadow_spike.rs`):
headless render-to-texture-then-readback, same discipline as
`animated_rect.rs`/`layout_tree.rs`. Sampled four points along one
horizontal line, straight out from a single non-corner edge (so each
point only ever sees one edge's falloff, not a corner's two-edge
falloff): deep interior (90px from any edge, past the 25px kernel
spread) reads ~255 alpha; the raw (unblurred) rect edge itself reads
127 (within an 80-180 tolerance band) -- a real Gaussian blur gives
~50% coverage exactly at a straight edge, which a hard-edged fill
could never produce; 10px outside the edge (still inside the kernel)
reads a nonzero value strictly less than the edge's own; 55px outside
(past the kernel) reads ~0. This is the actual falsifiable claim: a
non-blurred (or broken) `fill_blurred_rounded_rect` would either jump
straight from ~255 to ~0 at the raw edge, or paint uniformly across the
whole inflated rect -- either failure mode fails at least one of the
four assertions.

## Verification

```
$ cargo test -p engine-render --test shadow_spike -- --nocapture
test blurred_rounded_rect_shows_a_real_gaussian_falloff_at_its_edge ... ok

$ cargo test --workspace        # all green, 1 new test alongside every
                                  # existing one
$ cargo clippy --workspace --all-targets -- -D warnings   # clean
$ cargo fmt --check              # clean
```

## Next

`BUILD_TRACKER.md` updated: Phase 5 step 8 done (1 of 4 steps in this
phase). Next: step 9 -- ripple/state-layer via `Scene::push_layer` +
animated alpha (§7.3), the first place `InteractionState`/`RippleState`
(currently just sketched in ARCHITECTURE.md §7.3, not yet real code)
need an actual `Node`-side implementation.
