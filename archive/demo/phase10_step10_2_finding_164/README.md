# Demo: REVIEW.md Finding #164 -- Premultiplied-Alpha Fix

```bash
./demo/phase10_step10_2_finding_164/run_translucent_flat_fill_demo.sh
```

**The bug.** `walking_skeleton.frag` -- now `PipelineKind::FlatColor`,
the real shader behind `Polygon`/`Path` fill and (since the same-day
lyon migration follow-up) stroke -- output
`vec4(srgb_to_linear(frag_color.rgb), frag_color.a)`: RGB was NOT
multiplied by alpha before the GPU's own premultiplied-alpha blend
equation (`ONE`, `ONE_MINUS_SRC_ALPHA`) ran, unlike every other real
fragment shader in this codebase (`sdf_rounded_rect.frag`,
`sdf_rect_styled.frag`, `sdf_ellipse.frag`, `msdf.frag`), all of which
already premultiply. For fully-opaque colors (`alpha == 1.0`) the two
are bit-identical, so this was invisible in every demo written so far --
including this same day's own `path_and_polygon_demo.rs` -- since none
of them ever draw translucent flat-fill geometry.

**The fix.** `main()` now premultiplies before writing `out_color`:
`vec4(linear_color * frag_color.a, frag_color.a)`, the exact pattern
every other real shader in this codebase already uses.

**What this demo proves, not just "didn't crash."** Draws one
genuinely translucent (alpha `120/255`) flat-filled rectangle over the
swapchain's own real background (read back directly, not assumed) and
computes two independent Rust reference values: what the CORRECT
premultiplied blend should produce, and what the OLD, unfixed
non-premultiplied shader would have produced. The real GPU readback
matches the correct reference almost exactly (`[169, 77, 139]` expected
vs. `[169, 77, 139]` measured in a real run) and is measurably different
from the broken one (`[233, 100, 187]`) -- confirmed by temporarily
reverting the shader to its old, buggy form during development and
re-running this exact demo: it failed with `got 233, expected 169`,
proving this demo would genuinely have caught the original bug, not
passed either way.

**Every other real consumer of this shader re-verified unaffected**
(`walking_skeleton`, `svg_morph_demo`, `svg_tessellation_demo`,
`text_shaping_demo`, `self_intersecting_fill_demo`,
`path_and_polygon_demo`, `atlas_packing_demo`, `headless`) -- all draw
only fully-opaque geometry, so their own pixel assertions are bit-for-bit
unchanged. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across
the whole workspace.
