# Demo: Phase 3, Step 3.3.3 -- Stencil-and-Cover Fallback Rendering

```bash
./demo/phase3_step3_3_3/run_stencil_and_cover_demo.sh
```

**What's actually new here:** every prior tessellation step (3.3.1's
ear-clipping, 3.3.2's morphing) required a *simple*, non-self-intersecting
polygon. This step handles the case they explicitly can't: a genuine
self-intersecting path -- a classic pentagram, five circle points
connected in `0, 2, 4, 1, 3` order, crossing its own boundary five times.
The example itself confirms `tre_svg::triangulate` rejects this path
(`SvgError::NotSimplePolygon`) before ever reaching the stencil-and-cover
code, so this demo proves a *real* gap being filled, not a strawman.

**A real, substantial addition to the shared Vulkan RHI surface**, not a
self-contained demo trick: every swapchain (`VulkanSwapchain` and
`HeadlessSwapchain`) now owns its own stencil image, `begin_frame` always
attaches it, and `create_pipeline` declares a matching stencil format
internally -- all 10 pre-existing examples were re-verified against this
change (see `documentation/REVIEW.md`'s "Phase 3 Step 3.3.3
Implementation" section for a real validation-layer regression this
surfaced and fixed: `separateDepthStencilLayouts` needed enabling for a
stencil-only image view/layout to be valid on a combined depth+stencil
format).

**No new shader.** `create_stencil_and_cover_pipelines` builds two
`VkPipeline`s from the *existing* flat-color `walking_skeleton` shader --
a stencil pass (color writes masked off, a stencil op that only writes: a
single `INVERT` for `EvenOdd`, two-sided `INCREMENT_AND_WRAP`/
`DECREMENT_AND_WRAP` for `NonZero`) and a cover pass (normal color
writes, a `stencil != 0` test that resets to `0` on pass so the next
shape starts clean). The entire technique is pipeline *state*, not new
shader code.

**Verified by reading back real pixels under both fill rules** at the
exact point where they're supposed to disagree: the pentagram's center
has winding number 2 (filled under `NonZero`) but is crossed an even
number of times (empty under `EvenOdd`) -- confirmed via an independent
Python winding-number/ray-casting calculation before any Rust code was
written, then matched exactly by the real GPU render.

**A real, second correctness bug found and fixed while building this
demo** (see `documentation/REVIEW.md`): `tre-svg::triangulate`'s
ear-validity checks (from Step 3.3.1) only ever compare a candidate
diagonal against the *currently remaining* boundary during clipping --
they do not, by themselves, guarantee detecting every self-intersecting
*original* polygon. This exact pentagram clipped cleanly with no
diagonal ever conflicting with a remaining edge, silently producing a
plausible-looking but wrong triangulation instead of being rejected.
Fixed by adding an explicit, global self-intersection pre-check
(`has_self_intersection`) that runs once before clipping ever starts,
independent of the clipping process -- and locked in as a `tre-svg` unit
test regression.
