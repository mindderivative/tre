# Plan: Phase 6, Step 6.4.2 -- Wiring `push_layer`/`pop_layer` to Real Render-to-Texture

## Goal

Make `Canvas::push_layer`/`pop_layer` actually acquire a transient
render target, redirect rendering into it, composite it back onto the
swapchain as a real textured quad, and release it -- using Step
6.4.1's RHI capability (`begin_render_to_texture`/`end_render_to_texture`/
`register_bindless`/`deregister_bindless`/`resume_swapchain_rendering`),
driven end to end by a real recorded `Canvas` scene through
`execute_frame`, not hand-written RHI calls. Close out the
`CommandType::PushLayer | CommandType::PopLayer => {}` empty match arm
`execute_frame` has carried since Step 6.2.

## Scope decisions

Grounded in the real current code (`crates/tre-engine/src/lib.rs`
unless noted):

1. **`LayerDesc` has no compositing position today, and `push_layer`
   hardcodes one.** `LayerDesc` (line 361) is `{ width, height,
   format }` -- no `x`/`y`. `push_layer` (line 932) unconditionally
   writes `clip_bounds: ScissorRect { x: 0, y: 0, width: desc.width,
   height: desc.height }` -- position is always the origin. No real
   caller exists yet (`grep` for `push_layer(` outside `#[cfg(test)]`
   returns nothing) so this was never exercised. Fix: add `pub x:
   i32, pub y: i32` to `LayerDesc` (matching `ScissorRect::x/y`'s own
   `i32`, not `u32` -- `ScissorRect` at line 52-57), and have
   `push_layer` write `desc.x`/`desc.y` instead of the literal `0`s.

2. **`layer_depth: u32` (line 487) is a bare counter, not a stack --
   but `pop_layer` needs the *original* pushed `LayerDesc` to bake
   composite geometry.** The counter alone (used only for the
   debug push/pop balance panic, lines 954-961 and the two
   `debug_assert_eq!(self.layer_depth, 0, ...)` calls at 1350/1412)
   can't recover width/height/x/y at pop time. Fix: replace
   `layer_depth: u32` with `layer_stack: Vec<LayerDesc>`. `push_layer`
   pushes `*desc`; `pop_layer` pops it (the `.expect(...)` on `pop`
   replaces today's `checked_sub(1).expect(...)`, same panic message
   contract); `layer_stack.len()` replaces `layer_depth` at both
   `debug_assert_eq!` call sites.

3. **No pipeline id exists yet for a textured-quad composite draw.**
   `PipelineKind` (line 144-149) has exactly `SdfRoundedRect = 0` and
   `MsdfText = 1` -- deliberately excluding a textured-quad pipeline
   (line 138-143's own comment: "not reachable through any real
   `Canvas` drawing method today"). That stops being true once
   `pop_layer` itself emits a textured-quad draw. Fix: add
   `TexturedQuad = 2`. Real callers (the 3 existing `execute_frame`
   demos, plus the new capstone demo) must `registry.register(2,
   ...)` the existing bindless-textured pipeline `render_to_texture_demo.rs`
   already builds by hand (Step 6.4.1) -- no new RHI-side pipeline
   needed, only registering the one that already exists.

4. **The composite quad's texture index isn't known until execute
   time, but its geometry (position/size) is known at record time.**
   `pop_layer` bakes real vertices (4 verts matching the layer's
   `x/y/width/height`, `uv` in `0.0..1.0` -- NOT `draw_rounded_rect`'s
   center-relative SDF convention at line 1032-1037, since
   `TexturedQuad`'s fragment shader samples a real texture, not an
   SDF) into `self.vertices`/`self.indices`, same
   `base_vertex`/`base_index` pattern as `draw_rounded_rect` (line
   1018-1019), applying the active transform/alpha the same way (line
   1026-1030). The emitted `PopLayer` command carries real
   `pipeline_state_id: PipelineKind::TexturedQuad as u16`,
   `element_count: 6`, `vertex_offset: base_index` -- but
   `texture_handle: NO_TEXTURE` as a placeholder, since the real
   bindless index only exists after `execute_frame` calls
   `register_bindless` on the just-rendered-into texture.
   `execute_frame`'s own `PopLayer` handling substitutes the
   real, just-registered index in place of `command.texture_handle`
   when it calls `bind_texture` -- exactly mirroring how `PushScissor`/
   `PopScissor` already substitute `full_window` for `FULL_WINDOW_CLIP`'s
   sentinel (line 2215-2219) rather than trusting the IR's own literal
   value.

5. **`LayerDesc::format` (`TextureFormat`) needs an IR home, and
   `TextureFormat` has no integer repr to reuse directly.**
   `TextureFormat` (line 322) is a plain 3-variant enum, no `#[repr]`.
   Rather than give the whole public `TextureFormat` type a `#[repr(u16)]`
   (wider blast radius -- it's also used by `create_texture`/
   `acquire_transient_target`'s public signatures), add two small
   private `fn texture_format_to_u16`/`fn u16_to_texture_format`
   conversions local to this module. `push_layer`'s own emitted
   `PushLayer` command has every non-`clip_bounds` field unused today
   (`pipeline_state_id: 0, texture_handle: 0, element_count: 0,
   vertex_offset: 0` -- lines 936-940) -- repurpose
   `pipeline_state_id` to carry `texture_format_to_u16(desc.format)`.
   This matches the precedent already set on this exact command:
   `clip_bounds` is already repurposed on `PushLayer` to carry
   width/height instead of an actual clip rectangle (decision 1 above,
   and the existing doc comment at line 927-931).

6. **`execute_frame` cannot reach any of Step 6.4.1's device-level
   calls with its current parameter list.** Its signature (line
   2190-2197) is `(frame, registry, vertex_buffer, index_buffer,
   full_window, cmd_buffer)` -- no `&dyn RhiDevice`.
   `acquire_transient_target`/`release_transient_target`/
   `register_bindless`/`deregister_bindless` are all `RhiDevice`
   methods (lines 2002, 2008, 2061, 2081); `begin_render_to_texture`/
   `end_render_to_texture`/`resume_swapchain_rendering` are
   `RhiCommandBuffer` methods already reachable via `cmd_buffer`. Fix:
   add a `device: &dyn RhiDevice` parameter. Real signature change --
   the 3 existing call sites (`canvas_batch_flattening_demo.rs`,
   `canvas_sub_canvas_demo.rs`, `canvas_state_stack_demo.rs`, each
   confirmed via `grep -l "execute_frame("` above) all pass an
   already-in-scope `&VulkanDevice` (each demo already owns one to
   call `begin_frame`/`create_pipeline`/etc.), so each call site adds
   one argument, no new construction needed.

7. **Single-level layer scope, matching 6.4.1's own explicit
   boundary.** `execute_frame` tracks the in-flight layer in a local
   `Option<(Box<dyn RhiTexture>, u32, i32, i32)>` (texture, bindless
   index, x, y) -- not a stack -- set on `PushLayer`, taken on
   `PopLayer`. True nested layers (a `PushLayer` while another is
   already active) are out of scope here for the same reason 6.4.1
   scoped `resume_swapchain_rendering` to one level (its own doc
   comment, `RhiCommandBuffer` trait, line 2123-2131): no real scene
   needs it yet. If a nested `PushLayer` is hit, `execute_frame`
   panics with a clear message (matching this codebase's established
   "unsupported caller state panics, doesn't silently misbehave"
   convention -- e.g. `pop_layer`'s own unbalanced-call panic) rather
   than silently overwriting the outer layer's state.

8. **Test scaffolding needs a `FakeDevice`.** No `RhiDevice` fake
   exists in the test module today (only `FakePipeline`/`FakeBuffer`/
   `FakeCommandBuffer`, lines 3654/3718/3748) -- `execute_frame`'s
   existing unit tests never needed one. Add a minimal `FakeDevice`
   implementing `RhiDevice`, returning a `FakeTexture` (new, minimal:
   just enough to satisfy `RhiTexture`'s trait surface) from
   `acquire_transient_target`, recording calls through the same
   `RecordedCall` enum `FakeCommandBuffer` already uses (extended with
   `AcquireTransientTarget`, `RegisterBindless`, `DeregisterBindless`,
   `ReleaseTransientTarget` variants) so a single assertion list can
   verify full call ordering across both fakes.

9. **The capstone demo is new, not an extension of an existing one.**
   `canvas_sub_canvas_demo.rs` (the only demo whose name suggests
   layering) is actually Phase 5 Step 5.2.3's own closed capstone
   (its own header: "the capstone of Step 5.2" -- `SubCanvas`/
   `FrameArena`/thread-stitching, unrelated to render-to-texture
   layers). Add a new `crates/tre-rhi-vulkan/examples/
   canvas_layer_composite_demo.rs` instead of repurposing it.

## Tasks

1. `tre-engine`: add `x: i32, y: i32` to `LayerDesc`; fix
   `push_layer`'s `clip_bounds` to use them.
2. `tre-engine`: replace `RenderingCanvas::layer_depth: u32` with
   `layer_stack: Vec<LayerDesc>`; update `push_layer`/`pop_layer`/both
   `debug_assert_eq!` call sites.
3. `tre-engine`: add `PipelineKind::TexturedQuad = 2`.
4. `tre-engine`: add private `texture_format_to_u16`/
   `u16_to_texture_format` conversions.
5. `tre-engine`: rewrite `pop_layer` to bake real composite-quad
   vertices/indices and emit the real `PopLayer` command as described
   in decision 4; `push_layer` emits `pipeline_state_id:
   texture_format_to_u16(desc.format)`.
6. `tre-engine`: add `device: &dyn RhiDevice` to `execute_frame`;
   implement real `PushLayer`/`PopLayer` handling (decisions 6-7);
   panic on nested `PushLayer`.
7. `tre-engine` tests: add `FakeDevice`/`FakeTexture`; extend
   `RecordedCall`; add tests covering: a real push/pop cycle issues
   the expected call sequence in order (acquire, begin-render,
   [draw of the layer's own content], end-render, register-bindless,
   resume-swapchain, set-pipeline/bind-texture-with-the-real-index/
   draw-indexed, deregister-bindless, release); a nested `PushLayer`
   panics with a clear message; `pop_layer` without a matching
   `push_layer` still panics (regression check on decision 2's
   rewrite); `LayerDesc.x/y` land correctly in the baked quad's vertex
   positions.
8. Update the 3 existing `execute_frame` call sites
   (`canvas_batch_flattening_demo.rs`, `canvas_sub_canvas_demo.rs`,
   `canvas_state_stack_demo.rs`) to pass `&device` and to
   `registry.register(2, ...)` the textured-quad pipeline (even though
   none of the three currently call `push_layer`, the compiler
   requires the new argument regardless).
9. New `crates/tre-rhi-vulkan/examples/canvas_layer_composite_demo.rs`:
   a real `Canvas` scene using `push_layer`/draw content/`pop_layer`,
   flattened and driven entirely through `execute_frame` (no hand-written
   RHI calls, unlike `render_to_texture_demo.rs`). Real pixel
   assertions proving the composited result, mirroring
   `render_to_texture_demo.rs`'s own two-assertion shape (composited
   interior opaque, composited-but-transparent area shows background).
10. `.github/workflows/ci.yml`: add the new demo to the
    `vulkan-validation` job, after `render_to_texture_demo`.
11. Update `documentation/ARCHITECTURE.md` (Section 6 annotations),
    `documentation/IMPLEMENTATION.md` (Step 6.4.2 write-up),
    `documentation/REVIEW.md` (any bugs found), `demo/phase6_step6_4_2/`
    (README + run script + output screenshot).

## Verification plan

- `cargo test -p tre-engine`: full suite green, including new
  `FakeDevice`-based tests.
- `cargo clippy --all-targets` clean.
- Run all Vulkan examples locally under the project's existing
  regression-check convention (every demo, not just the new one) --
  zero regressions, matching the standard established at 6.4.1.
- New demo's own real GPU pixel assertions pass locally.
- Commit, push (only on explicit "push it"), confirm CI green via
  `gh run watch` (accessibility-validation's pre-existing, documented
  failure expected and unrelated).

## Explicitly out of scope

- Nested layers (a `PushLayer` while another is already active) --
  panics instead, matching 6.4.1's own explicit single-level scope
  boundary. Real future work once a real scene needs it.
- Visual filters/blur on a composited layer (DESIGN.md Section 6.2) --
  Phase 7's job, not Phase 6's.
- Giving `TextureFormat` itself a public `#[repr(u16)]` -- the private
  local conversion functions (decision 5) are a smaller, safer change.
- The accessibility-validation CI failure (REVIEW.md #126) -- remains
  explicitly deferred per prior user instruction, unrelated to this step.
