# Plan: Phase 5, Step 5.3.1 -- `Canvas::tag_accessibility_node`

## Scope decisions

**Step 5.3 ("Spatial Accessibility Tagging") is split into three
sub-steps**, mapping directly onto IMPLEMENTATION.md's own three-task
list -- the same pattern that closed Steps 3.3, 4.2, 4.3, 5.1, and 5.2:

- **5.3.1 (this plan):** `Canvas::tag_accessibility_node(node_id,
  bounds, role)` -- the recording API and its IR-adjacent data,
  correctly threaded through Step 5.2's `SubCanvas`/`FrameArena`
  machinery. Nothing about a "parallel metadata extractor" or an OS
  accessibility bridge is meaningfully buildable or testable without
  real tagged spatial data existing first -- the same "state stack
  first" reasoning that ordered 5.1's own three sub-steps.
- **5.3.2 (next):** the real Linux AT-SPI2 bridge (a new crate,
  publishing a minimal, real accessible tree over D-Bus) and the
  concurrent extractor that walks a flattened frame's tagged nodes and
  publishes them without blocking GPU submission -- TECHNICAL.md
  Section 2.3's own platform split (Windows UIA / macOS NSAccessibility
  explicitly deferred, matching every prior OS-integration step's own
  "Linux complete; Windows/macOS deferred" precedent: Step 1.1's
  windowing, Step 1.2's input, Step 4.1's font discovery).
- **5.3.3 (last):** the capstone -- a real demo tagging a small scene,
  publishing it, and verifying via a real AT-SPI2/D-Bus query that the
  reported bounds match what was actually rendered, closing the loop
  DESIGN.md Section 5.2 promises ("100% alignment between what is
  visually rendered on screen and what is reported to assistive
  technology").

**`bounds` is provided in local space (matching every other drawing
primitive's own convention) and transformed -- as a real axis-aligned
bounding box of all four transformed corners, not just an offset
top-left.** `draw_rounded_rect` already established the pattern of
transforming individual corners rather than assuming a simple
translation, precisely because the active transform can rotate; a
tagged node's stored world-space bounds must stay a genuine
axis-aligned rect an OS accessibility API can consume directly (AT-SPI2's
`Component::GetExtents`, UIA's `BoundingRectangle`), so this sub-step
computes the min/max of the four transformed corners rather than
naively transforming just one point and reusing the local width/height
-- correct under translation, scale, *and* rotation, not just the
common case.

**Bounds are stored as `f32`, not immediately rounded to the integer
rects OS accessibility APIs ultimately want.** Rounding to whatever
integer convention a specific OS bridge needs is that bridge's own
concern (5.3.2), not something to commit to before any real bridge
exists to consume it.

**No clip-stack intersection this sub-step -- transform only, matching
`draw_rounded_rect`'s own established scope.** DESIGN.md's "100%
alignment" claim would, in full, mean a tagged node's reported bounds
should be clipped to whatever's actually visible on screen (a node
partially scrolled out of view). Intersecting against the active clip
stack is a real, separate enhancement with no specified behavior to
build against yet; this sub-step reports full transformed bounds
regardless of clipping, disclosed here rather than silently assumed
away.

**`role` is a small, concrete starter enum, not an attempt at AT-SPI2's
full ~130-role taxonomy or literal bitflags.** DESIGN.md's own
`role_flags` parameter name is the only hint given, and no consumer
exists yet to demand richness beyond "what kind of element is this" --
a plain `AccessibilityRole` enum (`Generic`, `Button`, `TextLabel`,
`Image`) starts genuinely useful and stays trivially extensible once
5.3.2's real OS bridge reveals which additional roles it actually
needs, the same "adapt the doc sketch to what's concretely buildable"
precedent Step 5.1.2 already established for `DynamicTextLayout`/
`Paint`.

**Tagged nodes are a flat per-frame list, not a tree the engine
builds.** DESIGN.md's own signature carries no parent-node parameter --
the UI framework already owns the real widget tree and its hierarchy;
the engine's only job is reporting each tagged node's *rendered*
spatial position back, keyed by whatever `node_id` the framework
itself assigns and already tracks. Building or validating tree
structure inside `tre-engine` would be scope well beyond what this
signature asks for.

**Correctly threading tagged nodes through `SubCanvas`/`stitch_into` is
required in this sub-step, not deferred.** `SubCanvas` already
`Deref`/`DerefMut`s to every `RenderingCanvas` method for free
(Step 5.2.1) -- including, automatically, whatever `tag_accessibility_
node` this sub-step adds. If `stitch_into` didn't also carry a
sub-canvas's own tagged nodes into the shared `FrameArena`, that data
would be silently dropped the instant a real caller tagged a node from
a worker thread, a real, easily-triggered correctness bug this sub-step
must not introduce even by omission. Accessibility nodes need no
rebasing at all when merged (unlike indices/`vertex_offset`, nothing
about an `AccessibilityNode` references a position in another array),
which keeps this integration simpler than the vertex/index/command
case Step 5.2.2 solved.

## Goal

`Canvas::tag_accessibility_node(&mut self, node_id: AccessibilityNodeId,
x: f32, y: f32, width: f32, height: f32, role: AccessibilityRole)`
records one tagged node's real, transform-correct world-space bounds
into a flat per-canvas list; `RenderingCanvas::flatten`/`FrameArena::
flatten` both surface the accumulated nodes via a new `FlattenedFrame::
accessibility_nodes` field; a `SubCanvas` on a worker thread tagging a
node and later calling `stitch_into` carries that data into the shared
`FrameArena` correctly, proven by both a rotation-correctness unit test
(a rotated tagged rect's stored bounds are the real axis-aligned
bounding box of its four rotated corners, not the naive untransformed
rectangle) and a real multi-thread test extending Step 5.2.3's own
capstone scene.

## Tasks

1. **`AccessibilityNodeId(pub u64)`** and **`AccessibilityRole`**
   (`Generic`, `Button`, `TextLabel`, `Image` to start) -- new, small
   public types in `tre-engine`.

2. **`AccessibilityNode`**: `{ node_id: AccessibilityNodeId, x: f32, y:
   f32, width: f32, height: f32, role: AccessibilityRole }` -- the
   stored, already-world-space record.

3. **`RenderingCanvas::tag_accessibility_node(&mut self, node_id,
   x, y, width, height, role)`**: transforms all four local corners via
   the active `Affine2` (same `state.transform.transform_point` call
   `draw_rounded_rect` already uses), computes their axis-aligned
   min/max, and appends the resulting `AccessibilityNode` to a new
   `accessibility_nodes: Vec<AccessibilityNode>` field.

4. **`FlattenedFrame` gains `accessibility_nodes: Vec<AccessibilityNode>`**;
   `RenderingCanvas::flatten` and the shared `segment_and_flatten`
   free function are updated to thread it through unchanged (a flat
   move, no sort/merge logic needed -- accessibility nodes never
   interact with `flatten_run`'s marker-segmentation or batch-merging
   at all).

5. **`FrameArena` gains a fourth `tre_memory::ScatterArena
   <AccessibilityNode>`** (`AccessibilityNode` is `Copy`, fitting the
   existing primitive with no changes to `tre-memory` itself);
   `RenderingCanvas::stitch_into`/`SubCanvas::stitch_into` reserve and
   copy a source's own tagged nodes into it (a literal bulk copy, no
   rebasing needed, simpler than the vertex/index/command case);
   `FrameArena::flatten` includes the merged result in the
   `FlattenedFrame` it produces.

6. **Unit tests**: a single tagged node's local bounds transform
   correctly under pure translation (a simple, hand-computed sanity
   check); a tagged node under a *rotating* transform produces the
   real axis-aligned bounding box of its four rotated corners, not the
   naive untransformed rect (the one genuinely tricky correctness case
   this sub-step exists to get right); tagging from a `SubCanvas` and
   calling `stitch_into` carries the node into a `FrameArena` correctly;
   a real multi-thread test (extending Step 5.2.3's own capstone
   scene) in which multiple worker threads each tag a node alongside
   their rect, confirming every tagged node survives concurrent
   stitching with its correct bounds regardless of thread scheduling.

7. **Docs**: IMPLEMENTATION.md Step 5.3.1 subsection; REVIEW.md entry.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- No demo this sub-step -- matching Steps 4.3.1/4.3.2/5.2.1's own
  precedent: tagged data has nowhere real to go yet (no OS bridge
  exists until 5.3.2), so the real end-to-end proof is deferred to the
  5.3.3 capstone.
- All pre-existing examples re-run manually as a regression check,
  since `FlattenedFrame`'s new field and the `flatten()`/`stitch_into`
  changes touch a shared code path every example goes through.

## Explicitly out of scope for this sub-step

- Any OS accessibility bridge (AT-SPI2, UIA, NSAccessibility) -- Step
  5.3.2.
- The "parallel metadata extractor" running alongside RHI submission --
  also 5.3.2, since it has nothing real to extract *to* yet.
- Clip-stack intersection of reported bounds -- transform-only, a
  disclosed, deliberate simplification (see Scope decisions).
- Any parent/child tree structure built inside the engine -- tagged
  nodes stay a flat per-frame list; the UI framework owns the real
  hierarchy.
- A rich, AT-SPI2-taxonomy-complete `AccessibilityRole` -- starts with
  4 concrete variants, extensible once a real OS bridge (5.3.2) reveals
  what it actually needs.
- Any change to `tre-rhi-vulkan`'s pipeline/shader code, or to
  `tre-memory` itself (reuses `ScatterArena` exactly as it already
  exists).
