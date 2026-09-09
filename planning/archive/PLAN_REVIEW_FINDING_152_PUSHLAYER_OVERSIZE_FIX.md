# Plan: Fix the Real PushLayer/PopLayer Oversized-Borrow Bug (REVIEW.md #152)

## Goal

Fix a real, currently-shipped correctness bug in `execute_frame`'s
`PushLayer`/`PopLayer` compositing (Step 6.4.2), found while checking
whether Step 7.2.2 would be building on solid ground: whenever
`RhiDevice::acquire_transient_target`'s own documented "oversized
borrow" fallback hands back a texture larger than a layer actually
requested, that layer's own inner content silently renders into the
wrong place and vanishes from the final composite -- no error, no
validation warning, just missing content. Confirmed via a real,
GPU-backed repro: push/pop a 200x150 layer (fresh alloc, then
released), then push/pop a never-before-requested 50x40 layer -- the
pool hands back the freed 200x150 texture, and the second layer's own
content (a rect meant to nearly fill it) reads back as pure background
at its own center.

This is the same underlying mechanism as REVIEW.md finding #130's real
root cause (`RhiCommandBuffer::draw_indexed`'s NDC-mapping push constant
using the render target's *real* dimensions, which can silently diverge
from what a caller's own vertex data assumed) -- just reached through
the standard, bindless `PushLayer`/`PopLayer` path instead of a
hand-rolled non-bindless demo. Fixing it here, at the shared
`begin_render_to_texture`/`begin_render_to_texture_no_end` trait level,
closes the *general* case, not just the one call site #130's own fix
patched around.

## Root cause, precisely

`begin_render_to_texture`/`begin_render_to_texture_no_end`
(`tre-rhi-vulkan/src/lib.rs`) currently set `self.width`/`self.height`
-- `RhiCommandBuffer::draw_indexed`'s own push-constant `screen_size`
source -- from `texture.dimensions()`, i.e. the render target's *real*
physical size. Any command recorded assuming a *different*, smaller
logical size (exactly what `PushLayer`'s own inner `DrawGeometry`
commands do -- their vertex positions are baked at `Canvas` record time
against the `LayerDesc`'s own requested `width`/`height`, before the
real texture is ever acquired) gets its NDC mapping computed against
the wrong, larger `screen_size`, confining the actual draw to a small
corner of the oversized backing image.

## The fix, precisely (verified by working out the full math, not just asserted)

Viewport/scissor/render-area can stay driven by the texture's *real*
dimensions unchanged -- NDC always spans -1..1 across whatever the
*current* viewport actually is, so leaving the viewport at the real
(possibly larger) size just means the intended content draws
proportionally "stretched" to fill it. Crucially, this stretch is
exactly undone later: `PopLayer`'s own composite quad samples that
texture across its full, un-scaled `(0,0)`-`(1,1)` UV range (unchanged)
and draws it back at the `LayerDesc`'s own real, requested on-screen
size (unchanged) -- so the encode-with-intended-size /
decode-via-normalized-UV round trip is a mathematical identity
regardless of what the intermediate real texture's own physical size or
aspect ratio happens to be. **The only thing that actually needs to
change is what feeds `self.width`/`self.height`**: it must be the
caller's own intended/logical size (which every real caller already
knows -- it's exactly what they passed to `acquire_transient_target`),
not `texture.dimensions()`. No UV rescaling, no dynamic vertex-buffer
rewrites, no viewport/scissor changes needed anywhere.

## Scope decisions

1. **`RhiCommandBuffer::begin_render_to_texture`/`begin_render_to_
   texture_no_end` gain two new parameters, `width: u32, height: u32`
   -- the caller's own intended/logical size.** Used only to set
   `self.width`/`self.height`; every other line of both methods
   (barrier, `cmd_begin_rendering`'s render area, viewport, scissor) is
   unchanged, still driven by `texture.dimensions()`. This is a real
   trait signature change, so every real call site updates in the same
   pass -- `execute_frame`'s `PushLayer` handling, `render_to_texture_
   demo.rs`, `dual_kawase_nonbindless_experiment.rs`, and `dual_kawase_
   blur_demo.rs` (5 call sites) -- each already tracks its own intended
   size as a local value, so every update is mechanical, not a design
   change.
2. **`dual_kawase_blur_demo.rs`'s own existing raw-`cmd_draw_indexed`
   workaround (REVIEW.md #130's real fix) is left exactly as it is.**
   This general fix makes that workaround technically redundant for
   *new* code, but reverting an already-shipped, already-verified,
   already-CI-green step to prove a point is real scope creep with real
   regression risk, not owed by this bug fix. Its own header comment
   gets one added sentence disclosing the redundancy honestly, without
   rewriting or re-deriving anything else in it.
3. **Verification happens at two levels, matching this project's own
   established discipline of never trusting a fake/mock alone for a
   real-hardware timing/sizing bug.** A `tre-engine` unit test proves
   `execute_frame`'s `PushLayer` handling passes the *requested* size to
   `begin_render_to_texture`, not whatever an oversized `FakeTexture`
   claims -- a `FakeDevice`/`FakeTexture` pairing deliberately returns a
   texture larger than requested, the same shape as the real pool's own
   fallback. A new, permanent, real GPU-backed demo (promoted from this
   session's own throwaway repro, not left as a demo-then-discard
   exercise) proves the fix against the *real* `VulkanDevice`/`Acquire
   TransientTarget` oversized-borrow path exactly as it actually occurs
   in production, added to `ci.yml`'s `vulkan-validation` job so this
   exact regression can never silently return.
4. **No change to `RhiDevice::acquire_transient_target`'s own
   oversized-borrow behavior.** It is deliberate, load-bearing, and
   correctly documented (DESIGN.md Section 2.6's "no dynamic RHI
   allocation inside the render tick" -- oversized-borrow is what avoids
   a synchronous allocation whenever *any* already-pooled, even
   larger, texture could serve). The bug was always in how a caller's
   NDC math reacted to that fallback, never in the fallback itself.

## Tasks

1. `crates/tre-rhi-vulkan/src/lib.rs`: add `width: u32, height: u32`
   parameters to `VulkanCommandBuffer`'s `begin_render_to_texture`/
   `begin_render_to_texture_no_end`; use them only for `self.width`/
   `self.height`.
2. `crates/tre-engine/src/lib.rs`: update the `RhiCommandBuffer` trait
   signatures (with a doc-comment explanation of why); update
   `execute_frame`'s `PushLayer` handling to pass `command.clip_bounds.
   width`/`.height`; update `FakeCommandBuffer`'s own impl and the
   `RecordedCall::BeginRenderToTexture`/`BeginRenderToTextureNoEnd`
   variants to carry width/height; update existing test assertions;
   add the new oversized-`FakeTexture` regression test (scope decision
   3).
3. Update the 3 example files' own call sites (`render_to_texture_
   demo.rs`, `dual_kawase_nonbindless_experiment.rs`, `dual_kawase_
   blur_demo.rs`) to pass their own already-known intended size.
4. New permanent demo, `crates/tre-rhi-vulkan/examples/layer_oversize_
   regression_demo.rs` (real assertions, not `eprintln`-only): the
   two-frame push/pop sequence this session's own scratch repro used,
   asserting the second, smaller layer's own content composites
   correctly even though it borrows the first, larger layer's freed
   texture. Add to `ci.yml`'s `vulkan-validation` job.
5. `documentation/REVIEW.md`: new finding #152, full account (root
   cause, the "stretch is undone by normalized UV" proof, the fix,
   real verification).
6. `documentation/IMPLEMENTATION.md`: Step 6.4.2's own write-up gets a
   dated addendum disclosing and closing this bug (matching how Step
   7.2.1 already carries multiple dated addenda).
7. `documentation/ARCHITECTURE.md`: new dated annotation for the
   changed `begin_render_to_texture`/`begin_render_to_texture_no_end`
   signatures, matching this section's own established per-change
   annotation pattern.
8. One added sentence in `dual_kawase_blur_demo.rs`'s own header
   (scope decision 2).

## Verification plan

- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the
  workspace.
- The new `tre-engine` unit test (oversized `FakeTexture`) passes.
- The new `layer_oversize_regression_demo.rs`'s real GPU pixel
  assertions pass, run manually at least twice to confirm not a
  one-off.
- Full regression sweep: every pre-existing Vulkan demo re-run
  manually, zero regressions -- `render_to_texture_demo.rs`,
  `canvas_layer_composite_demo.rs`, `dual_kawase_blur_demo.rs`, and
  `dual_kawase_nonbindless_experiment.rs` specifically, since their own
  call sites changed.
- Commit; push only on explicit "push it"; `gh run watch` after any
  push (`accessibility-validation`'s pre-existing, documented failure
  expected and unrelated).

## Explicitly out of scope

- Reverting or simplifying `dual_kawase_blur_demo.rs`'s own non-bindless
  conversion now that this general fix makes it not strictly necessary
  -- real, separate, optional future cleanup, not owed by this bug fix
  (scope decision 2).
- Any change to `RhiDevice::acquire_transient_target`'s own
  oversized-borrow logic -- it was never the bug (scope decision 4).
- Step 7.2.2 itself (wiring Dual-Kawase blur to `push_layer`/
  `pop_layer`) -- this fix is the explicitly-requested prerequisite,
  not the step itself; picked back up once this closes and verifies
  clean.
