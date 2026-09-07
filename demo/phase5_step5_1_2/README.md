# Demo: Phase 5, Step 5.1.2 -- Canvas Text Rendering (draw_text)

```bash
./demo/phase5_step5_1_2/run_canvas_draw_text_demo.sh
```

`tre-engine`'s first real wiring into `tre-text`/`tre-atlas`:
`Canvas::draw_text` shapes and renders a real word ("TEXT") against a
real cascade font, going through a real `AtlasOwner` background thread
end to end.

`draw_text` is called twice against the same atlas, proving both halves
of its documented cache contract:

- **Frame 1** (every glyph a cache miss): `draw_text` fires a real
  `request_insert` per distinct glyph and emits zero commands -- nothing
  renders yet, "report, don't block, render nothing this frame."
- **Frame 2** (every glyph now resolved, after polling the real
  background thread to completion): a fresh `draw_text` call against the
  same atlas handle emits one real textured `DrawGeometry` command per
  glyph, rendered through the existing, unmodified
  `bindless_textured.vert`/`msdf.frag` pipeline (Step 4.2.4) -- read back
  as real GPU pixels confirming the word actually rendered, scanning each
  glyph's own on-screen quad (not just its center) for non-background
  fill.

**Scope note.** Every glyph renders as a fixed `px_size`-square quad
(matching the MSDF atlas entry's own fixed square aspect exactly), not
each glyph's true design-space bounding box -- the same simplification
`atlas_concurrency_demo` already uses. See `PLAN.md`'s archived copy
(`planning/archive/PLAN_PHASE5_STEP5_1_2.md`) for the full reasoning.
