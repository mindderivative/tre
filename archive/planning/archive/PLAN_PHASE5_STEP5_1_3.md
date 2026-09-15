# Plan: Phase 5, Step 5.1.3 -- Real Sort Key, Overlay Routing, Real Batch Flattening

## Scope decisions

**Third and closing sub-step of Step 5.1** -- the capstone, per 5.1.1's own
plan: it needs multiple real draw kinds/pipelines coexisting (5.1.1's
`draw_rounded_rect`, 5.1.2's `draw_text`) to be meaningfully provable,
the same reason Step 4.2's own concurrency-wiring capstone came last.
Three genuinely separable pieces, same 3-way task split IMPLEMENTATION.md's
own Step 5.1 task list already names:

- **The real 64-bit sort key** (ARCHITECTURE.md Section 4.1), replacing
  every `sort_key: 0` placeholder.
- **`begin_overlay`/`end_overlay`** (DESIGN.md Section 7.2, `Canvas::
  begin_overlay(OverlayLayerPriority)`), routing content into the
  Layer-ID-$\ge$10000 overlay plane and resetting the clip stack.
- **Real batch flattening** (ARCHITECTURE.md Section 4.2), replacing
  `flatten()`'s Phase 0 trivial pass-through with the documented
  sort-then-merge-adjacent algorithm.

**`OverlayLayerPriority` doesn't exist in code -- same class of gap
5.1.2 found with `DynamicTextLayout`/`Paint`.** DESIGN.md Section 6.2
names `Canvas::begin_overlay(OverlayLayerPriority)` but never defines
the type. This step defines it concretely and minimally: a plain
`pub struct OverlayLayerPriority(pub u16)`, added to a fixed
`OVERLAY_LAYER_BASE = 10_000` (ARCHITECTURE.md Section 4.1's own
documented base) to produce the actual 16-bit Layer ID. Each
`begin_overlay` call's priority is absolute, not composed with any
enclosing overlay's own priority -- nested-overlay-priority composition
is a real but unrequested refinement, left out of scope.

**Two completely different meanings of "layer" already coexist in this
codebase, and this step must not conflate them.** The sort key's
"Layer ID" (ARCHITECTURE.md Section 4.1: standard content 0-9999 vs.
overlay/popup 10000+) is a pure *paint-order/sort* concept with no
render-target implications. `Canvas::push_layer`/`pop_layer`/
`LayerDesc` (DESIGN.md Section 5, already real since Phase 0) is a
completely separate *offscreen compositing* concept (an actual GPU
render-target redirect, still deferred -- nothing consumes it yet).
`begin_overlay` only ever touches the former; it has no interaction
with `push_layer`/`pop_layer` at all.

**Depth ID is a single global, monotonically increasing per-frame
counter, not a per-widget traversal index -- there is no widget tree
or z-index concept anywhere in this imperative `Canvas` API.**
ARCHITECTURE.md Section 4.1 describes Depth ID as "the original
depth-first traversal index (post z-index resolution)," but no calling
UI framework or z-index resolver exists yet to produce one; `Canvas`
itself is a flat, imperative recorder. The natural, honest mapping for
*this* layer of the stack is: Depth ID = the order in which
`draw_rounded_rect`/`draw_text` calls are actually made, i.e. a
`next_depth_id: u32` field incremented once per `DrawGeometry` command.
Z-index-aware reordering of that counter is a UI-framework-level
concern for a future phase, not this one.

**Real batch flattening treats every non-`DrawGeometry` command
(`PushScissor`/`PopScissor`/`PushLayer`/`PopLayer`) as a hard barrier --
sorting/merging never reorders a draw across one.** ARCHITECTURE.md
Section 4.2 explicitly frames "single draw call per layer plane" as a
*soft* target, not a structural guarantee, and calls out clip-crossing
batching as a deliberately deferred, measurement-driven optimization
("a clip-bucketing secondary pass is the natural future fix"). Treating
scissor markers as barriers (in addition to the layer markers, which
obviously must be barriers -- reordering a draw across a `PushLayer`/
`PopLayer` pair would draw it into the wrong render target) is the
conservative, unambiguously-correct choice this step takes advantage
of: within one *marker-free run* of consecutive `DrawGeometry` commands,
every command already shares the same `clip_bounds` by construction
(nothing changed the clip stack within the run), so the "sort, then
merge adjacent commands sharing Layer+Pipeline+Texture *and* identical
`clip_bounds`" algorithm reduces to "sort within the run, merge
adjacent commands sharing Layer+Pipeline+Texture" -- exactly
ARCHITECTURE.md Section 4.2's documented three-step algorithm, applied
per run rather than across the whole frame. DESIGN.md Section 8's own
worked example (`Rect -> Text -> Rect -> Rect(Overlay)`, zero markers
between any of them) is the single-run case, and produces exactly the
3 batches the doc's own diagram shows.

**Merging rewrites `indices`, never `vertices`.** Every index value is
an absolute reference into the (unmoved) `vertices` array; merging two
commands means concatenating their two (possibly non-adjacent) index
slices into a freshly built, contiguous `indices` buffer and updating
the merged command's `vertex_offset`/`element_count` accordingly --
`UiDrawCommand::vertex_offset` is `RhiCommandBuffer::draw_indexed`'s
`start_index` parameter (confirmed via every existing demo's own
`draw_indexed(count, 0, 0)` call), so a merged command's indices must
actually be contiguous in the buffer the RHI reads from, not merely
"assumed contiguous."

**Since `next_depth_id` is strictly increasing and never reset, no two
`DrawGeometry` commands in one frame can ever share the same full
64-bit sort key.** This makes the per-run sort a true total order (no
ties), so `sort_unstable_by_key` is safe to use in place of a stable
sort -- there is nothing tied to preserve the order of.

**This step adds zero new external or cross-crate dependencies** --
unlike 5.1.2, everything here is internal `tre-engine` logic (bit
packing, a new stack, an index-rewriting pass).

## Goal

Every `DrawGeometry` command `draw_rounded_rect`/`draw_text` emit
carries a real 64-bit sort key (Layer/Pipeline/Texture/Depth, per
ARCHITECTURE.md Section 4.1); `Canvas::begin_overlay`/`end_overlay`
route subsequent draws into the overlay plane (Layer ID $\ge$ 10000)
and reset the active clip to the full window, restoring both on
`end_overlay`; `flatten()` performs the real sort-and-merge algorithm
per marker-delimited run, verified against DESIGN.md Section 8's own
worked example (`Rect1 -> Text -> Rect2 -> OverlayRect` collapsing to
exactly 3 batches) both at the IR level (unit tests) and with a real
GPU render proving the merged batches still draw every shape at its own
correct, distinct position.

## Tasks

1. **`OverlayLayerPriority(pub u16)`** and `const OVERLAY_LAYER_BASE:
   u16 = 10_000`.

2. **New `RenderingCanvas` fields**: `next_depth_id: u32` (starts `0`,
   incremented once per `DrawGeometry` command, never reset);
   `overlay_stack: Vec<u16>` (empty = standard content, Layer ID `0`;
   `.last()` = the active overlay Layer ID); `saved_clip_stacks:
   Vec<Vec<ScissorRect>>` (one entry pushed per `begin_overlay`, holding
   the clip stack that was active before the reset, for `end_overlay`
   to restore).

3. **`Canvas::begin_overlay(&mut self, priority: OverlayLayerPriority)`**:
   `layer_id = OVERLAY_LAYER_BASE.checked_add(priority.0).expect(...)`,
   pushed onto `overlay_stack`; `std::mem::take(&mut self.clip_stack)`
   saved onto `saved_clip_stacks` (leaving `clip_stack` genuinely empty
   -- the existing "no clip, full window" sentinel meaning, not a
   literal pushed rect); emits a real `PushScissor` command with the
   shared `FULL_WINDOW_CLIP` const (task 4) as `clip_bounds`, matching
   `push_clip`'s own established "every clip-stack change is a real,
   auditable IR event" precedent.
   **`Canvas::end_overlay(&mut self)`**: pops both stacks (panicking on
   an unmatched call, same precedent as `restore()`/`pop_clip()`/
   `pop_layer()`), restores `clip_stack`, emits a real `PopScissor`.

4. **Extract the full-window sentinel** (`ScissorRect { x: 0, y: 0,
   width: u32::MAX, height: u32::MAX }`, currently duplicated inline in
   `draw_rounded_rect` and `emit_glyph_quad`) into a shared
   `const FULL_WINDOW_CLIP: ScissorRect`, reused by both existing call
   sites plus `begin_overlay`'s new one.

5. **`compute_sort_key(layer_id: u16, pipeline_state_id: u16,
   texture_handle: u32, depth_id: u32) -> u64`**: packs
   `(layer_id << 48) | (pipeline_state_id << 32) | ((texture_handle &
   0xFFF) << 20) | (depth_id & 0xF_FFFF)` per ARCHITECTURE.md Section
   4.1's canonical layout, with a debug-mode assert on `texture_handle`
   exceeding the 12-bit Texture ID field and on `depth_id` exceeding the
   20-bit Depth ID field -- the latter is the exact debug assert
   ARCHITECTURE.md Section 4.1 already documents as required ("the
   Canvas asserts in debug builds... if a single frame's node count
   would still overflow 20 bits"); the release-mode "splits the
   offending layer's content into two sequential sub-frame passes"
   behavior the same paragraph describes is **not** implemented by this
   step (see Explicitly out of scope).

6. **Rewire `draw_rounded_rect` and `emit_glyph_quad`**: replace
   `sort_key: 0` with a real `compute_sort_key(self.active_layer_id(),
   pipeline_state_id, texture_handle, depth_id)`, where `depth_id =
   self.next_depth_id` is read then incremented
   (`checked_add(1).expect(...)`) once per call. `active_layer_id(&self)
   -> u16` is a small private helper: `self.overlay_stack.last().copied
   ().unwrap_or(0)`.

7. **Real `flatten()`**: extend the existing debug-balance assert to
   also check `overlay_stack.is_empty()` (an unmatched `begin_overlay`
   is a programmer error, same class as unbalanced `save`/`push_clip`/
   `push_layer`). Segment `self.commands` into maximal runs bounded by
   any non-`DrawGeometry` command (every marker passes through
   unchanged, in its exact original position); for each run:
   `sort_unstable_by_key(|c| c.sort_key)` (safe -- see scope decisions
   on why ties are impossible), then a linear sweep merging adjacent
   commands whose `sort_key >> 20` (the top 44 bits: Layer+Pipeline+
   Texture) and `clip_bounds` both match, concatenating each merged
   command's original index slice into a freshly built `indices` buffer
   and updating `vertex_offset`/`element_count` accordingly. `vertices`
   is returned unmoved (only `indices`/`commands` are rebuilt).

8. **Audit existing tests/demos for the new merging behavior.** Any two
   *adjacent* `DrawGeometry` commands (no intervening marker) that share
   Layer+Pipeline+Texture+`clip_bounds` now merge into one
   `UiDrawCommand` -- a real, intended behavior change from today's
   strict 1:1 call-to-command mapping. Tests asserting on *vertex* data
   are unaffected (vertices never move); a known, already-identified
   case that needs a real fix: `canvas_state_stack_demo.rs`'s Rect A and
   Rect B (both default pipeline/texture/layer, both drawn before any
   `push_clip`, so both share the same full-window `clip_bounds`) will
   merge into one command, shifting `RECT_C_COMMAND_INDEX` from `3` to
   `2` -- update the constant and its surrounding comment. Full audit
   during implementation, not enumerated exhaustively here.

9. **Unit tests**: `compute_sort_key`'s bit-packing (each field lands in
   its documented bit range, hand-computed expected `u64` values,
   matching this crate's own established style); `next_depth_id`
   increments monotonically across multiple draws and is never reset by
   `begin_overlay`/`push_clip`/`push_layer`; `begin_overlay`/
   `end_overlay` balance panics (unmatched call, same precedent as
   `restore`/`pop_clip`/`pop_layer`) and the `flatten()`-time debug
   assert; `begin_overlay` sets the active Layer ID to
   `10_000 + priority` and resets clip to full window, `end_overlay`
   restores the prior clip exactly; **the DESIGN.md Section 8 worked
   example reproduced exactly** (`draw_rounded_rect` "Rect1" ->
   `draw_text` "Text" -> `draw_rounded_rect` "Rect2" ->
   `begin_overlay`/`draw_rounded_rect` "OverlayRect"/`end_overlay`) --
   asserts exactly 3 real `DrawGeometry` commands survive `flatten()`
   (batch 0 = Rect1+Rect2 merged, `element_count == 12`; batch 1 = Text
   alone; batch 2 = OverlayRect alone, distinct Layer ID); two draws
   sharing Layer+Pipeline+Texture but under *different* `clip_bounds`
   must **not** merge; two draws sharing Layer+Pipeline+Texture+
   `clip_bounds` but separated by a `push_clip`/`pop_clip` pair that
   nets out to the *same* rect must still **not** merge (markers are
   hard barriers regardless of net clip effect -- proves the
   conservative-barrier design decision, not just the common case).

10. **New demo** (`crates/tre-rhi-vulkan/examples/canvas_batch_flattening_demo.rs`,
    `demo/phase5_step5_1_3/`): reproduces DESIGN.md Section 8's worked
    example with real geometry (two non-overlapping SDF rects sharing a
    pipeline/texture, a real atlas-backed text glyph reusing 5.1.2's
    established real-cascade-font/real-`AtlasOwner` pattern, and a
    `begin_overlay`-routed rect), rendered through the existing
    `sdf_rounded_rect`/`bindless_textured.vert`+`msdf.frag` pipelines
    (no new shader work) -- read back as real pixels confirming each of
    the 4 logical shapes still renders at its own correct, distinct
    position and color despite two of them sharing one merged draw
    call, plus an IR-level assertion that `flatten()` really did produce
    only 3 commands for 4 logical draws.

11. **Docs**: IMPLEMENTATION.md Step 5.1.3 subsection (closing Step 5.1
    in full across all three sub-steps); REVIEW.md entry; ARCHITECTURE.md
    Section 4.2 gets a short addendum noting this codebase's own
    conservative "every marker is a hard barrier" choice as the concrete
    resolution of the section's existing "soft target" caveat, if this
    reads as a new convention worth recording there (final call during
    implementation).

12. **CI**: add `canvas_batch_flattening_demo` to the `vulkan-validation`
    job's example list.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- `canvas_batch_flattening_demo` run under `VK_LAYER_KHRONOS_validation`,
  zero errors.
- Every existing example re-run manually (this step touches `flatten()`
  itself, the shared codepath every single example that draws anything
  goes through -- broader regression exposure than 5.1.1/5.1.2, which
  only touched specific call sites).
- `canvas_state_stack_demo` specifically re-verified pixel-correct after
  its `RECT_C_COMMAND_INDEX` fix.
- CI: push, confirm green via `gh run watch`.

## Explicitly out of scope for this sub-step

- Nested-overlay-priority composition (each `begin_overlay` call's
  priority stays absolute).
- The release-mode "split the offending layer's content into two
  sequential sub-frame passes" behavior ARCHITECTURE.md Section 4.1
  documents for a Depth ID overflow -- a debug-mode assert only.
- Cross-marker-boundary batching (the clip-crossing optimization
  ARCHITECTURE.md Section 4.2 itself already defers pending real
  profiling data).
- Any change to `push_layer`/`pop_layer`'s own deferred offscreen
  render-target redirect -- `begin_overlay` never touches it.
- Z-index resolution / widget-tree traversal ordering -- Depth ID stays
  pure call-order, since no such concept exists in this imperative
  `Canvas` API yet.
- Any change to `tre-rhi-vulkan`'s pipeline/shader code -- reuses
  `sdf_rounded_rect`/`bindless_textured.vert`/`msdf.frag` exactly as
  prior demos already do.
