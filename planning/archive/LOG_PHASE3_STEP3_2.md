# Log: Phase 3, Step 3.2 -- Analytical SDF Rounded Rectangles

## A real, latent gap found and fixed: `params` was never wired

`VulkanDevice::create_pipeline`'s vertex attribute descriptions had only
ever declared `position`/`uv`/`color` (locations 0-2) since Phase 0 --
`UiVertex::params` has existed in the vertex format the entire time, but
no shader before this one ever declared a `location = 3` input, so nothing
was ever visibly wrong. Found by direct code inspection while scoping this
step's blast radius, not by a failure. Fixed by adding location 3
(`R32G32B32_SFLOAT`, offset 20) to the universal pipeline layout, matching
the existing precedent for the bindless descriptor set and push-constant
range. Recorded as REVIEW.md finding #84.

## A real discovery during demo development: pixel-aligned flat edges have
no fractional AA coverage

The first version of `sdf_rounded_rect_demo`'s AA-transition check scanned
pixels along the rect's flat left edge (placed at an exact integer canvas
coordinate) looking for a genuinely blended pixel, and found a hard 0/1
transition instead -- no partial coverage anywhere. Hand-computing the SDF
at the actual sampled pixel centers explained it: on a flat, axis-aligned
edge, `fwidth(d)` is exactly 1 pixel, so the entire 1-pixel analytical AA
ramp falls exactly between the two nearest pixel centers (at the standard
half-integer sample offsets) whenever the true edge sits on an integer
coordinate -- both bracketing samples land exactly on the ramp's clamp
boundaries. Not a bug in the shader math or in `fwidth` -- an inherent
property of sampling a 1px-wide analytical ramp at pixel centers when the
edge happens to be pixel-aligned.

**Fix:** rewrote the check to scan a block of pixels around a rounded
corner's arc instead. The arc's non-axis-aligned gradient has no such
alignment, and the scan reliably finds several genuinely partial-alpha
pixels there (confirmed: e.g. `[89, 75, 75, 255]` against a background of
`[80, 63, 63, 255]` and foreground of pure white). This is also the more
representative check anyway -- proving the rounding itself, not a flat
edge the old flat-color shader already rendered correctly, is this step's
actual goal. Recorded as REVIEW.md finding #85 (not a defect).

## What worked without needing a fix

- The SDF formula and premultiplied-alpha output were correct on the
  first real run: the interior assertion (`[255, 255, 255, 255]`) and the
  cut-away-corner assertion (exact match against the dynamically-read
  background) both passed immediately, with no iteration needed on the
  shader math itself -- only on where in the image to look for the AA
  transition.
- All 7 pre-existing examples (`walking_skeleton`, `multi_window`,
  `headless`, `input_demo`, `memory_pools_demo`, `bindless_textures_demo`,
  `gc_demo`) ran cleanly under `VK_LAYER_KHRONOS_validation` with the new
  vertex attribute and the extended `draw_rounded_rect` signature -- zero
  validation errors, only the expected benign performance warning that
  older shaders don't consume the new `location = 3` input.
- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the
  workspace on the first attempt after adding 3 targeted
  `#[allow(clippy::float_cmp)]`s to new exact-arithmetic unit tests
  (the same pattern Step 3.1 established).

## Verification performed

- `cargo test --workspace`: all tests pass, including 4 `draw_rounded_rect`
  unit tests (1 pre-existing, updated for the new signature; 3 new, for
  the `uv`/`params` encoding and radius clamping).
- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D
  warnings`: clean.
- `sdf_rounded_rect_demo` run manually against the real GPU (AMD/Radeon,
  Wayland session) under the Vulkan validation layer: all three pixel
  assertions (interior, cut-away corner, AA transition band) pass; output
  PNG visually inspected and shows a correctly rounded, anti-aliased white
  rectangle on a dark background.
- All 7 pre-existing Vulkan examples re-run manually, zero validation
  errors.
