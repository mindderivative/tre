# Plan: Phase 7, Step 7.1 -- Linear sRGB Conversions & HDR

## Goal

Implement IMPLEMENTATION.md's own pre-existing Step 7.1 outline (3
tasks: HDR swapchain configuration, shader-side sRGB-to-linear
conversion before blending, HDR-to-SDR tone mapping) -- and, in doing
so, close out REVIEW.md finding #92, a real, already-documented defect
that named this exact step as its own deferred fix.

## Scope decisions

1. **The defect is real, precisely documented, and already deferred to
   this exact step -- REVIEW.md finding #92 (Phase 4 Step 4.2.1).**
   Its own worked example: the headless swapchain's format is
   `vk::Format::B8G8R8A8_SRGB`, so the GPU auto-encodes whatever a
   fragment shader outputs on store. Every real fragment shader passes
   the vertex color straight through unconverted -- a mid-tone gray
   `150` is treated as already-linear and sRGB-encoded a second time on
   store, round-tripping to `202`. Correct only for values that are
   fixed points of the gamma curve (`0`/`255` per channel); wrong for
   everything else. `UiVertex::color`'s own doc comment (`tre-engine/
   src/lib.rs`) already promises "sRGB converted to Linear in shader" --
   a promise never kept. Confirmed via direct inspection: **4 real
   fragment shaders** are affected, not the 2 the finding's own title
   names -- `walking_skeleton.frag`, `sdf_rounded_rect.frag`,
   `bindless_textured.frag`'s own no-texture-bound fallback branch, and
   `msdf.frag`, all read `frag_color` with zero conversion.

2. **Fix: one canonical `srgb_to_linear(vec3)` GLSL helper (TECHNICAL.md
   Section 6.2's exact piecewise formula), applied to `frag_color.rgb`
   only (never `.a` -- alpha carries no gamma curve) before any
   coverage/AA/blend math, in all 4 shaders.** No shared GLSL `#include`
   mechanism exists in this project's `build.rs` (plain `glslc`
   invocations, one file at a time), so the ~4-line helper is duplicated
   per file -- consistent with how each shader is already a small,
   independently-readable, self-contained unit. Shader outputs stay
   linear; no per-target-format branching is needed, because **both**
   real render-target formats in this codebase already expect genuinely
   linear shader output: an `_SRGB`-typed swapchain attachment
   auto-encodes on store (the whole point of using that format), and
   `Rgba16Float` transient layer targets have no implicit gamma curve at
   all and are conventionally linear by format.

3. **Texture-sampling paths need no change.** The MSDF atlas texture is
   `Rgba8Unorm` -- deliberately non-color distance-field data, per
   `TextureFormat`'s own existing doc comment ("must never pass through
   an `_SRGB` format's automatic gamma transform, which would corrupt it
   at every value except the two endpoints"). The layer-composite's
   sampled texture (`PipelineKind::TexturedQuad`) is the layer's own
   `Rgba16Float` transient target -- already linear by format, confirmed
   at Step 6.4.1. `Canvas` has no `draw_image`/similar method that would
   sample a genuinely sRGB-encoded color texture, so no real texture-read
   conversion path exists to fix today.

4. **CPU-side `RenderingCanvas`'s own `premultiply_alpha` (scales
   already-packed sRGB byte values by a linear `state.alpha` scalar) is
   a real, smaller, disclosed nuance -- deliberately out of scope.** It
   is not the defect REVIEW.md #92/TECHNICAL.md Section 6.2's own
   rationale name (the GPU fixed-function blend equation's color space);
   fixing it would require reworking how alpha is threaded through the
   IR entirely, a materially different and larger change.

5. **Verification: round-trip an opaque, non-fixed-point color through
   the fix rather than reconstructing the full blend pipeline as a
   reference calculation.** `srgb_to_linear` and the swapchain's own
   hardware auto-encode-on-store are exact inverses, so a fully OPAQUE
   rect (coverage `1.0` deep in its interior -- no AA-edge/derivative
   uncertainty, no CPU-side `premultiply_alpha` interaction since
   `state.alpha` defaults to `1.0`) drawn with a genuinely non-fixed-point
   color must, after the fix, read back as real GPU output equal to its
   own authored color (within a small, disclosed 8-bit tolerance) --
   before the fix, it would not (finding #92's own `150`->`202` example).
   This is simpler, more precise, and more directly tied to the exact
   documented defect than an alpha-blended reference-math design would
   be, and needs no new blend-equation modeling.

6. **Task 1 (HDR swapchain configuration): a real, buildable, honestly-
   scoped slice, not full end-to-end HDR.** `VulkanDevice::new`'s real
   surface-format search (`crates/tre-rhi-vulkan/src/lib.rs:2088-2092`)
   today only ever looks for `B8G8R8A8_SRGB`, falling back to
   `formats[0]`. Upgraded to prefer a real HDR-capable format
   (`R16G16B16A16_SFLOAT` at an appropriate wide-gamut colorspace) when
   the real physical device/surface actually reports support, falling
   back to the existing SDR search otherwise -- a real, always-correct,
   capability-query-driven code path. `HeadlessSwapchain`'s own
   hardcoded `HEADLESS_FORMAT` stays untouched: it exists to match
   lavapipe's own real SDR-only software-rasterizer capabilities, not to
   negotiate with a real display. Verified against the 3 real demos that
   already construct a genuine windowed `VulkanSwapchain` against the
   real display (`walking_skeleton.rs`/`input_demo.rs`/`multi_window.rs`
   -- already exercised live in this project's own CI under
   Xvfb+lavapipe) -- re-run to confirm the new logic still selects
   correctly. This dev machine's and CI's software Vulkan ICD almost
   certainly report SDR-only support, so the fallback path is what
   actually gets exercised for real here -- an honest, disclosed
   environmental limit, reported either way, not a gap in the logic
   itself or a claim of proven HDR output this project cannot back up.

7. **Task 3 (tone mapping): the canonical formula as a real, pure,
   unit-tested function only -- not wired into any real render path.**
   Lives in `tre-math`, matching `Affine2`'s own "pure math, no RHI
   dependency, build and prove the primitive before its exact consumer
   exists" precedent (TECHNICAL.md Section 7.2's own write-up). Not
   wired anywhere real: task 1's own buildable format-selection logic
   never actually selects a genuine HDR format on any hardware available
   to this project today, so there is no real trigger to wire it to yet.
   The real OS-level "brightness metadata hook" DESIGN.md Section 11.2
   describes stays real, disclosed, deferred future work -- confirmed via
   a real repo-wide `grep` (`headroom`/`HDR_WHITE`/`brightness_metadata`/
   `EDR`) that no such hook exists anywhere in code today.

8. **REVIEW.md finding #92 gets updated in place, not re-numbered.** Its
   own existing "Resolution" column already reads "deferred to Step
   7.1" -- this step closes it out where it already lives, appending a
   dated update to its own narrative and table row rather than opening a
   new finding for the same already-tracked bug.

## Tasks

1. Add the canonical `srgb_to_linear(vec3)` GLSL helper and apply it to
   `frag_color.rgb` (before any coverage/blend math) in all 4 real
   fragment shaders: `walking_skeleton.frag`, `sdf_rounded_rect.frag`,
   `bindless_textured.frag`, `msdf.frag`.
2. `crates/tre-rhi-vulkan/src/lib.rs`: upgrade `VulkanDevice::new`'s
   surface-format selection per scope decision 6 (HDR-capable
   preference, SDR fallback, an observable log line of what was
   actually selected).
3. `tre-math`: add a real `tone_map(linear: f32, headroom: f32) -> f32`
   per TECHNICAL.md Section 6.3's exact piecewise formula; unit tests
   covering identity at/below `1.0`, correct compression above it,
   continuity at the `L = 1.0` boundary, monotonicity, and asymptotic
   approach to `1.0 + headroom`.
4. New demo (`crates/tre-rhi-vulkan/examples/linear_color_demo.rs`)
   proving the shader fix per scope decision 5: draws an opaque rect
   with a genuinely non-fixed-point color, reads back real GPU pixels,
   asserts the round-tripped color matches its own authored value
   within a disclosed tolerance.
5. `.github/workflows/ci.yml`: add the new demo to the
   `vulkan-validation` job.
6. `demo/phase7_step7_1/`: README + run script + output screenshot,
   matching every prior demo-bearing step.
7. Update `documentation/IMPLEMENTATION.md` (Step 7.1 write-up) and
   `documentation/REVIEW.md` (finding #92's own narrative + table row,
   updated in place to reflect the real fix). `documentation/
   ARCHITECTURE.md`/`TECHNICAL.md` updated only if this step's own work
   reveals something not already correctly documented there -- expected
   to need none, since TECHNICAL.md's own canonical formulas are already
   correct and unchanged by this step.

## Verification plan

- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the
  workspace, including the new `tre-math` unit tests.
- New demo's own real GPU pixel assertion passes (round-trips the
  authored mid-tone color within tolerance).
- **This step changes 4 shared shaders nearly every existing demo
  depends on -- a genuinely global blast radius, unlike Step 6.5's own
  scene-composition-only change.** A full regression sweep across every
  real Vulkan demo is essential here, matching the discipline Step
  6.4.1 already applied when it touched shared `VulkanCommandBuffer`/
  `begin_frame` code. Zero regressions are expected -- existing demos
  deliberately use only gamma-invariant (pure `0`/`255` per channel)
  colors, per `sdf_rounded_rect_demo.rs`'s own header comment and
  finding #92's own text -- but this must be confirmed by actually
  re-running every demo, not assumed from that reasoning alone.
- The 3 real windowed-swapchain demos specifically checked for the new
  HDR-preferring format-selection logic (`walking_skeleton`/
  `input_demo`/`multi_window`).
- Commit; push only on explicit "push it"; `gh run watch` after any push
  (`accessibility-validation`'s pre-existing, documented failure
  expected and unrelated).

## Explicitly out of scope

- True end-to-end HDR rendering verified against a real HDR-capable
  display -- neither this dev machine nor CI's software Vulkan ICD has
  one.
- The real OS-level brightness-metadata hook (DESIGN.md Section 11.2) --
  no such API surface exists anywhere in this codebase or `tre-platform`
  today; confirmed via repo-wide grep.
- CPU-side `premultiply_alpha`'s own sRGB-vs-linear premultiplication
  imprecision -- a real, smaller, disclosed nuance, distinct from the
  GPU blend-equation defect this step targets.
- Any texture-sampling shader path needing sRGB decode -- `Canvas` has
  no `draw_image`/similar method that would sample a genuinely
  sRGB-encoded (not distance-field, not already-linear) texture yet.
- Upgrading `walking_skeleton_demo`'s own qualitative, screenshot-only
  verification to a programmatic pixel assertion -- it is an explicitly
  self-labeled Phase 0 placeholder demo; its shader still gets the real
  fix (task 1), but this step's own real proof lives in a new, dedicated
  demo (task 4), matching how `sdf_rounded_rect_demo`/`atlas_packing_
  demo` already established programmatic verification separately from
  Phase 0's own placeholder.
- Wide-gamut (Display P3) color-space-aware compositing math beyond
  swapchain format selection.
