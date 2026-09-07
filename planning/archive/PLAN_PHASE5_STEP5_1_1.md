# Plan: Phase 5, Step 5.1.1 -- Canvas Drawing-Context State Stack

## Scope decisions (confirmed with the project owner, 2026-09-07)

**Step 5.1 ("The Canvas Command Recorder") is split into three
sub-steps**, the same way Steps 3.3/4.2/4.3 were each split into
independently-plannable, independently-testable chunks:

- **5.1.1 (this plan):** the Drawing Context's hierarchical state stack
  -- `save`/`restore` (transform + alpha) and the separate `push_clip`/
  `pop_clip` scissor stack DESIGN.md Section 6.1's own architecture
  diagram lists as a distinct mechanism -- wired into the one real
  primitive `RenderingCanvas` already has (`draw_rounded_rect`, from
  Phase 0).
- **5.1.2 (next):** `draw_text`, the first real wiring of `tre-engine`
  into `tre-text`/`tre-atlas` (MSDF glyph rendering driven through the
  `Canvas` API instead of every demo hand-building its own vertex
  arrays, as every text/SVG demo through Phase 4 has done).
- **5.1.3 (last):** the real 64-bit sort key (ARCHITECTURE.md Section
  4.1), `begin_overlay` (Layer ID routing), and real batch flattening --
  the capstone, needing multiple real draw kinds/pipelines coexisting
  (from 5.1.1/5.1.2) to be meaningfully provable, the same reason Step
  4.2's own concurrency-wiring capstone came last.

**Why this split, and why the state stack goes first.** IMPLEMENTATION.md's
own task list for Step 5.1 bundles "build the `RenderingCanvas` API with
Drawing Contexts (`draw_rect`, `draw_text`, `push_layer`, `save`,
`restore`)" into one task, but `draw_text` alone is a substantial,
separable integration problem (a new cross-crate dependency from
`tre-engine` into `tre-text`/`tre-atlas` that doesn't exist today), and
neither `draw_text` nor the real sort key/overlay routing (task 3) can be
meaningfully tested without the state stack this sub-step builds first
-- every draw call needs to respect the active transform/alpha/clip
before it's worth composing multiple draw kinds together at all.

**`RenderingCanvas` is not started from scratch -- Phase 0 already built
a real, working stub.** `crates/tre-engine/src/lib.rs` already has
`RenderingCanvas` with `draw_rounded_rect` (a real analytical-SDF
rounded rect, IMPLEMENTATION.md Step 3.2), `push_layer`/`pop_layer` (IR
markers with a debug-only balance counter), a `flatten()` pass-through,
and the exact `CommandType`/`UiDrawCommand` structs ARCHITECTURE.md
Section 3.2 specifies -- including `PushScissor`/`PopScissor` variants
that have existed since Phase 0 but have never once been emitted. This
sub-step is extending that real stub with real state, not building a
parallel new API.

**`save`/`restore` and `push_clip`/`pop_clip` are two separate stacks,
not one -- DESIGN.md's own architecture diagram (Section 6, the
"Rendering Canvas" box) lists four distinct mechanisms: a "Drawing
Context State Stack (Matrix Transform, Alpha, Blend)," a "Primitive
Recording API," a "Compositing Layer Stack (PushLayer/PopLayer)," and a
separate "Dynamic Scissor / Mask Clip Stack (PushClip, PopClip)."**
`save`/`restore` snapshot only transform and alpha (blend mode stays
deferred, matching `LayerDesc`'s own doc comment: "opacity/blend-mode/
blur-radius fields belong here once a later phase implements those
visual filters"); clipping gets its own independent `push_clip`/
`pop_clip` pair with its own balance counter, mirroring `push_layer`/
`pop_layer`'s already-established `layer_depth` pattern (a new
`clip_depth`), not folded into the same stack as transform/alpha.

**`transform`/`set_alpha` are new mutator methods this sub-step adds --
not named in IMPLEMENTATION.md's own task list, but required for
`save`/`restore` to have anything real to snapshot.** `Canvas::transform
(&Affine2)` composes the given matrix onto the current top-of-stack
transform via `tre_math::Affine2::compose` (already built, Phase 3 Step
3.1 -- reused directly, not reimplemented); `Canvas::set_alpha(f32)`
multiplies the current top-of-stack's alpha by the given factor, so
nested `save()`/`set_alpha()`/`restore()` brackets compound correctly
(a child at local alpha 0.5 inside a parent already at effective alpha
0.5 renders at effective 0.25, standard nested-group-opacity semantics).

## Goal

`RenderingCanvas::draw_rounded_rect` respects real hierarchical state:
its four emitted vertices are transformed by the active `Affine2` (not
emitted in raw local coordinates as today), its color's alpha channel is
scaled by the active effective alpha, and its `UiDrawCommand::clip_bounds`
reflects the active clip rect (not the hardcoded "full window" sentinel
Phase 0 left in place) -- all correctly composed across nested `save`/
`restore` and `push_clip`/`pop_clip` brackets, proven by both unit tests
(hand-computed transformed/alpha-scaled/clip-intersected expected values,
matching this crate's own existing testing style) and a new real GPU
demo rendering several nested-transform rects and reading back real
pixels.

## Tasks

1. **`CanvasState { transform: Affine2, alpha: f32 }`** plus a
   `state_stack: Vec<CanvasState>` field on `RenderingCanvas`,
   initialized with one entry (`Affine2::IDENTITY`, `alpha: 1.0`).

2. **`Canvas::save()`**: pushes a copy of the current top-of-stack state.
   **`Canvas::restore()`**: pops it.
   ### Panics
   Panics if called with nothing left to pop below the initial entry --
   an unbalanced `restore()` is a programmer error (DESIGN.md Section
   2.6), matching `pop_layer`'s own existing precedent exactly.

3. **`Canvas::transform(&Affine2)`**: `self.top_mut().transform =
   self.top().transform.compose(matrix)` (child-relative composition,
   DESIGN.md Section 7.1's `world_child = world_parent * local_child`
   convention -- reusing `tre_math::Affine2::compose`, not
   reimplementing matrix multiplication here).

4. **`Canvas::set_alpha(f32)`**: `self.top_mut().alpha *= factor`.

5. **`clip_stack: Vec<ScissorRect>`** (separate from `state_stack`,
   starting empty -- an empty stack means "no clip, full window," the
   same sentinel `draw_rounded_rect` already hardcodes today) plus a
   `clip_depth: u32` debug-only balance counter mirroring `layer_depth`'s
   existing pattern exactly.
   **`Canvas::push_clip(&ScissorRect)`**: intersects the given rect with
   the current clip (or uses it directly if the stack is empty), pushes
   the intersection, and emits a real `PushScissor` `UiDrawCommand` (the
   variant has existed since Phase 0; this is its first real emission).
   **`Canvas::pop_clip()`**: pops the clip stack and emits a real
   `PopScissor` command.
   ### Panics
   Panics on an unbalanced `pop_clip()`, same precedent as `restore()`/
   `pop_layer()`.

6. **Rewire `draw_rounded_rect`**: apply the current top-of-stack's
   `transform` to each of the four emitted vertex positions
   (`Affine2::transform_point`, already built); scale the given `rgba`'s
   alpha byte by the current effective alpha (`u32::to_le_bytes`/
   `rgba8`'s own established byte order, TECHNICAL.md/this crate's own
   `rgba8` doc comment); set the emitted `UiDrawCommand::clip_bounds` to
   the current top of `clip_stack` (or the existing full-window sentinel
   if empty) instead of the hardcoded sentinel it uses unconditionally
   today.

7. **`flatten()`'s existing debug-assert** (currently only checking
   `layer_depth == 0`) extended to also assert `clip_depth == 0` and
   `state_stack.len() == 1` -- an unreleased clip or an unbalanced
   `save()` should fail loudly at the same point an unbalanced
   `push_layer()` already does, not silently leak into the next frame.

8. **Unit tests**, extending this crate's own existing
   hand-computed-expected-value style (`draw_rounded_rect_encodes_uv_as_
   center_relative_offset...` is the established precedent):
   - A translated `save()`/`transform()`/`draw_rounded_rect()`/
     `restore()` bracket produces vertices at the exact expected
     translated positions; a rect drawn after `restore()` is back at its
     raw, untransformed positions.
   - Nested `save()`/`transform()` calls compose correctly (two nested
     translations produce the vertex positions their combined offset
     predicts -- exercising `Affine2::compose`'s already-tested
     semantics through the Canvas, not re-deriving matrix math here).
   - Nested `set_alpha()` calls compound multiplicatively across `save`/
     `restore` (0.5 inside 0.5 yields an emitted alpha byte matching
     effective 0.25, rounded the same way `rgba8` callers elsewhere in
     this crate already round float-to-u8 channel values).
   - `push_clip()` intersects correctly (a narrower clip nested inside a
     wider one yields the narrower rect; `pop_clip()` restores the wider
     one exactly).
   - Unbalanced `restore()`/`pop_clip()` each panic with a clear message.
   - `flatten()`'s extended debug-assert fires for an unbalanced `save()`
     or `push_clip()`, not just an unbalanced `push_layer()`.

9. **New example** (`crates/tre-rhi-vulkan/examples/canvas_state_stack_demo.rs`,
   `demo/phase5_step5_1_1/`): several rounded rects drawn through real
   nested `save`/`transform`/`set_alpha`/`restore` and `push_clip`/
   `pop_clip` brackets -- e.g. a parent-relative translated child rect,
   a partially-transparent nested rect, and a rect clipped to a smaller
   region than its own full quad -- rendered through the existing,
   unmodified `sdf_rounded_rect` pipeline (Step 3.2) and read back as
   real pixels: the transformed rect lands where the composed matrix
   predicts, the alpha-scaled rect blends visibly against the
   background (a real, non-1.0/0.0 blended pixel, not just "some color"),
   and the clipped rect's pixels outside its clip rect are confirmed
   still background. Exact scene layout TBD during implementation.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- `canvas_state_stack_demo` run under `VK_LAYER_KHRONOS_validation`, zero
  errors -- reuses the existing, unmodified `sdf_rounded_rect` pipeline,
  so this confirms no regression, not new RHI surface area.
- `sdf_rounded_rect_demo` (the existing caller of `draw_rounded_rect`,
  which never uses `save`/`transform`/`push_clip`) re-run manually to
  confirm the rewired vertex/alpha/clip logic is a genuine no-op at the
  default identity/full-alpha/no-clip state -- its own existing pixel
  assertions must still pass unchanged.
- All 17 pre-existing examples (16 plus this sub-step's own new one is
  the 17th) re-run manually against real Vulkan hardware.
- CI: add `canvas_state_stack_demo` to the `vulkan-validation` job's
  example list; push, confirm green.

## Explicitly out of scope for this sub-step

- `draw_text`, `draw_path`, `draw_svg` as real `Canvas` methods -- Step
  5.1.2. This sub-step only rewires the one primitive that already
  exists (`draw_rounded_rect`).
- Blend mode in the state stack -- deferred with `LayerDesc`'s own
  visual-filter fields, per "Scope decisions" above.
- The real 64-bit sort key, `begin_overlay`, and real batch flattening
  (still "sort_key: 0, one element sorts itself") -- Step 5.1.3.
- Wiring `push_layer`/`pop_layer` to a real transient render target --
  already explicitly deferred since Phase 0 (this struct's own doc
  comment); still nothing downstream consumes it.
- Any change to `tre-rhi-vulkan`'s actual pipeline/shader code -- this
  sub-step is Canvas-API-layer only, reusing the existing
  `sdf_rounded_rect` pipeline exactly as `sdf_rounded_rect_demo` already
  does.
