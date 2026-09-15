# Demo: Phase 5, Step 5.1.3 -- Real Sort Key, Overlay Routing, Real Batch Flattening

```bash
./demo/phase5_step5_1_3/run_canvas_batch_flattening_demo.sh
```

The capstone of Step 5.1: reproduces DESIGN.md Section 8's own worked
example exactly --

```
Submit order: Rect1(P1,Tex0) -> Text(P2,AtlasA) -> Rect2(P1,Tex0) -> OverlayRect(P1,Tex0)
Real flattening: Batch 0 = Rect1+Rect2 merged | Batch 1 = Text | Batch 2 = OverlayRect
```

`Rect1` and `Rect2` share Layer 0, the SDF-rect pipeline, and texture 0
-- real batch flattening (`Canvas::flatten`) merges them into a single
`DrawGeometry` command even though `Text` was recorded in between them.
`OverlayRect` shares that same pipeline and texture, but `begin_overlay`
routes it to Layer ID 10000+, so it stays its own, separate batch.

Both the IR-level command count (exactly 3, asserted before any GPU
work happens) and a real GPU render are checked -- this is the first
demo in this codebase to record more than one `draw_indexed` call and
switch pipelines within a single frame, proving the merged batches
still render every one of the 4 logical shapes (two plain rects, one
overlay rect, one real MSDF text glyph) at its own correct, distinct
position and color, not an overdrawn or corrupted blob.
