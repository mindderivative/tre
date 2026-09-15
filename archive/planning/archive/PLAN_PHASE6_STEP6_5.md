# Plan: Phase 6, Step 6.5 -- The Combining Capstone

## Goal

Close out Phase 6 with one real recorded `Canvas` scene, submitted as a
single `execute_frame` call, that exercises multiple real pipelines,
real clipping, and real layer compositing *together* -- proving they
interoperate correctly, not just each in isolation. This is a
pre-existing, planned item, not a new invention: `planning/archive/
PLAN_PHASE6_STEP6_1.md`'s own "Scope decisions" section named it
explicitly: "6.5: a combining capstone -- one real scene exercising
multiple pipelines and real clipping together in one submitted frame
(layer compositing joins once 6.4 makes it real), matching Step 5.3.3's
own precedent of a final capstone proving previously-separate pieces
together." Step 6.4 (6.4.1 + 6.4.2) is now done, so this is ready.

## Scope decisions

1. **Every `CommandType` variant now has real execution -- this step
   proves them together, it adds no new `tre-engine` surface.**
   `execute_frame` (`crates/tre-engine/src/lib.rs`) fully handles
   `DrawGeometry`/`PushScissor`/`PopScissor`/`PushLayer`/`PopLayer`
   today (Steps 6.2/6.3/6.4.2). No prior demo has combined all of them
   in one scene: `canvas_state_stack_demo.rs` combines `DrawGeometry`+
   `PushScissor`/`PopScissor` (+ transform/alpha, unrelated to
   `execute_frame`); `canvas_layer_composite_demo.rs` combines
   `DrawGeometry`+`PushLayer`/`PopLayer`. Neither combines clipping and
   layering in the same frame, and neither uses more than one real
   content pipeline.

2. **Three real pipeline ids, three real formats, no id used at two
   formats in one frame -- a genuine `PipelineRegistry` constraint
   already documented in `canvas_layer_composite_demo.rs`'s own header
   comment** ("`PipelineRegistry` maps one id to exactly one pipeline
   object per frame, so a scene that also drew directly to the swapchain
   with the same pipeline kind would need a second id"). Resolved by
   giving each pipeline exactly one role in the scene:
   - `PipelineKind::SdfRoundedRect` (id 0): a rounded rect drawn
     **directly onto the swapchain**, wrapped in `push_clip`/`pop_clip`,
     built against `HEADLESS_FORMAT`. Deliberately sized larger than its
     own clip rect on every side (`canvas_state_stack_demo.rs`'s own
     Rect C precedent), so real scissor cropping has something to prove.
   - `PIPELINE_MSDF_TEXT`/`PipelineKind::MsdfText` (id 1): real shaped
     text drawn via `draw_text`, called **inside** a `push_layer`/
     `pop_layer` bracket -- its quads land in the layer's own local
     coordinate space, built against the layer's own `Rgba16Float`
     format (`canvas_draw_text_demo.rs`'s own `bindless_textured.vert`+
     `msdf.frag` pipeline, just retargeted).
   - `PipelineKind::TexturedQuad` (id 2): the layer's own composite
     draw (`pop_layer`'s own baked geometry, Step 6.4.2), built against
     `HEADLESS_FORMAT`, exactly as `canvas_layer_composite_demo.rs`
     already does it.

3. **Text needs a real, already-resolved atlas before the real scene is
   recorded -- `draw_text`'s own documented cache-miss-then-hit contract
   (`canvas_draw_text_demo.rs`'s own established two-frame pattern).**
   A throwaway "warm-up" `Canvas`/`draw_text` call records the real cache
   misses and fires real `request_insert`s; the demo polls the real
   background `AtlasOwner` thread (same bounded retry loop
   `canvas_draw_text_demo.rs` already uses) until every distinct glyph
   resolves, then builds the real, single combined scene in a *second*
   `Canvas`. The resolved atlas is uploaded once via `RhiDevice::
   create_texture` (already bindless-registered by construction, unlike
   `acquire_transient_target`'s own transient targets) before the real
   scene's `draw_text` call, so its `texture_handle` is a real, already-
   known bindless index at record time -- no placeholder needed (unlike
   the layer's own composite texture, which genuinely isn't known until
   `execute_frame` runs).

4. **Verification reuses two already-established, real-pixel-scanning
   patterns instead of inventing new ones.** Clip cropping: an
   inside-clip pixel reads real foreground, the corresponding pixel
   inside the drawn geometry but outside the clip reads real background
   (`canvas_state_stack_demo.rs`'s own Rect C check). Composited text:
   each glyph's own on-screen quad -- recomputed independently from the
   same pen-accumulation formula `draw_text` itself uses, offset by the
   layer's own composite origin since the glyphs live inside a
   composited layer -- is scanned for *any* non-background pixel
   (`canvas_draw_text_demo.rs`'s own glyph-quad scan, not a single
   center-pixel assertion, since an open counter can be background at
   its exact center). A point inside the layer's own composited bounds
   but with no glyph there must show real background, proving the layer
   was genuinely cleared to transparent (`render_to_texture_demo.rs`'s
   own "composited transparent area" check).

## Tasks

1. New `crates/tre-rhi-vulkan/examples/canvas_combined_scene_demo.rs`:
   - Real cascade font + shaped word, real `AtlasOwner`, warm-up
     `draw_text` call + poll-to-resolution (`canvas_draw_text_demo.rs`'s
     own pattern, reused not reinvented).
   - Upload the resolved atlas via `create_texture`, get its real
     bindless index.
   - Build the real combined scene on a fresh `Canvas`:
     `push_clip`/`draw_rounded_rect` (oversized vs. the clip)/
     `pop_clip`, then `push_layer`/`draw_text` (using the real atlas
     index)/`pop_layer`.
   - `flatten()`, register all 3 pipelines against their own real
     formats, one `execute_frame` call.
   - Real pixel assertions per scope decision 4.
   - Write an output PNG.
2. `.github/workflows/ci.yml`: add the new demo to the
   `vulkan-validation` job, after `canvas_layer_composite_demo`.
3. `demo/phase6_step6_5/`: README + run script + output screenshot,
   matching every prior demo-bearing step.
4. Update `documentation/IMPLEMENTATION.md` (Step 6.5 write-up,
   including marking Phase 6 itself closed if nothing else is
   outstanding), `documentation/ARCHITECTURE.md` (only if this step's
   own work reveals anything not already annotated -- expected to need
   none, since no new `tre-engine`/RHI surface is added), and
   `documentation/REVIEW.md` (only if a real bug is found, matching
   this project's own honest-disclosure precedent -- not invented
   preemptively).

## Verification plan

- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the
  workspace (no `tre-engine` changes expected this step, so its own 59
  tests should be unaffected -- confirmed, not assumed).
- New demo's own real GPU pixel assertions pass locally.
- All pre-existing Vulkan demos (the 22 from Step 6.4.2, per the exact
  list in `.github/workflows/ci.yml`'s `vulkan-validation` job) re-run
  manually, zero regressions -- this step is scene-composition only, no
  shared engine/RHI code is touched, but the project's own standing
  discipline is to confirm this, not assume it from that reasoning alone.
- Commit; push only on explicit "push it"; `gh run watch` after any push
  (`accessibility-validation`'s pre-existing, documented failure
  expected and unrelated).

## Explicitly out of scope

- Nested layers, visual filters/blur on the composited layer, and the
  accessibility-validation CI fix -- unrelated, already-documented
  deferrals from prior steps, unaffected by this one.
- Any new `tre-engine`/RHI surface -- this step is a pure integration
  proof of existing capability, matching Step 5.3.3's own "capstone"
  precedent (a new demo combining already-built pieces, not new engine
  work).
