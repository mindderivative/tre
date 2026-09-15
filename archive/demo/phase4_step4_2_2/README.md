# Demo: Phase 4, Step 4.2.2 -- MSDF Glyph Generation

```bash
./demo/phase4_step4_2_2/run_msdf_generation_demo.sh
```

**Real MSDF generation via `fdsm`, not a from-scratch first attempt** at
a genuinely intricate, failure-prone algorithm (edge coloring,
true/pseudo-distance handling, sign correction -- `msdfgen`'s own author
wrote a full thesis on it). `fdsm` is a pure-Rust reimplementation of that
same published algorithm, keeping this workspace's pure-Rust font stack
(`rustybuzz`, `skrifa`) intact -- no new C library.

**Deliberately a glyph with a true hole, `'O'`, not another hole-free
letter.** Step 4.1's own demo explicitly named multi-contour hole
rendering as something its ear-clipping-based approach couldn't do,
deferred to "however Step 4.2's MSDF approach ends up handling it." MSDF
handles contours and winding natively via the distance field itself, with
no triangulation at all -- this is the step that actually closes that
gap, proven directly on the exact case that couldn't work before.

**No GPU, no shader, no Vulkan at all -- a real, deliberate change of
pace.** No pipeline in this project samples an arbitrary texture yet
(every one so far is flat-color, analytical-SDF, or stencil-and-cover),
so this step stops at the CPU-side pixel buffer Step 4.2.3's shader will
actually consume, rather than inventing throwaway GPU plumbing that step
would have to properly rebuild anyway. This is the first demo in this
project with no RHI/Vulkan involvement at all, and lives in
`crates/tre-text/examples/`, not `tre-rhi-vulkan/examples/`.

**Verified by an independent scanline check, not a hardcoded pixel
guess.** Rather than picking exact pixel coordinates for "inside the
ring" and "inside the hole" (fragile -- it would depend on this
particular font's exact proportions), this demo scans the MSDF's own
vertical-center row and counts outside-to-inside transitions: a solid
shape crossed through its center produces exactly one (entering once,
leaving once); a genuine ring produces exactly two separate ones (the
left wall, then the hole, then the right wall) -- a signature no solid
shape can produce. The median-of-3 evaluation itself is hand-rolled in
this demo's own code, independent of `fdsm`'s internal (and private)
`median`/`median3` helpers, so this is a real second implementation
checking the first, not a tautology.

**Two output images**, both real, both from the actual generated MSDF:
`msdf_generation_output.png` is a 256x256 preview rendered via `fdsm`'s
own CPU-side `render_msdf` (the same median-of-channels evaluation
`msdfgen`'s own reference tooling uses for exactly this purpose) --
sharp, correctly anti-aliased, showing a clean hollow ring.
`msdf_raw_output.png` is the same 32x32 MSDF's raw stored channel values,
nearest-neighbor upscaled with no interpolation, so the actual distance-
field bytes (not their rendered coverage) can be inspected directly.
