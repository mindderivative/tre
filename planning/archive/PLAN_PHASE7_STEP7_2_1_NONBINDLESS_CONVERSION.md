# Plan: Close REVIEW.md Finding #130 -- Convert the Real Dual-Kawase Implementation to Non-Bindless Sampling

## Goal

Convert `dual_kawase_blur_demo.rs`'s real 5-stage Dual-Kawase chain
(L0 -> L1 -> L2 downsample, L2 -> U1 -> U0 upsample, composite U0 onto
the swapchain) from sampling through the persistent bindless texture
array to the plain, conventionally-bound descriptor approach
`dual_kawase_nonbindless_experiment.rs` already proved works, under the
identical real render-to-texture lifecycle, across 4 consistent runs.
If this closes the demo's own real pixel assertions, REVIEW.md finding
#130 closes for good and Step 7.2.1 completes.

## Scope decisions

1. **Graduate the experiment's own proven infrastructure into the real
   demo file, not a new RHI trait method.** IMPLEMENTATION.md's own
   Step 7.2.1 header already commits to "hand-written RHI calls -- no
   Canvas/IR involvement" for this capability; the fix is specifically
   "don't route this through the bindless array," not "add new engine
   surface." One plain `COMBINED_IMAGE_SAMPLER` descriptor set layout,
   one matching pipeline layout, hand-rolled pipeline creation (mirrored
   from `VulkanDevice::create_pipeline`'s own real state, matching the
   experiment), and raw `ash` binds via `RhiCommandBuffer::raw_handle()`
   (bypassing `set_pipeline`'s unconditional bindless rebind) -- all
   exactly as already proven, just applied to all 5 real hops instead
   of the experiment's single one.
2. **One descriptor set per real hop (5 total: L0, L1, L2, U1, U0),
   allocated from one pool up front.** Matches the experiment's own
   "one set per read" pattern exactly -- simplest and already proven,
   not the more complex alternative of reusing fewer sets with
   sequential rewrites.
3. **A new shader, `kawase_upsample_nonbindless.frag`, is the only new
   shader needed.** `kawase_downsample_nonbindless.frag` and
   `passthrough_nonbindless.frag` already exist from the experiment and
   are reused unchanged; the upsample math needs its own non-bindless
   twin, byte-for-byte the same 8-tap formula as `kawase_upsample.frag`.
4. **`register_bindless`/`deregister_bindless` are removed from this
   demo entirely** -- replaced by direct `vkUpdateDescriptorSets` calls
   pointing each hop's own descriptor set at the real transient
   target's own `ImageView`, exactly as the experiment does.
   `acquire_transient_target`/`release_transient_target` calls are
   unchanged -- only the sampling mechanism changes, not the transient
   pool's own lifecycle.
5. **`dual_kawase_blur_demo.rs` keeps its name and file identity** (now
   a real, working demo, not a broken reproduction) -- its own header
   doc comment is rewritten to reflect the real, fixed status and the
   real investigation history, matching this project's own "correct
   stale status text directly, don't leave it stale" discipline
   (findings #115/#146). Added to `ci.yml`'s `vulkan-validation` job.
6. **`dual_kawase_nonbindless_experiment.rs` is kept, not deleted** --
   its own header is updated to note the real implementation now
   applies its finding, matching this project's own precedent of
   keeping real, already-proven reproduction/diagnostic files rather
   than discarding them once superseded.
7. **Scope stops at closing Step 7.2.1.** Step 7.2.2 (wiring this real
   capability to `push_layer`/`pop_layer`) is real, separate future
   work -- not attempted in this same pass, matching this project's own
   established one-step-at-a-time rhythm.

## Tasks

1. Write `crates/tre-rhi-vulkan/shaders/kawase_upsample_nonbindless.frag`
   (byte-identical math to `kawase_upsample.frag`, plain `sampler2D`).
2. Add it to `build.rs`.
3. Rewrite `dual_kawase_blur_demo.rs`: real non-bindless descriptor
   set layout/pool/5 sets/pipeline layout/3 custom pipelines
   (downsample, upsample, composite); replace every
   `register_bindless`/`bind_texture`/`deregister_bindless` call with a
   direct descriptor write + raw pipeline/descriptor-set bind; update
   its own header doc comment and real pixel assertions' own framing
   (they already test the right thing -- interior stays foreground,
   edge shows real bleed -- no assertion logic needs to change, only
   the mechanism producing the pixels).
4. Update `documentation/IMPLEMENTATION.md` Step 7.2.1's status header
   and `documentation/REVIEW.md` finding #130's own disposition once
   verified working -- from "STOPPED"/"root cause narrowed" to
   "Fixed"/closed, with the real fix described.
5. Add `dual_kawase_blur_demo.rs` to `ci.yml`'s `vulkan-validation` job.
6. Update `dual_kawase_nonbindless_experiment.rs`'s own header noting
   the real implementation now applies its finding.

## Verification plan

- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the
  workspace.
- `dual_kawase_blur_demo.rs`'s own real pixel assertions (interior stays
  mostly foreground after blur; edge shows genuine partial blend, not
  pure background) pass for real against actual GPU hardware -- the
  same assertions that have panicked every prior run.
- Re-run manually at least once to confirm not a one-off pass.
- All pre-existing demos re-run manually, zero regressions (none of
  their own code changes, but the shared `build.rs`/`ci.yml` do).

## Explicitly out of scope

- Step 7.2.2 (wiring to `push_layer`/`pop_layer`) -- real, separate
  future work once this closes.
- Promoting the non-bindless pattern into new, reusable `RhiDevice`/
  `RhiCommandBuffer` trait surface -- matches this step's own original
  "hand-written RHI calls" scope; a real trait method is only worth
  adding once a second real caller needs the identical pattern.
