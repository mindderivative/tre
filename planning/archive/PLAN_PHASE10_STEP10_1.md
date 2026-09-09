# Plan: Phase 10, Step 10.1 -- Efficient Shape Primitives for External UI Frameworks

## Status

**Planning only -- no code written yet.** This plan, and the canonical
struct/trait reference it points to (`documentation/ARCHITECTURE.md`
Section 7), are this request's own deliverable: "create files... with a
completed architecture and plan for the accessible shape calls." A
future session implements against this plan; only then does this file
get executed and archived per this project's own standing Phase/Step
process.

## Renumbering (done as part of this same request)

Phase 10 previously had two steps: 10.1 (`tre-ffi` C-ABI crate), 10.2
(Python bindings). Renumbered to make room for this new step ahead of
the C-ABI crate, so the ABI has a real, designed shape-primitive surface
to expose rather than being built first and bent around later:

- Old 10.1 (`tre-ffi` C-ABI crate) -> **new 10.2**
- Old 10.2 (Python bindings) -> **new 10.3**
- **New 10.1** (this plan) -- efficient shape primitives

`documentation/IMPLEMENTATION.md` already reflects this renumbering and
carries the new Step 10.1's own task list; this file elaborates it.

## Goal

Design (this pass) and, in a future pass, build a retained-mode shape
primitive layer -- `Rectangle`, `Circle`/`Ellipse`, `Polygon`/`Star`,
`Path`, sharing common layout properties -- efficient enough for an
external UI framework to create once and mutate across many frames,
translating into the engine's existing, already-proven immediate-mode
IR/radix-sort/batch/RHI pipeline (Phases 5, 6, 9) rather than building
a second rendering path. **Updated 2026-09-09:** Python, the project's
own primary UI framework, reaches this layer via Phase 10 Step 10.3's
direct PyO3 binding (a `tre-python` crate depending on `tre-engine`
directly), not Step 10.2's `tre-ffi` C-ABI -- confirmed with the project
owner for real performance reasons; `tre-ffi` remains the real,
necessary boundary for every other language. See DESIGN.md Section
2.7's "Two Real Paths" for the full account.

## Real investigation done before writing any struct

Before designing anything, checked what this codebase already has for
each concept the user's own spec named, rather than guessing or
duplicating:

- **2D points:** no `Vec2` struct exists anywhere -- every real call site
  (`UiVertex::position`, `tre_math::lerp_points_batch`) uses plain
  `[f32; 2]`. Decision: `Vec2` is a *type alias* for `[f32; 2]`, not a
  new nominal struct (see ARCHITECTURE.md Section 7.1's own reasoning).
- **2D transforms:** `tre_math::Affine2` already exists and is the
  pipeline's own canonical, composable transform (`CanvasState::
  transform`'s own field type). Decision: `Transform2D` is a *separate*,
  decomposed (position/scale/rotation) representation for animation
  ergonomics, converted to a real `Affine2` once per frame during
  flattening via `Affine2`'s own existing `compose`/`from_translation`/
  `from_rotation`/`from_scale` methods -- no new matrix math.
- **Color:** already a packed `u32` everywhere (`UiVertex::color`,
  `rgba8`). Decision: `Color` is a type alias for `u32`, not a new
  struct.
- **Clip rects:** `ScissorRect` already exists and is exactly what
  `clip_bounds` needs. Reused directly, not reinvented as a new `Rect`.
- **Corner radius:** `RenderingCanvas::draw_rounded_rect` (Phase 3 Step
  3.2) supports only one *uniform* `radius: f32` -- its own doc comment
  already named per-corner radii as "a real, separate technique deferred
  until DESIGN.md's `CornerRadii`-taking `Canvas` API exists to need
  them." This step's own `Rectangle::corner_radius: CornerRadii` is
  exactly that named future API's data-model half; the shader work is
  not.
- **Blend modes:** `LayerDesc`'s own doc comment (Phase 6 Step 6.4.1)
  already named blend mode as belonging to "a later phase [that]
  implements those visual filters." No blend mode beyond the default
  premultiplied-alpha "over" (ARCHITECTURE.md Section 6.1) exists
  anywhere in the renderer today.
- **Gradients:** DESIGN.md's own architecture diagram already names a
  "Dynamic Gradient & Pattern Fill Evaluator" as a future box; zero
  evaluator code exists anywhere in the codebase (confirmed via grep).
- **Path stroking:** `tre-svg`'s existing tessellator (Phase 3 Step
  3.3.1-3.3.3) only *fills* paths (ear-clipping + stencil-and-cover
  fallback) -- no shape anywhere in this engine strokes an outline
  (`border_color`/`border_thickness`) separately from its fill.
- **Slot-table reuse:** `tre_memory::SwmrSlotTable` (Step 4.3.1) and
  `ScatterArena` (Step 5.2.2) are both real, proven generational/lock-
  free primitives, but neither fits storing a large, in-place-mutable
  `ShapeSlot`: `SwmrSlotTable`'s value is a bare `u64`, and
  `ScatterArena` is write-once-then-consumed per frame. A new, small,
  hand-built `ShapeRegistry` (generational slot arena) is needed --
  matching this project's own established "hand-build the primitive"
  precedent rather than reaching for the `slotmap` crate.

## Scope decisions

1. **Data model and retained-store architecture now; new GPU rendering
   work later, explicitly itemized, not silently assumed done.** This
   step's real task list (below) is buildable against *existing*
   rendering capability for the common cases (uniform-radius rectangles,
   filled paths) plus the full registry/flattening machinery. Every
   field the user's own spec named that has no existing rendering
   support -- non-uniform corner radius, corner smoothing, ellipse/arc
   sweep, polygon/star geometry, vertex rounding, path stroking, borders
   on any shape, gradients, non-`Normal` blend modes -- is itemized in
   ARCHITECTURE.md Section 7.5's own "Implementation status" note and
   IMPLEMENTATION.md's "Explicitly out of scope" list, not silently
   presented as already working. This directly avoids repeating this
   project's own past mistake (REVIEW.md finding #154: a "canonical"
   ARCHITECTURE.md/TECHNICAL.md section describing a radix sort that was
   never actually built).
2. **`enum ShapePrimitive` dispatch, not `Box<dyn Primitive>`.**
   TECHNICAL.md Section 9.1 bans dynamic type inspection in hot paths;
   the per-frame flattening pass is a hot path. A closed, four-variant
   enum costs one branch and zero heap allocation or vtable indirection
   per shape, matching `CommandType`/`UiDrawCommand`'s own existing
   design.
3. **Shapes compile down into the existing immediate-mode `Canvas` API,
   never a second rendering path.** The flattening pass calls
   `RenderingCanvas::draw_rounded_rect`/(future) `draw_path`/etc.
   exactly as any other caller would -- the retained layer's only job is
   deciding *when* to re-issue those calls (via `layout_dirty`/
   `active_animations`), not replacing the IR/sort/batch/RHI pipeline
   Phases 5, 6, and 9 already built and proved.
4. **A new, small `ShapeRegistry` primitive, hand-built.** Confirmed via
   real investigation (above) that neither existing `tre-memory`
   primitive fits; building a small generational slot arena is
   consistent with this project's own precedent, not scope creep.
5. **Lives inside `tre-engine` as a new module, not a new crate.**
   Both real future consumers -- Step 10.2's `tre-ffi` (for non-Python
   languages) and Step 10.3's `tre-python` (Python's own direct PyO3
   binding, updated 2026-09-09 to depend on `tre-engine` directly rather
   than routing through `tre-ffi`) -- depend on `tre-engine` directly,
   so nothing outside `tre-engine` itself needs this as a separate crate
   yet, matching Step 9.1's own "a primitive earns promotion to a shared
   crate once a second real caller needs it, not speculatively"
   precedent.
6. **`Vec2`/`Color` are type aliases, not new nominal structs;
   `CornerRadii` is a genuinely new nominal type.** Reuses this
   codebase's own existing `[f32; 2]`/`u32` conventions everywhere an
   equivalent already exists; only introduces a new type where the
   concept itself (four ordered corner radii) has no existing
   equivalent.

## Tasks (for the future implementation pass)

1. `crates/tre-engine/src/shapes.rs` (new module): `PrimitiveCommon`,
   `Transform2D` (+ `to_affine2`), `Vec2`/`Color` type aliases,
   `BlendMode`, `Visibility`, the `Primitive` accessor trait.
2. Same module: `FillStyle`, `GradientId`, `CornerRadii` (+
   `CornerRadii::uniform`).
3. Same module: `Rectangle`, `Circle`, `Polygon`, `PathCommand`,
   `LineCap`, `LineJoin`, `Path`, `ShapePrimitive` enum.
4. Same module: `ShapeId`, `AnimationId`, `ShapeSlot`, `ShapeRegistry`
   (insert/remove/get/get_mut, generational stale-handle rejection --
   real unit tests for insert/remove/reuse/stale-generation-rejected,
   matching this project's own `SwmrSlotTable`/`ScatterArena` test
   discipline).
5. The per-frame flattening pass: walk dirty/animating `ShapeSlot`s,
   resolve `Transform2D` -> real `Affine2`, call the existing
   `RenderingCanvas::draw_rounded_rect` for `Rectangle` shapes using
   `CornerRadii::uniform` (the one case with real rendering support
   today); a real demo proving a `ShapeRegistry`-driven rectangle
   renders identically to the same shape drawn directly via today's
   immediate-mode API (a batching-equivalence-style pixel-diff test,
   matching Phase 9 Step 9.1's own established verification pattern).
6. Documentation: mark ARCHITECTURE.md Section 7 / IMPLEMENTATION.md
   Step 10.1 "Status: Complete" once real, and record whatever real
   discrepancies the implementation pass finds (this project's
   established discipline every prior step has followed).

## Verification plan (for the future implementation pass)

- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean.
- `ShapeRegistry` unit tests: insert/get round-trips; remove then
  reuse the freed slot; a stale `ShapeId` (referencing a removed,
  reused slot) is rejected, not silently resolved to the wrong shape.
- The flattening pass's own real GPU demo: a `Rectangle` created via
  `ShapeRegistry`, animated across several frames (position/opacity),
  pixel-matches the identical shape drawn directly via
  `draw_rounded_rect` every frame -- proves the retained layer is a
  pure convenience wrapper, not a second, divergent rendering path.
- Re-run under Phase 9 Step 9.2's own real `RenderTickGuard` -- shape
  creation/mutation/removal and the flattening pass itself must add no
  new steady-state allocation.
- Full manual regression sweep of all pre-existing Vulkan demos (this
  step touches no existing rendering code paths, so zero regressions
  are expected, but this project's own standing discipline confirms
  that rather than assuming it).

## Explicitly out of scope (this planning pass, and the future
## implementation pass both)

- All new GPU/tessellation rendering work itemized in ARCHITECTURE.md
  Section 7.5's "Implementation status" note -- non-uniform corner
  radius, corner smoothing, ellipse/arc rendering, polygon/star
  geometry generation, path stroking, borders on any shape, gradient
  evaluation, non-`Normal` blend modes. Each is real, separate,
  disclosed future work, not silently assumed complete.
- The `tre-ffi` C-ABI wrapper around `ShapeId` for non-Python languages
  (Step 10.2's own job) and the direct PyO3 `#[pyclass]` wrapping it
  for Python (Step 10.3's own job, updated 2026-09-09 to bind directly
  to `tre-engine` rather than through `tre-ffi`) -- this step defines
  the Rust-side data model and registry only.
- Hit-testing logic itself (`hit_testable: bool` is stored, but no
  actual point-in-shape test is built this step).
- An animation-timeline/tweening system -- `active_animations`/
  `AnimationId` are storage/signaling only; owning and stepping real
  animations is DESIGN.md Section 12.4's already-established separate
  concern.
