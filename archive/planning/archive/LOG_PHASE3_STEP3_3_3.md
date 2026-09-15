# Log: Phase 3, Step 3.3.3 -- Stencil-and-Cover Fallback Rendering

## Bug 1 (caught immediately, before any example ran): stencil-only aspect
mask needs `separateDepthStencilLayouts` enabled

The very first attempt at re-running the 7 pre-existing headless examples
after wiring the stencil attachment into `begin_frame` failed every
single one with a real Vulkan validation error:

```
vkCmdPipelineBarrier(): pImageMemoryBarriers[0].image has depth/stencil
format VK_FORMAT_D32_SFLOAT_S8_UINT, but its aspectMask is
VK_IMAGE_ASPECT_STENCIL_BIT. ... If the separateDepthStencilLayouts
feature is not enabled, then the aspectMask member ... must include both
VK_IMAGE_ASPECT_DEPTH_BIT and VK_IMAGE_ASPECT_STENCIL_BIT.
```

Since this project never reads or writes depth, the plan's design used a
stencil-only image view/aspect mask/layout (`STENCIL_ATTACHMENT_OPTIMAL`)
on the combined depth+stencil image every physical device is guaranteed
to support one of. That's only valid Vulkan with
`VK_KHR_separate_depth_stencil_layouts` (core in Vulkan 1.2, which this
device already targets) explicitly enabled at device creation -- an easy
thing to miss since the feature struct is a separate opt-in, not implied
by targeting API version 1.2 alone.

**Fix:** added `PhysicalDeviceSeparateDepthStencilLayoutsFeatures`
alongside the existing `dynamic_rendering`/`descriptor_indexing` feature
structs already chained into `VkDeviceCreateInfo`. All 7 examples passed
immediately after, with no other changes needed -- confirming the rest of
the stencil-attachment design (per-swapchain image, always-attached in
`begin_frame`, declared in every pipeline's `PipelineRenderingCreateInfo`)
was correct on the first attempt.

## Bug 2 (caught by this step's own demo): ear-clipping's crossing check
doesn't guarantee catching every self-intersecting polygon

The demo's own first design assumption -- "a classic pentagram will be
rejected by `triangulate()` with `NotSimplePolygon`" -- turned out to be
false. `triangulate()` happily returned `Ok([[0,4,3],[3,2,1],[3,1,0]])`
for a genuinely self-intersecting pentagram (real, non-adjacent edges 0-1
and 2-3 cross at approximately (120.8, 109.8), confirmed by hand
computation).

Root cause: the ear-validity checks built in Step 3.3.1 (no vertex
strictly inside the candidate triangle; no *remaining* edge properly
crosses the candidate diagonal) are checks performed *during* the
clipping process, against whatever boundary happens to still remain at
that point. They were never a global "is this whole polygon simple"
check. For this specific pentagram, the sequence of ears the algorithm
chose to clip never happened to produce a diagonal that conflicted with a
remaining edge -- even though the *original* polygon's boundary
genuinely self-intersects. The algorithm silently produced a
plausible-looking triangulation that does not correspond to any real
interpretation of the pentagram's shape under either fill rule, rather
than erroring.

This is a more concerning class of bug than a rendering artifact: a
silent wrong-answer for input the function is specifically supposed to
either handle correctly or reject.

**Fix:** added `has_self_intersection`, an explicit, global check (every
pair of non-adjacent original edges tested for a proper crossing) that
runs once, before ear-clipping ever starts. Independent of the clipping
process entirely, so it does not depend on which specific ears happen to
get chosen. Added `rejects_a_classic_self_intersecting_pentagram` as a
permanent `tre-svg` unit test regression for exactly this case.

## What worked without needing further iteration

- The per-swapchain stencil image creation/destruction (mirrored between
  `VulkanSwapchain` and `HeadlessSwapchain`, sharing
  `headless::allocate_and_bind_image`) worked correctly on the first
  attempt, including `multi_window`'s two independently-sized swapchains
  each getting their own correctly-sized stencil image.
- The two-PSO stencil-and-cover pipeline construction itself (stencil ops
  for both fill rules, the shared cover-pass stencil op, reusing the
  existing `walking_skeleton` shader) rendered the pentagram correctly
  under both fill rules on the very first real GPU run, exactly matching
  values independently computed via a Python winding-number/ray-casting
  reference before any Rust code was written.
- `draw_indexed`'s existing per-draw push-constant logic (screen size)
  required no changes to support switching between two different
  pipelines within one frame.

## Verification performed

- `cargo test --workspace`: all tests pass, including 1 new `tre-engine`
  usage (`FillRule`), and `tre-svg`'s test suite growing to 24 tests
  (including the new pentagram self-intersection regression).
- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D
  warnings`: clean.
- `stencil_and_cover_demo` run manually against the real GPU (AMD/Radeon,
  Wayland session) under the Vulkan validation layer: the
  `NotSimplePolygon` confirmation, and both fill rules' pixel assertions,
  all pass; output PNG visually inspected and shows a correctly-filled
  solid five-pointed star (`NonZero` fill, including the central
  pentagon).
- **All 10 pre-existing examples** (7 headless + 3 windowed) re-run
  manually after the shared RHI surface changes, zero validation errors
  -- the explicit, elevated verification bar this step's own plan called
  for, given it touches `begin_frame`/`create_pipeline` for the first
  time in Phase 3.
