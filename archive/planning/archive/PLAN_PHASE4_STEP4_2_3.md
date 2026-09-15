# Plan: Phase 4, Step 4.2.3 -- MSDF Evaluation Shader & Real Anti-Aliased Glyph Render

## Scope decisions

**The step that actually resolves the jagged-'X' observation from Step
4.1's demo.** Everything built in Steps 4.2.1-4.2.2 (the packer, the
MSDF generator) has been CPU-side or GPU-adjacent bookkeeping; this is
the first sub-step where a glyph reaches the screen through real,
resolution-independent anti-aliasing, using the exact canonical formula
TECHNICAL.md Section 5.3 already specifies -- implemented here, not
re-derived.

**Reuses Step 2.1's existing bindless-texture infrastructure directly --
no new descriptor-set or pipeline-creation code.** Checked against the
real, already-working `bindless_textured.vert`/`.frag` pair
(`create_texture` -> `.bindless_index()` -> `cmd_buffer.bind_texture(0,
index)` -> `draw_indexed`, all via the *same* generic `create_pipeline`
every other shader pair already uses): an MSDF texture is uploaded and
sampled exactly the same way a plain color texture already is. The only
real difference is what the fragment shader *does* with the sampled
value -- median-of-channels + `fwidth`-based opacity instead of a direct
texture read. This step adds a new `msdf.frag` only; `bindless_textured.vert`
is reused *unchanged* as the vertex stage (its inputs/outputs -- position,
uv, color, the `screen_size`/`texture_index` push constants -- are
already exactly what MSDF sampling needs, and `build.rs` already compiles
every shader file independently, not as fixed vert/frag pairs, so pairing
an existing vertex shader with a new fragment shader needs no build
changes beyond one new `compile_shader` call).

**A new `TextureFormat::Rgba8Unorm` variant is required, not optional.**
The existing two variants (`Bgra8Srgb`, `Rgba16Float`) are both meant for
*color*; an MSDF texel is a *distance encoding*, not a color, and must
never be gamma-corrected at all -- sampling it through an `_SRGB` format
would silently corrupt the encoded distance at every value except the
two endpoints (the exact class of bug Finding #92, Step 4.2.1, already
documented for *color* data; this would be the same defect class hitting
*geometry* data instead, which is worse). `Rgba8Unorm` maps to
`vk::Format::R8G8B8A8_UNORM` -- linear, 4 bytes/pixel, no gamma anywhere
in the read path. `fdsm`'s own output is RGB8 (3 bytes/pixel); the demo
pads it to RGBA8 (unused alpha channel set to `255`) before upload, since
4-byte-aligned formats have far better, more universal GPU support than
tightly-packed 3-byte ones.

**Demo glyph: `'O'` again, deliberately.** Reusing the same hole-having
glyph from Step 4.2.2 makes this step's demo the payoff moment for the
*entire* Step 4.2 arc so far: a real font's real glyph, with a real hole,
rendered through a real GPU shader with real anti-aliasing -- not a new,
narrower case. A hole-free letter would prove less with the same amount
of work.

**Fill color: pure white, matching every prior demo's convention.**
Finding #92 (Step 4.2.1: `walking_skeleton.frag`/`sdf_rounded_rect.frag`
never perform the sRGB-to-linear conversion `UiVertex::color` documents,
deferred to Step 7.1) is still unfixed, and this step's own new
`msdf.frag` inherits the same gap -- so the exact-equality pixel checks
below (fully inside the ring, fully outside/in the hole) use white, the
one color invariant under that gap either way. The AA-boundary check
below needs no such care: "this pixel's color sits strictly between the
fill color and the background," not an exact value, holds regardless of
any monotonic per-channel transform applied uniformly to both endpoints.

## Goal

Render a real glyph's real MSDF through a real GPU shader implementing
TECHNICAL.md Section 5.3's exact formula, at a large enough on-screen
size that the earlier jaggedness would be obvious if it were still
present -- proven by pixel-exact checks deep inside the ring and deep
inside the hole, *and* by a genuinely fractional (neither pure fill nor
pure background) pixel at the glyph's own edge, the concrete, measurable
signature of real anti-aliasing rather than a hard binary edge.

## Tasks

1. **`TextureFormat::Rgba8Unorm`** added to `tre-engine`'s shared enum;
   mapped to `vk::Format::R8G8B8A8_UNORM` and `4` bytes/pixel in
   `tre-rhi-vulkan`'s existing format/byte-size tables (the same two
   small match arms `Bgra8Srgb`/`Rgba16Float` already have).

2. **`msdf.frag`** (new shader, `crates/tre-rhi-vulkan/shaders/msdf.frag`):
   samples the bound bindless texture (identical structure to
   `bindless_textured.frag`'s `texture(sampler2D(bindless_textures[...],
   bindless_sampler), frag_uv)` call, including its `texture_index ==
   0xFFFFFFFFu` fallback), computes `sigDist = median(r,g,b) - 0.5` and
   `opacity = clamp(sigDist / fwidth(sigDist) + 0.5, 0.0, 1.0)` (TECHNICAL.md
   Section 5.3's exact formula), and outputs premultiplied-alpha color
   (`vec4(frag_color.rgb * opacity, frag_color.a * opacity)`, matching
   `sdf_rounded_rect.frag`'s own closing line and ARCHITECTURE.md Section
   6.1's blend-state convention) -- no new vertex shader; reuses
   `bindless_textured.vert` unchanged. `build.rs` gets one new
   `compile_shader` call.

3. **New example**
   (`crates/tre-rhi-vulkan/examples/msdf_rendering_demo.rs`,
   `demo/phase4_step4_2_3/`): discovers a real cascade font, extracts
   `'O'`'s outline, generates its `32x32` MSDF (`tre_text::generate_msdf`,
   Step 4.2.2, unmodified), pads RGB8 to RGBA8, uploads it via
   `create_texture(..., TextureFormat::Rgba8Unorm, ...)`, binds it via
   `bind_texture`, and renders one quad through the new `msdf.frag`
   pipeline at a generous on-screen size (large enough that residual
   jaggedness would be visually obvious). Reads back real pixels: a point
   deep in the ring's own material is exactly white; a point deep in the
   hole (and one clearly outside the whole glyph) is exactly the
   background; and a point straddling the ring's own boundary (found the
   same way Step 4.2.2's demo found its scanline transitions, not
   guessed) is a real, strictly-intermediate blend between the two --
   proof of genuine sub-pixel anti-aliasing, not a hard edge.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- `msdf_rendering_demo` run under `VK_LAYER_KHRONOS_validation`, zero
  errors -- new RHI surface this time (`TextureFormat::Rgba8Unorm`, a new
  fragment shader), so this is real, not just a regression check.
- All 13 pre-existing Vulkan examples re-run manually, unaffected.
- CI: add `msdf_rendering_demo` to the `vulkan-validation` job's example
  list; push, confirm green.

## Explicitly out of scope for this sub-step

- Multi-window atlas concurrency (task 4, the bounded MPSC ring buffer
  and `AtomicU64` slot table) -- Step 4.2.4.
- Integrating Step 4.2.1's `AtlasPacker` at all -- this demo uploads one
  glyph's MSDF as its own dedicated texture (matching
  `bindless_textures_demo`'s own precedent of separate dedicated
  textures), not a shared packed atlas; wiring the packer, the generator,
  and this shader together into one real shared-atlas texture is a later
  integration concern once Step 4.2.4's concurrency model exists to
  guard it.
- Fixing Finding #92 (the sRGB gamma gap) -- still Step 7.1's job, per
  Step 4.2.1's own disposition; this step's demo works around it with a
  pure-white fill, same as every prior colored-rendering demo.
- Wiring MSDF rendering into `RenderingCanvas`'s public API, DESIGN.md
  Section 8.1's full shader-family unification (one `PipelineStateId`
  branching between analytical-SDF/plain-texture/MSDF modes) -- a real,
  already-documented future consolidation, but every rendering technique
  in this project so far (flat-color, analytical SDF, bindless-textured,
  stencil-and-cover) has shipped as its own dedicated shader pair first;
  unifying them is naturally Phase 6's batching/sorting concern, where
  cross-technique batching actually starts to matter, not this step's.
