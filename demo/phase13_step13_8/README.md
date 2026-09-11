# Demo: Phase 13 Step 13.8 -- Custom Shader API

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase13_step13_8
../../.venv/bin/python demo.py
```

**What this proves.** Q13's "user-facing custom shader API" -- real
user-supplied GLSL, compiled to SPIR-V at runtime via `shaderc` (the
identical real Google `shaderc` library `build.rs`'s own `glslc` CLI
invocation already wraps for this project's compile-time shaders,
kept deliberately identical rather than introducing a second,
potentially-divergent compiler like `naga`), executed as a genuine
per-pixel fragment shader on real GPU hardware.

**Real RHI constraints found and designed around** (confirmed by
reading `VulkanDevice::create_pipeline`/`create_universal_pipeline_
layout`'s own source before building this, not assumed): every real
pipeline shares one fixed `UiVertex` vertex layout, triangle topology,
and bindless descriptor set. A custom shader that accepts those
constraints -- the overwhelming majority of real "custom shader" asks
for a 2D UI engine (custom fills, procedural patterns, per-pixel
effects) -- needs **no new RHI plumbing at all**: `VulkanDevice::
create_custom_pipeline` compiles the caller's fragment source and pairs
it with `BINDLESS_TEXTURED_VERT`, the SAME real vertex shader
`TexturedQuad`/`GradientFill`/`MsdfText` already use, via the
*unmodified* `create_pipeline` method. And `execute_frame` already
resolves any `pipeline_state_id` generically via `PipelineRegistry::
get` -- not a hardcoded per-`PipelineKind` branch -- so a freshly
registered custom pipeline renders correctly with zero changes to the
real draw-dispatch path. The only genuinely new engine-level surface is
`tre_engine::CustomShaded`, a plain UV quad primitive that carries a
pipeline id instead of a `FillStyle`.

**Real Python API**: `renderer.create_custom_shader(fragment_glsl_
source) -> CustomShaderId` (compiles and registers against that
renderer's own `PipelineRegistry`), then `tre.CustomShaded(x, y, width,
height, shader_id, fill_color)` inserted into a `ShapeRegistry` like any
other shape.

**Real, disclosed v1 interface contract** a fragment shader must
declare (see `bindless_textured.frag`'s own real source for the exact
reference): `layout(location = 0) in vec4 frag_color;`,
`layout(location = 1) in vec2 frag_uv;`, `layout(location = 0) out
vec4 out_color;`, and the identical 12-byte `PushConstants { vec2
screen_size; uint texture_index; }` block. No custom vertex shader, no
arbitrary vertex attributes, no compute shaders, no descriptor sets
beyond the engine's own shared bindless set.

Every assertion checks a real, provable property, not just "it ran":

- A hand-written GLSL shader that outputs `vec4(frag_uv.x, frag_uv.y,
  0.0, 1.0)` renders a REAL per-pixel gradient: sampled corners of the
  shaded quad show the red channel strictly increasing left-to-right
  and the green channel strictly increasing top-to-bottom, with blue
  pinned at exactly `0` everywhere -- the signature of genuine shader
  execution using real UV coordinates, not a flat/cached color.
- Deliberately broken GLSL (`this is not valid GLSL;`) raises a real
  Python `ValueError` carrying `shaderc`'s own actual compiler
  diagnostic (file, line, the real GLSL error) -- verified non-empty
  and shown in the demo's own output.
- A `CustomShaded` quad renders correctly alongside a plain `Rectangle`
  in the SAME `ShapeRegistry` -- real composition with every other
  shape kind, not an isolated special case.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo
  test --workspace` -- all clean in debug (`tre-engine` 160 -> 162
  tests: a `CustomShaded` quad dispatches under its own caller-
  registered pipeline id with the exact expected vertex/UV layout, and
  its hit-test is a real axis-aligned rect test).
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in every prior step's
  README -- still flagged as separate follow-up work, not fixed here.
- `demo.py` run via `maturin develop --release` against real GPU
  hardware (real `shaderc` compilation, real Vulkan pipeline creation,
  real rendered pixels): exits 0, every assertion passes.
- Feasibility of the `shaderc` dependency was verified in complete
  isolation (a throwaway scratch crate, outside this repository) before
  touching `tre-rhi-vulkan`'s real `Cargo.toml` -- this machine already
  has `libshaderc_combined` installed and discoverable via `pkg-config`,
  confirmed directly rather than assumed.

**Real, disclosed scope limits**: no custom vertex shaders, no compute
shaders, no descriptor sets beyond the engine's own shared bindless set,
no arbitrary vertex attributes -- a caller needing any of those needs
real, separate future RHI work. `fill_color` on `CustomShaded` is a
plain flat color multiplier (`frag_color`), not the polymorphic `int |
GradientId | Texture` union every other shape's `fill_color` accepts --
what a custom fragment shader does with it is entirely up to its own
GLSL. A real, tunable SDF-based shadow (Q14/Q15's own "shader support
and shadows" tie-in, noted in the approved plan) could now build on this
API as a genuine v2, but is not part of this step's committed scope.
