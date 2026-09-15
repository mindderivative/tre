# Plan: Phase 6, Step 6.3 -- Real Scissor Execution

## Scope decisions (confirmed via direct code investigation, 2026-09-08)

**`RhiCommandBuffer::set_scissor` is a real, working Vulkan method today
with zero real callers.** `canvas_batch_flattening_demo.rs`/
`canvas_sub_canvas_demo.rs`'s own render loops -- both now routed
through Step 6.2's `execute_draw_geometry_batches` -- explicitly skip
every non-`DrawGeometry` command, including `PushScissor`/`PopScissor`.
`canvas_state_stack_demo.rs`'s own source says outright: "not with a GPU
scissor test that doesn't exist... that wiring is Step 5.1.3/Phase 6's
real batch-flattening job" -- Step 5.1.3 built the sort/merge half; this
step is the real wiring that comment was always pointing at.

**Both real emitters of `PushScissor` put the fully-resolved clip rect
directly into the command's own `clip_bounds` -- confirmed by reading
`push_clip`/`begin_overlay`'s real source (`tre-engine/src/lib.rs:789-
860`).** `push_clip` intersects with the current top-of-stack and stores
the result (`clip_bounds: intersected`); `begin_overlay` stores
`FULL_WINDOW_CLIP` (entering the overlay plane resets to unclipped).
`PopScissor`/`PopLayer` -- from both `pop_clip` and `end_overlay` --
always carry a zeroed `clip_bounds`, confirmed by reading both: neither
restores anything into the command itself, so a real executor needs its
own runtime stack to know what to revert to, not the `Pop` command's own
fields.

**A real, previously-unconsidered correctness hazard, found by reading
`VulkanDevice::begin_frame`'s own real source
(`tre-rhi-vulkan/src/lib.rs:1849-1856`), not assumed:** `begin_frame`
already calls a real `cmd_set_scissor` covering the real framebuffer
extent (`{0, 0, width, height}`) before returning the command buffer --
so a scene that never calls `push_clip`/`begin_overlay` at all (both
existing demos, before this step) is *already* correctly, safely
scissored; this step does not need to unconditionally re-apply a "full
window" scissor at frame start. But `FULL_WINDOW_CLIP`
(`tre-engine`'s own IR-level sentinel, `{x:0, y:0, width: u32::MAX,
height: u32::MAX}`) is a CPU-side-only value -- passing it literally to
a real `set_scissor` call would be an invalid, out-of-bounds scissor
rect on real hardware. `begin_overlay`'s own `PushScissor` command
carries this exact sentinel directly (see above), and popping a clip
stack back to empty represents the same "no active clip" concept -- so
a real executor must substitute the real framebuffer extent for this
sentinel at both points, using a `full_window: &ScissorRect` the caller
supplies (the only place that genuinely knows real pixel dimensions;
`tre-engine` itself has no notion of framebuffer size). Both existing
demos already have this value on hand as `CANVAS_WIDTH`/`CANVAS_HEIGHT`.

**Confirmed via `ARCHITECTURE.md` Section 4.2's own real barrier
policy:** every command within one maximal `DrawGeometry` run between
two markers already shares identical `clip_bounds` by construction
(nothing changes the clip stack mid-run). This means scissor only needs
to be (re-)applied when a `PushScissor`/`PopScissor` marker is actually
processed -- never redundantly re-checked before each individual
`DrawGeometry` command, matching Step 6.2's own "no per-draw state-
caching, correctness first" precedent for `set_pipeline`/`bind_texture`.

**`execute_draw_geometry_batches` is renamed to `execute_frame`.** Its
scope is no longer just draw batches -- it now walks the full command
stream (`DrawGeometry` and scissor markers this step, `PushLayer`/
`PopLayer` in Step 6.4) -- and this project prefers a direct rename over
a compatibility shim for internal, actively-evolving code with exactly
two real callers (both already being touched this step regardless).
Renaming now, rather than in Step 6.4, avoids a second rename later.

**No new demo file.** `canvas_batch_flattening_demo`/`canvas_sub_canvas_
demo` both already emit real `PushScissor`/`PopScissor` via `begin_
overlay`/`end_overlay`, but neither nests a *narrower* `push_clip`
inside that overlay bracket -- their own overlay scissor state is just
"unclipped," identical to `begin_frame`'s own default, so wiring real
execution for them is a genuine no-op for rendered pixels, proven by
their own unchanged existing assertions (same pattern as Steps 6.1/6.2).
The one demo that *does* have a genuinely narrower clip is `canvas_
state_stack_demo` -- its own source comment names exactly this future
step ("not a GPU scissor test that doesn't exist"). Real proof this step
adds: rewiring it onto `execute_frame` (replacing its own single,
command-stream-ignorant `draw_indexed` call, the same pattern `sdf_
rounded_rect_demo`/`canvas_draw_text_demo`/`canvas_accessibility_demo`
still use) and upgrading its own clip check from IR-level-only to a real
GPU pixel-readback proof that content outside the clip rect is genuinely
cropped.

## Goal

`execute_frame` (renamed from `execute_draw_geometry_batches`) processes
`PushScissor`/`PopScissor` markers with a real runtime clip stack,
calling `RhiCommandBuffer::set_scissor` with each marker's own resolved
rect (substituting the caller-supplied `full_window` for the
`FULL_WINDOW_CLIP` sentinel wherever it appears). `canvas_batch_
flattening_demo`/`canvas_sub_canvas_demo` are updated to call the
renamed function with zero change to their own existing assertions.
`canvas_state_stack_demo` is rewired onto the same function and gains a
real, new GPU pixel-readback assertion: a pixel inside its own existing
clip rect shows real foreground content; the corresponding pixel just
outside it (where the unclipped geometry would otherwise have painted)
shows background, proving the clip genuinely took effect on real
hardware, not just in the IR.

## Tasks

1. **Rename `execute_draw_geometry_batches` to `execute_frame`**
   (`tre-engine`); add a `full_window: &ScissorRect` parameter and a
   `let mut clip_stack: Vec<ScissorRect> = Vec::new();` local, real
   `match command.kind` in place of the current `if kind !=
   DrawGeometry { continue }`:
   ```rust
   match command.kind {
       CommandType::DrawGeometry => {
           // unchanged from Step 6.2
       }
       CommandType::PushScissor => {
           let resolved = if command.clip_bounds == FULL_WINDOW_CLIP {
               *full_window
           } else {
               command.clip_bounds
           };
           clip_stack.push(resolved);
           cmd_buffer.set_scissor(&resolved);
       }
       CommandType::PopScissor => {
           clip_stack.pop();
           let restored = clip_stack.last().copied().unwrap_or(*full_window);
           cmd_buffer.set_scissor(&restored);
       }
       CommandType::PushLayer | CommandType::PopLayer => {
           // Step 6.4
       }
   }
   ```
   `FULL_WINDOW_CLIP` is private to `tre-engine`'s own module, referenced
   directly since `execute_frame` lives in the same file -- no visibility
   change needed. Every `PushScissor`/`PopScissor` always issues a real
   `set_scissor` call unconditionally (no redundant-call elision) --
   matches "Scope decisions" above on why per-command dispatch, not
   per-draw caching, is correct here, and Step 6.2's own precedent of
   correctness before profiling-driven optimization.

2. **Rewire `canvas_batch_flattening_demo.rs`/`canvas_sub_canvas_demo.rs`**:
   update their call sites to the renamed `execute_frame`, passing
   `&ScissorRect { x: 0, y: 0, width: CANVAS_WIDTH, height: CANVAS_HEIGHT }`
   as `full_window` (both already define these constants). No other
   change -- their own existing assertions must still pass unchanged.

3. **Rewire `canvas_state_stack_demo.rs`** onto `execute_frame` in place
   of its own current single `draw_indexed` call (mirroring task 2, plus
   building a `PipelineRegistry` with just `SdfRoundedRect` registered,
   the only pipeline this demo's own scene uses).

4. **Extend `canvas_state_stack_demo.rs`'s own verification**: after the
   real render, read back one pixel known to be inside the demo's
   existing clip rect (where the clipped-but-not-fully-cropped-away rect
   still paints) and confirm real foreground color; read back one pixel
   just outside the clip rect, where the *unclipped* geometry would have
   painted a visible pixel, and confirm it is background -- the real,
   GPU-level proof this demo's own source comment has been pointing at
   since Step 5.1.1. Exact pixel coordinates TBD during implementation,
   derived from the demo's own already-fixed scene layout.

5. **Unit tests** (`tre-engine`, extending Step 6.2's own `execute_
   draw_geometry_batches_*` tests, renamed to `execute_frame_*`):
   - Existing 3 tests (marker-skipping/dispatch-correctness,
     unregistered-id panic, empty-frame-no-op) updated for the new
     `full_window` parameter and renamed function -- no behavioral
     change to what they assert.
   - A new test: a hand-built frame with `PushScissor(rect A)`, a
     `DrawGeometry`, a nested `PushScissor(rect B)`, a `DrawGeometry`,
     `PopScissor`, another `DrawGeometry`, `PopScissor` -- asserts the
     exact `RecordedCall::SetScissor` sequence: `A`, (draw), `B`,
     (draw), `A` (restored from the stack, not `full_window`, since one
     level remained), (draw), `full_window` (stack now empty). This is
     the real proof the runtime stack -- not just reading each draw's
     own `clip_bounds` -- is what's driving `set_scissor`, and that
     nested restoration is exact, not just "any narrower rect."
   - A new test: a `PushScissor` command whose own `clip_bounds` is
     literally `FULL_WINDOW_CLIP` (reproducing `begin_overlay`'s real
     emission) asserts `SetScissor(full_window)` was called, not
     `SetScissor` with the raw sentinel's `u32::MAX` fields -- the real
     regression guard for the correctness hazard "Scope decisions"
     above found.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- `canvas_batch_flattening_demo`/`canvas_sub_canvas_demo` re-run against
  real Vulkan hardware: every existing assertion must still pass
  unchanged -- the real proof real scissor execution is a genuine no-op
  for scenes that never nest a narrower clip inside their own overlay
  bracket.
- `canvas_state_stack_demo` re-run against real Vulkan hardware: its
  existing IR-level clip assertion stays (still real, still checked),
  plus the new real pixel-readback assertions from task 4 must pass --
  the actual, new proof this step exists to deliver.
- All other pre-existing examples re-run manually end to end, zero
  regressions.
- CI: no new example to add.

## Explicitly out of scope for this sub-step

- `PushLayer`/`PopLayer` execution -- Step 6.4. The `match` arm for
  these is added now (task 1) so `execute_frame` compiles exhaustively,
  but its body stays empty -- these markers are recorded but produce no
  RHI calls yet, exactly matching how `PushScissor`/`PopScissor`
  themselves were left inert through Step 6.2.
- Redundant-scissor-call elision (state-caching to skip a `set_scissor`
  call when the new rect matches the currently-applied one) -- a real,
  deferred future optimization if profiling ever shows it matters, not
  a correctness requirement this step needs.
- Color/HDR work -- Phase 7 (Step 6.1's plan).
