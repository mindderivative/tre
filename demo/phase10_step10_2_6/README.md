# Demo: Phase 10 Step 10.2.6 -- Zero-Allocation Live Verification

```bash
./demo/phase10_step10_2_6/run_shape_registry_zero_alloc_demo.sh
```

**What this closes.** The sixth and final gap `PLAN.md` scheduled after
Step 10.2's own follow-ups, and ARCHITECTURE.md Section 7.5's own
disclosed gap: `tre_memory::RenderTickGuard`/`DebugAllocGuard` (Step
9.2) already proved `main_loop_demo`'s own hand-drawn scene is
genuinely zero-allocation, but no demo had ever wrapped a
`ShapeRegistry::flatten_into`-driven scene in that same real,
self-checking guard, even though it reuses the exact same already-
proven `reset()`/`flatten_into` machinery underneath.

**What's real now.** Modeled directly on `main_loop_demo.rs`'s own
established warm-up-then-guard pattern (a single-threaded, headless
simplification -- this step is about `ShapeRegistry`/`RenderingCanvas`
specifically, not the full 8-stage pipeline Steps 8.1.2/9.2 already
proved): one `root: RenderingCanvas` stitches into one persistent
`FrameArena`, exactly like `main_loop_demo`'s own root canvas does
alongside its worker canvases. 120 frames of a real, mutating, MIXED
scene render with zero heap allocations after a single, expected
warm-up pass:

- A solid `Rectangle`.
- A gradient-filled, non-circular `Circle` (the exact ellipse SDF, Step
  10.2.4).
- A texture-filled `Polygon`.
- A solid-fill `Polygon` under a non-`Normal` `BlendMode` (Step
  10.2.3's real `VK_KHR_dynamic_rendering_local_read` path).
- A bordered, partial-arc `Circle` (Step 10.2.5's rounded stroke caps).

Real, continuously-updating mutation, not a frame replayed unchanged:
position and solid-fill color update every frame via `ShapeRegistry::
get_mut`, and a gradient's own stop colors update via a new
`ShapeRegistry::gradient_mut` method (added for this step -- no prior
API let a caller update an already-registered gradient's stops without
registering an unbounded, ever-growing new one every frame, since
`GradientId`'s own table has no generational reuse).

**Two real, previously-undetected allocations found and fixed along the
way** (REVIEW.md finding #176) -- this step's own guard is the first to
ever check these code paths under real allocation pressure:

- `generate_polygon_points`/`fan_from_center` each returned a freshly
  heap-allocated `Vec` on every `Polygon` flatten. Fixed via new `_into`
  siblings writing into `ShapeRegistry`'s own persistent scratch
  buffers.
- `bounding_box_uvs` (Step 10.2.2's own texture-UV helper, shared by
  `Polygon` and `Path`) did the same. Fixed the same way.

**One real, deeper gap disclosed, not hidden.** `lyon`-backed
tessellation (`tessellate_fill`/`tessellate_stroke`, used by any
`Path`'s own fill/stroke and any BORDERED `Polygon`) still constructs
fresh tessellator/path/`VertexBuffers` objects on every call -- a
substantially larger reuse redesign than this step's own two fixes,
the same category of deferred gap `main_loop_demo.rs`'s own Step 9.2
already disclosed for RHI submission and `std::thread::scope`. This
demo's own scene deliberately uses only borderless shapes and no `Path`
so it never exercises that gap; a bordered `Polygon`/`Path` shape is
still real, correct, and unaffected by this step -- just not yet
zero-allocation.

![shape registry zero alloc output](shape_registry_zero_alloc_output.png)
