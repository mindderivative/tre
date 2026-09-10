# Plan: Phase 10 Step 10.2.6 — Zero-Allocation Live Verification for the Shape System

**Status: Complete (2026-09-09).** This was the sixth and final step in
the Step 10.2.1–10.2.6 roadmap -- see `documentation/IMPLEMENTATION.md`'s
own "Step 10.2.6: Zero-Allocation Live Verification" write-up for the
full technical account of what shipped, `documentation/REVIEW.md`
finding #176 (two real, previously-undetected per-frame allocations
found and fixed, plus one real, deeper gap disclosed and deliberately
not fixed), and `demo/phase10_step10_2_6/README.md` for the
verification summary. This file is the original plan, archived
unchanged below from the combined `PLAN.md` roadmap once this
sub-step's own real work began. With this step's completion, all of
Steps 10.2.1–10.2.6 -- and with them, every gap Step 10.2's own original
implementation disclosed -- are closed.

**What actually shipped, in one paragraph.** Built exactly as planned:
`shape_registry_zero_alloc_demo.rs`, modeled on `main_loop_demo.rs`'s
own warm-up-then-guard pattern, wrapping a real, mutating, mixed
`ShapeRegistry` scene (covering gradient/texture/blend-mode/exact-
ellipse/rounded-cap fill and border features from every one of the
other five sub-steps) in `RenderTickGuard` across 120 frames. Being the
very first real GPU render to check `ShapeRegistry::flatten_into`'s own
`Polygon`/texture-fill code paths under actual allocation pressure, it
immediately found two real, previously-undetected per-frame
allocations (`generate_polygon_points`/`fan_from_center`, `bounding_
box_uvs`) -- both fixed via reuse-friendly `_into` siblings and new
`ShapeRegistry`-owned scratch buffers, exactly the kind of "real
verification finds a real bug" outcome this project's own standing
discipline expects and welcomes, not something to be surprised by. A
third, deeper, `lyon`-tessellation-related allocation source was also
found and deliberately NOT fixed this pass -- disclosed honestly,
matching `main_loop_demo.rs`'s own precedent for its own two disclosed
exclusions, rather than silently expanding this step's own scope to
chase it.

---

## Original plan, as written

### Investigation

- `tre_memory::RenderTickGuard`/`DebugAllocGuard` (Step 9.2) already prove
  `main_loop_demo`'s own per-frame span is genuinely zero-allocation, after
  a documented warm-up frame. `ShapeRegistry`/`RenderingCanvas::flatten_
  into` reuse the exact same already-proven `reset()`/`draw_*`/
  `flatten_into` machinery Step 9.2 verified — but no demo has ever wrapped
  a shape-registry-driven scene in the guard itself (ARCHITECTURE.md
  Section 7.5's own disclosed gap).

**Confirmed exactly as researched** -- the guard's own reuse machinery
worked as designed once threaded through `ShapeRegistry`'s own new
scratch buffers; the real surprise was in code this investigation never
called out by name (`generate_polygon_points`/`fan_from_center`/
`bounding_box_uvs`), not in the guard mechanism itself.

### Scope decisions

- New demo (not a retrofit of `shape_full_rendering_demo`, to keep that
  demo's own existing, stable pixel-correctness assertions untouched):
  builds a representative mixed scene — `Rectangle`/`Circle`/`Polygon`/
  `Path`, with borders, and (since this step runs last) gradients,
  textures, and non-`Normal` blend modes too, so every new code path from
  10.2.1–10.2.5 is covered by the same zero-allocation proof, not just the
  original Step 10.2 surface.
- Proves the REAL, dynamic-update case, not a static strawman: mutates
  shape properties (position, color, gradient stops) across repeated
  frames inside the guard, matching this project's own standing discipline
  of testing what real UI usage actually does (continuous updates), not
  just a frame replayed unchanged.

**Implemented differently, first bullet only:** a real `Path` shape
(and a bordered `Polygon`) were deliberately left OUT of the final
scene once the `lyon`-tessellation allocation gap was found -- including
one would have made every guarded frame panic on a real, but
substantially-larger-to-fix, allocation this step's own bounded scope
does not redesign (REVIEW.md finding #176 has the full account). The
non-`Normal` `BlendMode` feature is still fully covered, via a
borderless `Polygon` instead of a `Path` -- both route through the exact
same shared `draw_polygon_fill` dispatch, so no real feature coverage
is lost by this substitution. **Second bullet: implemented exactly as
planned**, plus a new `ShapeRegistry::gradient_mut` method (not written
here originally) needed to make "mutates... gradient stops" possible at
all without an unbounded per-frame `Vec` growth in `self.gradients`.

### Tasks

1. New `shape_registry_zero_alloc_demo.rs`, modeled on `main_loop_demo.rs`'s
   own established warm-up-then-guard pattern.
2. A representative, mutating mixed scene exercising every shape kind and
   (once landed) every new fill/blend feature.
3. `RenderTickGuard`-wrapped `flatten_into`/`canvas.flatten()` across many
   repeated frames; a real, hard assertion of zero allocations post-warm-up.
4. Update ARCHITECTURE.md Section 7.5 to close the disclosed gap once
   proven, matching `main_loop_demo`'s own precedent exactly.

**All four tasks completed as written**, task 2's own scene narrowed as
described above, and task 3's own "hard assertion" took the form of the
guard's own real panic-on-violation behavior (matching `main_loop_
demo.rs`'s own precedent exactly: the demo simply completing 120 frames
without panicking IS the zero-allocation proof, not a separate counted
assertion) -- which is exactly what caught, and forced a real fix for,
the two allocations described above before this step could honestly be
called done.
