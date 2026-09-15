# Plan: Phase 7, Step 7.2.1 -- Real Dual-Kawase Blur RHI Capability

## Goal

Build and prove the real Dual-Kawase downsample/upsample blur
capability on real Vulkan hardware, driven entirely by hand-written RHI
calls -- no `Canvas`/IR involvement (Step 7.2.2's job, matching Step
6.4.1/6.4.2's own precedent). Split from "Step 7.2: Visual Filters
(PushLayer Blurs)" after real investigation found genuinely new
shader/algorithm surface was needed and a real semantic fork (own-
content vs. true backdrop blur) that DESIGN.md doesn't resolve on its
own -- confirmed with the project owner via AskUserQuestion: split into
7.2.1/7.2.2, own-content blur only, true backdrop sampling explicitly
deferred.

## Scope decisions

1. **Real investigation found the split's own initial framing
   overstated how much new RHI surface is needed.** `RhiCommandBuffer::
   begin_render_to_texture`'s own existing implementation
   (`crates/tre-rhi-vulkan/src/lib.rs`) unconditionally calls
   `cmd_end_rendering` before beginning the next scope, with its own
   safety comment already stating "dynamic rendering permits any number
   of begin/end pairs within one command buffer, just never nested."
   This means a real, multi-level downsample/upsample chain needs **no
   new `RhiDevice`/`RhiCommandBuffer` trait methods at all** -- it's a
   correct sequencing of `acquire_transient_target`/
   `begin_render_to_texture`/`end_render_to_texture`/`register_bindless`/
   `deregister_bindless`/`release_transient_target`, all of which
   already exist from Step 6.4.1. This step's real new surface is
   narrower than originally framed: two new shaders and a demo that
   chains existing calls correctly.

2. **Own-content blur only, not true backdrop blur -- per the project
   owner's own confirmed choice.** This step blurs a layer's own newly-
   rendered content, not whatever is visually behind it on the
   swapchain. A true backdrop blur (DESIGN.md Section 6.2's "macOS
   Vibrancy"/"Windows Acrylic" framing) needs a real capability to
   snapshot the swapchain's existing pixels into a sampleable texture --
   confirmed absent via a real grep (`cmd_copy_image`/`cmd_blit_image`/
   `blit`, zero matches in `tre-rhi-vulkan`). Real, disclosed future
   work, not silently dropped.

3. **The canonical Dual-Kawase formula is the standard, widely-cited
   technique (Bjørge, "Bandwidth-Efficient Rendering," SIGGRAPH 2015),
   added as a new TECHNICAL.md Section 5.5** -- confirmed via grep that
   no canonical Dual-Kawase formula exists anywhere in this project's
   docs today, matching this project's own established pattern (the SDF
   rounded-rect formula/MSDF formula/sRGB formula each live once in
   TECHNICAL.md, referenced rather than re-derived at each real call
   site). Downsample: a 5-tap filter (center weight 4, four diagonal
   taps at `+-halfpixel` weight 1 each, sum/8). Upsample: an 8-tap ring
   filter around `halfpixel`/`2*halfpixel` offsets with no center sample
   (axis taps weight 1, diagonal taps weight 2, sum/12) -- the standard
   formulation, not re-derived here beyond stating it once.

4. **Two new shaders share the existing `bindless_textured.vert`**
   (`kawase_downsample.frag`/`kawase_upsample.frag`), matching how
   `msdf.frag`/`bindless_textured.frag` already share that same vertex
   shader (Step 4.2.3's own precedent) -- nothing about the vertex stage
   needs to differ for a full-target textured quad. Each fragment shader
   takes the source texture's own reciprocal-dimensions as a push-
   constant `half_pixel: vec2`, per the canonical formula's own offset
   convention.

5. **Verification: a small, isolated opaque square, well surrounded by
   background, chained through real downsample-then-upsample passes.**
   After the real blur, a point just outside the square's own original
   hard boundary must show a genuine partial blend (neither pure
   background nor pure foreground) -- proving real blur spread, the same
   "genuine partial blend, not an extreme" methodology `canvas_state_
   stack_demo.rs`'s own Rect B check already established. A point deep
   in the square's own interior must stay mostly foreground (not washed
   to uniform gray) -- proving the blur is a real, bounded operation at
   this radius, not a no-op or an over-blur that erases the shape
   entirely.

6. **Demo is hand-written RHI calls only, matching `render_to_texture_
   demo.rs`'s own Step 6.4.1 precedent exactly.** `RenderingCanvas` is
   used purely as a convenient vertex-data builder for the square (the
   same non-`Canvas`-flow use `render_to_texture_demo.rs` already
   established) -- no `push_layer`/`pop_layer`/`execute_frame`
   involvement anywhere in this step; that wiring is Step 7.2.2's own
   job, exactly mirroring how 6.4.1 stayed hand-written and 6.4.2 wired
   it to the IR.

7. **Blend modes and group opacity are not this step's scope, and never
   were.** DESIGN.md Section 6.2 names them under the same "Visual
   Filter Pipeline" heading as blur, but IMPLEMENTATION.md's own real
   Step 7.2 task list (the one actually being executed) never mentions
   either -- confirmed by re-reading its exact 3 tasks, all Dual-Kawase-
   specific. Not a deferral of planned work, since it was never planned
   here to begin with.

## Tasks

1. `documentation/TECHNICAL.md`: new Section 5.5 "Dual-Kawase Blur" --
   the canonical downsample/upsample sample-offset/weight formulas per
   scope decision 3.
2. Two new shaders: `crates/tre-rhi-vulkan/shaders/kawase_downsample.
   frag`, `kawase_upsample.frag` (paired with the existing
   `bindless_textured.vert`).
3. New demo (`crates/tre-rhi-vulkan/examples/dual_kawase_blur_demo.rs`):
   hand-chains a real multi-level downsample-then-upsample pass sequence
   using only already-existing RHI calls, per scope decisions 1/6;
   real pixel assertions per scope decision 5.
4. `.github/workflows/ci.yml`: add the new demo to the
   `vulkan-validation` job.
5. `demo/phase7_step7_2_1/`: README + run script + output screenshot,
   matching every prior demo-bearing step.
6. Update `documentation/IMPLEMENTATION.md` -- split "Step 7.2: Visual
   Filters (PushLayer Blurs)" the same way Step 6.4 was split: the
   existing heading stays as the original aspirational outline (matching
   the `## Phase 6` correction note's own precedent for how a pre-
   existing outline entry gets superseded by real, numbered sub-steps
   without being silently rewritten), with a new "Step 7.2.1" write-up
   below it. `documentation/ARCHITECTURE.md` updated only if real new
   trait surface needs annotating -- expected to need none, since no
   `RhiCommandBuffer`/`RhiDevice` method signatures change.

## Verification plan

- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the
  workspace (no `tre-engine` changes expected this step).
- New demo's own real GPU pixel assertions pass (edge bleed outside the
  square's original boundary; bounded, non-washed-out interior).
- Full regression sweep across all 24 pre-existing Vulkan demos -- the
  two new shaders are not wired into any existing pipeline id or
  consumer, so zero existing demo should be affected; confirmed by
  actually re-running every one, not assumed from that reasoning alone.
- Commit; push only on explicit "push it"; `gh run watch` after any push
  (`accessibility-validation`'s pre-existing, documented failure
  expected and unrelated).

## Explicitly out of scope

- True backdrop blur (sampling the swapchain's existing content behind
  a layer) -- needs a real image-copy/snapshot RHI capability this step
  doesn't build; real, disclosed future work per the project owner's own
  scope choice.
- Wiring this capability to `Canvas::push_layer`/`pop_layer`/`LayerDesc`
  -- Step 7.2.2's own job.
- Blend modes (Multiply/Screen/Overlay/Soft Light/Color Dodge) and group
  opacity -- named in DESIGN.md's own "Visual Filter Pipeline" heading
  but never part of IMPLEMENTATION.md's own real Step 7.2 task list.
- Runtime-tunable blur radius/quality (variable chain depth chosen by a
  caller) -- this step proves the mechanism at one real, fixed chain
  depth; making it a real, exposed parameter is future work once a real
  consumer (Step 7.2.2) needs it.
