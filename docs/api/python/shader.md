# Shader

*New in 0.5.1.* A `Shader` is WGSL that paints a node. This page covers the
object and the `shader` property; how to use it is the
[Shaders guide](../../guide/shaders.md), and the design is
[WGSL shaders](../../design/wgsl.md).

!!! note "Fill and effect"
    A `mode="fill"` shader paints the node's box, clipped to its rounded
    corners, *behind* the node's own paint (a `fill` with some transparency
    tints it, a border draws over it). A `mode="effect"` shader transforms the
    node's own rendered content; see [Effects](#effects).

```python
import tre

glow = tre.Shader(
    """
    fn shade(p: Pixel) -> vec4<f32> {
        let d = distance(p.uv, vec2<f32>(0.5, 0.5));
        return vec4<f32>(u.tint.rgb * (1.0 - d), 1.0);
    }
    """,
    uniforms={"tint": (1.0, 0.6, 0.2, 1.0)},
)
panel.set(shader=glow)
```

You write one function, `fn shade(p: Pixel) -> vec4<f32>`. `p.uv` is the
position in the node, 0 to 1, with the origin at the top left; `p.px` is the
same in pixels. Your uniforms are the fields of `u`, and `frame.size` and
`frame.time` are the node's size in pixels and the seconds since the window
opened. The result is straight-alpha RGBA.

## `Shader(wgsl, uniforms=None, inputs=None, mode="fill", animated=False)`

| Argument | |
| --- | --- |
| `wgsl` | the source. It must define `shade`, and no entry points of its own |
| `uniforms` | `dict[str, float | tuple]` — a number is an `f32`; a tuple of 2, 3 or 4 numbers is a `vec2`, `vec3` or `vec4`. Names must be WGSL identifiers that are not reserved |
| `inputs` | `dict[str, Node]` — nodes of one window, read as `input_<name>(uv)` (see [Inputs](#inputs)) |
| `mode` | `"fill"` paints the node's box behind its content; `"effect"` transforms the node's own rendered content, read as `content(uv)` |
| `animated` | `True` for a shader that reads `frame.time`: the node is repainted every frame (only its box), and the window keeps drawing frames while it is on screen. Otherwise it redraws only when something changes and a still window sleeps |

Read-only properties: `wgsl`, `mode`, `animated`, `uniforms` (a new dict), and
`inputs` (a new dict of the same `Node`s).

**`set(*, uniforms)`** replaces the uniforms all at once. A mistake raises and
changes nothing. A `Shader` is shared: give it to several nodes and one `set`
updates them all.

`==` compares identity, so `node.get("shader") == shader` after
`node.set(shader=shader)`.

## Errors

A mistake in the source, a missing or misshapen `shade`, or a bad name raises
**`tre.ShaderError`**, a `ValueError`, when the shader is created — no GPU
needed. Its `line`, `column` and `source_line` place the problem in *your*
source, not tre's generated code; each is `None` when the compiler gives the
problem no position (a type mismatch, say). `str(error)` prints the line and a
caret under the column.

Wrong types (`uniforms={"a": "x"}`, `True`, a tuple of 5) raise `TypeError` or
`ValueError`; a name tre provides (`Pixel`, `frame`, `u`, `content`, …) used in
your source is reported with the list of names tre provides.

## Effects

In `mode="effect"`, `content(uv)` returns the node's own rendered pixels --
its paint and all its descendants, drawn as if the node were the whole
window -- at `uv` (0 to 1, from the top left), as straight-alpha RGBA. What
`shade` returns **replaces** the node and its subtree: a shining text, a blur
over a panel's children, a colour grade.

```python
shine = tre.Shader(
    """
    fn shade(p: Pixel) -> vec4<f32> {
        let c = content(p.uv);
        let band = smoothstep(0.0, 0.1, 1.0 - abs(p.uv.x - u.phase));
        return vec4<f32>(c.rgb + band * 0.4 * c.a, c.a);
    }
    """,
    uniforms={"phase": 0.5}, mode="effect",
)
label.set(shader=shine)
```

- The content is the node's **box**: what a node paints outside it (a shadow,
  overflowing children) is not in `content` and not in the result. A node's
  own transform and opacity apply to the result, as they would to the node.
- The node stays where it was laid out: hit testing, focus and accessibility
  are unchanged, and its descendants still receive input. A `canvas` or a
  `text_input` inside an effect is drawn through it.
- An effect inside an effect works: the inner one runs first. A fill shader on
  a descendant paints into the content.
- Any change in the subtree repaints the whole box, since a blur spreads it,
  and re-renders the content once; an idle effect costs nothing. An effect that
  only needs its own `uniforms` to change reuses the content.

## Inputs

`inputs={"photo": node}` lets `shade` call `input_photo(uv)`, which returns
the input's colour at `uv` (0 to 1, from the top left) as straight-alpha RGBA,
linearly filtered and clamped at the edges. An input is:

- an **`image` or `video` node**: its image pixels, always, even if that node
  also has a shader. A shader on an image node can name that node as its own
  input, to grade or distort its own picture; or
- **any other node that has a shader**: that shader's output. One shader
  reads another, and the other runs first, even if its node is off-screen or
  hidden (it is needed). Set the other node's shader before naming it.

Anything else raises `ValueError` when the shader is created. Setting a shader
that would end up reading itself, through its inputs, raises
`ValueError("... would read itself ...")` and changes nothing.

A shader repaints when an input's content changes: a new frame pushed to a
video node, or a change anywhere upstream.

```python
photo = window.create("image", rgba=pixels, pixel_width=w, pixel_height=h,
                      width=200, height=200)
graded = window.create("box", width=200, height=200, shader=tre.Shader(
    "fn shade(p: Pixel) -> vec4<f32> {"
    "    let c = input_photo(p.uv);"
    "    return vec4<f32>(1.0 - c.rgb, c.a);"
    "}",
    inputs={"photo": photo},
))
```

## What it costs

A shader node costs one texture the size of its box and one render pass, run
only when something changed: the shader's source or uniforms, an input, or the
node's size. An effect also costs a second texture (its content) and an
offscreen render of its subtree, redone only when the subtree changes. An idle
shader costs nothing per frame. Only visible, on-screen nodes
run: a hidden node, a fully transparent one, or one scrolled out of view runs
no pass. A node wider or taller than 8192 pixels, or a shader the GPU refuses,
is logged once and paints as if it had no shader; it never stops the window.

## The `shader` property

Every kind of node takes `shader`: `node.set(shader=shader)`,
`window.create(kind, shader=shader)`, `node.get("shader")`. `None` clears it.
Every input node must belong to the same window as the node, or `set` raises
and changes nothing.
