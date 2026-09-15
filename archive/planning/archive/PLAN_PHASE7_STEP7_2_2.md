# Plan: Phase 7, Step 7.2.2 -- Wire Dual-Kawase Blur to push_layer/pop_layer

## Goal

Let a caller request a real, GPU-accelerated backdrop blur on a
compositing layer via `Canvas::push_layer`/`pop_layer`, using the
Dual-Kawase chain Step 7.2.1 already proved works on real hardware.
Own-content blur only (already-decided project scope, `planning/
archive/PLAN_PHASE7_STEP7_2_1.md` item 2) -- a layer's own newly-drawn
content gets blurred before compositing, not whatever is visually
behind it on the swapchain (true backdrop blur needs a real swapchain
snapshot/copy capability this project doesn't have yet, unchanged,
real future work).

Confirmed with the project owner before writing any code (two real
design forks, neither resolvable by precedent alone):
1. **`LayerDesc` gains a simple `blur: bool`** -- fixed chain depth
   matching the already-proven demo, no tunable radius/quality yet.
   Matches this project's own incremental precedent (7.2.1 explicitly
   deferred tunability to "once a real consumer needs it" -- this step
   is that consumer, and the smallest real capability is the honest
   next increment, not the ceiling).
2. **One purpose-built trait method** (`RhiCommandBuffer::
   apply_layer_blur`) encapsulates the whole non-bindless downsample/
   upsample chain as one opaque operation, rather than several smaller
   primitives `execute_frame` would orchestrate itself. Matches this
   project's own precedent for `begin_render_to_texture_no_end` -- added
   narrowly for exactly the chaining need it served, not as a generic
   primitive family speculatively built ahead of a second real need.

## A real, load-bearing finding from this step's own pre-work

Before writing this plan, a scratch test confirmed something that
changes this step's own design: **the bindless array genuinely still
cannot sample a texture while rendering into a *different* offscreen
target, even with REVIEW.md finding #152's general fix applied.** A
plain two-hop chain (draw a square into L0, downsample L0 -> L1 via the
*original*, bindless `kawase_downsample.frag`, entirely through
standard `RhiCommandBuffer`/`RhiDevice` trait calls -- `register_
bindless`/`set_pipeline`/`bind_texture`/`draw_indexed`, exactly
`PopLayer`'s own existing pattern) still read back the destination's
own center as background, not foreground. This means REVIEW.md finding
#130's original symptom was never *fully* explained by #152's fix
alone -- there is a second, still-unexplained defect specific to
bindless sampling under this exact condition. Given `dual_kawase_blur_
demo.rs`'s own non-bindless approach is already proven working across
many real runs, this step reuses that exact approach for the *internal*
downsample/upsample chain rather than attempting to re-litigate why
bindless still fails here (a real, separate, future investigation --
see "Explicitly out of scope").

The **final** composite step (blurred result -> swapchain) is unaffected
by this: sampling a bindless texture while rendering into the
*swapchain* (not another offscreen target) is exactly what `PopLayer`'s
own existing, already-proven composite step already does today. So
`apply_layer_blur` only needs to produce a final, sample-ready blurred
texture -- `execute_frame`'s own existing `register_bindless`/composite/
`deregister_bindless` machinery handles compositing it, completely
unchanged, just given the blurred texture instead of the raw one.

## Design

`RhiCommandBuffer::apply_layer_blur(&mut self, device: &dyn RhiDevice, source: &dyn RhiTexture, width: u32, height: u32) -> Box<dyn RhiTexture>`:
- `source` is the popped layer's own already-`end_render_to_texture`'d
  content (sampling-ready); `width`/`height` are its own intended/
  logical size (REVIEW.md finding #152's own parameter convention).
- Internally: acquires L1 (half), L2 (quarter), U1 (half) transient
  targets, chains downsample L0(`source`)->L1->L2 then upsample
  L2->U1->U0 (full), exactly `dual_kawase_blur_demo.rs`'s own proven
  4-hop shape and non-bindless mechanism (one reusable descriptor set,
  updated via `vkUpdateDescriptorSets` before each hop; two pipelines,
  `kawase_downsample_nonbindless.frag`/`kawase_upsample_nonbindless.
  frag`, already existing from Step 7.2.1; every draw issued via a raw
  `cmd_draw_indexed` call, matching REVIEW.md finding #130's own real
  fix). Releases L1/L2/U1 internally; returns U0, already `end_render_
  to_texture`'d (sampling-ready) -- the caller (here, `execute_frame`)
  owns and releases it exactly like any other transient target it got
  from `acquire_transient_target` directly.
- The blur-specific descriptor set layout/pool/one reusable set/
  sampler/pipeline layout/2 pipelines are created lazily, once, cached
  on `VulkanDevice` behind a `Mutex<Option<BlurResources>>` -- the same
  established pattern `transient_pool` already uses -- so no caller
  (`execute_frame` included) needs any awareness that blur pipelines
  exist at all; the first real `PushLayer{blur: true}` in the process
  pays a one-time setup cost, matching how the transient pool's own
  first-ever request of a new size already pays a one-time allocation.

`LayerDesc` gains `blur: bool`. Threading it through the IR: `PushLayer`
already smuggles `desc.format` through `pipeline_state_id`
(`texture_format_to_u16`); `blur` rides `PopLayer`'s own currently-
unused `texture_handle` field as `0`/`1` (that field is always
overwritten with the real bindless index at execute time regardless, so
it has no other meaning to preserve -- matching this project's own
established "reuse an otherwise-inert IR field rather than widen
`UiDrawCommand`" precedent).

`execute_frame`'s `PopLayer` handling: after `end_render_to_texture`,
branch on the smuggled `blur` flag -- if set, call `apply_layer_blur`,
release the original (now-unneeded) layer texture, and register/
composite/deregister/release the *returned* blurred texture instead;
if unset, the existing code path is untouched.

## Scope decisions

1. Own-content blur only; true backdrop blur stays real, disclosed
   future work (unchanged from 7.2.1's own decision).
2. `blur: bool`, fixed chain depth (confirmed with the project owner).
3. One purpose-built `apply_layer_blur` trait method (confirmed with
   the project owner) -- not a primitive family, not exposed to any
   other RHI backend surface beyond the trait signature itself (DX12/
   Metal stay `unimplemented!()` stubs, matching every other real
   Vulkan-only capability in this codebase today).
4. Blur pipelines/descriptors are lazily created and cached on
   `VulkanDevice`, not pre-registered by the application -- no
   `PipelineRegistry`/`PipelineKind` entry for them, since they use a
   deliberately different, non-bindless pipeline layout `PipelineKind`'s
   own bindless-only assumption doesn't fit.
5. Nested layers remain unsupported (Step 6.4.1/6.4.2's own existing
   single-level scope, untouched) -- blur adds no new nesting.
6. The still-unexplained "bindless sampling while rendering into a
   different offscreen target" defect this step's own pre-work newly
   surfaced is disclosed as a known gap, not investigated further here
   -- real, separate future work (see "Explicitly out of scope").

## Tasks

1. `crates/tre-engine/src/lib.rs`: `LayerDesc` gains `blur: bool`;
   `push_layer`/`pop_layer` thread it through the IR (`PopLayer`'s own
   `texture_handle` field, per Design); `RhiCommandBuffer` trait gains
   `apply_layer_blur`; `execute_frame`'s `PopLayer` handling branches on
   the flag; `FakeCommandBuffer`/`RecordedCall` test doubles updated;
   new unit test(s) proving the branch and IR round-trip.
2. `crates/tre-rhi-vulkan/src/lib.rs`: `VulkanDevice` gains a lazily-
   initialized `blur_resources: Mutex<Option<BlurResources>>`;
   `VulkanCommandBuffer::apply_layer_blur` implements the real 4-hop
   chain, reusing `kawase_downsample_nonbindless.frag`/`kawase_upsample_
   nonbindless.frag` (already exist) and mirroring `dual_kawase_blur_
   demo.rs`'s own proven pipeline-creation/descriptor-update/raw-draw
   mechanics exactly.
3. New demo (`crates/tre-rhi-vulkan/examples/layer_blur_demo.rs`):
   `push_layer(&LayerDesc { blur: true, .. })`, draw real content inside
   it, `pop_layer()`, `execute_frame` -- real pixel assertions matching
   `dual_kawase_blur_demo.rs`'s own methodology (bounded interior stays
   foreground; a point just outside the original content's hard edge
   shows genuine partial blend), now proven through the real `Canvas`/
   `execute_frame` path instead of hand-written RHI calls -- the same
   graduation `canvas_layer_composite_demo.rs` was to `render_to_
   texture_demo.rs`. Add to `ci.yml`'s `vulkan-validation` job.
4. `documentation/IMPLEMENTATION.md`: Step 7.2.2's own write-up (real
   design, real bug if any found, real verification).
5. `documentation/ARCHITECTURE.md`: new dated annotation for
   `apply_layer_blur` and `LayerDesc.blur`.
6. `documentation/DESIGN.md`: Section 6.2's own "Implementation status"
   bullet updated -- own-content blur is now wired to real layers, not
   just RHI-level plumbing; true backdrop blur still disclosed as
   deferred.
7. `demo/phase7_step7_2_2/`: README + run script + output screenshot.

## Verification plan

- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the
  workspace.
- New demo's own real GPU pixel assertions pass, re-run manually at
  least twice to confirm not a one-off.
- Full regression sweep: every pre-existing Vulkan demo re-run
  manually, zero regressions -- `canvas_layer_composite_demo.rs`
  specifically, since `execute_frame`'s `PopLayer` handling changed.
- Commit; push only on explicit "push it"; `gh run watch` after any
  push (`accessibility-validation`'s pre-existing, documented failure
  expected and unrelated).

## Explicitly out of scope

- True backdrop blur (sampling the swapchain's existing content behind
  a layer) -- unchanged from 7.2.1's own decision.
- Tunable blur radius/quality -- `blur: bool` only this step; a real,
  exposed parameter is future work once a caller actually needs it.
- Investigating *why* the bindless array still can't sample a texture
  while rendering into a different offscreen target, even after
  REVIEW.md finding #152's fix (this step's own pre-work newly found
  and disclosed, not chased further -- `dual_kawase_blur_demo.rs`'s own
  already-proven non-bindless mechanism sidesteps it entirely, and
  re-opening that investigation is a real, separate undertaking with no
  bearing on whether this step's own real capability works).
- Nested layers, blend modes, group opacity -- unchanged from every
  prior step's own scope.
