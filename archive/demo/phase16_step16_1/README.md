# Demo: Phase 16 Step 16.1 -- SDF-Based Soft Shadows ("Shadow v2")

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase16_step16_1
../../.venv/bin/python demo.py
```

**What this proves.** GUI Readiness recommendation 10: "the real Dual-
Kawase blur behind shadows today has a fixed chain depth -- no tunable
radius yet. A v2, gated on the new custom shader API, could offer a
continuously adjustable, SDF-based soft shadow instead." Built entirely
on the existing custom shader API (Phase 13 Step 13.8) -- no changes at
all to `LayerDesc.blur`'s own fixed 4-hop Dual-Kawase chain
(`BlurResources` in `crates/tre-rhi-vulkan/src/lib.rs`), which stays
exactly as it was and remains fully supported.

`tre.sdf_shadow_shader_source()` returns real GLSL a caller compiles
once via `renderer.create_custom_shader(...)`; `tre.shadow_sdf_params(x,
y, width, height, radius_px, blur_px)` returns everything needed to
actually draw it: the enlarged `CustomShaded` quad bounds and the
normalized `(radius_frac, sigma_x_frac, sigma_y_frac)` params to set on
it.

**Why normalized params, not pixels.** A general rounded-box shadow
needs four independent numbers (half-width, half-height, corner radius,
blur sigma), but `UiVertex.params` only has three float slots --
growing it would bloat every pipeline in the engine, not just this one
(`crates/tre-engine/src/gpu_style.rs`'s own doc comment). The fix: the
shader works in each axis' own local space, where the box's half-extent
is implicitly `1.0`; corner radius and blur sigma become fractions of
the box's own half-extent per axis. Real, disclosed v1 scope limit: for
a non-square box, a "radius_frac" corner isn't a literal circular arc
in true pixel units (the two axes are independently normalized) --
visually correct for modest aspect ratios (cards, buttons, tooltips), a
real approximation for extreme ones, the same category of trade-off as
`sdf_rect_styled.frag`'s own corner-smoothing/squircle deviation.

**A real bug found only once this actually rendered.** A fragment
shader is never invoked outside the triangles it's rasterized on -- the
very first real render showed a hard cutoff at the `CustomShaded`
quad's own edge regardless of `blur_px`, because the quad was drawn at
exactly the shadow's own logical size, leaving no rasterized room for
the soft edge to extend into. Fixed by having `shadow_sdf_params`
return the quad ENLARGED by `blur_px` on every side (the same real
margin concept `shadow_layer_bounds` already uses for the v1 blur
path), with the shader rescaling `frag_uv` by `(1 + sigma_frac)` per
axis -- exactly the ratio between the enlarged quad's own half-extent
and the logical box's real half-extent -- so the logical box's own edge
still lands at the right place inside the larger quad.

**The falloff is a real SDF soft edge, not a literal Gaussian blur.**
The shader reuses the *exact* already-proven, IQ-derived rounded-box
signed-distance function `sdf_rect_styled.frag`'s own `sd_rounded_box`
uses (adapted to a single uniform radius -- per-corner radii would need
a 4th float this budget doesn't have), then applies a continuous
`smoothstep` falloff around the zero-distance edge. This is genuinely,
continuously tunable (a plain float, zero recompilation to retune) and
rounds corners for free via the reused SDF -- but its falloff curve is
not a literal Gaussian convolution of a box (that's the separate
erf-based technique real 2D box-shadow shaders use). What's real and
verified below is the *continuous* tunability itself.

**Engine changes, all additive:** `tre_engine::CustomShaded` gains a
real `params: [f32; 3]` field (previously always hardcoded to zero by
`draw_custom_shaded_quad`); `PyCustomShaded` gains matching
`param_x`/`param_y`/`param_z` fields; `VulkanDevice::
create_custom_pipeline` now pairs a custom fragment shader with
`SDF_ROUNDED_RECT_VERT` instead of `BINDLESS_TEXTURED_VERT` -- a strict
superset (it additionally forwards `frag_params`), so every pre-existing
custom shader that only declares `frag_color`/`frag_uv` still links
correctly (legal SPIR-V interface matching: a fragment shader may
consume a subset of a vertex shader's outputs) -- verified directly by
rerunning `demo/phase13_step13_8/demo.py` unchanged.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` all clean in debug (`tre-python` gains 4 new
`shadow_sdf_params` unit tests: a square box's equal sigma fractions on
both axes plus its own enlarged quad bounds, a wide box's smaller sigma
fraction on its longer axis, a radius clamped at the box's own smaller
half-dimension, and a zero-size box degrading to zero fractions -- not
a panic -- while still returning a real, non-degenerate enlarged quad).
`--release` clean apart from the same 5 pre-existing, already-disclosed
`debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`,
unrelated to this step).

`demo.py`, run via `maturin develop --release` against a real headless
GPU renderer:

- **Continuous tunability, proven directly**: the same square shape
  rendered at `blur_px` of 0, 8, and 16 produces real measured ink
  spreads of 0, 6, and 13 pixels past its own nominal edge -- strictly
  increasing and tracking the requested blur closely (the shader's own
  smoothstep falloff reaches exactly zero at `d == sigma`, i.e. at
  `blur_px` pixels past the edge), unlike the old blur's fixed hop
  count.
- **The reused SDF still rounds correctly**: `radius_frac=0` keeps a
  plain rectangle (its own literal corner is real ink); a `radius_frac`
  of `0.95` on the same square box visibly rounds that exact corner
  pixel away to real background, while a flat edge's own midpoint stays
  real ink.
- **v1 is unaffected**: Phase 13 Step 13.4's own blur-based shadow
  check (`canvas.layer(blur=True)` + `shadow_layer_bounds`) is
  reproduced verbatim and still shows the same real, non-linear blur
  gradient across the shadow's own former hard edge -- v2 is additive,
  not a replacement.

**Real, disclosed remaining scope**: single uniform corner radius, not
per-corner (a 4th float this budget doesn't have); non-square-box
radius/sigma are per-axis-normalized approximations, not literal pixel
circles; the falloff is an SDF soft edge, not a true Gaussian
convolution of a box.
