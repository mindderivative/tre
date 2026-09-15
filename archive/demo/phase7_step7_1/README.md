# Demo: Phase 7, Step 7.1 -- Linear sRGB Conversions & HDR

```bash
./demo/phase7_step7_1/run_linear_color_demo.sh
```

Proves the real shader-side sRGB-to-linear conversion fix -- REVIEW.md
finding #92, first found at Phase 4 Step 4.2.1 and explicitly deferred
to this exact step. No prior demo could prove this: every one of them
deliberately uses only gamma-invariant (`0`/`255` per channel) colors,
per `sdf_rounded_rect_demo.rs`'s own header comment, specifically so
this defect's fix (or lack of one) couldn't affect their own assertions
either way.

**The bug, precisely:** the headless swapchain's format is
`B8G8R8A8_SRGB`, so the GPU automatically sRGB-encodes whatever a
fragment shader outputs on store. Before this step, every shader passed
`UiVertex::color` straight through -- an sRGB-authored value treated as
already-linear, then sRGB-encoded a *second* time. Finding #92's own
worked example: a mid-tone gray `150` round-trips to `202`.

**This demo proves the fix directly, not via a proxy.** It draws a fully
opaque rect (coverage `1.0` deep in its own interior -- no AA-edge
derivative uncertainty, no CPU-side alpha-premultiplication interaction)
with a genuinely non-fixed-point color, `rgb(150, 100, 200)`. The new
`srgb_to_linear` GLSL helper (added to all 4 real fragment shaders --
`walking_skeleton.frag`, `sdf_rounded_rect.frag`,
`bindless_textured.frag`, `msdf.frag`) and the swapchain's own hardware
sRGB-encode-on-store are exact inverses, so the real GPU readback must
equal the original authored color -- and on this project's own real dev
machine, it does, **exactly**: `[150, 100, 200]` in, `[150, 100, 200]`
back out.

The demo also computes what the *old*, unfixed double-encoding would
have produced (a real, independent Rust reference implementation of the
canonical sRGB encode formula) and confirms the real, fixed result is
measurably different from it -- `[202, 168, 229]`, exactly matching
finding #92's own `150`->`202` worked example on the red channel -- proving
the fix has real, provable effect, not just "didn't crash."

**A real, mid-implementation finding, not assumed in the original
plan.** Task 1 (preferring an HDR-capable `R16G16B16A16_SFLOAT` swapchain
format) was implemented and then reverted after actually running it
against this project's own dev machine: the real surface reports that
format only under `colorspace SRGB_NONLINEAR`, not a genuine wide-gamut/
extended-linear colorspace. A float format has no implicit hardware
encode-on-store the way an `_SRGB` format does, so presenting this
step's own now-genuinely-linear shader output through an
`SRGB_NONLINEAR`-tagged float surface would very likely display too
dark -- a real, disclosed correctness risk this step declines to ship.
`VulkanSwapchain::new` still selects `B8G8R8A8_SRGB` as before, and now
logs what the real surface *also* reports, so real future HDR work has
an observed starting fact instead of an assumption. See
`documentation/IMPLEMENTATION.md`'s own Step 7.1 write-up for the full
account.
