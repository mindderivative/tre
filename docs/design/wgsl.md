# WGSL shaders (proposed)

!!! note "Proposed (2026-09-30) — awaiting the owner's decision"
    This is the design for [issue #43](https://github.com/mindderivative/tre/issues/43),
    scoped as research first. The survey is on the issue; its conclusions are
    [below](#what-the-survey-found). Each question ends with a recommended
    answer and a **Decided** line, left open until the owner decides.
    Nothing here is built.

## The goal

Issue #43 asked to migrate the engine's shading pipeline to WGSL: leave
GLSL and SPIR-V, drop build-time shader compilers, and make it easy to write
"custom 2D post-processing effects, batch renderers, and material shaders".

The first two are already true. The third is the real question: what should
an application, or a framework above tre, be able to do with its own WGSL?
This page proposes an answer.

## What the survey found

- **tre has no shaders to migrate.** There are no `.wgsl`, `.glsl` or `.spv`
  files, no pipelines, no `naga`, and no `build.rs` in the engine. The GLSL
  and SPIR-V in the repository are in `archive/`, the retired Vulkan engine.
- **`vello_gpu` already uses WGSL.** At the pinned commit its shaders are
  written in WESL (a WGSL extension), linked at build time into WGSL strings,
  and loaded with `wgpu`'s `ShaderSource::Wgsl`. It has no hook for a custom
  shader or paint. Its filter layers implement only blur, and its layer masks
  panic.
- **There is a seam that doesn't need `vello` to change.** tre keeps one
  `wgpu::Texture` per image node and `vello_gpu` draws it as an external
  texture. Anything holding the `Device` and `Queue` can write that texture.
- **`naga` is already in the build** (30.0.1, through `wgpu`), so WGSL can be
  parsed and validated on the CPU with no new crate.

## Questions

**Q1. What can an app supply?**

- **(recommended)** A fragment shader that paints a node: a new kind,
  `"shader"`. It covers procedural fills, gradients, noise, animated
  backgrounds, and material effects.
- A post-effect over a node's children or the backdrop (blur, grading).
  This needs the subtree rendered to a texture first, with damage,
  transforms and clipping all following; `vello`'s own filters can't be
  extended. It could build on the answer above later, with texture inputs.
- A custom render or compute pass. Unbounded in scope; no.
- Replacing `vello`'s own shaders. They belong to a pinned upstream commit;
  no.

**Decided:** _pending._

**Q2. What is the API?**

- **(recommended)** `window.create("shader", wgsl=SOURCE, uniforms={...})`.
  A leaf that can also take children. It paints its box like any other node
  (`fill` behind the shader, `stroke_*` and `corner_radius` around it, the
  shader clipped to the rounded box, as an image is since 0.5.1), so it
  needs no special cases in layout, hit testing or the accessibility tree.
- A `shader` property on any box. More flexible, but every kind's paint code
  would have to handle it.

**Decided:** _pending._

**Q3. What does the shader write?**

- **(recommended)** One function, with tre writing the rest:

  ```wgsl
  fn shade(p: Pixel) -> vec4<f32> {
      let wave = sin(p.uv.x * 12.0 + frame.time);
      return vec4<f32>(u.tint.rgb * (0.5 + 0.5 * wave), 1.0);
  }
  ```

  tre supplies the vertex stage (a quad over the node), `struct Pixel { uv:
  vec2<f32>, px: vec2<f32> }`, a `frame` block (`size`, `time`), and the app's
  `uniforms` as a generated struct `u`. The app never writes a binding or a
  vertex shader, so the bind-group layout stays tre's to change.
- The whole module, vertex stage, bindings and all. Maximum freedom, and the
  binding layout becomes public API.

  The output is straight-alpha RGBA, like an `image`; the colour convention is
  settled by the first pixel tests.

**Decided:** _pending._

**Q4. How do uniforms get in?**

- **(recommended)** A dict: `uniforms={"tint": (1.0, 0.4, 0.2, 1.0),
  "amount": 0.5}`. A number is an `f32`, a tuple of two, three or four is a
  `vec2`, `vec3`, `vec4`. tre generates the struct, with WGSL's layout rules,
  and the names must be WGSL identifiers. `set(uniforms=...)` replaces them,
  checked first, atomically, as every `set` is; `get("uniforms")` reads them
  back. Changing a name or a type rebuilds the pipeline; changing a value
  only rewrites the buffer.
- Fixed numbered slots (`u0`..`u7`). Simple, but unreadable shaders.

Animating a uniform with `node.animate` is a natural follow-up, not part of
the first release.

**Decided:** _pending._

**Q5. Where does it run relative to the `vello` scene?**

- **(recommended)** Into the node's own texture, before the scene renders.
  tre records an extra pass per shader node into the frame's command encoder,
  after `prepare` and before `render_into`. The texture is the node's box
  size in surface pixels (the renderer has no scale factor), capped at 8192
  like an image, and `vello` then draws it as an external texture. So z-order,
  transforms, opacity layers, clipping, borders and partial redraw all work as
  they do for an image, with no change to `vello`.
- After the scene, as an overlay. Breaks z-order and clipping.
- Inside `vello`. There is no hook.

**Decided:** _pending._

**Q6. How are mistakes reported?**

- **(recommended)** Parse and validate with `naga` when `wgsl` is created or
  set, on the CPU, with no GPU needed, so a headless test catches a bad
  shader. A mistake raises `ValueError` carrying the line, the column and the
  offending source line, in the style of the other `set` errors. A problem
  only the GPU finds is logged once, and the node paints nothing; it never
  takes down the frame loop (glyph errors are handled the same way today).
- Validate only at first render. Mistakes then surface late, and only where a
  GPU exists.

**Decided:** _pending._

**Q7. When does it redraw?**

- **(recommended)** Only when something changed: a uniform, the node's size,
  or the source. A shader that reads `frame.time` sets `animated=True`, and
  is then redrawn every frame, keeping the loop awake like any running
  animation; without it, the loop still sleeps when nothing changes. The
  damage tracker fingerprints the uniforms, and the time when animated, so
  partial redraw repaints exactly the shader's box. `time` is the window's
  clock, so `window.advance()` makes it deterministic in tests.
- Always redraw. Simple, and it makes an idle window burn a GPU.

**Decided:** _pending._

**Q8. What does it cost?**

One offscreen pass and one texture per shader node, reused from frame to
frame and reallocated when the node is resized. Cost grows with the number of
shader nodes and their areas; that goes in the docs.

**Q9. Is it the same on every platform?**

- **(recommended)** Yes. WGSL is the one source, and `naga` translates it for
  the backend: SPIR-V on Vulkan, MSL on Metal, HLSL on DX12. The pass and the
  uniform layout are the same everywhere. Limits that differ by device (the
  largest texture, uniform buffer sizes) are checked and reported.
- Testing: Linux CI runs the GPU pixel tests on a software Vulkan and they
  block; macOS and Windows run them as informational steps. Real-hardware
  checks on a Mac and on Windows would go on the milestone, as they did for
  custom windowing.

**Decided:** _pending._

**Q10. Whose code is it?**

A shader is code that runs on the GPU. `naga` validates it, but doesn't bound
a loop, so a buggy or hostile shader can hang a GPU.

- **(recommended)** Document it as trusted source, and keep it out of the
  declarative view formats in the first release: a `"shader"` is created from
  Python, not read from a YAML or JSON view that might come from elsewhere.
- Accept it in views too. Convenient, and it makes untrusted-shader hangs a
  data-file problem.

**Decided:** _pending._

## What it does not do

- No texture inputs in the first release: a shader can't yet sample an image,
  a video frame or a subtree. That is the way to effects (blur a node's
  children, colour-grade a video) and is the natural second release.
- No compute shaders, no custom render passes, no changes to `vello`'s own
  shaders.
- No GLSL or SPIR-V input. `naga` can read both; tre wouldn't offer them.

## Milestones

If the owner accepts the recommendations, a minor release (0.6.0, since it
adds a node kind):

- **M1** — this design, decided.
- **M2** — the `shader` kind: its data model, `naga` validation with
  positioned errors, uniforms, and the Python API and type stub. CPU only, so
  it is testable headless.
- **M3** — the GPU pass: the generated module, the per-node texture, binding
  it as an external texture, and pixel tests for fill, uniforms, rounded
  clipping and size changes.
- **M4** — animation and partial redraw: `animated`, the damage
  fingerprint, the loop staying awake, `window.advance()` driving `time`;
  `examples/shader.py` and a guide page.
- **M5** — release, with the Tesserae compatibility check and the hardware
  checks on a Mac and on Windows.
