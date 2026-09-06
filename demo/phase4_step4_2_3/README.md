# Demo: Phase 4, Step 4.2.3 -- MSDF Evaluation Shader & Real Anti-Aliased Glyph Render

```bash
./demo/phase4_step4_2_3/run_msdf_rendering_demo.sh
```

**The payoff moment for the whole Step 4.2 arc, and the concrete fix for
the jagged 'X' observed in Step 4.1's demo.** Everything built in Steps
4.2.1-4.2.2 (the bin-packer, the MSDF generator) was CPU-side or
GPU-adjacent bookkeeping; this is the first sub-step where a glyph
actually reaches the screen with real, resolution-independent
anti-aliasing -- TECHNICAL.md Section 5.3's exact canonical formula
(median-of-channels signed distance, `fwidth`-based opacity), implemented
here rather than re-derived.

**No new descriptor sets, no new pipeline-creation code -- reuses Step
2.1's existing bindless-texture infrastructure exactly as-is.** The MSDF
texture is uploaded via the same `create_texture` -> `bindless_index()`
-> `bind_texture` -> `draw_indexed` flow `bindless_textures_demo` already
established. `msdf.frag` is even paired with `bindless_textured.vert`
*unchanged* -- no new vertex shader was needed at all, since that
existing shader's inputs (position, uv, color) and push constants
(`screen_size`, `texture_index`) were already exactly what MSDF sampling
requires.

**A new `TextureFormat::Rgba8Unorm`, because an MSDF texel is a distance
encoding, not a color.** The two existing texture formats
(`Bgra8Srgb`, `Rgba16Float`) are both meant for color data; sampling MSDF
data through an `_SRGB` format would silently corrupt the encoded
distance at every value except the two endpoints -- the same defect
class Step 4.2.1's Finding #92 already documented for actual *color*
data, but worse here since it would corrupt *geometry*.

**Deliberately reuses `'O'`**, the same hole-having glyph from Step
4.2.2, so this demo is the real end-to-end proof: a genuine font's
genuine glyph, with a genuine hole, rendered through a genuine GPU shader
with genuine anti-aliasing -- not a narrower, easier case chosen just for
this step.

**Verified by scanning the actual rendered pixels, not predicting exact
screen coordinates.** Bilinear texture filtering (already enabled on the
existing bindless sampler) means the precise on-screen transition point
between "inside" and "outside" is a function of GPU sampling, not
something to hand-predict from the source texel grid. This demo instead
scans the glyph's own center row after rendering and classifies each
pixel by how close it sits to white vs. background: a genuine ring
produces real stretches of both, *and* at least one pixel at each
ring-wall crossing whose value is strictly between the two -- the
concrete, measurable signature of real sub-pixel anti-aliasing that a
hard binary edge could never produce.
