# WGSL shaders (0.5.1)

!!! note "Decided (2026-09-30) — being scoped"
    This is the design for [issue #43](https://github.com/mindderivative/tre/issues/43),
    decided by the project owner: shader support ships in **0.5.1**, and
    0.5.1 is not pushed until it is complete. Each question below ends with
    its **Decided** line. The work is tracked in
    [#54](https://github.com/mindderivative/tre/issues/54). Nothing here is
    built yet.

## The goal

A framework above tre should be able to bring its own WGSL and apply it to
any node: shining text, a fractal background behind a layout panel, a colour
grade over a video. The owner's two examples set the scope:

- *"I want shining text"* — the shader needs the text's own rendered pixels as
  input, and its output is the text.
- *"Fractal backgrounds on a layout panel"* — the shader paints the panel's
  box.

Issue #43 itself asked to migrate the shading pipeline to WGSL. That part is
already true, as the survey shows.

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
  texture. Anything holding the `Device` and `Queue` can write that texture,
  and `vello` can render a scene into any texture.
- **`naga` is already in the build** (30.0.1, through `wgpu`), so WGSL can be
  parsed and validated on the CPU with no new crate.
- **tre handles no GPU errors today.** It has no device-lost callback, no
  uncaptured-error handler and no error scopes, though `wgpu` offers all of
  them ([#65](https://github.com/mindderivative/tre/issues/65)).

## What an app writes

```python
import tre

shine = tre.Shader(SHINE_WGSL, uniforms={"phase": 0.0}, mode="effect", animated=True)
title.set(shader=shine)                       # shining text: a text node

fractal = tre.Shader(FRACTAL_WGSL, uniforms={"zoom": 1.5}, mode="fill", animated=True)
panel.set(shader=fractal)                     # a fractal behind a layout panel
```

```wgsl
// mode="effect": the node's rendered pixels come in, the result goes out.
fn shade(p: Pixel) -> vec4<f32> {
    let c = content(p.uv);
    let band = smoothstep(0.0, 0.15, 1.0 - abs(p.uv.x - u.phase) * 4.0);
    return vec4<f32>(c.rgb + band * c.a * 0.6, c.a);
}
```

The colour convention (straight or premultiplied alpha) and the exact names
the prelude supplies are settled by the first pixel tests. The shape is fixed
by the decisions below.

## Decisions

**Q1. What can an app supply?** ([#55](https://github.com/mindderivative/tre/issues/55))

A fill shader (a node paints its box from the app's fragment function), texture
inputs (a shader can sample an image or video node), and subtree effects (a
node's own rendered content is the shader's input). A custom compute or render
pass, and replacing `vello`'s own shaders, are out.

**Decided:** all three, in 0.5.1.

**Q2. What is the API?** ([#56](https://github.com/mindderivative/tre/issues/56))

**Decided:** a `shader` property on **every** node kind, taking a `tre.Shader`
object: `node.set(shader=Shader(wgsl, uniforms, inputs, mode, animated))`, and
`None` clears it. The owner: apply it to *any* node kind, with the shader
taking the node's properties into account. So the shader sees the node's size,
corner radius and rendered pixels.

- `mode="fill"` paints the node's box behind its content, clipped to its
  rounded corners (a fractal behind a panel).
- `mode="effect"` renders the node's own paint and descendants, passes them to
  the shader, and draws the result in the node's place (shining text).

A `Shader` is validated once when it is created, can be reused across nodes,
and `Shader.set(uniforms=...)` updates every node using it. A `"shader"` child
kind is not part of this; it can be added later as a convenience over the
property.

**Q3. What does the shader write?** ([#57](https://github.com/mindderivative/tre/issues/57))

**Decided:** one function, `fn shade(p: Pixel) -> vec4<f32>`. tre supplies the
vertex stage, `Pixel { uv, px }`, a `frame` block (`size`, `time`), the app's
uniforms as a generated struct `u`, `content(uv)` for an effect, and one
sampler per named input. The app never writes a binding or a vertex shader, so
the bind-group layout stays tre's to change.

**Q4. How do uniforms get in?** ([#58](https://github.com/mindderivative/tre/issues/58))

**Decided:** floats and float vectors. A number is an `f32`; a tuple of two,
three or four numbers is a `vec2`, `vec3` or `vec4`; a colour is a `vec4`.
Names must be WGSL identifiers. tre generates the struct with WGSL's layout
rules. Changing a name or a type rebuilds the pipeline; changing a value only
rewrites the buffer.

**Q5. Where does the shader work run?** ([#59](https://github.com/mindderivative/tre/issues/59))

**Decided:** one pass graph. For each visible shader node, in dependency order,
before the final scene:

1. For an effect, or a shader with inputs, render the node's content (its own
   paint and descendants) into an offscreen texture with `vello`.
2. Run the shader's pass, writing the node's result texture.
3. The main scene draws that texture as an external texture in the node's
   place.

Transforms, opacity, clipping and partial redraw stay `vello`'s. An effect
inside an effect, and an input that is itself a shader node, run in
dependency order. Textures are the node's box size in surface pixels (the
renderer has no scale factor), capped at 8192 a side as for images.

**Q6. How are mistakes reported?** ([#60](https://github.com/mindderivative/tre/issues/60))

**Decided:** `naga` parses and validates the WGSL when the `Shader` is created
or set, on the CPU with no GPU, raising `ValueError` with the line, column and
the source line. A problem only the GPU finds is logged once and the node paints
as if it had no shader; it never takes down the frame loop.

**Q7. When does it redraw?** ([#61](https://github.com/mindderivative/tre/issues/61))

**Decided:** only when something changed: a uniform, the node's size, the
source, an input texture, or, for an effect, anything in its subtree. A shader
that reads `frame.time` sets `animated=True` and redraws every frame, keeping
the loop awake like any animation; otherwise the loop still sleeps. `time` is
the window's clock, so `window.advance()` makes it deterministic. Partial redraw
repaints exactly the shader node's box.

**Q8. Which shader nodes run?** ([#62](https://github.com/mindderivative/tre/issues/62))

**Decided:** only visible, on-screen ones. A hidden, culled or off-screen shader
node runs no pass and renders no offscreen content. There is no cap on how many
shader nodes exist; the pass list comes from the same paint walk as the scene.
The cost is one pass and one texture per shader node (and an offscreen render
for an effect), and that goes in the docs.

**Q9. How is it verified on macOS and Windows?** ([#63](https://github.com/mindderivative/tre/issues/63))

**Decided:** CI only for 0.5.1. Linux runs the GPU pixel tests on a software
Vulkan and they block; macOS and Windows run them as informational steps. WGSL
is one source, and `naga` translates it for the backend (SPIR-V on Vulkan, MSL
on Metal, HLSL on DX12). Real-hardware checks are a held Backlog item, like the
title-bar checks (#47, #48), and don't block 0.5.1.

**Q10. Whose code is a shader?** ([#64](https://github.com/mindderivative/tre/issues/64))

**Decided:** Python only. A `Shader` is a Python object; the declarative view
formats can't create one. Where its source comes from, and any checking of the
data a framework feeds it, is the framework's (Tesserae's) job: tre only
reports an error at creation time (Q6). Separately, and for **every** node, not
only shaders, tre should detect and report a hung or lost GPU where `wgpu`
makes that possible ([#65](https://github.com/mindderivative/tre/issues/65)).
`wgpu` reports a hang as **Device Lost**, which the OS driver triggers when a
shader loops or a command buffer runs too long (TDR on Windows, with Linux and
Metal's own timeouts); a software adapter has no such timeout, so a stall
watchdog covers it. The callbacks only run when the device is polled, so tre
polls it each loop turn; and a lost device can't be restored, so tre reports
the loss and ends the run rather than rebuilding (the owner's decision on
#65). A GPU error no longer panics, and a stall watchdog is off by default.
The shader milestones use this, so a hung shader is reported rather than
leaving a frozen window.

## What it does not do

- No compute shaders, no custom render passes, no changes to `vello`'s own
  shaders.
- No GLSL or SPIR-V input. `naga` can read both; tre doesn't offer them.
- No `"shader"` kind in the declarative view formats.
- No checking of a framework's data beyond the creation-time error.

## Milestones

Under the umbrella [#54](https://github.com/mindderivative/tre/issues/54),
for 0.5.1:

- **M1** — this design, decided.
- **GPU error handling** (#65) — the device-lost and error handlers, so the
  shader passes can report failures. First, since M3 depends on it.
- **M2** — the `Shader` object and the node property: validation with positioned
  errors, the module assembler, uniforms, `node.set(shader=...)`, the type stub.
  CPU only, so it is testable headless.
- **M3** (landed locally, #67) — the GPU pass for fill shaders: the pass graph's first version, the
  per-node texture and pipeline caches, binding as an external texture, culling,
  and pixel tests.
- **M4** (landed locally, #68) — texture inputs: naming an image or video node, binding it, dependency
  order, damage when an input changes.
- **M5** (landed locally, #69) — effects: the offscreen render of a node's content, `content(uv)`,
  nesting, and pixel tests (the shining text).
- **M6** (landed locally, #70) — animation and redraw, `animated`, the window clock, the loop staying
  awake; examples (shining text, a fractal panel) and a guide page.
- **M7** — verification: CI on all three systems, the Tesserae compatibility run,
  and the held hardware-checks item.
