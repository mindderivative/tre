# Plan: Phase 3, Step 3.2 -- Analytical SDF Rounded Rectangles

## Scope decisions (confirmed with the project owner, 2026-09-06)

Corresponds to IMPLEMENTATION.md's Step 3.2. Task 2 ("emit exactly 4
vertices/6 indices per rectangle") is **already done** -- `RenderingCanvas::
draw_rounded_rect` has emitted exactly that shape since Phase 0, per its
own doc comment ("that shader is IMPLEMENTATION.md Phase 3.2's job, out of
scope for this walking skeleton"). This step's real remaining work is
tasks 1 and 3: the actual SDF fragment shader and its `fwidth`-based
anti-aliasing, which the current flat-color placeholder shader has never
evaluated.

**Uniform corner radius only, not per-corner `CornerRadii`.**
IMPLEMENTATION.md Step 3.2 task 1's formula --
$d(\mathbf{p}) = \Vert\max(\mathbf{q},0)\Vert + \min(\max(q_x,q_y),0) - r$
-- takes a single scalar $r$. DESIGN.md's eventual `Canvas::
draw_rounded_rect(&Rect, &CornerRadii, &Paint)` API (plural radii, one per
corner) is later, UI-framework-facing surface this project hasn't built
yet -- extending the SDF to four independently-selected radii is a real,
known technique but not what this step's literal formula specifies.
Building it now would be scope creep ahead of the API surface that would
actually need it.

**A new, dedicated shader pair, not a modification to an existing one.**
This project has already established the precedent of one shader pair per
distinct rendering technique rather than a single unified shader from the
start -- `bindless_textured.{vert,frag}` (Phase 2 Step 2.1) was built
alongside `walking_skeleton.{vert,frag}`, not merged into it. DESIGN.md
Section 8.1.2's eventual "shader-mode tag" unifying SDF-rect/texture/MSDF
into one pipeline is explicitly a later-phase concern (it exists to batch
across techniques once MSDF text -- Phase 4 -- gives batching something
real to prove); building that unification now, with only one of its three
modes actually implemented, would be premature. This step adds `sdf_
rounded_rect.{vert,frag}` as its own pipeline, leaving `walking_skeleton`
and `bindless_textured` untouched.

**`UiVertex::uv` is repurposed as the SDF query point, exactly as
ARCHITECTURE.md Section 3.1 already anticipates.** That field's own doc
comment already reads "Texture coordinates **or SDF bounds**" -- this
isn't a new hack, it's what the canonical vertex format was designed to
carry. For this shader, `uv` at each corner is set to that corner's
position *relative to the rectangle's center*, in the same pixel units as
`position` itself (not the `0..1` texture-coordinate convention
`draw_rounded_rect`'s flat-color path currently uses). Interpolating these
four corner values linearly across the quad's two triangles reproduces the
exact local `(x, y)` offset at every fragment -- the standard, exact
technique for evaluating a box SDF from a single quad, not an
approximation. `params` becomes `[radius, half_width, half_height]`.

**`draw_rounded_rect`'s signature changes for real, not a parallel
method.** The function is already named for what it's supposed to do and
its own doc comment already says the real implementation is this step's
job -- adding a `radius: f32` parameter and switching its vertex emission
to the SDF convention above is completing the function, not introducing a
new one alongside it. All 7 existing call sites (across 5 examples and 1
unit test) pass `0.0`, preserving their exact current visual output
(they're rendered by the untouched flat-color shader, which was already
ignoring `uv`/`params` and will continue to). Radius is clamped to
`min(half_width, half_height)` (and to `>= 0.0`) before being stored --
an uncapped radius produces a self-overlapping, visually wrong shape from
this exact formula, not a crash, but a real, easy caller mistake worth
guarding against at the one place it's constructed.

**Correct premultiplied-alpha output is a real, checked requirement, not
an afterthought.** ARCHITECTURE.md Section 6.1's blend state (`src=ONE,
dst=ONE_MINUS_SRC_ALPHA`, already configured in `create_pipeline`) expects
the fragment shader's own output to already be premultiplied. The existing
flat-color placeholder never had to get this right (it always outputs
opaque colors). This shader computes a genuine fractional alpha at
rounded edges via `fwidth`, so `out_color = vec4(frag_color.rgb * alpha,
frag_color.a * alpha)` -- multiplying *both* channels by the computed AA
alpha -- is required for correct edge blending against the clear color,
and is exactly what the new demo's pixel assertions check for.

**`create_pipeline` gains the `params` vertex attribute it never had.**
Checked directly: `UiVertex` has always had a `params: [f32; 3]` field,
but `create_pipeline`'s vertex attribute descriptions only ever declared
`position`/`uv`/`color` (locations 0-2) -- `params` has been present in
every vertex buffer since Phase 0 but never wired as a shader-readable
attribute. Adding location 3 (`R32G32B32_SFLOAT`, offset 20) is required
for this shader to read it at all, and -- matching the same "one universal
pipeline layout" precedent as the bindless descriptor set and the 12-byte
push-constant range -- applies to every pipeline uniformly; existing
shaders simply don't declare a `location = 3` input and keep working
unmodified.

## Goal

A real rounded rectangle, analytically anti-aliased at its curved corners
via a genuine SDF evaluation and `fwidth` screen-space derivatives, proven
by reading back actual pixels: the interior is the foreground color, a
point in the cut-away corner region is the background color, and the
transition between them is a real blend, not a hard edge -- not merely "a
shader that compiles and doesn't crash."

## Tasks

1. **`create_pipeline` gains a `params` vertex attribute** (location 3,
   `R32G32B32_SFLOAT`, offset 20) alongside the existing three.

2. **`RenderingCanvas::draw_rounded_rect` gains a `radius: f32`
   parameter.** Clamped to `[0.0, min(half_width, half_height)]` before
   use. Each of the 4 emitted vertices' `uv` becomes that corner's offset
   from the rect's center (`(±half_width, ±half_height)`); `params`
   becomes `[radius, half_width, half_height]` on all 4 (uniform per
   quad, not truly per-vertex data, but stored per-vertex since that's
   the vertex format's only channel for it). All 7 existing call sites
   updated to pass `0.0`.

3. **New shader pair** (`crates/tre-rhi-vulkan/shaders/sdf_rounded_rect.
   {vert,frag}`): the vertex shader is `walking_skeleton.vert`'s
   screen-to-NDC placeholder plus passing `uv`/`params` through
   unmodified. The fragment shader evaluates IMPLEMENTATION.md Step 3.2
   task 1's exact formula using `uv` as $\mathbf{p}$ and `params` as
   $(r, b_x, b_y)$, computes `alpha = clamp(0.5 - d/fwidth(d), 0.0, 1.0)`
   (task 3's exact formula), and outputs premultiplied
   `vec4(color.rgb * alpha, color.a * alpha)`.

4. **New example** (`examples/sdf_rounded_rect_demo.rs`,
   `demo/phase3_step3_2/`): headless, draws one real rounded rectangle
   (a real nonzero radius, e.g. $40\text{px}$ on a $300\times200$ rect)
   through the new shader, reads back the rendered pixels, and asserts:
   the rect's interior center is exactly the foreground color; a point in
   the bounding box's corner clearly outside the rounding arc (with
   margin past the $\sim\!1\text{px}$ AA transition band, not adjacent to
   it) is exactly the background clear color; and at least one point in
   the actual AA transition band is neither exactly foreground nor
   exactly background -- a real, checked blend, not just "it didn't
   crash."

5. **CI**: add `sdf_rounded_rect_demo` to the `vulkan-validation` job's
   example list.

6. **New unit test(s)** in `tre-engine` (no GPU needed, alongside the
   existing `draw_rounded_rect_emits_one_command_with_four_vertices_six_
   indices` test, updated for the new signature): the emitted `uv`/
   `params` values match the documented convention exactly for a known
   input, and an oversized requested radius is clamped rather than stored
   as-is.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- All five pre-existing Vulkan examples re-run manually with
  `VK_LAYER_KHRONOS_validation` enabled, zero errors -- confirming the new
  `params` vertex attribute and the extended `draw_rounded_rect` signature
  don't disturb the unmodified flat-color rendering path.
- `sdf_rounded_rect_demo`'s pixel assertions (interior/corner/AA-band)
  pass locally under validation, and the demo's PNG output is inspected
  visually as a sanity check alongside the programmatic assertions.
- CI: push, confirm `sdf_rounded_rect_demo` passes on Mesa lavapipe.

## Explicitly out of scope for this step

- Independent per-corner radii (`CornerRadii`) -- the formula this step
  implements takes one scalar radius; four-corner support is a real,
  separate technique for whenever DESIGN.md's `CornerRadii`-taking
  `Canvas` API actually gets built.
- Shader unification across SDF-rect/texture/MSDF modes via a shared
  "shader-mode tag" (DESIGN.md Section 8.1.2) -- deferred until Phase 4's
  MSDF text gives batching across techniques something real to prove.
- Stroke/border rendering, drop shadows, or any SDF variant beyond the
  plain filled rounded rect this step's exact formula describes.
- Wiring `draw_rounded_rect` into the sort/batch/execute pipeline (Phase
  6) -- this step, like Step 2.2's transient pool before it, proves the
  primitive directly via a dedicated demo, not through machinery that
  doesn't exist yet.
