# Demo: Phase 10, Step 10.1 -- Efficient Shape Primitives

```bash
./demo/phase10_step10_1/run_shape_registry_demo.sh
```

**A real retained-mode layer, proven equivalent to the existing
immediate-mode API, not a second rendering path.** `tre-engine`'s new
`shapes` module (`ShapeRegistry`, `ShapeId`, `Rectangle`/`Circle`/
`Polygon`/`Path`, `PrimitiveCommon`, `Transform2D`, and friends --
ARCHITECTURE.md Section 7) lets an external UI framework create a shape
once, hold a stable handle to it, and mutate a handful of properties
across many frames instead of re-issuing every draw call every frame.
This demo is the real proof that layer is faithful: it draws the
identical rounded rectangle twice -- once via today's real immediate-
mode `RenderingCanvas::draw_rounded_rect`, once via
`ShapeRegistry::insert` + `ShapeRegistry::flatten_into` -- renders both
through the real, unmodified rendering pipeline, and asserts the two
framebuffers are byte-for-byte identical.

**What's real today.** `PrimitiveCommon`/`Transform2D` (resolved to a
real `tre_math::Affine2` once per frame via `Affine2::compose`), the
hand-built generational `ShapeRegistry` (insert/remove/get/get_mut,
stale-handle rejection), and a `Rectangle` with a *uniform* corner
radius, `FillStyle::Solid` fill, no border, and no corner smoothing --
exactly what today's real `draw_rounded_rect` shader supports.

**What's disclosed, not silently missing.** `Circle`/`Polygon`/`Path`,
non-uniform corner radii, squircle `corner_smoothing`, border rendering
on any shape, and `FillStyle::Gradient`/non-`Normal` `BlendMode` are all
real fields in the data model (matching the full property spec this
step was designed against) but have no rendering support anywhere in
this engine yet -- `ShapeRegistry::flatten_into` panics loudly
(`unimplemented!`) on any of them rather than silently skipping or
rendering something wrong. See ARCHITECTURE.md Section 7.5's own
"Implementation status" note and `tre-engine/src/shapes.rs`'s own module
doc comment for the full, itemized list.

**Verified.** 12 new `tre-engine` unit tests (insert/remove/reuse/stale-
handle rejection, `Transform2D::to_affine2`'s own composition order,
dirty/animating flattening semantics, `Hidden`/`Collapsed` visibility,
and panics on every not-yet-supported field combination). This demo ran
3 times against real GPU hardware, byte-for-byte identical every time.
Full workspace `fmt`/`clippy`/`build`/`test` clean, and a full manual
regression sweep of every pre-existing Vulkan demo confirms zero
regressions -- this step is purely additive (a new module, no existing
public API signature changed).
