# Plan: Phase 6, Step 6.2 -- The Generic Frame Executor

## Scope decisions (confirmed with the project owner, 2026-09-08)

**This is the real work the pre-existing outline's own "Step 6.2:
Dynamic Index Stitching" task 3 named but never detailed** ("Emit a
single `RhiCommandBuffer::draw_indexed` call for the entire aggregated
batch") -- see IMPLEMENTATION.md's Phase 6 correction note and Step
6.1's own plan (`planning/archive/PLAN_PHASE6_STEP6_1.md`) for the full
account of that outline's real status. Tasks 1-2 of that original
step (the sweep + index-offset consolidation) are already real, via Step
5.1.3's `flatten_run`. This step builds task 3, using Step 6.1's
`PipelineRegistry`.

**Confirmed via direct comparison of both real call sites this step
replaces:** `canvas_batch_flattening_demo.rs:242-256` and
`canvas_sub_canvas_demo.rs:359-373` are byte-for-byte identical --
both loop over `frame.commands`, skip non-`DrawGeometry` markers, branch
on `pipeline_state_id == PIPELINE_MSDF_TEXT` to choose a pipeline object
and texture-bind policy, then bind vertex/index buffers and call
`draw_indexed`. With Step 6.1's registry and unified `NO_TEXTURE`
sentinel now real, that branch collapses to a single, generic
`registry.get(command.pipeline_state_id)` lookup plus an unconditional
`bind_texture(0, command.texture_handle)` -- no per-pipeline-kind
special case needed at all, which is exactly what makes this step
buildable now and not before.

**Vertex/index buffer upload stays the caller's job, not this
function's.** Both demos build their buffers via `VulkanDevice::
upload_buffer` (`tre-rhi-vulkan/src/lib.rs:1194`) -- a concrete,
Vulkan-specific inherent method, not part of the `RhiDevice` trait.
A generic, backend-agnostic executor living in `tre-engine` (matching
`PipelineRegistry`'s own placement, Step 6.1) cannot call it. This also
matches DESIGN.md's own frame-lifecycle numbering, where "Buffer
Packing" (item 7) is a distinct stage from "Sorting, Flattening &
Batching" (item 6) -- this step's function receives already-created
`&dyn RhiBuffer` handles as parameters, exactly as both demos' existing
local `vertex_buffer`/`index_buffer` variables already are by the time
their own loops run today.

**Still only two pipeline kinds are reachable through `Canvas`, for the
same reason Step 6.1 registered only two -- this step doesn't change
that.** The new function is proven correct and generic (it works for
however many kinds a caller registers), but its only two real, current
callers (the two demos above) still only ever emit `SdfRoundedRect`/
`MsdfText` commands, since `Canvas` still has no `draw_image`/
`draw_path`/`draw_svg`. No new demo is built this step: updating the
two existing demos to call the new shared function, keeping their own
already-real pixel-readback assertions as the regression check, is the
real proof -- a brand-new demo would exercise the identical code path
those two already do, adding process, not new coverage.

**An unregistered pipeline id is a panic, not a `Result`, in this
function -- a deliberate choice, not an oversight.** `PipelineRegistry::
get` itself returns `Option` (Step 6.1), staying honest at the primitive
level that a lookup can miss. But this function's own two real callers
always register every pipeline `Canvas` can emit before rendering a
single frame -- a command whose id was never registered is a static
setup bug (the registry doesn't match what `Canvas` can produce), not a
transient, recoverable-mid-frame condition in the sense `EngineError`'s
own variants describe (device loss, swapchain-out-of-date, resource
exhaustion). Matches this crate's established `pop_layer`/`restore`/
`PipelineRegistry::register`-style precedent for invalid caller state.

## Goal

A real, generic function in `tre-engine` -- taking a `FlattenedFrame`, a
`PipelineRegistry`, a vertex buffer, an index buffer, and a `&mut dyn
RhiCommandBuffer` -- drives every `DrawGeometry` batch through the RHI
by resolving `pipeline_state_id` via the registry, with no per-pipeline-
kind special case. `canvas_batch_flattening_demo.rs` and
`canvas_sub_canvas_demo.rs` are both rewired to call it, replacing their
own duplicated loops, with zero change to their own already-real pixel
assertions -- proving this is a genuine behavioral no-op, not just a
new function that happens to compile.

## Tasks

1. **`execute_draw_geometry_batches`** (`tre-engine`, alongside
   `PipelineRegistry`):
   ```rust
   pub fn execute_draw_geometry_batches(
       frame: &FlattenedFrame,
       registry: &PipelineRegistry,
       vertex_buffer: &dyn RhiBuffer,
       index_buffer: &dyn RhiBuffer,
       cmd_buffer: &mut dyn RhiCommandBuffer,
   ) {
       for command in &frame.commands {
           if command.kind != CommandType::DrawGeometry {
               continue;
           }
           let pipeline = registry.get(command.pipeline_state_id)
               .unwrap_or_else(|| panic!(
                   "no pipeline registered for id {}",
                   command.pipeline_state_id
               ));
           cmd_buffer.set_pipeline(pipeline);
           cmd_buffer.bind_texture(0, command.texture_handle);
           cmd_buffer.bind_vertex_buffer(vertex_buffer, 0);
           cmd_buffer.bind_index_buffer(index_buffer, 0);
           cmd_buffer.draw_indexed(
               command.element_count,
               command.vertex_offset,
               0,
           );
       }
   }
   ```
   `PushScissor`/`PopScissor`/`PushLayer`/`PopLayer` commands are
   skipped (`continue`), exactly matching both demos' current behavior
   -- real handling is Steps 6.3/6.4, not this one. Vertex/index buffer
   offset is always `0`: both real callers upload one whole buffer per
   frame today (no ring-buffer segmenting yet), matching their own
   existing `bind_vertex_buffer(&vertex_buffer, 0)` calls exactly --
   this function does not change that, only extracts the loop around it.

2. **Rewire `canvas_batch_flattening_demo.rs`**: build a
   `PipelineRegistry`, `register(PipelineKind::SdfRoundedRect as u16,
   Box::new(rect_pipeline))` and `register(PipelineKind::MsdfText as
   u16, Box::new(msdf_pipeline))` in place of the two bare local
   variables, replace the hand-rolled loop (lines 242-256) with one call
   to `execute_draw_geometry_batches`. No change to scene setup, buffer
   upload, or any existing assertion.

3. **Rewire `canvas_sub_canvas_demo.rs`** identically (lines 359-373).

4. **Unit tests** (`tre-engine`, extending `PipelineRegistry`'s own
   established test-double style):
   - A minimal `FakeCommandBuffer` (`RhiCommandBuffer` double) recording
     every call made against it (pipeline handle, texture index, buffer
     handles, draw args) in order, and a `FakeBuffer` (`RhiBuffer`
     double) with a distinguishable `raw_handle()`.
   - A hand-built `FlattenedFrame` mixing `PushScissor`/`PopScissor`
     markers with two `DrawGeometry` commands using two different
     registered pipeline ids: asserts the marker commands produce no
     recorded calls at all, and each `DrawGeometry` command produces
     exactly the expected `set_pipeline`/`bind_texture`/
     `bind_vertex_buffer`/`bind_index_buffer`/`draw_indexed` call
     sequence, with the *correct* pipeline object and the command's own
     `texture_handle`/`element_count`/`vertex_offset` -- not just "some
     calls happened."
   - A `DrawGeometry` command whose `pipeline_state_id` was never
     registered panics with a clear message naming the id.
   - An empty `frame.commands` produces zero calls (no panic, no
     spurious draw).

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- `canvas_batch_flattening_demo` and `canvas_sub_canvas_demo` re-run
  against real Vulkan hardware: every existing assertion (exact batch
  count/content at the IR level, real non-background pixels at every
  rect's/glyph's own position) must still pass unchanged -- the real
  proof this step is a genuine no-op for rendered output, now produced
  via shared code instead of duplicated code.
- All other pre-existing examples re-run manually end to end, zero
  regressions (this step touches no shader/pipeline-creation code, only
  the two demos' own render-loop bodies plus a new, currently-optional-
  to-call `tre-engine` function).
- CI: no new example to add; the two rewired demos are already in the
  `vulkan-validation` job's example list.

## Explicitly out of scope for this sub-step

- `PushScissor`/`PopScissor` execution (real `set_scissor` calls) --
  Step 6.3.
- `PushLayer`/`PopLayer` execution (real transient-target acquisition
  and compositing) -- Step 6.4.
- Any change to how vertex/index buffers are created or uploaded, or to
  ring-buffer-based per-frame packing -- "Buffer Packing" is its own
  distinct frame-lifecycle stage (DESIGN.md item 7) this step's function
  deliberately receives already-built buffer handles for, not builds.
- Registering or exercising the plain-textured-quad, flat-vertex-color,
  stencil, or cover pipeline kinds -- unreachable through `Canvas` today,
  same reasoning as Step 6.1.
- A `Result`-returning error path for an unregistered pipeline id -- see
  "Scope decisions" above for why this is a deliberate panic.
- Color/HDR work -- moved to its own Phase 7 (Step 6.1's plan).
