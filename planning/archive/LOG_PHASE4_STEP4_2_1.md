# Log: Phase 4, Step 4.2.1 -- Guillotine Atlas Bin-Packing

## Finding 1 (caught while writing this step's own demo): the very first
non-invariant colors this project has ever rendered exposed a real,
previously-invisible gap between what `UiVertex::color`'s doc comment
promises and what the shaders actually do

Every prior demo in this project (Phase 0 through Step 4.1) has rendered
exclusively pure white (`rgba8(255,255,255,255)`) fills. This demo is the
first to draw genuinely distinct, non-white colors -- and its very first
attempt (a palette of ordinary mid-tone colors) produced completely wrong
pixel values on readback: a gray `[150,150,150]` rendered back as
`[202,202,202]`, not `[150,150,150]`.

Root cause, confirmed by hand computation (not guessed): the headless
swapchain's color format is `vk::Format::B8G8R8A8_SRGB`
(`crates/tre-rhi-vulkan/src/headless.rs`), which means the GPU
automatically encodes whatever linear color the fragment shader outputs
into sRGB on store. `UiVertex::color`'s own doc comment says "sRGB
converted to Linear in shader" -- but neither `walking_skeleton.frag` nor
`sdf_rounded_rect.frag` actually performs that conversion; both pass
`in_color` straight through unchanged (`out_color = frag_color;`). The
vertex attribute format for `color` is `R8G8B8A8_UNORM` (a plain linear
normalize, no gamma awareness at fetch time either). Net effect: a color
authored as sRGB (as every UI color normally is) gets treated as if it
were already linear, then gets sRGB-*encoded* on store -- one gamma
operation applied where the correct pipeline needs a decode-then-encode
round-trip. For any value where decode and encode aren't both identity
(i.e. anything except pure 0 or 255 per channel), the result is visibly
wrong: `150/255 = 0.5882` sRGB-encodes to `0.7907`, i.e. `202/255` --
exactly what was observed.

This is not a new regression. `walking_skeleton.frag`'s own header
comment already calls it a "Phase 0 placeholder," and `TECHNICAL.md`
Section 6 states the canonical sRGB<->linear formula while noting
`DESIGN.md` Section 11.1 and `IMPLEMENTATION.md` Section 7.1 are where it
actually gets referenced/implemented -- Phase 7, Step 7.1 ("Linear sRGB
Conversions & HDR"), not yet built. Every demo before this one used only
white/black, both fixed points of any gamma curve (`encode(0)=0`,
`encode(1)=1`), which is precisely why five prior phases of GPU-rendered
demos never noticed.

**Not fixed here** -- implementing Step 7.1 now would be doing Phase 7's
work out of order, and DESIGN.md/IMPLEMENTATION.md describe more complete
color-management scope there (HDR, tone mapping) than a one-line shader
patch would actually deliver correctly. Instead, this demo's own palette
was changed to use only the 7 "pure" (each channel exactly 0 or 255)
colors -- fixed points under any correct gamma curve, so this demo's own
pixel assertions are exact and correct both today and after Step 7.1
eventually lands, with nothing here needing revisiting later.

## Finding 2 (caught by this step's own demo, a self-authored bug not an
engine defect): `read_pixels_bgra8`'s real BGRA memory order needs an
explicit swap once color order actually matters

After fixing Finding 1's palette, the demo still failed: a rectangle
requested as pure red `[255,0,0]` rendered back as `[0,0,255]` (blue).
Root cause: `HeadlessSwapchain::read_pixels_bgra8` is correctly named --
it returns real BGRA memory byte order, matching the `B8G8R8A8` format.
This demo's own `pixel_at` closure, copied from the exact pattern every
prior demo already used, returned those bytes unswapped and compared them
against a `[R,G,B,A]`-ordered expectation. Every prior demo's use of
white/black masked this too (channel order is irrelevant when R=G=B).

**Fix:** `pixel_at` in this demo now explicitly swaps indices 0 and 2
before returning, so its output is genuinely `[R,G,B,A]` order, matching
how `PALETTE` itself is written. Not an engine bug -- `read_pixels_bgra8`
behaves exactly as documented; this was this demo's own first attempt at
being the first caller to actually care about channel order.

## What worked without needing further iteration

- The Guillotine packer itself (`AtlasPacker::insert`, Best Area Fit
  selection, the split-leftover comparison choosing between the two
  candidate cuts by actual resulting area) produced correct,
  non-overlapping placements on the very first real run -- all 12 of 12
  varied requests packed successfully into the 256x256 demo atlas with
  zero overlaps, and all 5 unit tests (including the hand-worked-out
  100x100/90x10 split example) passed on the first attempt.
- Reusing the existing, completely unmodified flat-color pipeline for a
  wholly new kind of content (packed atlas rectangles, not glyphs or SVG
  paths) required no RHI changes at all.

## Verification performed

- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D
  warnings` / `cargo test --workspace`: all clean, including 5 new
  `tre-atlas` unit tests.
- `atlas_packing_demo` run manually against the real GPU (AMD/Radeon,
  Wayland session) under the Vulkan validation layer: all 12 rectangles'
  own-center and one unpacked-probe pixel assertions pass; output PNG
  visually inspected and shows 12 clearly distinct, non-overlapping
  colored rectangles.
- **All 12 pre-existing examples** re-run manually after adding the new
  `tre-atlas` crate to the workspace, zero validation errors and zero
  regressions -- this sub-step added no new RHI/GPU surface at all.
